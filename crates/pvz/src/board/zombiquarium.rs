//! Zombiquarium: a fish tank where the "fish" are snorkel zombies and the
//! player feeds them brains to keep them (and their sun) coming.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Challenge.cpp` (`Zombiquarium*`,
//! `Challenge::InitLevel` and `Update`), `Lawn/Zombie.cpp`
//! (`UpdateZombiquarium`, `ZombiquariumFindClosestBrain`,
//! `ZombieInitialize`'s aquarium block), `Lawn/GridItem.cpp` (brains) and
//! `Lawn/Plant.cpp` (`GetCost`).
//!
//! The goal is 1000 sun; clicking the trophy packet spends it and wins. If
//! every fish dies the tank is lost. Fish are real `Zombie`s driven by
//! `Zombie::update_zombiquarium` instead of the walking code.
//!
//! Not yet: the tank's wave overlay and the tutorial arrows/advice chain.

use bevy::prelude::*;

use crate::audio::{FoleyType, Sounds};
use crate::reanim::EffectSystem;
use crate::resources::GameResources;

use super::coin::{CoinMotion, CoinType};
use super::message::MessageStyle;
use super::zombie::{AquariumEvents, Fx};
use super::{BOARD_WIDTH, Board, MiniGame};

/// `ZOMBIQUARIUM_WINNING_SCORE`.
pub const WINNING_SCORE: i32 = 1000;
pub const SNORKEL_COST: i32 = 100;
pub const TROPHY_COST: i32 = 1000;
/// A brain costs this many sun and there can be three at once.
pub const BRAIN_COST: i32 = 5;
pub const MAX_BRAINS: usize = 3;

/// One of the tank's two packets (`SEED_ZOMBIQUARIUM_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AquariumPacket {
    Snorkel,
    Trophy,
}

impl Board {
    pub fn is_aquarium(&self) -> bool {
        self.minigame() == Some(MiniGame::Zombiquarium)
    }

    /// `Challenge::InitLevel`'s aquarium start: two fish and the two
    /// packets.
    pub(super) fn start_zombiquarium(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        if !self.is_aquarium() {
            return;
        }
        self.seed_bank.num_packets = 2;
        for packet in &mut self.seed_bank.packets[..2] {
            packet.packet_type = None;
            packet.active = true;
            packet.refreshing = false;
        }
        self.seed_bank.packets[0].aquarium = Some(AquariumPacket::Snorkel);
        self.seed_bank.packets[1].aquarium = Some(AquariumPacket::Trophy);
        self.seed_bank.update_width(res);
        if let Err(e) = res.load_image("IMAGE_AQUARIUM1", images) {
            error!("{e}");
        }
        if let Err(e) = res.load_image("IMAGE_BRAIN", images) {
            error!("{e}");
        }
        self.aquarium_brains.clear();
        self.aquarium_lost = false;
        self.spawn_aquarium_fish(effects, res, images, sounds);
        self.spawn_aquarium_fish(effects, res, images, sounds);
        let message = res.translate("[ADVICE_ZOMBIQUARIUM_CLICK_TO_FEED]");
        self.display_advice(res, &message, MessageStyle::HintTallFast, None);
    }

    /// `ZombiquariumSpawnSnorkle`: a fish anywhere in the tank.
    pub(super) fn spawn_aquarium_fish(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        let mut fx = Fx { effects, res, images, sounds };
        let id = self.add_zombie_in_row(crate::zombie::ZombieType::Snorkel, 0, 0, &mut fx);
        let x = fx.sounds.rng().range_f32(50.0, 650.0);
        let y = fx.sounds.rng().range_f32(100.0, 400.0);
        if let Some(zombie) = self.zombies.get_mut(id) {
            zombie.begin_zombiquarium(x, y, fx.sounds.rng(), fx.effects);
        }
    }

    /// `Challenge::ZombiquariumMouseDown`: five sun drops a brain at `point`.
    pub(super) fn aquarium_mouse_down(&mut self, point: Vec2, sounds: &mut Sounds) -> bool {
        if !self.is_aquarium() || self.has_level_award_dropped() {
            return false;
        }
        if self.aquarium_brains.len() >= MAX_BRAINS {
            return true;
        }
        if !self.take_sun_money(BRAIN_COST, sounds) {
            return true;
        }
        self.aquarium_brains.push((point.x, point.y));
        sounds.play_sample("SOUND_TAP");
        true
    }

    /// A click on one of the two packets (`ZombiquariumPacketClicked`).
    pub(super) fn aquarium_packet_clicked(
        &mut self,
        index: usize,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) -> bool {
        if !self.is_aquarium() || self.has_level_award_dropped() {
            return false;
        }
        let Some(packet) = self.seed_bank.packets.get(index).and_then(|p| p.aquarium.filter(|_| p.active)) else { return false };
        match packet {
            AquariumPacket::Snorkel => {
                if self.take_sun_money(SNORKEL_COST, sounds) {
                    self.spawn_aquarium_fish(effects, res, images, sounds);
                }
            }
            AquariumPacket::Trophy => {
                if self.sun_money >= TROPHY_COST {
                    self.take_sun_money(TROPHY_COST, sounds);
                    self.spawn_aquarium_award(effects, res, images, sounds);
                }
            }
        }
        true
    }

    /// `ZombiquariumPacketClicked`'s trophy: the level's award.
    fn spawn_aquarium_award(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        if self.level_award_spawned {
            return;
        }
        self.level_award_spawned = true;
        sounds.play_foley(FoleyType::SpawnSun);
        self.add_coin(
            BOARD_WIDTH as f32 / 2.0,
            200.0,
            CoinType::SilverSunflower,
            CoinMotion::Coin,
            effects,
            res,
            images,
            sounds,
        );
        self.clear_advice(None);
    }

    /// `Challenge::ZombiquariumUpdate`: the progress bar and the loss when
    /// every fish is gone.
    pub(super) fn update_zombiquarium(&mut self) {
        if !self.is_aquarium() {
            return;
        }
        let sun = self.sun_money.clamp(0, WINNING_SCORE);
        self.waves.progress_meter_width =
            sexy::tod::animate_curve(0, WINNING_SCORE, sun, 0, 500, sexy::tod::TodCurve::Linear);
        if self.level_award_spawned || self.aquarium_lost {
            return;
        }
        if !self.zombies.iter().any(|(_, z)| !z.is_dead_or_dying()) {
            self.aquarium_lost = true;
            self.pending_game_over = true;
        }
    }

    /// Carries out what the fish did: brains get eaten, suns get spat out.
    pub(super) fn apply_aquarium_events(
        &mut self,
        events: Vec<AquariumEvents>,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        for event in events {
            if let Some(index) = event.ate_brain
                && index < self.aquarium_brains.len()
            {
                self.aquarium_brains.remove(index);
            }
            if let Some((x, y)) = event.sun {
                self.add_coin(x, y, CoinType::Sun, CoinMotion::FromPlant, effects, res, images, sounds);
            }
        }
    }
}
