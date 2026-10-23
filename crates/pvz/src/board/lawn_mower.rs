//! The lawn mowers waiting at the left end of each row. They roll in during
//! the level intro, and when a zombie reaches one it starts up and mows
//! down everything in its row.
//!
//! Behaviour per ruslan831 decomp, `Lawn/LawnMower.cpp`
//! (`LawnMowerInitialize`, `Update`, `MowZombie`, `StartMower`, `Die`,
//! `Draw`, `GetLawnMowerAttackRect`) and
//! `ReanimatorCache::MakeCachedMowerFrame`/`DrawCachedMower`. Only the plain
//! lawn mower so far (no pool or roof cleaners, and it can't be squished).

use sexy::data_array::DataArray;
use sexy::reanim::LoopType;
use sexy::tod::{TodCurve, animate_curve_float};

use crate::audio::FoleyType;
use crate::graphics::Graphics;
use crate::reanim::{EffectSystem, ReanimationId, ReanimationType, draw_reanimator_frame};
use crate::resources::GameResources;

use super::zombie::{Fx, IRect, Zombie, rect_overlap};
use super::{Board, make_render_order, render_layer};

/// `LawnMowerState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LawnMowerState {
    RollingIn,
    Ready,
    Triggered,
}

#[derive(Debug, Clone)]
pub struct LawnMower {
    pub row: i32,
    pub pos_x: f32,
    pub pos_y: f32,
    pub render_order: i32,
    pub visible: bool,
    pub state: LawnMowerState,
    pub dead: bool,
    rolling_in_counter: i32,
    chomp_counter: i32,
    altitude: f32,
    /// The mower's spinning animation, once it's running.
    reanim: Option<ReanimationId>,
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
            dead: false,
            rolling_in_counter: 0,
            chomp_counter: 0,
            altitude: 0.0,
            reanim: None,
        }
    }

    /// `GetLawnMowerAttackRect`.
    fn attack_rect(&self) -> IRect {
        IRect::new(self.pos_x as i32, self.pos_y as i32, 50, 80)
    }

    /// `LawnMower::Update`, while playing (or rolling in).
    pub fn update(
        &mut self,
        playing: bool,
        board_row_y: impl Fn(f32, i32) -> f32,
        zombies: &mut DataArray<Zombie>,
        current_wave: i32,
        wave_row_got_lawn_mowered: &mut [i32],
        fx: &mut Fx,
    ) {
        if self.state == LawnMowerState::RollingIn {
            self.rolling_in_counter += 1;
            self.pos_x = animate_curve_float(0, 100, self.rolling_in_counter, -160.0, -21.0, TodCurve::EaseInOut);
            if self.rolling_in_counter == 100 {
                self.state = LawnMowerState::Ready;
            }
            return;
        }
        if !playing {
            return;
        }

        let attack = self.attack_rect();
        for (_, zombie) in zombies.iter_mut() {
            if zombie.row != self.row || zombie.phase == super::zombie::ZombiePhase::Mowered {
                continue;
            }
            if !zombie.effected_by_damage(127) {
                continue;
            }
            if rect_overlap(attack, zombie.zombie_rect()) > 0 && (self.state != LawnMowerState::Ready || zombie.has_head) {
                self.mow_zombie(zombie, current_wave, wave_row_got_lawn_mowered, fx);
            }
        }

        if self.state != LawnMowerState::Triggered {
            return;
        }
        let mut speed = 3.33;
        if self.chomp_counter > 0 {
            self.chomp_counter -= 1;
            speed = animate_curve_float(50, 0, self.chomp_counter, speed, 1.0, TodCurve::BounceSlowMiddle);
        }
        self.pos_x += speed;
        self.pos_y = board_row_y(self.pos_x + 40.0, self.row) + 23.0;
        // TODO: lawn mowers run into pools (they splash and die there).
        if self.pos_x > super::BOARD_WIDTH as f32 {
            self.die(fx.effects);
        }
        if let Some(id) = self.reanim {
            fx.effects.update_reanimation(id);
        }
    }

    /// `LawnMower::MowZombie`.
    fn mow_zombie(&mut self, zombie: &mut Zombie, current_wave: i32, wave_row_got_lawn_mowered: &mut [i32], fx: &mut Fx) {
        match self.state {
            LawnMowerState::Ready => {
                self.start_mower(current_wave, wave_row_got_lawn_mowered, fx);
                self.chomp_counter = 25;
            }
            LawnMowerState::Triggered => self.chomp_counter = 50,
            LawnMowerState::RollingIn => {}
        }
        fx.sounds.play_foley(FoleyType::Splat);
        zombie.mow_down(fx);
    }

    /// `LawnMower::StartMower`.
    fn start_mower(&mut self, current_wave: i32, wave_row_got_lawn_mowered: &mut [i32], fx: &mut Fx) {
        if self.state == LawnMowerState::Triggered {
            return;
        }
        // The running mower is a reanimation of its own.
        if let Ok((id, reanim)) = fx.effects.add_reanimation(0.0, 18.0, ReanimationType::LawnMower, fx.res, fx.images) {
            reanim.anim_rate = 70.0;
            reanim.loop_type = LoopType::Loop;
            reanim.is_attachment = true;
            reanim.override_scale(0.85, 0.85);
            reanim.set_frames_for_layer("anim_normal");
            self.reanim = Some(id);
        }
        fx.sounds.play_foley(FoleyType::Lawnmower);
        wave_row_got_lawn_mowered[self.row as usize] = current_wave;
        self.state = LawnMowerState::Triggered;
    }

    /// `LawnMower::Die`.
    pub fn die(&mut self, effects: &mut EffectSystem) {
        self.dead = true;
        if let Some(id) = self.reanim.take() {
            effects.kill(id);
        }
        // TODO: bonus mowers roll back in (mBonusLawnMowersRemaining).
    }

    /// `LawnMower::Draw`: its shadow, then the mower.
    pub fn draw(&self, board: &Board, effects: &EffectSystem, res: &GameResources, g: &mut Graphics) {
        if !self.visible {
            return;
        }
        let shadow = if board.stage_is_night() { "IMAGE_PLANTSHADOW2" } else { "IMAGE_PLANTSHADOW" };
        if let Some(shadow) = res.image(shadow) {
            g.draw_image(shadow, self.pos_x - 7.0, self.pos_y - self.altitude + 47.0);
        }
        let (x, y) = (self.pos_x + 6.0, self.pos_y - self.altitude);
        match self.reanim.filter(|_| self.state == LawnMowerState::Triggered) {
            Some(id) => {
                g.push_state();
                g.translate(x, y);
                effects.draw(g, id, res);
                g.pop_state();
            }
            None => {
                // DrawCachedMower(0, 19): the cached frame is drawn 20 left of
                // that, and the mower sits 10 into it, at 85%.
                draw_reanimator_frame(g, res, x - 10.0, y + 19.0, ReanimationType::LawnMower, Some("anim_normal"), 0.85);
            }
        }
    }
}
