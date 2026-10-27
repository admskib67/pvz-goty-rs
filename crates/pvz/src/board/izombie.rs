//! I, Zombie: the player buys zombies and sends them at five brains while
//! the lawn's plants fight back.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Challenge.cpp`
//! (`IZombieInitLevel`, `IZombiePlacePlants`, `IZombiePlaceZombie`,
//! `IZombieMouseDownWithZombie`, `IZombieUpdate`, `IZombieEatBrain`,
//! `IZombieScoreBrain`), `Lawn/Board.cpp` (`InitLevel`, the zombie seed
//! banks, `CanPlantAt`'s I, Zombie branch) and `Lawn/Plant.cpp` (`GetCost`).
//!
//! Zombie packets show the cached zombie art and cost sun; a zombie goes on
//! its side of the line (bungees on the other). Brains sit at the left edge;
//! eating all five wins. With no zombies, less than 50 sun and nothing
//! mid-action, the puzzle is lost. Endless rolls a new formation each wave
//! (harder with the streak) and remembers it.

use bevy::prelude::*;
use sexy::rand::MtRand;

use crate::plant::SeedType;
use crate::reanim::EffectSystem;
use crate::resources::GameResources;
use crate::zombie::ZombieType;

use super::grid_item::{GridItem, GridItemType};
use super::message::MessageStyle;
use super::zombie::Fx;
use super::{Board, GameMode, MAX_GRID_SIZE_X, MAX_GRID_SIZE_Y, PuzzleLevel, make_render_order, render_layer};

/// `I_ZOMBIE_WINNING_SCORE`: five brains, one per row.
pub const BRAIN_COUNT: i32 = 5;

impl Board {
    /// `Plant::GetCost` for the zombie packets.
    pub fn plant_cost_for_zombie(&self, zombie: ZombieType) -> i32 {
        use ZombieType as Z;
        match zombie {
            Z::Normal | Z::Imp => 50,
            Z::TrafficCone | Z::Polevaulter => 75,
            Z::Door => 100,
            Z::Pail | Z::Digger | Z::Bungee => 125,
            Z::Ladder | Z::Balloon => 150,
            Z::Football | Z::Zamboni => 175,
            Z::Pogo => 200,
            Z::Gargantuar => 300,
            Z::Dancer => 350,
            _ => 100,
        }
    }

    /// Any puzzle-page level.
    pub fn is_puzzle_level(&self) -> bool {
        matches!(self.mode, GameMode::Puzzle(_))
    }

    pub fn is_izombie_level(&self) -> bool {
        matches!(self.mode, GameMode::Puzzle(level) if level.is_izombie())
    }

    fn izombie_level(&self) -> Option<PuzzleLevel> {
        match self.mode {
            GameMode::Puzzle(level) if level.is_izombie() => Some(level),
            _ => None,
        }
    }

    /// `IZombieInitLevel`: the brains, plants and zombie shop.
    pub(super) fn start_izombie(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        rng: &mut MtRand,
    ) {
        if !self.is_izombie_level() {
            return;
        }
        self.sun_money = 150;
        self.seed_bank.num_packets = 0;
        self.seed_bank.update_width(res);
        self.waves = super::waves::WaveState::default();
        self.waves.num_waves = 0;
        self.survival_stage = 0;
        self.izombie_score = 0;
        self.izombie_setup(effects, res, images, rng);
        let message = res.translate("[ADVICE_I_ZOMBIE_EAT_ALL_BRAINS]");
        self.display_advice(res, &message, MessageStyle::HintFast, None);
    }

    pub(super) fn izombie_setup(&mut self, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>, rng: &mut MtRand) {
        // A new round: last formation's plants and its leftovers go away.
        self.plants.clear();
        self.zombies.clear();
        self.grid_items = sexy::data_array::DataArray::default();
        for row in 0..BRAIN_COUNT {
            self.grid_items.alloc(GridItem::izombie_brain(row));
        }
        let level = self.izombie_level().unwrap_or(PuzzleLevel::IZombie(1));
        let packets = izombie_packet_list(level);
        self.seed_bank.num_packets = packets.len();
        for (i, zombie) in packets.into_iter().enumerate() {
            let packet = &mut self.seed_bank.packets[i];
            packet.packet_type = None;
            packet.zombie = Some(zombie);
            packet.active = true;
            packet.refreshing = false;
        }
        self.seed_bank.update_width(res);
        self.izombie_place_plants(level, effects, res, images, rng);
    }

    /// `IZombiePlacePlants`'s `aColumns`: how far left of the line plants go.
    fn izombie_columns(&self) -> i32 {
        match self.izombie_level() {
            Some(PuzzleLevel::IZombie(1..=5)) => 4,
            Some(PuzzleLevel::IZombie(9)) => 6,
            _ => 5,
        }
    }

    fn izombie_place_plants(&mut self, level: PuzzleLevel, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>, rng: &mut MtRand) {
        if level == PuzzleLevel::IZombieEndless {
            self.izombie_place_endless(effects, res, images, rng);
            return;
        }
        let number = match level {
            PuzzleLevel::IZombie(n) => n,
            _ => 9,
        };
        let limit = self.izombie_columns();
        let fixed: &[(SeedType, i32, i32)] = match number {
            1 => &[(SeedType::Sunflower, 3, 2), (SeedType::Sunflower, 3, 3)],
            2 => &[(SeedType::Spikeweed, 3, 0), (SeedType::Sunflower, 2, 0), (SeedType::Sunflower, 3, 3)],
            3 => &[
                (SeedType::Potatomine, 3, 0),
                (SeedType::Sunflower, 2, 0),
                (SeedType::Potatomine, 2, 2),
                (SeedType::Sunflower, 2, 4),
                (SeedType::Torchwood, 3, 3),
            ],
            4 => &[
                (SeedType::Wallnut, 3, 0),
                (SeedType::Sunflower, 2, 0),
                (SeedType::Wallnut, 3, 1),
                (SeedType::Wallnut, 3, 2),
                (SeedType::Sunflower, 2, 2),
                (SeedType::Wallnut, 3, 3),
                (SeedType::Wallnut, 3, 4),
                (SeedType::Sunflower, 2, 4),
            ],
            5 => &[(SeedType::Sunflower, 3, 2), (SeedType::Sunflower, 3, 3)],
            6 => &[(SeedType::Garlic, 4, 1), (SeedType::Garlic, 4, 3)],
            7 => &[(SeedType::Sunflower, 4, 2), (SeedType::Sunflower, 4, 4)],
            8 => &[],
            9 => &[(SeedType::Tallnut, 5, 1), (SeedType::Torchwood, 5, 3)],
            _ => &[],
        };
        for &(seed, col, row) in fixed {
            self.new_plant(col, row, seed, effects, res, images, rng);
        }
        let rows: &[(SeedType, i32, i32)] = match number {
            1 => &[
                (SeedType::Sunflower, 7, -1),
                (SeedType::Squash, 3, -1),
                (SeedType::Peashooter, 6, -1),
                (SeedType::Snowpea, 2, -1),
            ],
            2 => &[
                (SeedType::Spikeweed, 1, 0),
                (SeedType::Peashooter, 1, 0),
                (SeedType::Snowpea, 2, 3),
                (SeedType::Sunflower, 1, 3),
                (SeedType::Sunflower, 4, -1),
                (SeedType::Spikeweed, 2, -1),
                (SeedType::Snowpea, 2, -1),
                (SeedType::Peashooter, 4, -1),
            ],
            3 => &[
                (SeedType::Torchwood, 2, -1),
                (SeedType::Sunflower, 5, -1),
                (SeedType::Peashooter, 7, -1),
                (SeedType::Splitpea, 1, -1),
            ],
            4 => &[
                (SeedType::Peashooter, 1, 0),
                (SeedType::Snowpea, 1, 1),
                (SeedType::Fumeshroom, 2, 2),
                (SeedType::Snowpea, 1, 3),
                (SeedType::Peashooter, 1, 4),
                (SeedType::Peashooter, 2, -1),
                (SeedType::Sunflower, 4, -1),
            ],
            5 => &[
                (SeedType::Cactus, 1, 1),
                (SeedType::Cactus, 1, 4),
                (SeedType::Magnetshroom, 1, -1),
                (SeedType::Sunflower, 5, -1),
                (SeedType::Peashooter, 8, -1),
                (SeedType::Snowpea, 2, -1),
            ],
            6 => &[
                (SeedType::Sunflower, 3, 1),
                (SeedType::Sunflower, 3, 3),
                (SeedType::Torchwood, 2, -1),
                (SeedType::Sunflower, 2, -1),
                (SeedType::Spikeweed, 3, -1),
                (SeedType::Snowpea, 1, -1),
                (SeedType::Peashooter, 5, -1),
                (SeedType::Squash, 2, -1),
                (SeedType::Kernelpult, 2, -1),
            ],
            7 => &[
                (SeedType::Sunflower, 6, -1),
                (SeedType::Potatomine, 9, -1),
                (SeedType::Chomper, 8, -1),
            ],
            8 => &[
                (SeedType::Wallnut, 3, -1),
                (SeedType::Magnetshroom, 2, -1),
                (SeedType::Peashooter, 8, -1),
                (SeedType::Squash, 2, -1),
                (SeedType::Potatomine, 2, -1),
                (SeedType::Sunflower, 8, -1),
            ],
            9 => &[
                (SeedType::Potatomine, 4, 0),
                (SeedType::Sunflower, 2, 0),
                (SeedType::Sunflower, 2, 1),
                (SeedType::Threepeater, 1, 1),
                (SeedType::Snowpea, 1, 1),
                (SeedType::Splitpea, 1, 1),
                (SeedType::Chomper, 3, 2),
                (SeedType::Sunflower, 2, 2),
                (SeedType::Squash, 1, 2),
                (SeedType::Peashooter, 3, 3),
                (SeedType::Sunflower, 2, 3),
                (SeedType::Sunflower, 1, 4),
                (SeedType::Fumeshroom, 1, 4),
                (SeedType::Scaredyshroom, 1, 4),
                (SeedType::Starfruit, 1, 4),
                (SeedType::Splitpea, 1, 4),
                (SeedType::Magnetshroom, 1, 4),
            ],
            _ => &[],
        };
        for &(seed, count, row) in rows {
            self.izombie_place_random(seed, count, row, limit, effects, res, images, rng);
        }
    }

    /// `IZombieInitLevel`'s endless branch: random formations, harder as
    /// the streak grows.
    fn izombie_place_endless(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        rng: &mut MtRand,
    ) {
        use SeedType as S;
        let stage = self.survival_stage;
        let mut puffs = if stage == 0 {
            0
        } else if stage == 1 {
            1
        } else {
            let low = (2 + stage / 3).clamp(2, 4);
            let high = (3 + stage / 2).clamp(2, 6);
            rng.range_int(low, high)
        };
        if stage >= 10 {
            let low = (2 + stage / 3).clamp(2, 5);
            let high = (3 + stage / 2).clamp(2, 7);
            puffs = rng.range_int(low, high);
        }
        let sunflowers = 8 - puffs;
        let limit = self.izombie_columns();
        self.izombie_place_random(S::Sunflower, sunflowers, -1, limit, effects, res, images, rng);
        self.izombie_place_random(S::Puffshroom, puffs, -1, limit, effects, res, images, rng);

        let formation = rng.range_int(0, 4);
        let first: i32;
        let list: &[(S, i32)] = if formation == 0 && stage >= 1 {
            first = rng.range_int(0, 4);
            match first {
                0 => &[(S::Snowpea, 9), (S::Splitpea, 4), (S::Repeater, 4)],
                1 => &[(S::Potatomine, 9), (S::Chomper, 8)],
                2 => &[(S::Spikeweed, 9), (S::Starfruit, 8)],
                3 => &[(S::Fumeshroom, 9), (S::Magnetshroom, 8)],
                _ => &[(S::Scaredyshroom, 12), (S::Sunflower, 5)],
            }
        } else {
            first = rng.range_int(0, 5);
            match first {
                0..=2 => &[
                    (S::Wallnut, 1), (S::Torchwood, 1), (S::Potatomine, 1), (S::Chomper, 2),
                    (S::Peashooter, 1), (S::Splitpea, 1), (S::Kernelpult, 1), (S::Threepeater, 1),
                    (S::Snowpea, 1), (S::Squash, 1), (S::Fumeshroom, 1), (S::Umbrella, 1),
                    (S::Starfruit, 1), (S::Magnetshroom, 1), (S::Spikeweed, 2),
                ],
                3 | 4 => &[
                    (S::Torchwood, 1), (S::Splitpea, 3), (S::Repeater, 1), (S::Kernelpult, 3),
                    (S::Threepeater, 1), (S::Snowpea, 3), (S::Umbrella, 1), (S::Magnetshroom, 1),
                    (S::Spikeweed, 3),
                ],
                _ => &[
                    (S::Potatomine, 4), (S::Chomper, 3), (S::Squash, 3), (S::Fumeshroom, 4),
                    (S::Spikeweed, 3),
                ],
            }
        };
        for &(seed, count) in list {
            self.izombie_place_random(seed, count, -1, limit, effects, res, images, rng);
        }
    }

    /// `IZombiePlacePlants`: empty squares in the allowed columns.
    #[allow(clippy::too_many_arguments)]
    fn izombie_place_random(
        &mut self,
        seed: SeedType,
        count: i32,
        row: i32,
        limit: i32,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        rng: &mut MtRand,
    ) {
        let rows: Vec<i32> = if row == -1 { (0..5).collect() } else { vec![row] };
        let mut cells: Vec<(i32, i32)> = rows
            .into_iter()
            .flat_map(|r| (0..limit).map(move |c| (c, r)))
            .filter(|&(c, r)| {
                // Wall-nuts and torchwoods only go in the rightmost three
                // columns of the allowed area.
                (seed != SeedType::Wallnut && seed != SeedType::Torchwood || limit - c <= 3)
                    && self.normal_plant_at(c, r).is_none()
                    && self.grid_square_type[c as usize][r as usize] != super::GridSquareType::Dirt
            })
            .collect();
        for i in (1..cells.len()).rev() {
            cells.swap(i, rng.next_range((i + 1) as u32) as usize);
        }
        for &(col, row) in cells.iter().take(count.max(0) as usize) {
            self.new_plant(col, row, seed, effects, res, images, rng);
        }
    }

    /// `Challenge::CanPlantAt` for an I, Zombie zombie seed.
    pub fn izombie_can_place(&self, col: i32, row: i32, zombie: ZombieType) -> bool {
        if !(0..MAX_GRID_SIZE_X as i32).contains(&col) || !(0..MAX_GRID_SIZE_Y as i32 - 1).contains(&row) {
            return false;
        }
        let limit = self.izombie_columns();
        if zombie == ZombieType::Bungee { col < limit } else { col >= limit }
    }

    /// Places a bought zombie (`IZombiePlaceZombie`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn izombie_place_zombie(
        &mut self,
        zombie_type: ZombieType,
        col: i32,
        row: i32,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut crate::audio::Sounds,
    ) {
        let mut fx = Fx { effects, res, images, sounds };
        let id = self.add_zombie_in_row(zombie_type, row, 0, &mut fx);
        let x = self.grid_to_pixel_x(col);
        if let Some(zombie) = self.zombies.get_mut(id) {
            if zombie_type == ZombieType::Bungee {
                zombie.target_col = col;
                zombie.render_order = make_render_order(render_layer::GRAVE_STONE, row, 7);
                zombie.place_x(x as f32);
            } else {
                zombie.place_x(x as f32 - 30.0);
            }
        }
    }

    /// `IZombieUpdate` + `IZombieEatBrain` + `IZombieScoreBrain`.
    pub(super) fn update_izombie(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut crate::audio::Sounds,
    ) {
        if !self.is_izombie_level() || self.has_level_award_dropped() || self.puzzle_award_pending {
            return;
        }
        let zombies: Vec<(i32, i32)> = self
            .zombies
            .iter()
            .filter(|(_, z)| !z.is_dead_or_dying() && z.zombie_type != ZombieType::Bungee && !z.is_walking_backwards())
            .map(|(_, z)| (z.row, z.pos_x as i32))
            .collect();
        for (row, x) in zombies {
            if x > 10 {
                continue;
            }
            let brain = self
                .grid_items
                .iter()
                .find(|(_, g)| g.item_type == GridItemType::IzombieBrain && g.grid_y == row && !g.dead)
                .map(|(id, _)| id);
            let Some(brain) = brain else { continue };
            if let Some(item) = self.grid_items.get_mut(brain) {
                item.counter = (item.counter - 1).max(0);
            }
            if self.grid_items.get(brain).is_some_and(|g| g.counter == 0) {
                if let Some(item) = self.grid_items.get_mut(brain) {
                    item.dead = true;
                }
                self.izombie_score += 1;
                sounds.play_sample("SOUND_GULP");
                self.award_brain(res, effects, images, sounds);
            }
        }
        // No zombies, under 50 sun: nothing more can be bought.
        let sun = self.sun_money + self.count_sun_being_collected();
        if sun < 50 && !self.zombies.iter().any(|(_, z)| !z.is_dead_or_dying()) {
            self.pending_game_over = true;
        }
    }

    pub(super) fn award_brain(&mut self, res: &mut GameResources, effects: &mut EffectSystem, images: &mut Assets<Image>, sounds: &mut crate::audio::Sounds) {
        let scored = self.izombie_score;
        self.waves.progress_meter_width =
            sexy::tod::animate_curve(0, BRAIN_COUNT, scored, 0, 500, sexy::tod::TodCurve::Linear);
        if scored < BRAIN_COUNT {
            return;
        }
        if !self.izombie_level().is_some_and(|l| l.is_endless()) {
            self.level_award_spawned = true;
            sounds.play_foley(crate::audio::FoleyType::SpawnSun);
            self.add_coin(
                400.0,
                300.0,
                super::coin::CoinType::SilverSunflower,
                super::coin::CoinMotion::Coin,
                effects,
                res,
                images,
                sounds,
            );
        } else if self.survival_stage % 3 == 0 {
            self.drop_puzzle_award(400.0, 300.0, effects, res, images, sounds);
            self.puzzle_award_pending = true;
        } else {
            self.survival_stage += 1;
            self.izombie_score = 0;
            self.izombie_setup(effects, res, images, &mut MtRand::new(self.board_rand_seed.wrapping_add(self.survival_stage as u32)));
            let message = res.translate("[ADVICE_MORE_IZOMBIE]").replace("{STREAK}", &self.survival_stage.to_string());
            self.display_advice(res, &message, MessageStyle::BigMiddle, None);
        }
    }

}

/// `InitLevel`'s per-level packet lists.
fn izombie_packet_list(level: PuzzleLevel) -> Vec<ZombieType> {
    use ZombieType as Z;
    match level {
        PuzzleLevel::IZombie(1) => vec![Z::Normal, Z::Pail, Z::Football],
        PuzzleLevel::IZombie(2) => vec![Z::Normal, Z::Door, Z::Pail],
        PuzzleLevel::IZombie(3) => vec![Z::Normal, Z::Pail, Z::Digger],
        PuzzleLevel::IZombie(4) => vec![Z::Normal, Z::Pail, Z::Ladder],
        PuzzleLevel::IZombie(5) => vec![Z::Normal, Z::Pail, Z::Bungee, Z::Balloon],
        PuzzleLevel::IZombie(6) => vec![Z::Normal, Z::Polevaulter, Z::Pail, Z::Gargantuar],
        PuzzleLevel::IZombie(7) => vec![Z::Normal, Z::Polevaulter, Z::Pail, Z::Dancer],
        PuzzleLevel::IZombie(8) => vec![Z::Imp, Z::TrafficCone, Z::Pail, Z::Bungee, Z::Digger, Z::Ladder],
        PuzzleLevel::IZombie(9) | PuzzleLevel::IZombieEndless => {
            vec![Z::Imp, Z::TrafficCone, Z::Polevaulter, Z::Pail, Z::Bungee, Z::Digger, Z::Ladder, Z::Football, Z::Dancer]
        }
        _ => vec![Z::Normal],
    }
}
