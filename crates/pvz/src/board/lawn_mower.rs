//! The lawn mowers waiting at the left end of each row.
//!
//! So far they only roll in during the level intro and sit there; they
//! can't be triggered yet.
//!
//! Behaviour per ruslan831 decomp, `Lawn/LawnMower.cpp`
//! (`LawnMowerInitialize`, `Update`, `Draw`) and
//! `ReanimatorCache::MakeCachedMowerFrame`/`DrawCachedMower`.

use sexy::tod::{TodCurve, animate_curve_float};

use crate::graphics::Graphics;
use crate::reanim::{ReanimationType, draw_reanimator_frame};
use crate::resources::GameResources;

use super::{Board, make_render_order, render_layer};

/// `LawnMowerState`.
#[allow(dead_code, reason = "triggered mowers come with zombies")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LawnMowerState {
    RollingIn,
    Ready,
}

#[derive(Debug, Clone)]
pub struct LawnMower {
    pub row: i32,
    pub pos_x: f32,
    pub pos_y: f32,
    pub render_order: i32,
    pub visible: bool,
    pub state: LawnMowerState,
    rolling_in_counter: i32,
    altitude: f32,
}

impl LawnMower {
    /// `LawnMowerInitialize`. Only the lawn mower kind so far (no pool or
    /// roof cleaners).
    pub fn new(board: &Board, row: i32) -> Self {
        let pos_x = -160.0;
        Self {
            row,
            pos_x,
            pos_y: board.pos_y_based_on_row(pos_x + 40.0, row) + 23.0,
            render_order: make_render_order(render_layer::LAWN_MOWER, row, 0),
            visible: true,
            state: LawnMowerState::Ready,
            rolling_in_counter: 0,
            altitude: 0.0,
        }
    }

    /// `LawnMower::Update`: rolling in from off screen (a mower earned back).
    pub fn update(&mut self) {
        if self.state == LawnMowerState::RollingIn {
            self.rolling_in_counter += 1;
            self.pos_x = animate_curve_float(0, 100, self.rolling_in_counter, -160.0, -21.0, TodCurve::EaseInOut);
            if self.rolling_in_counter == 100 {
                self.state = LawnMowerState::Ready;
            }
        }
        // TODO: mowing down zombies once they exist.
    }

    /// `LawnMower::Draw`: its shadow, then the mower.
    pub fn draw(&self, board: &Board, res: &GameResources, g: &mut Graphics) {
        if !self.visible {
            return;
        }
        let shadow = if board.stage_is_night() { "IMAGE_PLANTSHADOW2" } else { "IMAGE_PLANTSHADOW" };
        if let Some(shadow) = res.image(shadow) {
            g.draw_image(shadow, self.pos_x - 7.0, self.pos_y - self.altitude + 47.0);
        }
        // DrawCachedMower(0, 19): the cached frame is drawn 20 left of that,
        // and the mower sits 10 into it, at 85%.
        let (x, y) = (self.pos_x + 6.0, self.pos_y - self.altitude);
        draw_reanimator_frame(g, res, x - 10.0, y + 19.0, ReanimationType::LawnMower, Some("anim_normal"), 0.85);
    }
}
