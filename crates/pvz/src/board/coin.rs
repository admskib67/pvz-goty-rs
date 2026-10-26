//! Things to click on the lawn (`Coin`). So far sun: it falls from the
//! sky (or pops out of a plant), waits on the ground, fades away if
//! ignored, and flies to the sun counter when clicked; and the seed packet
//! the last zombie of a level drops, which ends the level.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Coin.cpp`: `CoinInitialize`,
//! `Update`, `UpdateFall`, `UpdateCollected`, `UpdateFade`, `GetColor`,
//! `Collect`, `ScoreCoin`, `MouseHitTest`, `GetDisappearTime`,
//! `IsLevelAward`, `GetFinalSeedPacketType` and `Draw`; money (silver, gold
//! and diamond) lands, flies to the coin bank and is added to the player's
//! coins. Not yet ported: the other level awards.

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
    /// Level 1-4's award (`COIN_SHOVEL`).
    Shovel,
    /// The other level awards (`COIN_NOTE`, `COIN_ALMANAC`, `COIN_CARKEYS`,
    /// `COIN_TACO`, `COIN_WATERING_CAN`).
    Note,
    Almanac,
    CarKeys,
    Taco,
    WateringCan,
    /// The silver sunflower trophy the last level ends with (`COIN_AWARD_SILVER_SUNFLOWER`).
    SilverSunflower,
    /// Worth 1, 5 and 100 coins (`COIN_SILVER`, `COIN_GOLD`, `COIN_DIAMOND`).
    Silver,
    Gold,
    Diamond,
}

impl CoinType {
    /// The picture of the awards that are just an item (`Coin::Draw`).
    fn award_image(self) -> Option<&'static str> {
        match self {
            Self::Note => Some("IMAGE_ZOMBIE_NOTE_SMALL"),
            Self::Almanac => Some("IMAGE_ALMANAC"),
            Self::CarKeys => Some("IMAGE_CARKEYS"),
            Self::Taco => Some("IMAGE_TACO"),
            Self::WateringCan => Some("IMAGE_WATERINGCAN"),
            Self::SilverSunflower => Some("IMAGE_SUNFLOWER_TROPHY"),
            _ => None,
        }
    }
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
    /// What the boss drops as he falls (`COIN_MOTION_FROM_BOSS`).
    FromBoss,
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
    /// Where the arrow sits relative to the coin's corner.
    arrow_offset: (f32, f32),
    /// `mNeedsBouncyArrow`: the very first money drop points itself out.
    pub needs_bouncy_arrow: bool,
    hit_ground: bool,
    /// Where the animation sits relative to the coin's corner.
    attach_offset: (f32, f32),
}

/// What a coin did this tick that the board needs to act on.
pub enum CoinEvent {
    /// Reached the counter: add this much sun (`ScoreCoin`).
    Scored(i32),
    /// Reached the coin bank: add this much money.
    Money(i32),
    /// First touched the ground (`PlayGroundSound`).
    Landed(CoinType),
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
            arrow_offset: (0.0, 0.0),
            needs_bouncy_arrow: false,
            hit_ground: false,
            attach_offset: (0.0, 0.0),
        };
        if coin.is_level_award() {
            let image = match ty {
                CoinType::Shovel => "IMAGE_SHOVEL",
                CoinType::FinalSeedPacket => "IMAGE_SEEDS",
                CoinType::SilverSunflower => "IMAGE_SUNFLOWER_TROPHY",
                _ => ty.award_image().unwrap_or("IMAGE_SEEDS"),
            };
            if let Some(image) = res.image(image) {
                coin.width = image.cel_width() as f32;
                coin.height = image.cel_height() as f32;
            }
            coin.render_order = make_render_order(render_layer::ABOVE_UI, 0, 0);
        } else if coin.is_money() {
            let (ty_reanim, shift, attach) = match ty {
                CoinType::Silver => (ReanimationType::CoinSilver, (-10.0, -8.0), (9.0, 9.0)),
                CoinType::Gold => (ReanimationType::CoinGold, (-10.0, -8.0), (9.0, 9.0)),
                _ => (ReanimationType::Diamond, (-15.0, -15.0), (-3.0, 4.0)),
            };
            coin.pos_x += shift.0;
            coin.pos_y += shift.1;
            coin.attach_offset = attach;
            match effects.add_reanimation(0.0, 0.0, ty_reanim, res, images) {
                Ok((id, reanim)) => {
                    reanim.set_position(coin.pos_x + attach.0, coin.pos_y + attach.1);
                    reanim.loop_type = sexy::reanim::LoopType::Loop;
                    reanim.anim_time = rng.range_f32(0.0, 0.99);
                    if ty == CoinType::Diamond {
                        reanim.anim_rate = rng.range_f32(50.0, 80.0);
                    } else {
                        reanim.anim_rate *= rng.range_f32(0.6, 1.0);
                    }
                    reanim.is_attachment = true;
                    coin.reanim = Some(id);
                }
                Err(e) => error!("{e}"),
            }
        } else {
            // AttachReanim at the coin's center.
            let (cx, cy) = (coin.width * 0.5, coin.height * 0.5);
            coin.attach_offset = (cx, cy);
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
            CoinMotion::FromBoss => {
                coin.vel_y = -5.0;
                coin.vel_x = -3.0;
                coin.pos_x = 750.0;
                coin.pos_y = if ty == CoinType::SilverSunflower { 130.0 } else { 245.0 };
                coin.ground_y = coin.pos_y + 40.0;
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
            _ => 1.0,
        }
    }

    /// `IsLevelAward`.
    pub fn is_level_award(&self) -> bool {
        matches!(
            self.ty,
            CoinType::FinalSeedPacket
                | CoinType::Shovel
                | CoinType::Note
                | CoinType::Almanac
                | CoinType::CarKeys
                | CoinType::Taco
                | CoinType::WateringCan
                | CoinType::SilverSunflower
        )
    }

    /// `Coin::IsMoney`.
    pub fn is_money(&self) -> bool {
        matches!(self.ty, CoinType::Silver | CoinType::Gold | CoinType::Diamond)
    }

    /// `IsSun`.
    pub fn is_sun(&self) -> bool {
        matches!(self.ty, CoinType::Sun | CoinType::SmallSun | CoinType::LargeSun)
    }

    /// `GetSunValue`.
    pub fn sun_value(&self) -> i32 {
        match self.ty {
            CoinType::Sun => 25,
            CoinType::SmallSun => 15,
            CoinType::LargeSun => 50,
            _ => 0,
        }
    }

    /// `GetCoinValue`.
    pub fn coin_value(&self) -> i32 {
        match self.ty {
            CoinType::Silver => 1,
            CoinType::Gold => 5,
            CoinType::Diamond => 100,
            _ => 0,
        }
    }

    /// `GetDisappearTime`.
    fn disappear_time(&self) -> i32 {
        if self.ty == CoinType::Diamond || self.arrow.is_some() { 1500 } else { 750 }
    }

    /// `GetColor`: sun fades as it nears the counter, and when it expires.
    fn color(&self) -> SexyColor {
        if self.is_being_collected && (self.is_sun() || self.is_money()) {
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
            if self.update_fall(effects, res, images, rng) {
                event = Some(CoinEvent::Landed(self.ty));
            }
        } else {
            event = self.update_collected(effects);
        }

        if let Some(system) = self.arrow.and_then(|a| effects.particle_mut(a)) {
            system.system_move(self.pos_x + self.arrow_offset.0, self.pos_y + self.arrow_offset.1);
        }

        // AttachmentUpdateAndMove / OverrideColor / OverrideScale.
        if let Some(id) = self.reanim {
            let color = self.color();
            effects.update_reanimation(id);
            if let Some(sun) = effects.get_mut(id) {
                let (mut ox, mut oy) = self.attach_offset;
                if self.ty == CoinType::Diamond {
                    ox += 18.0 - 18.0 * self.scale;
                    oy += 13.0 - 13.0 * self.scale;
                }
                sun.set_position(self.pos_x + ox, self.pos_y + oy);
                sun.color_override = [color.r, color.g, color.b, color.a];
                // Gold and silver fly as a flat dollar, then spin once they land.
                if matches!(self.ty, CoinType::Silver | CoinType::Gold) && (!self.hit_ground || self.is_being_collected) {
                    sun.color_override = [0, 0, 0, 0];
                }
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
    ) -> bool {
        let mut landed = false;
        if self.pos_y + self.vel_y < self.ground_y {
            self.pos_y += self.vel_y;
            if self.motion == CoinMotion::FromPlant {
                self.vel_y += 0.09;
            } else if matches!(self.motion, CoinMotion::Coin | CoinMotion::FromBoss) {
                self.vel_y += 0.15;
            }
            self.pos_x += self.vel_x;
            if self.pos_x > BOARD_WIDTH as f32 - self.width && self.motion != CoinMotion::FromBoss {
                self.pos_x = BOARD_WIDTH as f32 - self.width;
                self.vel_x = -0.4 - rng.range_f32(0.0, 0.4);
            } else if self.pos_x < 0.0 {
                self.pos_x = 0.0;
                self.vel_x = 0.4 + rng.range_f32(0.0, 0.4);
            }
        } else {
            if (self.is_level_award() || self.needs_bouncy_arrow) && self.arrow.is_none() {
                // The bouncing arrow (CoinGetsBouncyArrow) shows it's the one to click.
                let (mut ox, mut oy) = (self.width / 2.0, self.height / 2.0 - 60.0);
                let effect = if self.is_money() {
                    (ox, oy) = (ox + 12.0, oy + 21.0);
                    ParticleEffect::CoinPickupArrow
                } else {
                    ParticleEffect::SeedPacket
                };
                self.arrow_offset = (ox, oy);
                self.arrow = effects.add_particle(self.pos_x + ox, self.pos_y + oy, self.render_order + 1, effect, res, images, rng);
            }
            if !self.hit_ground {
                self.hit_ground = true;
                landed = true;
            }
            self.pos_y = self.ground_y;
            self.pos_x = float_round_to_int(self.pos_x) as f32;
            if !self.is_level_award() {
                self.disappear_counter += 1;
                // GetDisappearTime, then StartFade.
                if self.disappear_counter >= self.disappear_time() {
                    self.fade_count = 15;
                }
            }
        }
        if self.motion == CoinMotion::FromPlant {
            let final_scale = self.sun_scale();
            self.scale = if self.scale < final_scale { self.scale + 0.02 } else { final_scale };
        }
        landed
    }

    /// `Coin::UpdateCollected` for sun: glides to the sun counter.
    fn update_collected(&mut self, effects: &mut EffectSystem) -> Option<CoinEvent> {
        if self.is_level_award() {
            // The award glides to the middle of the screen. TODO: it also
            // grows to twice its size on the way.
            self.disappear_counter += 1;
            let (dest_x, dest_y) = (400.0 - self.width / 2.0, 200.0 - self.height / 2.0);
            let t = self.disappear_counter;
            self.scale = sexy::tod::animate_curve_float(0, 400, t, 1.01, 2.0, TodCurve::EaseInOut);
            self.pos_x = sexy::tod::animate_curve_float(0, 350, t, self.collect_x, dest_x, TodCurve::EaseOut);
            self.pos_y = sexy::tod::animate_curve_float(0, 350, t, self.collect_y, dest_y, TodCurve::EaseOut);
            return None;
        }
        let (dest_x, dest_y) = if self.is_money() { (39.0, 558.0) } else { (15.0, 0.0) };
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
        if self.collection_distance < if self.is_money() { 12.0 } else { 8.0 } {
            // ScoreCoin.
            self.die(effects);
            return Some(if self.is_money() { CoinEvent::Money(self.coin_value()) } else { CoinEvent::Scored(self.sun_value()) });
        }
        self.scale = (self.collection_distance * 0.05).clamp(0.5, 1.0) * self.sun_scale();
        None
    }

    /// What the award makes when it's picked up.
    pub fn pickup_sound(&self) -> &'static [&'static str] {
        match self.ty {
            CoinType::Shovel => &["SOUND_SHOVEL"],
            CoinType::Diamond => &["SOUND_DIAMOND"],
            CoinType::CarKeys | CoinType::Taco | CoinType::WateringCan => &["SOUND_TAP2"],
            _ => &["SOUND_SEEDLIFT", "SOUND_TAP2"],
        }
    }

    /// `Coin::Draw`: the sun is just its animation.
    pub fn draw(&self, g: &mut crate::graphics::Graphics, effects: &EffectSystem, res: &GameResources) {
        if self.ty == CoinType::Diamond
            && let Some(glow) = res.image("IMAGE_AWARDPICKUPGLOW")
        {
            g.push_state();
            g.set_colorize_images(true);
            g.set_color(self.color());
            g.draw_image(glow, self.pos_x - 56.0, self.pos_y - 66.0);
            g.pop_state();
        }
        if let Some(id) = self.reanim {
            effects.draw(g, id, res);
        }
        if matches!(self.ty, CoinType::Silver | CoinType::Gold) && !(self.hit_ground && !self.is_being_collected) {
            // The flat dollar, with its glow, while it's thrown and while it's collected.
            g.push_state();
            g.set_colorize_images(true);
            g.set_color(self.color());
            if let Some(glow) = res.image("IMAGE_REANIM_COINGLOW") {
                g.draw_image_cel_center_scaled(glow, self.pos_x - 14.0, self.pos_y - 12.0, 0, self.scale, self.scale);
            }
            let name = if self.ty == CoinType::Silver { "IMAGE_REANIM_COIN_SILVER_DOLLAR" } else { "IMAGE_REANIM_COIN_GOLD_DOLLAR" };
            if let Some(dollar) = res.image(name) {
                g.draw_image_cel_center_scaled(dollar, self.pos_x + 8.0, self.pos_y + 10.0, 0, self.scale, self.scale);
            }
            g.pop_state();
        }
        if self.ty == CoinType::Shovel
            && let Some(shovel) = res.image("IMAGE_SHOVEL")
        {
            g.push_state();
            if !self.is_being_collected {
                let v = super::seed_bank::flashing_gray(self.coin_age, 75) as u8;
                g.set_colorize_images(true);
                g.set_color(crate::graphics::SexyColor::rgba(v, v, v, 255));
            }
            g.draw_image(shovel, self.pos_x, self.pos_y);
            g.pop_state();
        }
        if let Some(image) = self.ty.award_image().and_then(|id| res.image(id)) {
            g.push_state();
            if !self.is_being_collected {
                let v = super::seed_bank::flashing_gray(self.coin_age, 75) as u8;
                g.set_colorize_images(true);
                g.set_color(crate::graphics::SexyColor::rgba(v, v, v, 255));
            }
            if self.ty == CoinType::SilverSunflower {
                // Behaviour per ruslan831 decomp, Coin::Draw: the trophy is drawn at 0.6.
                let scale = self.scale * 0.6;
                g.draw_image_cel_center_scaled(image, self.pos_x - 5.0, self.pos_y, 0, scale, scale);
            } else {
                g.draw_image_cel_center_scaled(image, self.pos_x, self.pos_y, 0, self.scale, self.scale);
            }
            g.pop_state();
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

impl super::Board {
    /// `Board::CanDropLoot`: money only starts dropping at level 11 of the first adventure.
    pub(super) fn can_drop_loot(&self) -> bool {
        !self.first_time_adventure || self.level >= 11
    }

    /// `Board::ShowCoinBank`.
    pub fn show_coin_bank(&mut self) {
        self.coin_bank_fade_count = 15 * 40;
    }

    /// `Board::DropLootPiece` for adventure: a dead zombie may drop money.
    /// Not yet: presents, potted plants and chocolate.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn drop_loot_piece(
        &mut self,
        x: i32,
        y: i32,
        drop_factor: i32,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut crate::audio::Sounds,
    ) {
        let mut hit = sounds.rng().next_range(30000) as i32;
        // Whack-a-zombie's zombies drop sun, more often the less the player has.
        if self.is_whack_a_zombie_level() {
            let max = match self.sun_money {
                s if s > 500 => 2800,
                s if s > 350 => 3100,
                s if s > 200 => 3700,
                _ => 5000,
            };
            if (2500 * drop_factor..=max * drop_factor).contains(&hit) {
                sounds.play_foley(crate::audio::FoleyType::SpawnSun);
                for dx in [20.0, 40.0, 60.0] {
                    let coin = Coin::new(x as f32 - dx, y as f32, CoinType::Sun, CoinMotion::Coin, effects, res, images, sounds.rng());
                    self.coins.alloc(coin);
                }
                return;
            }
        }
        if self.first_time_adventure && self.level == 11 && !self.dropped_first_coin && self.waves.current_wave > 5 {
            hit = 1000;
        }
        let ty = if hit < 14 * drop_factor {
            if self.packet_upgrades >= 1 { CoinType::Diamond } else { CoinType::Gold }
        } else if hit < 250 * drop_factor {
            CoinType::Gold
        } else if hit < 2500 * drop_factor {
            CoinType::Silver
        } else {
            return;
        };
        sounds.play_foley(crate::audio::FoleyType::SpawnSun);
        if matches!(ty, CoinType::Diamond) {
            sounds.play_foley(crate::audio::FoleyType::Chime);
        }
        let first = !self.dropped_first_coin && self.first_time_adventure && self.level == 11 && ty != CoinType::Diamond;
        let coin = Coin::new(x as f32 - 40.0, y as f32, ty, CoinMotion::Coin, effects, res, images, sounds.rng());
        let id = self.coins.alloc(coin);
        if first && let Some(coin) = self.coins.get_mut(id) {
            coin.needs_bouncy_arrow = true;
        }
        self.dropped_first_coin = true;
    }

    /// `Board::DrawUICoinBank`: the player's money, shown for a moment after collecting.
    pub(super) fn draw_coin_bank(&self, res: &GameResources, g: &mut crate::graphics::Graphics) {
        use crate::graphics::Justify;
        if self.coin_bank_fade_count <= 0 || self.scene != super::GameScene::Playing {
            return;
        }
        let Some(bank) = res.image("IMAGE_COINBANK") else { return };
        let (x, y) = (57.0, 599.0 - bank.height as f32);
        let alpha = (255 * self.coin_bank_fade_count / 15).clamp(0, 255) as u8;
        g.push_state();
        g.set_colorize_images(true);
        g.set_color(SexyColor::rgba(255, 255, 255, alpha));
        g.draw_image(bank, x, y);
        if let Some(font) = res.font("FONT_CONTINUUMBOLD14") {
            let color = SexyColor::rgba(180, 255, 90, alpha);
            g.tod_draw_string(font, x as i32 + 116, y as i32 + 24, &money_string(self.player_coins), color, Justify::Right);
        }
        g.pop_state();
    }
}

/// `LawnApp::GetMoneyString`: coins are worth ten dollars each.
pub fn money_string(coins: i32) -> String {
    let value = coins * 10;
    if value > 999_999 {
        format!("${},{:03},{:03}", value / 1_000_000, value % 1_000_000 / 1000, value % 1000)
    } else if value > 9999 {
        format!("${},{:03}", value / 1000, value % 1000)
    } else {
        format!("${value}")
    }
}
