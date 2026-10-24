//! Immediate-mode drawing modelled on SexyFramework's `Graphics`.
//!
//! The original draws everything with calls like `g->DrawImage(img, x, y)`
//! in 800×600 top-left, y-down coordinates. Screens push the same calls into
//! [`Graphics`] each frame, and `sync_draw_calls` turns them into reused
//! Bevy entities, layered by call order, so ported `Draw()` code stays close
//! to the reference. Axis-aligned images and rectangles become sprites;
//! transformed images (which can skew, e.g. reanimation parts) become
//! textured quad meshes with per-vertex color.

use std::collections::HashMap;

use bevy::asset::{AssetPath, RenderAssetUsages, embedded_asset, embedded_path};
use bevy::math::Affine2;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor, SpecializedMeshPipelineError};
use bevy::shader::ShaderRef;
use bevy::sprite::Anchor;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPipeline, Material2dPlugin};
use bevy::transform::TransformSystems;

use crate::resources::{FontRes, ImageRes};
use crate::{GAME_HEIGHT, GAME_WIDTH};

pub struct GraphicsPlugin;

impl Plugin for GraphicsPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "quad_material.wesl");
        app.add_plugins(Material2dPlugin::<QuadMaterial>::default())
            .add_plugins(Material2dPlugin::<AdditiveQuadMaterial>::default())
            .init_resource::<Graphics>()
            .init_resource::<DrawPools>()
            .add_systems(First, clear_graphics)
            .add_systems(PostUpdate, sync_draw_calls.before(TransformSystems::Propagate));
    }
}

/// A SexyFramework `Color`: 8-bit RGBA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SexyColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl SexyColor {
    pub const WHITE: Self = Self::rgba(255, 255, 255, 255);
    pub const BLACK: Self = Self::rgba(0, 0, 0, 255);

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Clamps like the original, which accepts out-of-range ints.
    pub fn with_alpha(self, a: i32) -> Self {
        Self { a: a.clamp(0, 255) as u8, ..self }
    }

    fn to_bevy(self) -> Color {
        Color::srgba_u8(self.r, self.g, self.b, self.a)
    }

    /// `ColorsMultiply`.
    pub fn multiply(self, o: Self) -> Self {
        let m = |a: u8, b: u8| (a as u32 * b as u32 / 255) as u8;
        Self::rgba(m(self.r, o.r), m(self.g, o.g), m(self.b, o.b), m(self.a, o.a))
    }
}

/// Pixel rectangle, like SexyFramework's `Rect`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SexyRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl SexyRect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    fn intersect(self, o: Self) -> Self {
        let (x0, y0) = (self.x.max(o.x), self.y.max(o.y));
        let (x1, y1) = ((self.x + self.w).min(o.x + o.w), (self.y + self.h).min(o.y + o.h));
        Self::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
    }

    fn is_empty(self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }
}

const SCREEN: SexyRect = SexyRect::new(0.0, 0.0, GAME_WIDTH, GAME_HEIGHT);

#[derive(Clone)]
struct GraphicsState {
    color: SexyColor,
    colorize_images: bool,
    /// In screen coordinates.
    clip: SexyRect,
    /// Offset added to every draw (`mTransX`/`mTransY`), so widgets can draw
    /// in their own coordinates.
    trans: Vec2,
    /// `DRAWMODE_ADDITIVE`: transformed images add their color to what's
    /// behind them instead of covering it.
    additive: bool,
    /// `SetScale`: images and glyphs grow about `scale_origin` (local coordinates).
    scale: f32,
    scale_origin: Vec2,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            color: SexyColor::WHITE,
            colorize_images: false,
            clip: SCREEN,
            trans: Vec2::ZERO,
            additive: false,
            scale: 1.0,
            scale_origin: Vec2::ZERO,
        }
    }
}

enum DrawCmd {
    Rect { rect: SexyRect, color: SexyColor },
    Image { image: Handle<Image>, dest: SexyRect, src: SexyRect, color: SexyColor },
    /// The `src` part of an image, centered on the origin of `transform`
    /// (y-down screen space).
    Matrix { image: Handle<Image>, image_size: Vec2, transform: Affine2, src: SexyRect, color: SexyColor, additive: bool },
}

/// This frame's draw calls. Cleared at the start of every frame.
#[derive(Resource, Default)]
pub struct Graphics {
    cmds: Vec<DrawCmd>,
    state: GraphicsState,
    saved: Vec<GraphicsState>,
}

impl Graphics {
    pub fn set_color(&mut self, color: SexyColor) {
        self.state.color = color;
    }

    /// When on, images are tinted by the current color (the original's colorize mode).
    /// Switches additive drawing on or off. Only transformed images
    /// (`draw_image_matrix*`, so reanimations and particles) honour it.
    pub fn set_additive(&mut self, on: bool) {
        self.state.additive = on;
    }

    pub fn set_colorize_images(&mut self, on: bool) {
        self.state.colorize_images = on;
    }

    /// Narrows the clip rectangle (intersects with the current one).
    pub fn clip_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let t = self.state.trans;
        self.state.clip = self.state.clip.intersect(SexyRect::new(x + t.x, y + t.y, w, h));
    }

    /// Moves the origin of later draws by (dx, dy), like `Graphics::Translate`.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.state.trans += Vec2::new(dx, dy);
    }

    /// `Graphics::SetScale`: later images and text are drawn `scale` times
    /// bigger, growing away from (`origin_x`, `origin_y`).
    pub fn set_scale(&mut self, scale: f32, origin_x: f32, origin_y: f32) {
        self.state.scale = scale;
        self.state.scale_origin = Vec2::new(origin_x, origin_y);
    }

    /// The current translation (`mTransX`, `mTransY`).
    pub fn translation(&self) -> Vec2 {
        self.state.trans
    }

    /// Saves color/clip state; the original does this by copying `Graphics`.
    pub fn push_state(&mut self) {
        self.saved.push(self.state.clone());
    }

    pub fn pop_state(&mut self) {
        self.state = self.saved.pop().unwrap_or_default();
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let t = self.state.trans;
        let rect = SexyRect::new(x + t.x, y + t.y, w, h).intersect(self.state.clip);
        if !rect.is_empty() && self.state.color.a > 0 {
            self.cmds.push(DrawCmd::Rect { rect, color: self.state.color });
        }
    }

    /// `DrawRect`: a one-pixel outline covering `w + 1` by `h + 1` pixels.
    pub fn draw_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.fill_rect(x, y, w + 1.0, 1.0);
        self.fill_rect(x, y + h, w + 1.0, 1.0);
        self.fill_rect(x, y + 1.0, 1.0, h - 1.0);
        self.fill_rect(x + w, y + 1.0, 1.0, h - 1.0);
    }

    pub fn draw_image(&mut self, image: &ImageRes, x: f32, y: f32) {
        let src = SexyRect::new(0.0, 0.0, image.width as f32, image.height as f32);
        self.draw_image_src(image, x, y, src);
    }

    /// Draws the `src` part of `image` with its top-left at (x, y).
    pub fn draw_image_src(&mut self, image: &ImageRes, x: f32, y: f32, src: SexyRect) {
        let s = self.state.scale;
        let o = self.state.scale_origin;
        let (x, y) = (o.x + (x - o.x) * s + self.state.trans.x, o.y + (y - o.y) * s + self.state.trans.y);
        let dest = SexyRect::new(x, y, src.w * s, src.h * s);
        let clipped = dest.intersect(self.state.clip);
        if clipped.is_empty() {
            return;
        }
        let src = SexyRect::new(
            src.x + (clipped.x - x) / s,
            src.y + (clipped.y - y) / s,
            clipped.w / s,
            clipped.h / s,
        );
        let color = self.image_color();
        self.cmds.push(DrawCmd::Image { image: image.handle.clone(), dest: clipped, src, color });
    }

    /// `DrawString` with an image font, in the current color. `y` is the
    /// baseline. Returns the text's advance width.
    pub fn draw_string(&mut self, font: &FontRes, x: i32, y: i32, text: &str) -> i32 {
        let c = self.state.color;
        let (glyphs, width) = font.font.layout(x, y, text, [c.r, c.g, c.b, c.a]);
        // DrawStringEx always draws glyphs colorized, with each layer's color.
        self.push_state();
        self.set_colorize_images(true);
        for glyph in glyphs {
            let [r, g, b, a] = glyph.color;
            self.set_color(SexyColor::rgba(r, g, b, a));
            let src = SexyRect::new(glyph.src.x as f32, glyph.src.y as f32, glyph.src.w as f32, glyph.src.h as f32);
            self.draw_image_src(&font.layer_images[glyph.layer], glyph.dest.0 as f32, glyph.dest.1 as f32, src);
        }
        self.pop_state();
        width
    }

    /// `Graphics::DrawImageBox`: stretches a 3×3-sliced image over `dest` by
    /// tiling its edges and middle.
    pub fn draw_image_box(&mut self, image: &ImageRes, dest: SexyRect) {
        let (w, h) = (image.width as f32, image.height as f32);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let (cw, ch) = ((image.width / 3) as f32, (image.height / 3) as f32);
        let (cmw, cmh) = (w - cw * 2.0, h - ch * 2.0);
        let src = |x: f32, y: f32, w: f32, h: f32| SexyRect::new(x, y, w, h);
        let (dx, dy, dw, dh) = (dest.x, dest.y, dest.w, dest.h);

        self.draw_image_src(image, dx, dy, src(0.0, 0.0, cw, ch));
        self.draw_image_src(image, dx + dw - cw, dy, src(cw + cmw, 0.0, cw, ch));
        self.draw_image_src(image, dx, dy + dh - ch, src(0.0, ch + cmh, cw, ch));
        self.draw_image_src(image, dx + dw - cw, dy + dh - ch, src(cw + cmw, ch + cmh, cw, ch));

        let cols = ((dw - cw * 2.0 + cmw - 1.0) / cmw).floor().max(0.0) as i32;
        let rows = ((dh - ch * 2.0 + cmh - 1.0) / cmh).floor().max(0.0) as i32;
        self.push_state();
        self.clip_rect(dx + cw, dy, dw - cw * 2.0, dh);
        for col in 0..cols {
            let x = dx + cw + col as f32 * cmw;
            self.draw_image_src(image, x, dy, src(cw, 0.0, cmw, ch));
            self.draw_image_src(image, x, dy + dh - ch, src(cw, ch + cmh, cmw, ch));
        }
        self.pop_state();
        self.push_state();
        self.clip_rect(dx, dy + ch, dw, dh - ch * 2.0);
        for row in 0..rows {
            let y = dy + ch + row as f32 * cmh;
            self.draw_image_src(image, dx, y, src(0.0, ch, cw, cmh));
            self.draw_image_src(image, dx + dw - cw, y, src(cw + cmw, ch, cw, cmh));
        }
        self.pop_state();
        self.push_state();
        self.clip_rect(dx + cw, dy + ch, dw - cw * 2.0, dh - ch * 2.0);
        for col in 0..cols {
            for row in 0..rows {
                let (x, y) = (dx + cw + col as f32 * cmw, dy + ch + row as f32 * cmh);
                self.draw_image_src(image, x, y, src(cw, ch, cmw, cmh));
            }
        }
        self.pop_state();
    }

    /// `Graphics::WriteWordWrapped`: `text` wrapped to `width`, starting with
    /// the top of its first line at `y`. `justify` is -1 for left, 0 for
    /// centered, 1 for right. Returns the text's height.
    #[allow(clippy::too_many_arguments)]
    pub fn write_word_wrapped(
        &mut self,
        font: &FontRes,
        x: i32,
        y: i32,
        width: i32,
        text: &str,
        line_spacing: i32,
        justify: i32,
    ) -> i32 {
        let (lines, height) = font.font.word_wrap(text, width, line_spacing);
        for (line, baseline) in lines {
            let line_x = match justify {
                0 => x + (width - font.font.string_width(&line)) / 2,
                1 => x + width - font.font.string_width(&line),
                _ => x,
            };
            self.draw_string(font, line_x, y + baseline, &line);
        }
        height
    }

    /// `TodDrawString`: `text` in `color`, its baseline at `y` and `x` its
    /// left, center or right edge.
    pub fn tod_draw_string(&mut self, font: &FontRes, x: i32, y: i32, text: &str, color: SexyColor, justify: Justify) {
        let x = match justify.horizontal() {
            0 => x - font.font.string_width(text) / 2,
            1 => x - font.font.string_width(text),
            _ => x,
        };
        self.push_state();
        self.set_color(color);
        self.draw_string(font, x, y, text);
        self.pop_state();
    }

    /// `TodDrawStringWrapped`: `text` wrapped to the rectangle's width, its
    /// first line's top at the rectangle's top (or the block centered in it,
    /// for the `*Middle` justifications). `{FORMAT}` codes aren't supported.
    pub fn tod_draw_string_wrapped(&mut self, font: &FontRes, rect: SexyRect, text: &str, color: SexyColor, justify: Justify) {
        let (x, w) = (rect.x as i32, rect.w as i32);
        let spacing = font.font.line_spacing();
        let (lines, height) = font.font.word_wrap(text, w, spacing);
        let mut y = rect.y as i32;
        if justify.vertical_middle() {
            y += (rect.h as i32 - height) / 2;
        }
        self.push_state();
        self.set_color(color);
        for (line, baseline) in lines {
            let line_x = match justify.horizontal() {
                0 => x + (w - font.font.string_width(&line)) / 2,
                1 => x + w - font.font.string_width(&line),
                _ => x,
            };
            self.draw_string(font, line_x, y + baseline, &line);
        }
        self.pop_state();
    }

    /// `Widget::WriteCenteredLine`: `text` centered across `width`, baseline at `y`.
    pub fn write_centered_line(&mut self, font: &FontRes, width: i32, y: i32, text: &str) {
        let x = (width - font.font.string_width(text)) / 2;
        self.draw_string(font, x, y, text);
    }

    /// `TodDrawImageCelCenterScaledF`: one cel, top-left at (x, y) before
    /// scaling about the cel's center.
    pub fn draw_image_cel_center_scaled(&mut self, image: &ImageRes, x: f32, y: f32, col: u32, sx: f32, sy: f32) {
        let (w, h) = (image.cel_width() as f32, image.cel_height() as f32);
        let src = SexyRect::new(col as f32 * w, 0.0, w, h);
        if sx == 1.0 && sy == 1.0 {
            self.draw_image_src(image, x, y, src);
            return;
        }
        let transform = Affine2::from_cols_array(&[sx, 0.0, 0.0, sy, x + w * 0.5, y + h * 0.5]);
        self.draw_image_matrix_src(image, transform, src);
    }

    /// `TodDrawImageCelF`: one cel of an image strip, top-left at (x, y).
    pub fn draw_image_cel(&mut self, image: &ImageRes, x: f32, y: f32, col: u32, row: u32) {
        let (w, h) = (image.cel_width() as f32, image.cel_height() as f32);
        self.draw_image_src(image, x, y, SexyRect::new(col as f32 * w, row as f32 * h, w, h));
    }

    /// `TodDrawStringMatrix`: text laid out from (0, baseline 0), then placed
    /// by `transform`.
    pub fn draw_string_matrix(&mut self, font: &FontRes, transform: Affine2, text: &str) {
        let c = self.state.color;
        let (glyphs, _) = font.font.layout(0, 0, text, [c.r, c.g, c.b, c.a]);
        self.push_state();
        self.set_colorize_images(true);
        for glyph in glyphs {
            let [r, g, b, a] = glyph.color;
            self.set_color(SexyColor::rgba(r, g, b, a));
            let src = SexyRect::new(glyph.src.x as f32, glyph.src.y as f32, glyph.src.w as f32, glyph.src.h as f32);
            let center = Vec2::new(glyph.dest.0 as f32 + src.w / 2.0, glyph.dest.1 as f32 + src.h / 2.0);
            let placed = transform * Affine2::from_translation(center);
            self.draw_image_matrix_src(&font.layer_images[glyph.layer], placed, src);
        }
        self.pop_state();
    }

    /// Draws `image` centered on `transform`'s origin, like `TodBltMatrix`.
    /// Clipping isn't applied to transformed images.
    pub fn draw_image_matrix(&mut self, image: &ImageRes, transform: Affine2) {
        let src = SexyRect::new(0.0, 0.0, image.width as f32, image.height as f32);
        self.draw_image_matrix_src(image, transform, src);
    }

    /// [`Self::draw_image_matrix`] for part of an image (e.g. one cel).
    pub fn draw_image_matrix_src(&mut self, image: &ImageRes, transform: Affine2, src: SexyRect) {
        let o = self.state.scale_origin;
        let scaling = Affine2::from_translation(o) * Affine2::from_scale(Vec2::splat(self.state.scale)) * Affine2::from_translation(-o);
        let transform = Affine2::from_translation(self.state.trans) * scaling * transform;
        let image_size = Vec2::new(image.width as f32, image.height as f32);
        let color = self.image_color();
        let additive = self.state.additive;
        self.cmds.push(DrawCmd::Matrix { image: image.handle.clone(), image_size, transform, src, color, additive });
    }

    pub fn color(&self) -> SexyColor {
        self.state.color
    }

    pub fn colorize_images(&self) -> bool {
        self.state.colorize_images
    }

    fn image_color(&self) -> SexyColor {
        if self.state.colorize_images { self.state.color } else { SexyColor::WHITE }
    }
}

/// `DrawStringJustification`.
#[allow(dead_code, reason = "the full set; screens use more of it as they're ported")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justify {
    Left,
    Right,
    Center,
    LeftMiddle,
    RightMiddle,
    CenterMiddle,
}

impl Justify {
    /// -1 left, 0 center, 1 right.
    fn horizontal(self) -> i32 {
        match self {
            Self::Left | Self::LeftMiddle => -1,
            Self::Center | Self::CenterMiddle => 0,
            Self::Right | Self::RightMiddle => 1,
        }
    }

    fn vertical_middle(self) -> bool {
        matches!(self, Self::LeftMiddle | Self::RightMiddle | Self::CenterMiddle)
    }
}

/// `TodScaleRotateTransformMatrix`: scale, then rotate by `rad`, then move to
/// (x, y). Positive angles turn counter-clockwise on screen.
pub fn scale_rotate_transform(x: f32, y: f32, rad: f32, scale_x: f32, scale_y: f32) -> Affine2 {
    let (sin, cos) = rad.sin_cos();
    Affine2::from_cols_array(&[cos * scale_x, -sin * scale_x, sin * scale_y, cos * scale_y, x, y])
}

fn clear_graphics(mut g: ResMut<Graphics>) {
    g.cmds.clear();
    g.saved.clear();
    g.state = GraphicsState::default();
}

/// Texture × vertex color, for transformed images. Our own material rather
/// than `ColorMaterial` so draw modes (e.g. additive) can be added later.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct QuadMaterial {
    #[texture(0)]
    #[sampler(1)]
    texture: Handle<Image>,
}

impl Material2d for QuadMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(AssetPath::from_path_buf(embedded_path!("quad_material.wesl")).with_source("embedded"))
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// [`QuadMaterial`] that adds to the background (`DRAWMODE_ADDITIVE`).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct AdditiveQuadMaterial {
    #[texture(0)]
    #[sampler(1)]
    texture: Handle<Image>,
}

impl Material2d for AdditiveQuadMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(AssetPath::from_path_buf(embedded_path!("quad_material.wesl")).with_source("embedded"))
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    fn specialize(
        _pipeline: &Material2dPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState {
                    color: BlendComponent { src_factor: BlendFactor::SrcAlpha, dst_factor: BlendFactor::One, operation: BlendOperation::Add },
                    alpha: BlendComponent { src_factor: BlendFactor::Zero, dst_factor: BlendFactor::One, operation: BlendOperation::Add },
                });
            }
        }
        Ok(())
    }
}

/// Entities reused across frames: sprites for axis-aligned draws, and quad
/// meshes for transformed ones, pooled per texture so an entity's material
/// never changes.
#[derive(Resource, Default)]
struct DrawPools {
    sprites: Vec<Entity>,
    /// How many sprites were visible last frame.
    sprites_shown: usize,
    quads: HashMap<(AssetId<Image>, bool), QuadPool>,
}

struct QuadPool {
    material: Handle<QuadMaterial>,
    additive_material: Option<Handle<AdditiveQuadMaterial>>,
    entities: Vec<(Entity, Handle<Mesh>)>,
    used: usize,
}

/// Screen (top-left, y-down) to world (centered, y-up) coordinates.
fn to_world(x: f32, y: f32) -> Vec2 {
    Vec2::new(x - GAME_WIDTH / 2.0, GAME_HEIGHT / 2.0 - y)
}

/// Depth step between draw calls, so later calls draw on top.
const Z_STEP: f32 = 0.001;

/// A quad showing `src` of an image, centered on `transform`'s origin.
fn quad_mesh(image_size: Vec2, transform: Affine2, src: SexyRect, color: SexyColor) -> Mesh {
    let (hw, hh) = (src.w / 2.0, src.h / 2.0);
    let corners = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)];
    let positions: Vec<[f32; 3]> = corners
        .iter()
        .map(|&(x, y)| {
            let p = transform.transform_point2(Vec2::new(x, y));
            to_world(p.x, p.y).extend(0.0).to_array()
        })
        .collect();
    let (u0, v0) = (src.x / image_size.x, src.y / image_size.y);
    let (u1, v1) = ((src.x + src.w) / image_size.x, (src.y + src.h) / image_size.y);
    let uvs = vec![[u0, v0], [u1, v0], [u1, v1], [u0, v1]];
    let color = color.to_bevy().to_linear().to_f32_array();

    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![color; 4])
        .with_inserted_indices(Indices::U16(vec![0, 1, 2, 0, 2, 3]))
}

#[allow(clippy::too_many_arguments)]
fn sync_draw_calls(
    g: Res<Graphics>,
    mut pools: ResMut<DrawPools>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<QuadMaterial>>,
    mut additive_materials: ResMut<Assets<AdditiveQuadMaterial>>,
    mut sprites: Query<(&mut Sprite, &mut Transform, &mut Visibility), Without<Mesh2d>>,
    mut quads: Query<(&mut Transform, &mut Visibility), With<Mesh2d>>,
) {
    let pools = &mut *pools;
    let mut sprite_count = 0;
    for pool in pools.quads.values_mut() {
        pool.used = 0;
    }

    for (i, cmd) in g.cmds.iter().enumerate() {
        let z = i as f32 * Z_STEP;

        if let DrawCmd::Matrix { image, image_size, transform, src, color, additive } = cmd {
            let pool = pools.quads.entry((image.id(), *additive)).or_insert_with(|| QuadPool {
                material: materials.add(QuadMaterial { texture: image.clone() }),
                additive_material: additive.then(|| additive_materials.add(AdditiveQuadMaterial { texture: image.clone() })),
                entities: Vec::new(),
                used: 0,
            });
            let mesh = quad_mesh(*image_size, *transform, *src, *color);
            let transform = Transform::from_xyz(0.0, 0.0, z);

            match pool.entities.get(pool.used) {
                Some((entity, handle)) => {
                    let _ = meshes.insert(handle, mesh);
                    match quads.get_mut(*entity) {
                        Ok((mut t, mut v)) => {
                            *t = transform;
                            *v = Visibility::Visible;
                        }
                        // Spawned last frame; its components land when commands apply.
                        Err(_) => {
                            commands.entity(*entity).insert((transform, Visibility::Visible));
                        }
                    }
                }
                None => {
                    let handle = meshes.add(mesh);
                    let mut entity = commands.spawn((Mesh2d(handle.clone()), transform, Visibility::Visible));
                    match &pool.additive_material {
                        Some(material) => entity.insert(MeshMaterial2d(material.clone())),
                        None => entity.insert(MeshMaterial2d(pool.material.clone())),
                    };
                    pool.entities.push((entity.id(), handle));
                }
            }
            pool.used += 1;
            continue;
        }

        let (sprite, transform) = match cmd {
            DrawCmd::Rect { rect, color } => (
                Sprite { color: color.to_bevy(), custom_size: Some(Vec2::new(rect.w, rect.h)), ..default() },
                Transform::from_translation(to_world(rect.x, rect.y).extend(z)),
            ),
            DrawCmd::Image { image, dest, src, color } => (
                Sprite {
                    image: image.clone(),
                    color: color.to_bevy(),
                    rect: Some(Rect::new(src.x, src.y, src.x + src.w, src.y + src.h)),
                    custom_size: Some(Vec2::new(dest.w, dest.h)),
                    ..default()
                },
                Transform::from_translation(to_world(dest.x, dest.y).extend(z)),
            ),
            DrawCmd::Matrix { .. } => unreachable!(),
        };
        match pools.sprites.get(sprite_count).map(|&e| (e, sprites.get_mut(e))) {
            Some((_, Ok((mut s, mut t, mut v)))) => {
                *s = sprite;
                *t = transform;
                *v = Visibility::Visible;
            }
            Some((entity, Err(_))) => {
                commands.entity(entity).insert((sprite, transform, Anchor::TOP_LEFT, Visibility::Visible));
            }
            None => {
                let entity = commands.spawn((sprite, transform, Anchor::TOP_LEFT, Visibility::Visible)).id();
                pools.sprites.push(entity);
            }
        }
        sprite_count += 1;
    }

    // Hide through commands rather than the query: it must work for every
    // pooled entity, including ones whose components were just inserted.
    for &entity in pools.sprites.iter().take(pools.sprites_shown).skip(sprite_count) {
        commands.entity(entity).insert(Visibility::Hidden);
    }
    pools.sprites_shown = sprite_count;
    for pool in pools.quads.values() {
        for (entity, _) in pool.entities.iter().skip(pool.used) {
            if let Ok((_, mut v)) = quads.get_mut(*entity) {
                *v = Visibility::Hidden;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipping_trims_dest_and_src_together() {
        let mut g = Graphics::default();
        let img = ImageRes { handle: Handle::default(), width: 100, height: 20, rows: 1, cols: 1 };
        g.push_state();
        g.clip_rect(240.0, 0.0, 30.0, 600.0);
        g.draw_image(&img, 230.0, 10.0);
        g.pop_state();
        g.draw_image(&img, 900.0, 0.0); // fully off-screen: dropped

        assert_eq!(g.cmds.len(), 1);
        let DrawCmd::Image { dest, src, .. } = &g.cmds[0] else { panic!() };
        assert_eq!(*dest, SexyRect::new(240.0, 10.0, 30.0, 20.0));
        assert_eq!(*src, SexyRect::new(10.0, 0.0, 30.0, 20.0));
    }

    #[test]
    fn scale_rotate_matches_original_matrix_layout() {
        // TodScaleRotateTransformMatrix maps (1, 0) to (cos, -sin) * scale.
        let m = scale_rotate_transform(5.0, 6.0, std::f32::consts::FRAC_PI_2, 2.0, 2.0);
        let p = m.transform_point2(Vec2::new(1.0, 0.0));
        assert!((p - Vec2::new(5.0, 4.0)).length() < 1e-5);
    }
}
