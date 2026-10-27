//! The pool's animated water: the picture behind the pool lawn rippled by
//! two layers of waves, with caustic light moving over it. Behaviour per
//! ruslan831 decomp, `Lawn/System/PoolEffect.cpp` (`PoolEffectDraw`,
//! `UpdateWaterEffect`, `BilinearLookupFixedPoint`, `PoolEffectUpdate`).
//!
//! The original ticks `mPoolCounter` on the board's update, and draws the
//! caustic picture again on every frame from it. The caustic depends only on
//! the counter, so it's rewritten on each tick here, and drawing just reads it.

use std::f32::consts::PI;

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::graphics::{Graphics, MeshVertex, SexyColor};
use crate::resources::GameResources;

/// `CAUSTIC_IMAGE_WIDTH` and `CAUSTIC_IMAGE_HEIGHT`.
const CAUSTIC_WIDTH: usize = 128;
const CAUSTIC_HEIGHT: usize = 64;
/// The caustic lookup table is a 256-texel square picture.
const LOOKUP_SIZE: usize = 256;
/// The water is a mesh of this many columns and rows of cells.
const COLUMNS: usize = 15;
const ROWS: usize = 5;

/// The pool's animated water and its counter.
pub struct PoolWater {
    /// `mPoolCounter`: ticks while the board runs with a pool.
    counter: i32,
    /// `mCausticGrayscaleImage`: the caustic picture, one byte per texel.
    lookup: Vec<u8>,
    /// `mCausticImage`: white, with the caustic light as its alpha.
    caustic: Handle<Image>,
}

impl PoolWater {
    /// `PoolEffectInitialize`. `None` if the pool's pictures aren't there.
    pub fn new(res: &GameResources, images: &mut Assets<Image>) -> Option<Self> {
        let decoded = res.decode_image("IMAGE_POOL_CAUSTIC_EFFECT")?;
        // Sampled to 256 texels square, the size the lookup is written for.
        let lookup = (0..LOOKUP_SIZE * LOOKUP_SIZE)
            .map(|i| {
                let (x, y) = (i % LOOKUP_SIZE, i / LOOKUP_SIZE);
                let sx = x * decoded.width as usize / LOOKUP_SIZE;
                let sy = y * decoded.height as usize / LOOKUP_SIZE;
                decoded.rgba[(sy * decoded.width as usize + sx) * 4]
            })
            .collect();
        let mut caustic = Image::new(
            Extent3d { width: CAUSTIC_WIDTH as u32, height: CAUSTIC_HEIGHT as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            vec![255; CAUSTIC_WIDTH * CAUSTIC_HEIGHT * 4],
            TextureFormat::Rgba8UnormSrgb,
            // The main world keeps the pixels: they're rewritten every tick.
            RenderAssetUsages::default(),
        );
        // The caustic picture is drawn twice across the pool, so it wraps.
        caustic.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            ..ImageSamplerDescriptor::linear()
        });
        let water = Self { counter: 0, lookup, caustic: images.add(caustic) };
        water.write_caustic(images);
        Some(water)
    }

    /// `mPoolCounter`.
    #[cfg(test)]
    pub fn counter(&self) -> i32 {
        self.counter
    }

    /// The caustic picture the water draws its light from.
    #[cfg(test)]
    pub fn caustic(&self) -> &Handle<Image> {
        &self.caustic
    }

    /// `PoolEffectUpdate` (and the board's `mPoolCounter++`): one 100 Hz tick.
    pub fn tick(&mut self, images: &mut Assets<Image>) {
        self.counter = self.counter.wrapping_add(1);
        self.write_caustic(images);
    }

    /// `BilinearLookupFixedPoint`: the lookup at (u, v), both in 16.16 fixed
    /// point and wrapping every 256 texels.
    fn bilinear(&self, u: u32, v: u32) -> u32 {
        let (time_u, time_v) = (u & 0xFFFF_0000, v & 0xFFFF_0000);
        let factor_u1 = ((u - time_u) & 0x0000_FFFE) + 1;
        let factor_v1 = ((v - time_v) & 0x0000_FFFE) + 1;
        let factor_u0 = 65536 - factor_u1;
        let factor_v0 = 65536 - factor_v1;
        let (iu0, iu1) = (((time_u >> 16) % 256) as usize, (((time_u >> 16) + 1) % 256) as usize);
        let (iv0, iv1) = (((time_v >> 16) % 256) as usize, (((time_v >> 16) + 1) % 256) as usize);
        let texel = |row: usize, col: usize| self.lookup[row * LOOKUP_SIZE + col] as u32;
        ((factor_u0 * factor_v1 / 65536) * texel(iv1, iu0) / 65536)
            + ((factor_u1 * factor_v1 / 65536) * texel(iv1, iu1) / 65536)
            + ((factor_u0 * factor_v0 / 65536) * texel(iv0, iu0) / 65536)
            + ((factor_u1 * factor_v0 / 65536) * texel(iv0, iu1) / 65536)
    }

    /// `UpdateWaterEffect`: the caustic light, two lookups per texel averaged
    /// and turned into alpha (white, a third of it, as the original does).
    fn write_caustic(&self, images: &mut Assets<Image>) {
        let counter = self.counter;
        let pool0 = counter.wrapping_shl(16);
        let pool1 = ((counter & 0xFFFF).wrapping_add(1)).wrapping_shl(16);
        let Some(mut image) = images.get_mut(&self.caustic) else { return };
        let Some(data) = image.data.as_mut() else { return };
        for y in 0..CAUSTIC_HEIGHT {
            let time_v1 = ((256 - y as i32) << 17) as u32;
            let time_v0 = ((y as i32) << 17) as u32;
            for x in 0..CAUSTIC_WIDTH {
                let time_u = (x as i32) << 17;
                let a1 = self.bilinear(time_u.wrapping_sub(pool1 / 6) as u32, time_v1.wrapping_add((pool0 / 8) as u32)) as u8;
                let a0 = self.bilinear(time_u.wrapping_add(pool0 / 10) as u32, time_v0) as u8;
                let a = ((a0 as u32 + a1 as u32) / 2) as u8;
                let alpha = if a >= 160 {
                    255 - 2 * (a - 160)
                } else if a >= 128 {
                    5 * (a - 128)
                } else {
                    0
                };
                data[(y * CAUSTIC_WIDTH + x) * 4 + 3] = alpha / 3;
            }
        }
    }

    /// Draws the water over the pool lawn's rectangle (`PoolEffectDraw`, the
    /// 3D path). `false` if the pictures for this stage aren't loaded.
    pub fn draw(&self, g: &mut Graphics, res: &GameResources, night: bool) -> bool {
        let (base_id, shading_id) = if night {
            ("IMAGE_POOL_BASE_NIGHT", "IMAGE_POOL_SHADING_NIGHT")
        } else {
            ("IMAGE_POOL_BASE", "IMAGE_POOL_SHADING")
        };
        let (Some(base), Some(shading), Some(pool)) = (res.image(base_id), res.image(shading_id), res.image("IMAGE_POOL")) else {
            return false;
        };
        let offsets = wave_offsets(self.counter);
        // The pictures are cut into the same cells the waves are worked out for.
        let (cell_w, cell_h) = (pool.width as f32 / COLUMNS as f32, pool.height as f32 / ROWS as f32);
        let base_pos = |x: usize, y: usize| Vec2::new(x as f32 * cell_w + 35.0, y as f32 * cell_h + 279.0);
        let grid_pos = |x: usize, y: usize| Vec2::new(704.0 / COLUMNS as f32 * x as f32 + 45.0, 30.0 * y as f32 + 288.0);
        let texel = |layer: usize, x: usize, y: usize| {
            let o = offsets[layer][x][y];
            Vec2::new(o.x + x as f32 / COLUMNS as f32, o.y + y as f32 / ROWS as f32)
        };
        // The caustic's own coordinates (the offsets already include x/15).
        let caustic_uv = |x: usize, y: usize| {
            let o = offsets[2][x][y];
            Vec2::new(o.x + x as f32 / COLUMNS as f32, o.y + y as f32 / ROWS as f32)
        };
        let caustic_color = |x: usize, y: usize| {
            let alpha = if x == 0 || x == COLUMNS || y == 0 {
                0x20
            } else if night {
                0x30
            } else if x <= 7 {
                0xC0
            } else {
                0x80
            };
            SexyColor::rgba(255, 255, 255, alpha)
        };

        let mut layers = [Vec::new(), Vec::new(), Vec::new()];
        let mut indices: Vec<u32> = Vec::new();
        for cx in 0..COLUMNS {
            for cy in 0..ROWS {
                // The cell's corners, and two triangles that split it along
                // the diagonal from its first corner to its third.
                let corners = [(cx, cy), (cx, cy + 1), (cx + 1, cy + 1), (cx + 1, cy)];
                let first = (layers[0].len()) as u32;
                indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
                for &(x, y) in &corners {
                    let white = SexyColor::WHITE;
                    layers[0].push(MeshVertex { pos: base_pos(x, y), uv: texel(0, x, y), color: white });
                    layers[1].push(MeshVertex { pos: base_pos(x, y), uv: texel(1, x, y), color: white });
                    layers[2].push(MeshVertex { pos: grid_pos(x, y), uv: caustic_uv(x, y), color: caustic_color(x, y) });
                }
            }
        }
        g.draw_mesh(&base.handle, &layers[0], &indices);
        g.draw_mesh(&shading.handle, &layers[1], &indices);
        g.draw_mesh(&self.caustic, &layers[2], &indices);
        true
    }
}

impl super::Board {
    /// The counter the water moves by (`mPoolCounter++` in `Board::Update`),
    /// on any level with a pool, except once the zombies have won.
    pub(super) fn update_pool_water(&mut self, images: &mut Assets<Image>) {
        if !self.stage_has_pool() || self.scene == super::GameScene::ZombiesWon || self.ice_trap_counter > 0 {
            return;
        }
        if let Some(water) = self.pool_water.as_mut() {
            water.tick(images);
        }
    }
}

/// `PoolEffectDraw`'s wave offsets, per layer (two wave layers and the
/// caustic's own) at each of the mesh's 16 by 6 points. Interior points only;
/// edge points stay where the grid puts them.
#[allow(clippy::needless_range_loop, reason = "indexes all three layers at one point, as the decomp does")]
fn wave_offsets(counter: i32) -> [[[Vec2; ROWS + 1]; COLUMNS + 1]; 3] {
    let mut out = [[[Vec2::ZERO; ROWS + 1]; COLUMNS + 1]; 3];
    let phase = counter as f32 * 2.0 * PI;
    let (t1, t2, t3, t4, t5) = (phase / 800.0, phase / 150.0, phase / 900.0, phase / 800.0, phase / 110.0);
    for x in 0..=COLUMNS {
        for y in 0..=ROWS {
            out[2][x][y] = Vec2::new(x as f32 / COLUMNS as f32, y as f32 / ROWS as f32);
            if x == 0 || x == COLUMNS || y == 0 || y == ROWS {
                continue;
            }
            let x_phase = x as f32 * 3.0 * 2.0 * PI / COLUMNS as f32;
            let y_phase = y as f32 * 3.0 * 2.0 * PI / ROWS as f32;
            out[0][x][y] = Vec2::new(
                (y_phase + t2).sin() * 0.002 + (y_phase + t1).sin() * 0.005,
                (x_phase + t5).sin() * 0.01 + (x_phase + t3).sin() * 0.015 + (x_phase + t4).sin() * 0.005,
            );
            out[1][x][y] = Vec2::new(
                (y_phase * 0.2 + t2).sin() * 0.015 + (y_phase * 0.2 + t1).sin() * 0.012,
                (x_phase * 0.2 + t5).sin() * 0.005 + (x_phase * 0.2 + t3).sin() * 0.015 + (x_phase * 0.2 + t4).sin() * 0.02,
            );
            out[2][x][y] += Vec2::new(
                (y_phase + t1 * 1.5).sin() * 0.004 + (y_phase + t2 * 1.5).sin() * 0.005,
                (x_phase * 4.0 + t5 * 2.5).sin() * 0.005 + (x_phase * 2.0 + t3 * 2.5).sin() * 0.04 + (x_phase * 3.0 + t4 * 2.5).sin() * 0.02,
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A water with a flat lookup, so the caustic depends only on the counter.
    fn water(lookup: u8) -> PoolWater {
        PoolWater { counter: 0, lookup: vec![lookup; LOOKUP_SIZE * LOOKUP_SIZE], caustic: Handle::default() }
    }

    #[test]
    fn bilinear_of_a_flat_picture_is_that_value() {
        let w = water(200);
        for (u, v) in [(0, 0), (0x1234_5678, 0x9ABC_DEF0), (0xFFFF_FFFF, 0x0000_8000)] {
            // Each of the four weighted terms rounds down, so allow a few units.
            let got = w.bilinear(u, v) as i32;
            assert!((got - 200).abs() <= 4, "{got} at ({u:#x}, {v:#x})");
        }
    }

    #[test]
    fn bilinear_moves_between_texels() {
        let mut w = water(0);
        // Column 1 bright, column 0 dark: halfway between them is halfway up.
        for row in 0..LOOKUP_SIZE {
            w.lookup[row * LOOKUP_SIZE + 1] = 200;
        }
        let halfway = w.bilinear(0x0000_8000, 0);
        assert!((halfway as i32 - 100).abs() <= 4, "halfway gives {halfway}");
    }

    #[test]
    #[allow(clippy::needless_range_loop, reason = "indexes the layer and the point together")]
    fn waves_move_and_stay_small() {
        // The offsets are a pure function of the counter, they move from one
        // tick to the next, and their amplitudes keep them near their cells.
        let a = wave_offsets(10);
        assert_eq!(a, wave_offsets(10), "the same tick gives the same waves");
        let moved = wave_offsets(11);
        assert_ne!(a[0][5][2], moved[0][5][2], "the waves move from one tick to the next");
        for layer in 0..2 {
            for x in 1..COLUMNS {
                for y in 1..ROWS {
                    let o = a[layer][x][y];
                    assert!(o.x.abs() < 0.05 && o.y.abs() < 0.05, "layer {layer} at ({x}, {y}): {o:?}");
                }
            }
        }
        // Edges carry no wave.
        assert_eq!(a[0][0][2], Vec2::ZERO);
        assert_eq!(a[1][COLUMNS][3], Vec2::ZERO);
    }

    #[test]
    fn caustic_alpha_follows_the_light() {
        let mut images = Assets::<Image>::default();
        let image = Image::new(
            Extent3d { width: CAUSTIC_WIDTH as u32, height: CAUSTIC_HEIGHT as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            vec![255; CAUSTIC_WIDTH * CAUSTIC_HEIGHT * 4],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        let handle = images.add(image);
        // A lookup of 255 everywhere gives a = 255, alpha 255 - 2*95 = 65, a third of which is about 21.
        let bright = PoolWater { caustic: handle.clone(), ..water(255) };
        bright.write_caustic(&mut images);
        let data = images.get(&handle).and_then(|i| i.data.as_ref()).expect("data kept");
        assert!((20..=23).contains(&data[3]), "bright light gives alpha {}", data[3]);
        // No light at all, no alpha.
        let dark = PoolWater { caustic: handle.clone(), ..water(0) };
        dark.write_caustic(&mut images);
        let data = images.get(&handle).and_then(|i| i.data.as_ref()).expect("data kept");
        assert_eq!(data[3], 0);
        assert_eq!(data[0], 255, "the picture stays white");
    }
}
