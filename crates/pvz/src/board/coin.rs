//! Things to click on the lawn (`Coin`). So far sun: it falls from the
//! sky (or pops out of a plant), waits on the ground, fades away if
//! ignored, and flies to the sun counter when clicked; and the seed packet
//! the last zombie of a level drops, which ends the level.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Coin.cpp`: `CoinInitialize`,
//! `Update`, `UpdateFall`, `UpdateCollected`, `UpdateFade`, `GetColor`,
//! `Collect`, `ScoreCoin`, `MouseHitTest`, `GetDisappearTime`,
//! `IsLevelAward`, `GetFinalSeedPacketType` and `Draw`. Not yet ported:
//! money and the other level awards.

use bevy::prelude::*;
use sexy::rand::MtRand;
use sexy::tod::{TodCurve, animate_curve, float_round_to_int};

use crate::graphics::SexyColor;
use crate::particle::ParticleEffect;
use crate::plant::SeedType;
use crate::reanim::{EffectSystem, ParticleSystemId, ReanimationId, ReanimationType};
use crate::resources::GameResources;

use super::{BOARD_WIDTH, make_render_order, render_layer};

/// `CoinType` (the sun kinds).
#[allow(dead_code, reason = "sun-producing plants come later")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoinType {
    Sun,
    SmallSun,
    LargeSun,
    /// The new plant a level ends with (`COIN_FINAL_SEED_PACKET`).
    FinalSeedPacket,
}

/// `CoinMotion`: where the coin comes from.
#[allow(dead_code, reason = "sun-producing plants come later")]
#[allow(clippy::enum_variant_names, reason = "the original's names")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoinMotion {
    FromSky,
    FromSkySlow,
    FromPlant,
    /// Money and awards that pop out of a dead zombie (`COIN_MOTION_COIN`).
    Coin,
}

#[derive(Debug, Clone)]
pub struct Coin {
    pub pos_x: f32,
    pub pos_y: f32,
    vel_x: f32,
    vel_y: f32,
    ground_y: f32,
    width: f32,
    height: f32,
    pub ty: CoinType,
    motion: CoinMotion,
    pub dead: bool,
    pub is_being_collected: bool,
    disappear_counter: i32,
    fade_count: i32,
    coin_age: i32,
    collection_distance: f32,
    scale: f32,
    pub render_order: i32,
    /// The sun's animation (attached to the coin).
    reanim: Option<ReanimationId>,
    /// The plant a `FinalSeedPacket` shows.
    pub award_seed: Option<SeedType>,
    collect_x: f32,
    collect_y: f32,
    /// The bouncing arrow over an award that's landed.
    arrow: Option<ParticleSystemId>,
}

/// What a coin did this tick that the board needs to act on.
pub enum CoinEvent {
    /// Reached the counter: add this much sun (`ScoreCoin`).
    Scored(i32),
}

impl Coin {
    /// `Coin::CoinInitialize`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        x: f32,
        y: f32,
        ty: CoinType,
        motion: CoinMotion,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        rng: &mut MtRand,
    ) -> Self {
        let mut coin = Self {
            pos_x: x,
            pos_y: y,
            vel_x: 0.0,
            vel_y: 0.0,
            ground_y: 0.0,
            width: 60.0,
            height: 60.0,
            ty,
            motion,
            dead: false,
            is_being_collected: false,
            disappear_counter: 0,
            fade_count: 0,
            coin_age: 0,
            collection_distance: 0.0,
            scale: 1.0,
            render_order: make_render_order(render_layer::COIN_BANK, 0, 1),
            reanim: None,
            award_seed: None,
            collect_x: 0.0,
            collect_y: 0.0,
            arrow: None,
        };
        if ty == CoinType::FinalSeedPacket {
            if let Some(seeds) = res.image("IMAGE_SEEDS") {
                coin.width = seeds.cel_width() as f32;
                coin.height = seeds.cel_height() as f32;
            }
            coin.render_order = make_render_order(render_layer::ABOVE_UI, 0, 0);
        } else {
            // AttachReanim at the coin's center.
            let (cx, cy) = (coin.width * 0.5, coin.height * 0.5);
            match effects.add_reanimation(0.0, 0.0, ReanimationType::Sun, res, images) {
                Ok((id, sun)) => {
                    sun.set_position(x + cx, y + cy);
                    sun.loop_type = sexy::reanim::LoopType::Loop;
                    sun.anim_rate = 6.0;
                    sun.is_attachment = true;
                    coin.reanim = Some(id);
                }
                Err(e) => error!("{e}"),
            }
        }

        match motion {
            CoinMotion::FromSky => {
                coin.vel_y = 0.67;
                coin.ground_y = (rng.next_range(250) + 300) as f32;
            }
            CoinMotion::FromSkySlow => {
                coin.vel_y = 0.33;
                coin.ground_y = (rng.next_range(250) + 300) as f32;
            }
            CoinMotion::FromPlant => {
                coin.vel_y = -1.7 - rng.range_f32(0.0, 1.7);
                coin.vel_x = -0.4 + rng.range_f32(0.0, 0.8);
                coin.ground_y = y + 15.0 + rng.next_range(20) as f32;
                coin.scale = 0.4;
            }
            CoinMotion::Coin => {
                coin.vel_y = -3.0 - rng.range_f32(0.0, 2.0);
                coin.vel_x = -0.5 + rng.range_f32(0.0, 1.0);
                coin.ground_y = (y + 45.0 + rng.next_range(20) as f32).clamp(80.0, 521.0);
                if coin.is_level_award() {
                    coin.ground_y -= 30.0;
                }
            }
        }
        coin.scale *= coin.sun_scale();
        coin
    }

    /// `GetSunScale`.
    fn sun_scale(&self) -> f32 {
        match self.ty {
            CoinType::SmallSun => 0.5,
            CoinType::LargeSun => 2.0,
            CoinType::Sun | CoinType::FinalSeedPacket => 1.0,
        }
    }

    /// `IsLevelAward`.
    pub fn is_level_award(&self) -> bool {
        self.ty == CoinType::FinalSeedPacket
    }

    /// `GetSunValue`.
    pub fn sun_value(&self) -> i32 {
        match self.ty {
            CoinType::Sun => 25,
            CoinType::SmallSun => 15,
            CoinType::LargeSun => 50,
            CoinType::FinalSeedPacket => 0,
        }
    }

    /// `GetColor`: sun fades as it nears the counter, and when it expires.
    fn color(&self) -> SexyColor {
        if self.is_being_collected {
            let alpha = (self.collection_distance * 0.035).clamp(0.35, 1.0) * 255.0;
            return SexyColor::WHITE.with_alpha(alpha as i32);
        }
        if self.fade_count > 0 {
            return SexyColor::WHITE.with_alpha(animate_curve(15, 0, self.fade_count, 255, 0, TodCurve::Linear));
        }
        SexyColor::WHITE
    }

    /// `Coin::Die`.
    pub fn die(&mut self, effects: &mut EffectSystem) {
        self.dead = true;
        if let Some(id) = self.reanim.take() {
            effects.kill(id);
        }
        if let Some(id) = self.arrow.take() {
            effects.remove_particle(id);
        }
    }

    /// `Coin::Collect`: starts flying to the sun counter (or, for an award,
    /// to the middle of the screen).
    pub fn collect(&mut self) {
        if self.dead {
            return;
        }
        self.collect_x = self.pos_x;
        self.collect_y = self.pos_y;
        self.is_being_collected = true;
        self.fade_count = 0;
    }

    /// `Coin::MouseHitTest`.
    pub fn hit_test(&self, p: Vec2) -> bool {
        let extra = if self.ty == CoinType::Sun { 15.0 } else { 0.0 };
        !self.dead
            && !self.is_being_collected
            && p.x >= self.pos_x - extra
            && p.x < self.pos_x + self.width + extra
            && p.y >= self.pos_y - extra
            && p.y < self.pos_y + self.height + extra
    }

    /// `Coin::Update`, while playing.
    pub fn update(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        rng: &mut MtRand,
    ) -> Option<CoinEvent> {
        self.coin_age += 1;
        let mut event = None;
        if self.fade_count != 0 {
            // UpdateFade: awards never fade.
            if !self.is_level_award() {
                self.fade_count -= 1;
                if self.fade_count == 0 {
                    self.die(effects);
                }
            }
        } else if !self.is_being_collected {
            self.update_fall(effects, res, images, rng);
        } else {
            event = self.update_collected(effects);
        }
        if let Some(system) = self.arrow.and_then(|a| effects.particle_mut(a)) {
            system.system_move(self.pos_x + self.width / 2.0, self.pos_y + self.height / 2.0 - 60.0);
        }

        // AttachmentUpdateAndMove / OverrideColor / OverrideScale.
        if let Some(id) = self.reanim {
            let color = self.color();
            effects.update_reanimation(id);
            if let Some(sun) = effects.get_mut(id) {
                sun.set_position(self.pos_x + self.width * 0.5, self.pos_y + self.height * 0.5);
                sun.color_override = [color.r, color.g, color.b, color.a];
                sun.override_scale(self.scale, self.scale);
            }
        }
        event
    }

    /// `Coin::UpdateFall`.
    fn update_fall(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        rng: &mut MtRand,
    ) {
        if self.pos_y + self.vel_y < self.ground_y {
            self.pos_y += self.vel_y;
            if self.motion == CoinMotion::FromPlant {
                self.vel_y += 0.09;
            } else if self.motion == CoinMotion::Coin {
                self.vel_y += 0.15;
            }
            self.pos_x += self.vel_x;
            if self.pos_x > BOARD_WIDTH as f32 - self.width {
                self.pos_x = BOARD_WIDTH as f32 - self.width;
                self.vel_x = -0.4 - rng.range_f32(0.0, 0.4);
            } else if self.pos_x < 0.0 {
                self.pos_x = 0.0;
                self.vel_x = 0.4 + rng.range_f32(0.0, 0.4);
            }
        } else {
            if self.is_level_award() && self.arrow.is_none() {
                // The bouncing arrow (CoinGetsBouncyArrow) shows it's the one to click.
                let (x, y) = (self.pos_x + self.width / 2.0, self.pos_y + self.height / 2.0 - 60.0);
                self.arrow = effects.add_particle(x, y, self.render_order + 1, ParticleEffect::SeedPacket, res, images, rng);
            }
            self.pos_y = self.ground_y;
            self.pos_x = float_round_to_int(self.pos_x) as f32;
            if !self.is_level_award() {
                self.disappear_counter += 1;
                // GetDisappearTime, then StartFade.
                if self.disappear_counter >= 750 {
                    self.fade_count = 15;
                }
            }
        }
        if self.motion == CoinMotion::FromPlant {
            let final_scale = self.sun_scale();
            self.scale = if self.scale < final_scale { self.scale + 0.02 } else { final_scale };
        }
    }

    /// `Coin::UpdateCollected` for sun: glides to the sun counter.
    fn update_collected(&mut self, effects: &mut EffectSystem) -> Option<CoinEvent> {
        if self.is_level_award() {
            // The award glides to the middle of the screen. TODO: it also
            // grows to twice its size on the way.
            self.disappear_counter += 1;
            let (dest_x, dest_y) = (400.0 - self.width / 2.0, 200.0 - self.height / 2.0);
            let t = self.disappear_counter;
            self.pos_x = sexy::tod::animate_curve_float(0, 350, t, self.collect_x, dest_x, TodCurve::EaseOut);
            self.pos_y = sexy::tod::animate_curve_float(0, 350, t, self.collect_y, dest_y, TodCurve::EaseOut);
            return None;
        }
        let (dest_x, dest_y) = (15.0, 0.0);
        let dx = (self.pos_x - dest_x).abs();
        let dy = (self.pos_y - dest_y).abs();
        if self.pos_x > dest_x {
            self.pos_x -= dx / 21.0;
        } else if self.pos_x < dest_x {
            self.pos_x += dx / 21.0;
        }
        if self.pos_y > dest_y {
            self.pos_y -= dy / 21.0;
        } else if self.pos_y < dest_y {
            self.pos_y += dy / 21.0;
        }
        self.collection_distance = (dx * dx + dy * dy).sqrt();
        if self.collection_distance < 8.0 {
            // ScoreCoin.
            self.die(effects);
            return Some(CoinEvent::Scored(self.sun_value()));
        }
        self.scale = (self.collection_distance * 0.05).clamp(0.5, 1.0) * self.sun_scale();
        None
    }

    /// `Coin::Draw`: the sun is just its animation.
    pub fn draw(&self, g: &mut crate::graphics::Graphics, effects: &EffectSystem, res: &GameResources) {
        if let Some(id) = self.reanim {
            effects.draw(g, id, res);
        }
        if self.ty == CoinType::FinalSeedPacket
            && let Some(seed) = self.award_seed
        {
            // Flashes until it's picked up.
            let gray = if self.is_being_collected { 255 } else { super::seed_bank::flashing_gray(self.coin_age, 75) };
            super::seed_bank::draw_seed_packet(g, res, self.pos_x, self.pos_y, seed, 0.0, gray, true);
        }
    }
}
