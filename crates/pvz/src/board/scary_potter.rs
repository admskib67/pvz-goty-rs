//! Vasebreaker: pots on the night lawn, broken for plants, zombies or sun.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Challenge.cpp`
//! (`ScaryPotterPopulate`, `ScaryPotterPlacePot`, `ScaryPotterOpenPot`,
//! `ScaryPotterMalletPot`, `ScaryPotterIsCompleted`,
//! `IsFinalScaryPotterStage`, `PuzzlePhaseComplete`) and `Lawn/GridItem.cpp`
//! (`DrawScaryPot`, `UpdateScaryPot`).
//!
//! A broken pot gives a usable seed packet (free to plant), a zombie at the
//! pot's column, or 1–3 suns. Levels 1–9 end when the pots and zombies are
//! gone; endless repopulates after every cleared board and remembers the
//! streak (the stage count). The mallet animation and the plantern's
//! "see inside" transparency aren't ported.

use bevy::prelude::*;
use sexy::rand::MtRand;

use crate::audio::{FoleyType, Sounds};
use crate::plant::SeedType;
use crate::reanim::EffectSystem;
use crate::resources::GameResources;
use crate::zombie::ZombieType;

use super::coin::{CoinMotion, CoinType};
use super::grid_item::{GridItem, GridItemState, GridItemType};
use crate::particle::ParticleEffect;
use super::message::MessageStyle;
use super::zombie::Fx;
use super::{Board, GameMode, MAX_GRID_SIZE_X, MAX_GRID_SIZE_Y, PuzzleLevel, render_layer};

/// What a pot holds (`ScaryPotType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaryPotKind {
    Seed,
    Zombie,
    Sun,
}

/// A pot's payload (`mSeedType`, `mZombieType`, `mSunCount`).
#[derive(Debug, Clone, Copy)]
pub struct ScaryPot {
    pub kind: ScaryPotKind,
    pub seed: Option<SeedType>,
    pub zombie: Option<ZombieType>,
    pub sun: i32,
}

/// One requested pot in `ScaryPotterPopulate`.
#[derive(Debug, Clone, Copy)]
struct PotSpec {
    kind: ScaryPotKind,
    seed: Option<SeedType>,
    zombie: Option<ZombieType>,
    count: i32,
}

impl Board {
    pub fn is_scary_potter_level(&self) -> bool {
        matches!(self.mode, GameMode::Puzzle(_))
    }

    fn puzzle_level(&self) -> Option<PuzzleLevel> {
        match self.mode {
            GameMode::Puzzle(level) => Some(level),
            _ => None,
        }
    }

    /// `Challenge::InitLevel` + `ScaryPotterPopulate`.
    pub(super) fn start_scary_potter(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        rng: &mut MtRand,
    ) {
        if !self.is_scary_potter_level() {
            return;
        }
        self.sun_money = 0;
        self.seed_bank.num_packets = 0;
        self.seed_bank.update_width(res);
        self.waves = super::waves::WaveState::default();
        self.waves.num_waves = 0;
        self.survival_stage = 0;
        self.scary_potter_populate(effects, res, images, rng);
        if let Some(level) = self.puzzle_level()
            && !level.is_endless()
        {
            let message = res.translate("[ADVICE_DESTROY_POTS_TO_FINISH_LEVEL]");
            self.display_advice(res, &message, MessageStyle::HintFast, None);
        }
    }

    /// The level's pot table (`ScaryPotterPopulate`'s switch).
    fn pot_specs(&self) -> (Vec<i32>, Vec<PotSpec>) {
        use SeedType::*;
        use ZombieType as Z;
        let spec = |kind, seed, zombie, count| PotSpec { kind, seed, zombie, count };
        let seed_pot = |seed, count| spec(ScaryPotKind::Seed, Some(seed), None, count);
        let zombie_pot = |zombie, count| spec(ScaryPotKind::Zombie, None, Some(zombie), count);
        let level = self.puzzle_level();
        let mut banned = vec![0, 1];
        let mut specs = Vec::new();
        match level {
            Some(PuzzleLevel::Vasebreaker(1)) | None => {
                banned.extend([2, 3]);
                specs.extend([
                    seed_pot(Peashooter, 5),
                    seed_pot(Snowpea, 5),
                    seed_pot(Squash, 5),
                    zombie_pot(Z::Normal, 6),
                    zombie_pot(Z::Pail, 3),
                    zombie_pot(Z::JackInTheBox, 1),
                ]);
            }
            Some(PuzzleLevel::Vasebreaker(2)) => {
                banned.push(8);
                specs.extend([
                    seed_pot(Leftpeater, 7),
                    seed_pot(Snowpea, 3),
                    seed_pot(Wallnut, 3),
                    seed_pot(Potatomine, 2),
                    zombie_pot(Z::Normal, 6),
                    zombie_pot(Z::Pail, 3),
                    zombie_pot(Z::JackInTheBox, 1),
                ]);
            }
            Some(PuzzleLevel::Vasebreaker(3)) => {
                specs.extend([
                    seed_pot(Leftpeater, 6),
                    seed_pot(Snowpea, 4),
                    seed_pot(Squash, 2),
                    seed_pot(Hypnoshroom, 3),
                    seed_pot(Wallnut, 3),
                    zombie_pot(Z::Normal, 8),
                    zombie_pot(Z::Pail, 2),
                    zombie_pot(Z::Dancer, 1),
                    zombie_pot(Z::JackInTheBox, 1),
                ]);
            }
            Some(PuzzleLevel::Vasebreaker(4)) => {
                banned.truncate(2);
                specs.extend([
                    seed_pot(Puffshroom, 11),
                    seed_pot(Hypnoshroom, 4),
                    seed_pot(Leftpeater, 4),
                    zombie_pot(Z::JackInTheBox, 8),
                    zombie_pot(Z::Normal, 7),
                    zombie_pot(Z::Football, 1),
                ]);
            }
            Some(PuzzleLevel::Vasebreaker(5)) => {
                banned.truncate(2);
                specs.extend([
                    seed_pot(Leftpeater, 6),
                    seed_pot(Pumpkinshell, 3),
                    seed_pot(Squash, 4),
                    seed_pot(Hypnoshroom, 2),
                    seed_pot(Snowpea, 2),
                    seed_pot(Magnetshroom, 3),
                    zombie_pot(Z::Normal, 6),
                    zombie_pot(Z::Pail, 5),
                    zombie_pot(Z::JackInTheBox, 1),
                    zombie_pot(Z::Football, 3),
                ]);
            }
            Some(PuzzleLevel::Vasebreaker(6)) => {
                banned.truncate(2);
                specs.extend([
                    seed_pot(Leftpeater, 7),
                    seed_pot(Squash, 2),
                    seed_pot(Tallnut, 5),
                    seed_pot(Threepeater, 2),
                    seed_pot(Torchwood, 4),
                    zombie_pot(Z::Normal, 7),
                    zombie_pot(Z::Polevaulter, 5),
                    zombie_pot(Z::Football, 2),
                    zombie_pot(Z::JackInTheBox, 1),
                ]);
            }
            Some(PuzzleLevel::Vasebreaker(7)) => {
                banned.push(2);
                specs.extend([
                    seed_pot(Spikeweed, 13),
                    seed_pot(Wallnut, 3),
                    seed_pot(Squash, 3),
                    zombie_pot(Z::Normal, 10),
                    zombie_pot(Z::Pail, 1),
                ]);
            }
            Some(PuzzleLevel::Vasebreaker(8)) => {
                banned.truncate(2);
                specs.extend([
                    seed_pot(Puffshroom, 7),
                    seed_pot(Wallnut, 3),
                    seed_pot(Squash, 5),
                    seed_pot(Leftpeater, 4),
                    zombie_pot(Z::JackInTheBox, 8),
                    zombie_pot(Z::Normal, 4),
                    zombie_pot(Z::Pogo, 4),
                ]);
            }
            Some(PuzzleLevel::Vasebreaker(9)) => {
                banned.truncate(2);
                specs.extend([
                    seed_pot(Leftpeater, 6),
                    seed_pot(Snowpea, 2),
                    seed_pot(Peashooter, 2),
                    seed_pot(Threepeater, 2),
                    seed_pot(Squash, 5),
                    seed_pot(Potatomine, 1),
                    seed_pot(Wallnut, 1),
                    seed_pot(Plantern, 1),
                    zombie_pot(Z::Normal, 8),
                    zombie_pot(Z::Pail, 5),
                    zombie_pot(Z::JackInTheBox, 1),
                    zombie_pot(Z::Gargantuar, 1),
                ]);
            }
            Some(PuzzleLevel::VasebreakerEndless) => {
                let extra = (self.survival_stage / 10).clamp(0, 8);
                banned.truncate(2);
                specs.extend([
                    seed_pot(Leftpeater, 6),
                    seed_pot(Snowpea, 2),
                    seed_pot(Peashooter, 1),
                    seed_pot(Threepeater, 2),
                    seed_pot(Squash, 5),
                    seed_pot(Potatomine, 1),
                    seed_pot(Wallnut, 1),
                    seed_pot(Plantern, 1),
                    spec(ScaryPotKind::Sun, None, None, 1),
                    zombie_pot(Z::Normal, 8 - extra),
                    zombie_pot(Z::Pail, 5),
                    zombie_pot(Z::JackInTheBox, 1),
                    zombie_pot(Z::Gargantuar, 1 + extra),
                ]);
            }
            _ => {}
        }
        (banned, specs)
    }

    fn scary_potter_populate(
        &mut self,
        _effects: &mut EffectSystem,
        _res: &mut GameResources,
        _images: &mut Assets<Image>,
        rng: &mut MtRand,
    ) {
        self.grid_items = sexy::data_array::DataArray::default();
        let (banned, specs) = self.pot_specs();
        let mut cells: Vec<(i32, i32)> = (0..MAX_GRID_SIZE_Y as i32 - 1)
            .flat_map(|row| (0..MAX_GRID_SIZE_X as i32).map(move |col| (col, row)))
            .filter(|(col, _)| !banned.contains(col))
            .collect();
        for i in (1..cells.len()).rev() {
            cells.swap(i, rng.next_range((i + 1) as u32) as usize);
        }
        let mut index = 0;
        let mut seed_pots = Vec::new();
        let mut gargantuars = Vec::new();
        for spec in specs {
            for _ in 0..spec.count {
                if index >= cells.len() {
                    break;
                }
                let (col, row) = cells[index];
                index += 1;
                let sun = if spec.kind == ScaryPotKind::Sun { rng.range_int(1, 3) } else { 0 };
                let pot = ScaryPot { kind: spec.kind, seed: spec.seed, zombie: spec.zombie, sun };
                let item = GridItem::scary_pot(col, row, pot);
                let id = self.grid_items.alloc(item);
                if spec.kind == ScaryPotKind::Seed {
                    seed_pots.push(id);
                }
                if spec.zombie == Some(ZombieType::Gargantuar) {
                    gargantuars.push(id);
                }
            }
        }
        // `ScaryPotterChangePotType`: two seed pots show a leaf.
        for _ in 0..2 {
            let pos = rng.next_range(seed_pots.len().max(1) as u32) as usize;
            if let Some(&id) = seed_pots.get(pos)
                && let Some(item) = self.grid_items.get_mut(id)
            {
                item.state = GridItemState::ScaryPotLeaf;
            }
        }
        let _ = gargantuars;
    }

    /// `ScaryPotterMalletPot` + `ScaryPotterOpenPot` for a pot on `(col, row)`.
    pub(super) fn break_scary_pot(
        &mut self,
        col: i32,
        row: i32,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) -> bool {
        if !self.is_scary_potter_level() || self.has_level_award_dropped() {
            return false;
        }
        let Some(id) = self
            .grid_items
            .iter()
            .find(|(_, g)| g.item_type == GridItemType::ScaryPot && g.grid_x == col && g.grid_y == row && !g.dead)
            .map(|(id, _)| id)
        else {
            return false;
        };
        let Some(pot) = self.grid_items.get(id).and_then(|g| g.pot) else { return false };
        if let Some(item) = self.grid_items.get_mut(id) {
            item.dead = true;
        }
        sounds.play_sample("SOUND_BONK");
        sounds.play_sample("SOUND_VASE_BREAKING");
        let effect = match pot.kind {
            ScaryPotKind::Seed => ParticleEffect::VaseShatterLeaf,
            ScaryPotKind::Zombie => ParticleEffect::VaseShatterZombie,
            ScaryPotKind::Sun => ParticleEffect::VaseShatter,
        };
        let (x, y) = (self.grid_to_pixel_x(col), self.grid_to_pixel_y(col, row));
        self.add_particle(x as f32 + 20.0, y as f32, render_layer::TOP, effect);
        self.flush_particle_requests(effects, res, images, sounds.rng());
        match pot.kind {
            ScaryPotKind::Seed => {
                if let Some(seed) = pot.seed {
                    let coin = self.add_coin(
                        x as f32 + 20.0,
                        y as f32,
                        CoinType::UsableSeedPacket,
                        CoinMotion::FromPlant,
                        effects,
                        res,
                        images,
                        sounds,
                    );
                    if let Some(coin) = self.coins.get_mut(coin) {
                        coin.usable_seed = Some(seed);
                    }
                }
            }
            ScaryPotKind::Zombie => {
                if let Some(zombie_type) = pot.zombie {
                    let mut fx = Fx { effects, res, images, sounds };
                    let zid = self.add_zombie_in_row(zombie_type, row, 0, &mut fx);
                    if let Some(zombie) = self.zombies.get_mut(zid) {
                        zombie.place_x(x as f32);
                    }
                }
            }
            ScaryPotKind::Sun => {
                for i in 0..pot.sun {
                    self.add_coin(
                        x as f32 + 15.0 * i as f32,
                        y as f32,
                        CoinType::Sun,
                        CoinMotion::FromPlant,
                        effects,
                        res,
                        images,
                        sounds,
                    );
                }
            }
        }
        if self.scary_potter_completed() {
            self.scary_potter_complete(effects, res, images, sounds);
        }
        true
    }

    /// `ScaryPotterIsCompleted`: no pots and no enemies.
    pub(super) fn scary_potter_completed(&self) -> bool {
        !self.grid_items.iter().any(|(_, g)| g.item_type == GridItemType::ScaryPot && !g.dead)
            && !self.zombies.iter().any(|(_, z)| z.has_head && !z.is_dead_or_dying() && !z.mind_controlled)
    }

    /// `IsFinalScaryPotterStage`.
    pub fn is_final_scary_potter_stage(&self) -> bool {
        self.puzzle_level().is_some_and(|l| !l.is_endless())
    }

    /// `PuzzlePhaseComplete` for endless, `SpawnLevelAward` for a level.
    pub(super) fn scary_potter_complete(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        if self.is_final_scary_potter_stage() {
            self.level_award_spawned = true;
            sounds.play_foley(FoleyType::SpawnSun);
            self.add_coin(
                400.0,
                300.0,
                CoinType::SilverSunflower,
                CoinMotion::Coin,
                effects,
                res,
                images,
                sounds,
            );
            return;
        }
        // Endless: clear the board, count the streak and deal a new one.
        for (_, zombie) in self.zombies.iter_mut() {
            if !zombie.is_dead_or_dying() {
                zombie.die_no_loot(effects);
            }
        }
        self.survival_stage += 1;
        sounds.play_sample("SOUND_HUGE_WAVE");
        let mut rng = MtRand::new(self.board_rand_seed.wrapping_add(self.survival_stage as u32));
        self.scary_potter_populate(effects, res, images, &mut rng);
        let streak = self.survival_stage;
        let message = res.translate("[ADVICE_MORE_SCARY_POTS]").replace("{STREAK}", &streak.to_string());
        self.display_advice(res, &message, MessageStyle::BigMiddle, None);
    }
}
