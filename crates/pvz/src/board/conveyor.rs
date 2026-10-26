//! The conveyor belt levels (wall-nut bowling, Big Trouble Little Zombie
//! 3-5, and the mini-boss levels 1-10, 2-10 and 3-10): new seed packets ride
//! in on a timer.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Challenge.cpp`:
//! `UpdateConveyorBelt` and its belts for wall-nut bowling, Little Trouble,
//! 1-10, 2-10 and 3-10.

use sexy::rand::MtRand;
use sexy::tod::{TodCurve, animate_curve};

use crate::plant::SeedType;

use super::Board;
use super::grid_item::GridItemType;

/// Wall-nut bowling's belt.
const WALLNUT_BOWLING_SEEDS: &[(SeedType, i32)] = &[(SeedType::Wallnut, 85), (SeedType::ExplodeONut, 15)];

/// Big Trouble Little Zombie's belt: the lily pad's weight falls as more are
/// planted (see `update_conveyor_belt`).
const LITTLE_TROUBLE_SEEDS: &[(SeedType, i32)] =
    &[(SeedType::Lilypad, 25), (SeedType::Wallnut, 15), (SeedType::Peashooter, 25), (SeedType::Cherrybomb, 35)];

/// The belt's plants and their weights on this level.
pub fn belt_seeds(board: &Board) -> &'static [(SeedType, i32)] {
    if board.is_wallnut_bowling_level() {
        WALLNUT_BOWLING_SEEDS
    } else if board.is_little_trouble_level() {
        LITTLE_TROUBLE_SEEDS
    } else {
        mini_boss_seeds(board.level)
    }
}

/// The belt's plants and their weights on a mini-boss level.
pub fn mini_boss_seeds(level: i32) -> &'static [(SeedType, i32)] {
    match level {
        10 => &[
            (SeedType::Peashooter, 20),
            (SeedType::Cherrybomb, 20),
            (SeedType::Wallnut, 15),
            (SeedType::Repeater, 20),
            (SeedType::Snowpea, 10),
            (SeedType::Chomper, 5),
            (SeedType::Potatomine, 10),
        ],
        20 => &[
            (SeedType::Gravebuster, 20),
            (SeedType::Iceshroom, 15),
            (SeedType::Doomshroom, 15),
            (SeedType::Hypnoshroom, 10),
            (SeedType::Scaredyshroom, 15),
            (SeedType::Fumeshroom, 15),
            (SeedType::Puffshroom, 10),
        ],
        // Behaviour per ruslan831 decomp, Challenge::UpdateConveyorBelt, level 30:
        // the pool's plants, most of them from the water (lily pads and tangle kelp).
        30 => &[
            (SeedType::Lilypad, 25),
            (SeedType::Squash, 5),
            (SeedType::Threepeater, 25),
            (SeedType::Tanglekelp, 5),
            (SeedType::Jalapeno, 10),
            (SeedType::Spikeweed, 10),
            (SeedType::Torchwood, 10),
            (SeedType::Tallnut, 10),
        ],
        _ => &[(SeedType::Peashooter, 100)],
    }
}

/// The weights `Challenge::UpdateConveyorBelt` picks from. `planted` counts
/// the plants on the board of a type, `in_bank` the packets on the belt and
/// `graves` the grave stones. The lily pad's weight falls from its table
/// value to 1 over 18 of them; a gravebuster is only picked while there are
/// more grave stones than gravebusters; with more than two choices a plant
/// with 4 (3) on the belt has weight 1 (5), and otherwise repeating the last
/// pick halves a weight.
pub fn belt_weights(
    seeds: &[(SeedType, i32)],
    planted: impl Fn(SeedType) -> i32,
    in_bank: impl Fn(SeedType) -> i32,
    graves: i32,
    last: Option<SeedType>,
) -> Vec<(SeedType, i32)> {
    let many = seeds.len() > 2;
    seeds
        .iter()
        .map(|&(seed, table)| {
            let on_belt = in_bank(seed);
            let total = planted(seed) + on_belt;
            let mut weight = table;
            match seed {
                SeedType::Gravebuster if graves <= total => return (seed, 0),
                SeedType::Lilypad => weight = animate_curve(0, 18, total, weight, 1, TodCurve::Linear),
                _ => {}
            }
            if many {
                if on_belt >= 4 {
                    weight = 1;
                } else if on_belt >= 3 {
                    weight = 5;
                } else if Some(seed) == last {
                    weight /= 2;
                }
            }
            (seed, weight)
        })
        .collect()
}

impl Board {
    /// `Challenge::UpdateConveyorBelt`: every so often a new packet rides in.
    pub(super) fn update_conveyor_belt(&mut self, rng: &mut MtRand) {
        if self.has_level_award_dropped() {
            return;
        }
        self.seed_bank.update_conveyor_belt();
        self.conveyor_belt_counter -= 1;
        if self.conveyor_belt_counter > 0 {
            return;
        }
        let on_belt = self.seed_bank.num_seeds_on_conveyor_belt();
        self.conveyor_belt_counter = if on_belt > 8 {
            1000
        } else if on_belt > 6 {
            500
        } else if on_belt > 4 {
            425
        } else {
            400
        };

        let graves = self.grid_items.iter().filter(|(_, g)| g.item_type == GridItemType::GraveStone && !g.dead).count() as i32;
        let picks = belt_weights(
            belt_seeds(self),
            |seed| self.plants.iter().filter(|(_, p)| !p.dead && p.seed == seed).count() as i32,
            |seed| self.seed_bank.count_of_type_on_conveyor_belt(seed) as i32,
            graves,
            self.last_conveyor_seed,
        );
        // TodPickFromWeightedArray.
        let total: i32 = picks.iter().map(|&(_, w)| w).sum();
        let mut roll = rng.next_range(total as u32) as i32;
        let mut chosen = picks[0].0;
        for &(seed, weight) in &picks {
            roll -= weight;
            if roll < 0 {
                chosen = seed;
                break;
            }
        }
        self.seed_bank.add_seed(chosen);
        self.last_conveyor_seed = Some(chosen);
    }
}
