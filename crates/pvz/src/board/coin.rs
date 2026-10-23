//! Things to click on the lawn (`Coin`). So far only sun: it falls from the
//! sky (or pops out of a plant), waits on the ground, fades away if
//! ignored, and flies to the sun counter when clicked.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Coin.cpp`: `CoinInitialize`,
//! `Update`, `UpdateFall`, `UpdateCollected`, `UpdateFade`, `GetColor`,
//! `Collect`, `ScoreCoin`, `MouseHitTest` and `GetDisappearTime`. Not yet
//! ported: money, prizes and level awards.

use bevy::prelude::*;
use sexy::rand::MtRand;
use sexy::tod::{TodCurve, animate_curve, float_round_to_int};

use crate::graphics::SexyColor;
use crate::reanim::{EffectSystem, ReanimationId, ReanimationType};
use crate::resources::GameResources;

use super::{BOARD_WIDTH, make_render_order, render_layer};

/// `CoinType` (the sun kinds).
#[allow(dead_code, reason = "sun-producing plants come later")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoinType {
    Sun,
    SmallSun,
    LargeSun,
}

/// `CoinMotion`: where the coin comes from.
#[allow(dead_code, reason = "sun-producing plants come later")]
#[allow(clippy::enum_variant_names, reason = "the original's names")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoinMotion {
    FromSky,
    FromSkySlow,
    FromPlant,
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
        };
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
        }
        coin.scale *= coin.sun_scale();
        coin
    }

    /// `GetSunScale`.
    fn sun_scale(&self) -> f32 {
        match self.ty {
            CoinType::SmallSun => 0.5,
            CoinType::LargeSun => 2.0,
            CoinType::Sun => 1.0,
        }
    }

    /// `GetSunValue`.
    pub fn sun_value(&self) -> i32 {
        match self.ty {
            CoinType::Sun => 25,
            CoinType::SmallSun => 15,
            CoinType::LargeSun => 50,
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
    }

    /// `Coin::Collect`: starts flying to the sun counter.
    pub fn collect(&mut self) {
        if self.dead {
            return;
        }
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
    pub fn update(&mut self, effects: &mut EffectSystem, rng: &mut MtRand) -> Option<CoinEvent> {
        self.coin_age += 1;
        let mut event = None;
        if self.fade_count != 0 {
            // UpdateFade.
            self.fade_count -= 1;
            if self.fade_count == 0 {
                self.die(effects);
            }
        } else if !self.is_being_collected {
            self.update_fall(rng);
        } else {
            event = self.update_collected(effects);
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
    fn update_fall(&mut self, rng: &mut MtRand) {
        if self.pos_y + self.vel_y < self.ground_y {
            self.pos_y += self.vel_y;
            if self.motion == CoinMotion::FromPlant {
                self.vel_y += 0.09;
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
            self.pos_y = self.ground_y;
            self.pos_x = float_round_to_int(self.pos_x) as f32;
            self.disappear_counter += 1;
            // GetDisappearTime, then StartFade.
            if self.disappear_counter >= 750 {
                self.fade_count = 15;
            }
        }
        if self.motion == CoinMotion::FromPlant {
            let final_scale = self.sun_scale();
            self.scale = if self.scale < final_scale { self.scale + 0.02 } else { final_scale };
        }
    }

    /// `Coin::UpdateCollected` for sun: glides to the sun counter.
    fn update_collected(&mut self, effects: &mut EffectSystem) -> Option<CoinEvent> {
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
    }
}
