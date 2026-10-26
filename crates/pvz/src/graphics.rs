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
    Matrix {
        image: Handle<Image>,
        image_size: Vec2,
        transform: Affine2,
        src: SexyRect,
        color: SexyColor,
        additive: bool,
        /// The clip rectangle, when it's narrower than the screen.
        clip: Option<SexyRect>,
    },
    /// Textured triangles already in screen space, with their colors cut by
    /// the clip rectangle (see [`Graphics::draw_mesh`]).
    Mesh { image: Handle<Image>, vertices: Vec<MeshVertex>, indices: Vec<u32>, additive: bool },
}

/// A corner of a [`Graphics::draw_mesh`] triangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshVertex {
    /// In the drawing's own coordinates, like the other draw calls.
    pub pos: Vec2,
    /// Where in the image it samples: 0 to 1 across it (past 1 when the
    /// image repeats).
    pub uv: Vec2,
    /// Multiplied with the texel.
    pub color: SexyColor,
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

    /// Lifts the clip rectangle, for tracks that ignore it (`mIgnoreClipRect`).
    pub fn reset_clip(&mut self) {
        self.state.clip = GraphicsState::default().clip;
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
    /// for the `*Middle` justifications). `{FORMAT}` codes from
    /// `gLawnStringFormats` change the colour and line spacing as it goes;
    /// unknown codes are dropped.
    pub fn tod_draw_string_wrapped(&mut self, font: &FontRes, rect: SexyRect, text: &str, color: SexyColor, justify: Justify) {
        self.tod_draw_string_wrapped_ex(font, rect, text, color, justify, true);
    }

    /// [`Self::tod_draw_string_wrapped`], with the `{METAL}` lines (which
    /// `TOD_FORMAT_HIDE_UNTIL_MAGNETSHROOM` hides until the player has a
    /// magnet-shroom) shown or not.
    pub fn tod_draw_string_wrapped_ex(
        &mut self,
        font: &FontRes,
        rect: SexyRect,
        text: &str,
        color: SexyColor,
        justify: Justify,
        show_metal: bool,
    ) {
        let text: Vec<char> = text.chars().collect();
        let wrap = TodWrap { font, x: rect.x as i32, width: rect.w as i32, justify, show_metal, max_chars: text.len() };
        let mut y = rect.y as i32;
        if justify.vertical_middle() {
            y += (rect.h as i32 - wrap.helper(self, &text, y, color, false)) / 2;
        }
        wrap.helper(self, &text, y, color, true);
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

    /// `TodDrawImageScaledF`: the image scaled from its top-left corner at
    /// (x, y).
    pub fn draw_image_scaled(&mut self, image: &ImageRes, x: f32, y: f32, sx: f32, sy: f32) {
        if sx == 1.0 && sy == 1.0 {
            self.draw_image(image, x, y);
            return;
        }
        let (w, h) = (image.width as f32, image.height as f32);
        let transform = Affine2::from_cols_array(&[sx, 0.0, 0.0, sy, x + w * sx * 0.5, y + h * sy * 0.5]);
        self.draw_image_matrix(image, transform);
    }

    /// Where on screen each image drawn so far ended up (its bounding box),
    /// so tests can check what's drawn where.
    #[cfg(test)]
    pub fn image_rects(&self) -> Vec<(Handle<Image>, SexyRect)> {
        self.cmds
            .iter()
            .filter_map(|cmd| match cmd {
                DrawCmd::Rect { .. } => None,
                DrawCmd::Image { image, dest, .. } => Some((image.clone(), *dest)),
                DrawCmd::Matrix { image, transform, src, .. } => {
                    let corners = [(-0.5, -0.5), (0.5, -0.5), (-0.5, 0.5), (0.5, 0.5)]
                        .map(|(u, v)| transform.transform_point2(Vec2::new(u * src.w, v * src.h)));
                    let (x0, y0) = corners.iter().fold((f32::MAX, f32::MAX), |(x, y), c| (x.min(c.x), y.min(c.y)));
                    let (x1, y1) = corners.iter().fold((f32::MIN, f32::MIN), |(x, y), c| (x.max(c.x), y.max(c.y)));
                    Some((image.clone(), SexyRect::new(x0, y0, x1 - x0, y1 - y0)))
                }
                DrawCmd::Mesh { image, vertices, .. } => {
                    let (x0, y0) = vertices.iter().fold((f32::MAX, f32::MAX), |(x, y), v| (x.min(v.pos.x), y.min(v.pos.y)));
                    let (x1, y1) = vertices.iter().fold((f32::MIN, f32::MIN), |(x, y), v| (x.max(v.pos.x), y.max(v.pos.y)));
                    Some((image.clone(), SexyRect::new(x0, y0, x1 - x0, y1 - y0)))
                }
            })
            .collect()
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
        let clip = self.state.clip;
        if clip.is_empty() {
            return;
        }
        let clip = (clip != SCREEN).then_some(clip);
        self.cmds.push(DrawCmd::Matrix { image: image.handle.clone(), image_size, transform, src, color, additive, clip });
    }

    /// `Graphics::DrawTrianglesTex`: textured triangles, three indices each,
    /// for warped pictures such as the pool. Like the original's 3D path
    /// (`D3DInterface::DrawTrianglesTex`, `TextureData::BltTriangles`), the
    /// clip rectangle is ignored: vertices are not faded or cut by it, so a
    /// pool cell straddling the screen edge keeps its colour while the board
    /// pans or the seed chooser turns.
    pub fn draw_mesh(&mut self, image: &Handle<Image>, vertices: &[MeshVertex], indices: &[u32]) {
        let o = self.state.scale_origin;
        let scaling = Affine2::from_translation(o) * Affine2::from_scale(Vec2::splat(self.state.scale)) * Affine2::from_translation(-o);
        let transform = Affine2::from_translation(self.state.trans) * scaling;
        let vertices = vertices
            .iter()
            .map(|v| MeshVertex { pos: transform.transform_point2(v.pos), uv: v.uv, color: v.color })
            .collect();
        self.cmds.push(DrawCmd::Mesh { image: image.clone(), vertices, indices: indices.to_vec(), additive: self.state.additive });
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

/// The state `{FORMAT}` codes change (`TodStringListFormat`).
#[derive(Debug, Clone, Copy, PartialEq)]
struct TodFormat {
    color: SexyColor,
    line_spacing_offset: i32,
    flags: u32,
}

/// `TOD_FORMAT_IGNORE_NEWLINES`: newlines are spaces (flavour text).
const FORMAT_IGNORE_NEWLINES: u32 = 1;
/// `TOD_FORMAT_HIDE_UNTIL_MAGNETSHROOM`.
const FORMAT_HIDE_UNTIL_MAGNETSHROOM: u32 = 2;

/// `gLawnStringFormats`: name, colour (`None` keeps the current one), line
/// spacing offset, flags.
const LAWN_STRING_FORMATS: [(&str, Option<SexyColor>, i32, u32); 12] = [
    ("NORMAL", Some(SexyColor::rgba(40, 50, 90, 255)), 0, 0),
    ("FLAVOR", Some(SexyColor::rgba(143, 67, 27, 255)), 0, FORMAT_IGNORE_NEWLINES),
    ("KEYWORD", Some(SexyColor::rgba(143, 67, 27, 255)), 0, 0),
    ("NOCTURNAL", Some(SexyColor::rgba(136, 50, 170, 255)), 0, 0),
    ("AQUATIC", Some(SexyColor::rgba(11, 161, 219, 255)), 0, 0),
    ("STAT", Some(SexyColor::rgba(204, 36, 29, 255)), 0, 0),
    ("METAL", Some(SexyColor::rgba(204, 36, 29, 255)), 0, FORMAT_HIDE_UNTIL_MAGNETSHROOM),
    ("KEYMETAL", Some(SexyColor::rgba(143, 67, 27, 255)), 0, FORMAT_HIDE_UNTIL_MAGNETSHROOM),
    ("SHORTLINE", None, -9, 0),
    ("EXTRASHORTLINE", None, -14, 0),
    ("CREDITS1", None, 3, 0),
    ("CREDITS2", None, 2, 0),
];

/// `TodWriteStringSetFormat`: the first format whose name starts the code.
/// Hidden `{METAL}` lines are transparent and take no room (as
/// `AlmanacDialog::DrawZombies` sets them).
fn set_format(code: &[char], format: &mut TodFormat, show_metal: bool) {
    for (name, color, spacing, flags) in LAWN_STRING_FORMATS {
        if code.len() >= name.len() && code.iter().zip(name.chars()).all(|(&a, b)| a == b) {
            let (mut color, mut spacing) = (color, spacing);
            if flags & FORMAT_HIDE_UNTIL_MAGNETSHROOM != 0 && !show_metal {
                color = color.map(|c| c.with_alpha(0));
                spacing = -17;
            }
            if let Some(color) = color {
                format.color = color;
            }
            format.line_spacing_offset = spacing;
            format.flags = flags;
            return;
        }
    }
}

/// `CharIsSpaceInFormat`.
fn is_space_in_format(c: char, format: &TodFormat) -> bool {
    c == ' ' || (format.flags & FORMAT_IGNORE_NEWLINES != 0 && c == '\n')
}

/// `TodDrawStringWrappedHelper` and the functions it calls.
struct TodWrap<'a> {
    font: &'a FontRes,
    x: i32,
    width: i32,
    justify: Justify,
    show_metal: bool,
    max_chars: usize,
}

impl TodWrap<'_> {
    /// `TodWriteString`: one line from `offset`, `len` characters long, with
    /// its codes applied. Returns its width. Like the original, measuring a
    /// line to justify it runs its codes too.
    #[allow(clippy::too_many_arguments)]
    fn write_string(&self, g: &mut Graphics, text: &[char], y: i32, format: &mut TodFormat, justify: Justify, draw: bool, offset: usize, len: usize) -> i32 {
        let f = &self.font.font;
        let mut x = self.x;
        if draw {
            let spare = self.width - self.write_string(g, text, y, format, Justify::Left, false, offset, len);
            match justify.horizontal() {
                1 => x += spare,
                0 => x += spare / 2,
                _ => {}
            }
        }
        let end = (offset + len).min(text.len());
        let mut chunk = String::new();
        let mut x_offset = 0;
        let flush = |g: &mut Graphics, chunk: &mut String, x_offset: &mut i32, format: &TodFormat| {
            if draw && !chunk.is_empty() {
                g.push_state();
                g.set_color(format.color);
                g.draw_string(self.font, x + *x_offset, y, chunk);
                g.pop_state();
            }
            *x_offset += f.string_width(chunk);
            chunk.clear();
        };
        let mut i = offset;
        while i < end {
            let c = text[i];
            if c == '{'
                && let Some(close) = text[i + 1..].iter().position(|&c| c == '}')
            {
                flush(g, &mut chunk, &mut x_offset, format);
                set_format(&text[i + 1..i + 1 + close], format, self.show_metal);
                i += close + 2;
                continue;
            }
            if format.flags & FORMAT_IGNORE_NEWLINES != 0 && is_space_in_format(c, format) {
                chunk.push(' ');
            } else if c != '\n' {
                chunk.push(c);
            }
            i += 1;
        }
        flush(g, &mut chunk, &mut x_offset, format);
        x_offset
    }

    /// `TodWriteWordWrappedHelper`: -1 past the end of the text.
    #[allow(clippy::too_many_arguments)]
    fn write_line(&self, g: &mut Graphics, text: &[char], y: i32, format: &mut TodFormat, draw: bool, offset: usize, len: usize) -> i32 {
        let len = if offset + len > self.max_chars {
            match self.max_chars.checked_sub(offset) {
                Some(len) if len > 0 => len,
                _ => return -1,
            }
        } else {
            len
        };
        self.write_string(g, text, y, format, self.justify, draw, offset, len)
    }

    /// `TodDrawStringWrappedHelper`: breaks the text into lines at spaces
    /// (or anywhere, for a word wider than the rectangle) and at newlines.
    /// Returns the text's height.
    fn helper(&self, g: &mut Graphics, text: &[char], top: i32, color: SexyColor, draw: bool) -> i32 {
        let f = &self.font.font;
        let mut format = TodFormat { color, line_spacing_offset: 0, flags: 0 };
        let mut y_offset = f.ascent - f.ascent_padding;
        let mut line_spacing = f.line_spacing() + format.line_spacing_offset;
        let (mut line_start, mut cur, mut cur_width) = (0, 0, 0);
        let mut prev = None;
        let mut space_pos: Option<usize> = None;
        while cur < text.len() {
            let mut c = text[cur];
            if c == '{' {
                if let Some(close) = text[cur + 1..].iter().position(|&c| c == '}') {
                    // Codes change the spacing here; the colour as each line is written.
                    let keep = format.color;
                    set_format(&text[cur + 1..cur + 1 + close], &mut format, self.show_metal);
                    format.color = keep;
                    line_spacing = f.line_spacing() + format.line_spacing_offset;
                    cur += close + 2;
                    continue;
                }
            } else if is_space_in_format(c, &format) {
                space_pos = Some(cur);
                c = ' ';
            } else if c == '\n' {
                space_pos = Some(cur);
                cur_width = self.width + 1;
                cur += 1;
            }
            cur_width += f.char_width_kern(c, prev);
            prev = Some(c);
            if cur_width <= self.width {
                cur += 1;
                continue;
            }
            if let Some(space) = space_pos {
                self.write_line(g, text, top + y_offset, &mut format, draw, line_start, space - line_start);
                cur = space + 1;
                if c != '\n' {
                    while cur < text.len() && is_space_in_format(text[cur], &format) {
                        cur += 1;
                    }
                }
            } else {
                if cur < line_start + 1 {
                    cur += 1;
                }
                if self.write_line(g, text, top + y_offset, &mut format, draw, line_start, cur - line_start) < 0 {
                    break;
                }
            }
            y_offset += line_spacing;
            line_start = cur;
            space_pos = None;
            cur_width = 0;
            prev = None;
        }
        if line_start < text.len() {
            if self.write_line(g, text, top + y_offset, &mut format, draw, line_start, text.len() - line_start) >= 0 {
                y_offset += line_spacing;
            }
        } else {
            y_offset += line_spacing;
        }
        f.descent() + y_offset - line_spacing
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
/// The corners of a transformed quad in game coordinates with their
/// texture coordinates, cut down to `clip` if there is one (the original
/// clips transformed images too). Empty when nothing's left.
fn quad_polygon(image_size: Vec2, transform: Affine2, src: SexyRect, clip: Option<SexyRect>) -> Vec<(Vec2, Vec2)> {
    let (hw, hh) = (src.w / 2.0, src.h / 2.0);
    let (u0, v0) = (src.x / image_size.x, src.y / image_size.y);
    let (u1, v1) = ((src.x + src.w) / image_size.x, (src.y + src.h) / image_size.y);
    let corners = [((-hw, -hh), (u0, v0)), ((hw, -hh), (u1, v0)), ((hw, hh), (u1, v1)), ((-hw, hh), (u0, v1))];
    let mut polygon: Vec<(Vec2, Vec2)> = corners
        .iter()
        .map(|&((x, y), (u, v))| (transform.transform_point2(Vec2::new(x, y)), Vec2::new(u, v)))
        .collect();
    let Some(clip) = clip else { return polygon };
    // Sutherland-Hodgman against each edge of the clip rectangle.
    // Each edge keeps the points where `sign * p[axis] >= limit`.
    let edges = [(0, 1.0, clip.x), (0, -1.0, -(clip.x + clip.w)), (1, 1.0, clip.y), (1, -1.0, -(clip.y + clip.h))];
    for (axis, sign, limit) in edges {
        let side = |p: Vec2| sign * p[axis];
        let inside = |p: Vec2| side(p) >= limit;
        let mut out = Vec::with_capacity(polygon.len() + 1);
        for i in 0..polygon.len() {
            let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
            if inside(a.0) {
                out.push(a);
            }
            if inside(a.0) != inside(b.0) {
                let t = (limit - side(a.0)) / (side(b.0) - side(a.0));
                out.push((a.0.lerp(b.0, t), a.1.lerp(b.1, t)));
            }
        }
        polygon = out;
        if polygon.len() < 3 {
            return Vec::new();
        }
    }
    polygon
}

/// The image, additive flag and mesh of a quad or mesh draw call. `None`
/// when nothing is left to draw.
fn quad_parts(cmd: &DrawCmd) -> Option<(&Handle<Image>, bool, Mesh)> {
    match cmd {
        DrawCmd::Matrix { image, image_size, transform, src, color, additive, clip } => {
            quad_mesh(*image_size, *transform, *src, *color, *clip).map(|mesh| (image, *additive, mesh))
        }
        DrawCmd::Mesh { image, vertices, indices, additive } => triangle_mesh(vertices, indices).map(|mesh| (image, *additive, mesh)),
        _ => None,
    }
}

/// A mesh of textured triangles already in screen space.
fn triangle_mesh(vertices: &[MeshVertex], indices: &[u32]) -> Option<Mesh> {
    if vertices.is_empty() || indices.is_empty() {
        return None;
    }
    let positions: Vec<[f32; 3]> = vertices.iter().map(|v| to_world(v.pos.x, v.pos.y).extend(0.0).to_array()).collect();
    let uvs: Vec<[f32; 2]> = vertices.iter().map(|v| v.uv.to_array()).collect();
    let colors: Vec<[f32; 4]> = vertices.iter().map(|v| v.color.to_bevy().to_linear().to_f32_array()).collect();
    Some(
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
            .with_inserted_indices(Indices::U32(indices.to_vec())),
    )
}

fn quad_mesh(image_size: Vec2, transform: Affine2, src: SexyRect, color: SexyColor, clip: Option<SexyRect>) -> Option<Mesh> {
    let polygon = quad_polygon(image_size, transform, src, clip);
    if polygon.is_empty() {
        return None;
    }
    let positions: Vec<[f32; 3]> = polygon.iter().map(|(p, _)| to_world(p.x, p.y).extend(0.0).to_array()).collect();
    let uvs: Vec<[f32; 2]> = polygon.iter().map(|(_, uv)| uv.to_array()).collect();
    let color = color.to_bevy().to_linear().to_f32_array();
    let indices = (1..polygon.len() as u16 - 1).flat_map(|i| [0, i, i + 1]).collect();

    Some(
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![color; polygon.len()])
            .with_inserted_indices(Indices::U16(indices)),
    )
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

        if matches!(cmd, DrawCmd::Matrix { .. } | DrawCmd::Mesh { .. }) {
            let Some((image, additive, mesh)) = quad_parts(cmd) else { continue };
            let pool = pools.quads.entry((image.id(), additive)).or_insert_with(|| QuadPool {
                material: materials.add(QuadMaterial { texture: image.clone() }),
                additive_material: additive.then(|| additive_materials.add(AdditiveQuadMaterial { texture: image.clone() })),
                entities: Vec::new(),
                used: 0,
            });
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
            DrawCmd::Matrix { .. } | DrawCmd::Mesh { .. } => unreachable!(),
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
    fn string_formats_match_by_prefix() {
        let codes = |s: &str| s.chars().collect::<Vec<_>>();
        let mut format = TodFormat { color: SexyColor::WHITE, line_spacing_offset: 0, flags: 0 };
        set_format(&codes("SHORTLINE"), &mut format, true);
        assert_eq!((format.color, format.line_spacing_offset), (SexyColor::WHITE, -9));
        set_format(&codes("FLAVOR"), &mut format, true);
        assert_eq!((format.color, format.flags), (SexyColor::rgba(143, 67, 27, 255), FORMAT_IGNORE_NEWLINES));
        // Unknown codes, like a zombie's hidden toughness, change nothing.
        set_format(&codes("270"), &mut format, true);
        assert_eq!(format.flags, FORMAT_IGNORE_NEWLINES);
        set_format(&codes("KEYMETAL"), &mut format, false);
        assert_eq!((format.color.a, format.line_spacing_offset), (0, -17));
        set_format(&codes("METAL"), &mut format, true);
        assert_eq!((format.color, format.line_spacing_offset), (SexyColor::rgba(204, 36, 29, 255), 0));
    }

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
    fn clipping_cuts_transformed_quads() {
        // A 10x10 quad centered on (5, 5), clipped to its top half.
        let transform = Affine2::from_translation(Vec2::new(5.0, 5.0));
        let src = SexyRect::new(0.0, 0.0, 10.0, 10.0);
        let size = Vec2::new(10.0, 10.0);
        let half = quad_polygon(size, transform, src, Some(SexyRect::new(0.0, 0.0, 10.0, 5.0)));
        assert_eq!(half.len(), 4);
        let max_y = half.iter().map(|(p, _)| p.y).fold(f32::MIN, f32::max);
        let max_v = half.iter().map(|(_, uv)| uv.y).fold(f32::MIN, f32::max);
        assert!((max_y - 5.0).abs() < 1e-5 && (max_v - 0.5).abs() < 1e-5);
        assert!(quad_polygon(size, transform, src, Some(SexyRect::new(20.0, 0.0, 5.0, 5.0))).is_empty());
    }

    #[test]
    fn meshes_ignore_the_clip_rectangle() {
        // DrawTrianglesTex doesn't clip: a vertex past the screen's edge (a pool
        // cell during the intro's pan) keeps its colour, with the default clip
        // and with a narrower one.
        let vertex = |x| MeshVertex { pos: Vec2::new(x, 10.0), uv: Vec2::ZERO, color: SexyColor::WHITE };
        let mut g = Graphics::default();
        g.draw_mesh(&Handle::default(), &[vertex(-30.0), vertex(850.0), vertex(60.0)], &[0, 1, 2]);
        let DrawCmd::Mesh { vertices, indices, .. } = &g.cmds[0] else { panic!() };
        assert_eq!(indices, &[0, 1, 2]);
        assert!(vertices.iter().all(|v| v.color.a == 255), "the default clip is the screen: nothing fades");
        assert_eq!(vertices[0].pos.x, -30.0);

        let mut g = Graphics::default();
        g.clip_rect(0.0, 0.0, 100.0, 100.0);
        g.draw_mesh(&Handle::default(), &[vertex(50.0), vertex(150.0), vertex(60.0)], &[0, 1, 2]);
        let DrawCmd::Mesh { vertices, .. } = &g.cmds[0] else { panic!() };
        assert!(vertices.iter().all(|v| v.color.a == 255), "a narrower clip doesn't fade either");
    }

    #[test]
    fn scale_rotate_matches_original_matrix_layout() {
        // TodScaleRotateTransformMatrix maps (1, 0) to (cos, -sin) * scale.
        let m = scale_rotate_transform(5.0, 6.0, std::f32::consts::FRAC_PI_2, 2.0, 2.0);
        let p = m.transform_point2(Vec2::new(1.0, 0.0));
        assert!((p - Vec2::new(5.0, 4.0)).length() < 1e-5);
    }
}
