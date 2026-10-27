//! The fog of the world 4 levels (`Board::UpdateFog` and friends): a bank of
//! fog covers the right of the lawn, fades in square by square, is pushed
//! back by planterns (and a little by torchwood), and is blown away for a
//! while by the blover.
//!
//! Behaviour per ruslan831 decomp, `Board::UpdateFog`, `ClearFogAroundPlant`,
//! `DrawFog`, `LeftFogColumn` and the fog part of `UpdateGame`.

use sexy::tod::{TodCurve, animate_curve_float};

use super::{BackgroundType, Board, GameScene, MAX_GRID_SIZE_X, MAX_GRID_SIZE_Y};
use crate::graphics::{Graphics, SexyColor};
use crate::plant::SeedType;
use crate::resources::GameResources;

/// The fog has one more row than the lawn (`MAX_GRID_SIZE_Y + 1`).
pub const FOG_ROWS: usize = MAX_GRID_SIZE_Y + 1;
/// How long a blown fog takes to come back (`FOG_BLOW_RETURN_TIME`).
pub const FOG_BLOW_RETURN_TIME: i32 = 2000;
/// What a blover sets the countdown to.
pub const BLOVER_BLOW_TIME: i32 = 4000;
/// The stormy night level (4-10) has no fog (`IsStormyNightLevel`).
pub(super) const STORMY_NIGHT_LEVEL: i32 = 40;
/// 4-5 is the adventure's scary-potter level, which is at night.
pub(super) const SCARY_POTTER_LEVEL: i32 = 35;

/// The fog's state.
#[derive(Debug, Clone)]
pub struct Fog {
    /// How thick the fog is over each square (`mGridCelFog`), 0 to 255.
    pub cel: [[i32; FOG_ROWS]; MAX_GRID_SIZE_X],
    /// How far the bank has been pushed to the right (`mFogOffset`).
    pub offset: f32,
    /// Counts down while the fog comes back after being blown or at the start (`mFogBlownCountDown`).
    pub blown_count_down: i32,
}

impl Default for Fog {
    fn default() -> Self {
        Self { cel: [[0; FOG_ROWS]; MAX_GRID_SIZE_X], offset: 0.0, blown_count_down: 0 }
    }
}

impl Board {
    /// `Board::StageHasFog`.
    pub fn stage_has_fog(&self) -> bool {
        // `Board::StageHasFog`: Invisighoul's fog lawn hides the fog too.
        if self.minigame() == Some(super::MiniGame::Invisighoul) {
            return false;
        }
        self.background == BackgroundType::Fog && self.level != STORMY_NIGHT_LEVEL
    }

    /// `Board::LeftFogColumn`: the leftmost column fog reaches on this level.
    pub fn left_fog_column(&self) -> i32 {
        match self.level {
            31 => 6,
            37..=40 => 4,
            _ => 5,
        }
    }

    fn max_fog_offset(&self) -> f32 {
        1065.0 - self.left_fog_column() as f32 * 80.0
    }

    /// The fog's part of `Board::InitLevel`: it starts off to the right and rolls in when play starts.
    pub(super) fn init_fog(&mut self) {
        if self.stage_has_fog() {
            self.fog.blown_count_down = 200;
            self.fog.offset = self.max_fog_offset();
        }
    }

    /// The fog's part of `Board::UpdateGame`, run every tick: where the bank is.
    pub(super) fn update_fog_offset(&mut self) {
        if !self.stage_has_fog() || self.fog.blown_count_down <= 0 {
            return;
        }
        let max = self.max_fog_offset();
        let intro = self.scene == GameScene::LevelIntro;
        let fog = &mut self.fog;
        if intro {
            fog.offset = animate_curve_float(200, 0, fog.blown_count_down, max, 0.0, TodCurve::EaseOut);
        } else if fog.blown_count_down < FOG_BLOW_RETURN_TIME {
            fog.offset = animate_curve_float(FOG_BLOW_RETURN_TIME, 0, fog.blown_count_down, max, 0.0, TodCurve::EaseOut);
        } else if fog.offset < max {
            // Just blown: the bank sweeps away to the right.
            let t = ((fog.offset * 1.1 + 5.0) / (max + 5.0)).clamp(0.0, 1.0);
            fog.offset = t * max;
        }
    }

    /// One tick of playing: the countdown runs.
    pub(super) fn tick_fog_countdown(&mut self) {
        if self.fog.blown_count_down > 0 {
            self.fog.blown_count_down -= 1;
        }
    }

    /// `Board::UpdateFog`: the fog thickens, and lights push it back.
    pub(super) fn update_fog(&mut self) {
        if !self.stage_has_fog() {
            return;
        }
        let count = self.fog.blown_count_down;
        let fade_in = if count > 0 && count < FOG_BLOW_RETURN_TIME {
            1
        } else if count > 0 {
            20
        } else {
            3
        };
        let left = self.left_fog_column() as usize;
        for x in left..MAX_GRID_SIZE_X {
            let max = if x == left { 200 } else { 255 };
            for y in 0..FOG_ROWS {
                self.fog.cel[x][y] = (self.fog.cel[x][y] + fade_in).min(max);
            }
        }
        let lights: Vec<_> = self
            .plants
            .iter()
            .filter(|(_, p)| !p.dead && !p.not_on_ground())
            .filter_map(|(_, p)| match p.seed {
                SeedType::Plantern => Some((p.col, p.row, 4)),
                SeedType::Torchwood => Some((p.col, p.row, 1)),
                _ => None,
            })
            .collect();
        for (col, row, size) in lights {
            self.clear_fog_around(col, row, size);
        }
    }

    /// `Board::ClearFogAroundPlant`: thins the fog near a light.
    fn clear_fog_around(&mut self, col: i32, row: i32, size: i32) {
        let count = self.fog.blown_count_down;
        let fade_out = if count > 0 && count < FOG_BLOW_RETURN_TIME {
            2
        } else if count > 0 {
            40
        } else {
            6
        };
        let left = self.left_fog_column();
        // The light stays where the plant is while the bank drifts.
        let offset_x = ((self.fog.offset + 50.0) / 100.0) as i32;
        let start_x = (col - size - offset_x).max(left);
        let end_x = (col + size - offset_x).min(MAX_GRID_SIZE_X as i32 - 1);
        let start_y = (row - size).max(0);
        let end_y = (row + size).min(MAX_GRID_SIZE_Y as i32);
        for x in start_x..=end_x {
            for y in start_y..=end_y {
                let dist_x = (x + offset_x - col).abs();
                let dist_y = (y - row).abs();
                if size == 4 {
                    // The plantern's light is a rounded block, not a diamond.
                    if dist_x > 3 || dist_y > 2 || dist_x + dist_y == 5 {
                        continue;
                    }
                } else if dist_x + dist_y > size {
                    continue;
                }
                let cel = &mut self.fog.cel[x as usize][y as usize];
                *cel = (*cel - fade_out).max(0);
            }
        }
    }

    /// `Board::DrawFog`.
    pub(super) fn draw_fog(&self, res: &GameResources, g: &mut Graphics) {
        let Some(image) = res.image("IMAGE_FOG") else { return };
        let cols = image.cols.max(1) as i32;
        let time = self.main_counter as f32 * std::f32::consts::TAU;
        for x in 0..MAX_GRID_SIZE_X {
            for y in 0..FOG_ROWS {
                let fade = self.fog.cel[x][y];
                if fade == 0 {
                    continue;
                }
                let look = self.grid_cels.look(x, y % MAX_GRID_SIZE_Y);
                let col = look % cols;
                let pos_x = x as f32 * 80.0 + self.fog.offset - 15.0;
                let pos_y = y as f32 * 85.0 + 20.0;
                let phase_x = 6.0 * std::f32::consts::PI * x as f32 / MAX_GRID_SIZE_X as f32;
                let phase_y = 6.0 * std::f32::consts::PI * y as f32 / FOG_ROWS as f32;
                let motion = 13.0 + 4.0 * (time / 900.0 + phase_y).sin() + 8.0 * (time / 500.0 + phase_x).sin();
                let color = (255.0 - look as f32 * 1.5 - motion * 1.5).clamp(0.0, 255.0) as u8;
                let lightness = (255.0 - look as f32 - motion).clamp(0.0, 255.0) as u8;
                g.push_state();
                g.set_colorize_images(true);
                g.set_color(SexyColor::rgba(color, color, lightness, fade.clamp(0, 255) as u8));
                g.draw_image_cel(image, pos_x, pos_y, col as u32, 0);
                if x == MAX_GRID_SIZE_X - 1 {
                    g.draw_image_cel(image, pos_x + 80.0, pos_y, col as u32, 0);
                }
                g.pop_state();
            }
        }
    }
}
