//! The lawn mowers waiting at the left end of each row. They roll in during
//! the level intro, and when a zombie reaches one it starts up and mows
//! down everything in its row.
//!
//! On a pool row the mower is the pool cleaner, if the player has bought it
//! from the store (`Board::pool_cleaner`); otherwise it's an ordinary mower
//! that runs into the water and is lost there.
//!
//! Behaviour per ruslan831 decomp, `Lawn/LawnMower.cpp`
//! (`LawnMowerInitialize`, `UpdatePool`, `MowZombie`, `Update`, `Die`,
//! `Draw`, `StartMower`, `GetLawnMowerAttackRect`) and
//! `ReanimatorCache::MakeCachedMowerFrame`/`DrawCachedMower`. The roof's
//! cleaner and the super mower aren't ported, and a mower can't be squished.

use sexy::data_array::{DataArray, Id};
use sexy::reanim::LoopType;
use sexy::tod::{TodCurve, animate_curve_float};

use crate::audio::FoleyType;
use crate::graphics::Graphics;
use crate::particle::ParticleEffect;
use crate::reanim::{EffectSystem, ReanimationId, ReanimationType, draw_reanimator_frame};
use crate::resources::GameResources;

use super::zombie::{Fx, IRect, Zombie, ZombiePhase, rect_overlap};
use super::{BOARD_WIDTH, Board, PlantRowType, make_render_order, render_layer};

/// `LawnMowerState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LawnMowerState {
    RollingIn,
    Ready,
    Triggered,
}

/// `LawnMowerType`, the kinds ported so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LawnMowerType {
    Lawn,
    /// The pool cleaner: it drives through the water and eats zombies there.
    Pool,
}

/// `MowerHeight`: where a pool cleaner is between the lawn and the water.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MowerHeight {
    Land,
    DownToPool,
    InPool,
    UpToLand,
}

#[derive(Debug, Clone)]
pub struct LawnMower {
    pub row: i32,
    pub kind: LawnMowerType,
    /// Whether the row is a pool (`Board::plant_row`), which decides what
    /// happens to a lawn mower that drives into it.
    pool_row: bool,
    pub pos_x: f32,
    pub pos_y: f32,
    pub render_order: i32,
    pub visible: bool,
    pub state: LawnMowerState,
    pub height: MowerHeight,
    pub dead: bool,
    rolling_in_counter: i32,
    chomp_counter: i32,
    altitude: f32,
    /// The mower's animation, once it's running.
    reanim: Option<ReanimationId>,
}

impl LawnMower {
    /// `LawnMowerInitialize`. A pool row gets the pool cleaner when the
    /// player has bought it, and any other row a lawn mower.
    pub fn new(board: &Board, row: i32) -> Self {
        let pool_row = board.plant_row[row as usize] == PlantRowType::Pool;
        let kind = if pool_row && board.pool_cleaner {
            LawnMowerType::Pool
        } else {
            LawnMowerType::Lawn
        };
        let pos_x = -160.0;
        Self {
            row,
            kind,
            pool_row,
            pos_x,
            pos_y: board.pos_y_based_on_row(pos_x + 40.0, row) + 23.0,
            render_order: make_render_order(render_layer::LAWN_MOWER, row, 0),
            visible: true,
            state: LawnMowerState::Ready,
            height: MowerHeight::Land,
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

    /// `LawnMower::Update`, while playing (or rolling in). `kelp_targets` are
    /// the zombies a tangle kelp is dragging under (`IsTangleKelpTarget`); the
    /// mower passes over them.
    #[allow(clippy::too_many_arguments, reason = "the mower's update takes what the decomp's does, plus the kelp's holds")]
    pub fn update(
        &mut self,
        playing: bool,
        board_row_y: impl Fn(f32, i32) -> f32,
        zombies: &mut DataArray<Zombie>,
        kelp_targets: &[Id<Zombie>],
        current_wave: i32,
        wave_row_got_lawn_mowered: &mut [i32],
        fx: &mut Fx,
    ) {
        if self.state == LawnMowerState::RollingIn {
            self.rolling_in_counter += 1;
            self.pos_x = animate_curve_float(
                0,
                100,
                self.rolling_in_counter,
                -160.0,
                -21.0,
                TodCurve::EaseInOut,
            );
            if self.rolling_in_counter == 100 {
                self.state = LawnMowerState::Ready;
            }
            return;
        }
        if !playing {
            return;
        }

        let attack = self.attack_rect();
        for (id, zombie) in zombies.iter_mut() {
            // Behaviour per ruslan831 decomp, LawnMower::Update: a zombie a
            // tangle kelp is dragging under isn't mowed.
            if zombie.row != self.row || zombie.phase == ZombiePhase::Mowered || kelp_targets.contains(&id) {
                continue;
            }
            if !zombie.effected_by_damage(127) {
                continue;
            }
            if rect_overlap(attack, zombie.zombie_rect()) > 0
                && (self.state != LawnMowerState::Ready || zombie.has_head)
            {
                self.mow_zombie(zombie, current_wave, wave_row_got_lawn_mowered, fx);
            }
        }

        if self.state != LawnMowerState::Triggered {
            return;
        }
        // The pool cleaner is slower than a mower.
        let mut speed = if self.kind == LawnMowerType::Pool {
            2.5
        } else {
            3.33
        };
        if self.chomp_counter > 0 {
            self.chomp_counter -= 1;
            speed = animate_curve_float(
                50,
                0,
                self.chomp_counter,
                speed,
                1.0,
                TodCurve::BounceSlowMiddle,
            );
        }
        self.pos_x += speed;
        self.pos_y = board_row_y(self.pos_x + 40.0, self.row) + 23.0;

        if self.kind == LawnMowerType::Pool {
            self.update_pool(fx);
        }
        if self.kind == LawnMowerType::Lawn && self.pool_row && self.pos_x > 50.0 {
            // A mower that drives into the pool sinks there.
            self.splash(fx, 67.0);
            fx.sounds.play_sample("SOUND_ZOMBIE_ENTERING_WATER");
            fx.sounds.stop_foley(FoleyType::Lawnmower);
            self.die(fx.effects);
            return;
        }
        if self.pos_x > BOARD_WIDTH as f32 {
            self.die(fx.effects);
        }
        if let Some(id) = self.reanim {
            fx.effects.update_reanimation(id);
        }
    }

    /// `LawnMower::UpdatePool`: the pool cleaner dips into the water when it
    /// reaches the pool, and climbs out again past it.
    fn update_pool(&mut self, fx: &mut Fx) {
        let in_pool_range = self.pos_x > 26.0 && self.pos_x < 660.0;
        if in_pool_range && self.height == MowerHeight::Land {
            self.splash(fx, 42.0);
            fx.sounds.play_foley(FoleyType::ZombieSplash);
            self.height = MowerHeight::DownToPool;
        } else if self.height == MowerHeight::DownToPool {
            self.altitude -= 2.0;
            if self.altitude <= -28.0 {
                self.altitude = 0.0;
                self.height = MowerHeight::InPool;
                self.play_track(fx, "anim_water", LoopType::Loop, 0, 0.0);
            }
        } else if self.height == MowerHeight::InPool && !in_pool_range {
            self.altitude = -28.0;
            self.height = MowerHeight::UpToLand;
            self.splash(fx, 42.0);
            fx.sounds.play_foley(FoleyType::PlantWater);
            self.play_track(fx, "anim_land", LoopType::Loop, 0, 0.0);
        } else if self.height == MowerHeight::UpToLand {
            self.altitude += 2.0;
            if self.altitude >= 0.0 {
                self.altitude = 0.0;
                self.height = MowerHeight::Land;
            }
        }

        // A sucking animation that has run its course goes back to swimming.
        let done_sucking = self
            .reanim
            .and_then(|id| fx.effects.get(id))
            .is_some_and(|r| r.loop_type == LoopType::PlayOnceAndHold && r.loop_count > 0);
        if self.height == MowerHeight::InPool && done_sucking {
            self.play_track(fx, "anim_water", LoopType::Loop, 10, 35.0);
        }
    }

    /// A splash where the mower meets the water: the splash animation, a
    /// pool splash particle `particle_dy` below the mower's top, and the sound
    /// is up to the caller.
    fn splash(&self, fx: &mut Fx, particle_dy: f32) {
        let (x, y) = (self.pos_x, self.pos_y + 25.0);
        if let Ok((_, splash)) =
            fx.effects
                .add_reanimation(x, y, ReanimationType::Splash, fx.res, fx.images)
        {
            splash.override_scale(1.2, 0.8);
            splash.render_order = self.render_order + 1;
        }
        let rng = fx.sounds.rng();
        fx.effects.add_particle(
            self.pos_x + 50.0,
            self.pos_y + particle_dy,
            self.render_order + 1,
            ParticleEffect::PlantingPool,
            fx.res,
            fx.images,
            rng,
        );
    }

    /// `PlayReanim` on the mower's own animation.
    fn play_track(
        &self,
        fx: &mut Fx,
        track: &str,
        loop_type: LoopType,
        blend_time: i32,
        anim_rate: f32,
    ) {
        if let Some(reanim) = self.reanim.and_then(|id| fx.effects.get_mut(id)) {
            reanim.play(track, loop_type, blend_time, anim_rate);
        }
    }

    /// `LawnMower::MowZombie`.
    fn mow_zombie(
        &mut self,
        zombie: &mut Zombie,
        current_wave: i32,
        wave_row_got_lawn_mowered: &mut [i32],
        fx: &mut Fx,
    ) {
        match self.state {
            LawnMowerState::Ready => {
                self.start_mower(current_wave, wave_row_got_lawn_mowered, fx);
                self.chomp_counter = 25;
            }
            LawnMowerState::Triggered => self.chomp_counter = 50,
            LawnMowerState::RollingIn => {}
        }
        match self.kind {
            LawnMowerType::Pool => {
                // The pool cleaner sucks the zombie up, wherever it is.
                fx.sounds.play_foley(FoleyType::Shoop);
                let track = if self.height == MowerHeight::InPool {
                    "anim_suck"
                } else {
                    "anim_landsuck"
                };
                self.play_track(fx, track, LoopType::PlayOnceAndHold, 10, 35.0);
                zombie.die_with_loot(fx);
            }
            LawnMowerType::Lawn => {
                fx.sounds.play_foley(FoleyType::Splat);
                zombie.mow_down(fx);
            }
        }
    }

    /// `LawnMower::StartMower`.
    fn start_mower(
        &mut self,
        current_wave: i32,
        wave_row_got_lawn_mowered: &mut [i32],
        fx: &mut Fx,
    ) {
        if self.state == LawnMowerState::Triggered {
            return;
        }
        // The running mower is a reanimation of its own.
        let (ty, track, scale, rate) = match self.kind {
            LawnMowerType::Lawn => (ReanimationType::LawnMower, "anim_normal", 0.85, 70.0),
            LawnMowerType::Pool => (ReanimationType::PoolCleaner, "anim_land", 0.8, 35.0),
        };
        if let Ok((id, reanim)) = fx.effects.add_reanimation(0.0, 18.0, ty, fx.res, fx.images) {
            reanim.anim_rate = rate;
            reanim.loop_type = LoopType::Loop;
            reanim.is_attachment = true;
            reanim.override_scale(scale, scale);
            reanim.set_frames_for_layer(track);
            if self.kind == LawnMowerType::Pool {
                // Its tracks keep their last frame, so it stays whole in the water.
                reanim.set_truncate_disappearing_frames(None, false);
            }
            self.reanim = Some(id);
        }
        match self.kind {
            LawnMowerType::Lawn => fx.sounds.play_foley(FoleyType::Lawnmower),
            LawnMowerType::Pool => fx.sounds.play_foley(FoleyType::PoolCleaner),
        }
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
    pub fn draw(
        &self,
        board: &Board,
        effects: &EffectSystem,
        res: &GameResources,
        g: &mut Graphics,
    ) {
        if !self.visible {
            return;
        }
        if self.height == MowerHeight::Land {
            let shadow = if board.stage_is_night() {
                "IMAGE_PLANTSHADOW2"
            } else {
                "IMAGE_PLANTSHADOW"
            };
            // The pool cleaner's shadow sits up and to the left of a mower's.
            let (dx, dy) = if self.kind == LawnMowerType::Pool {
                (-17.0, -8.0)
            } else {
                (0.0, 0.0)
            };
            if let Some(shadow) = res.image(shadow) {
                g.draw_image(
                    shadow,
                    self.pos_x - 7.0 + dx,
                    self.pos_y - self.altitude + 47.0 + dy,
                );
            }
        }

        let triggered = self.state == LawnMowerState::Triggered;
        let (mut x, mut y) = (self.pos_x + 6.0, self.pos_y - self.altitude);
        if self.kind == LawnMowerType::Pool {
            if triggered {
                x -= 10.0;
                y -= 7.0;
            } else {
                y -= 33.0;
            }
        }
        g.push_state();
        g.translate(x, y);
        // The pool cleaner is cut off by the water while it dips in and out.
        if self.kind == LawnMowerType::Pool
            && matches!(self.height, MowerHeight::UpToLand | MowerHeight::DownToPool)
        {
            g.clip_rect(-50.0, -50.0, 150.0, 132.0 + self.altitude);
        }
        match self.reanim.filter(|_| triggered) {
            Some(id) => effects.draw(g, id, res),
            // DrawCachedMower: the cached frame is drawn 20 left of the mower's
            // origin, and the mower sits 10 into it (and 25 down in the pool).
            None => match self.kind {
                LawnMowerType::Lawn => draw_reanimator_frame(
                    g,
                    res,
                    -10.0,
                    19.0,
                    ReanimationType::LawnMower,
                    Some("anim_normal"),
                    0.85,
                ),
                LawnMowerType::Pool => draw_reanimator_frame(
                    g,
                    res,
                    -10.0,
                    44.0,
                    ReanimationType::PoolCleaner,
                    None,
                    0.8,
                ),
            },
        }
        g.pop_state();
    }
}
