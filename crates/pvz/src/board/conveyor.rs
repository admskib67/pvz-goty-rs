//! The conveyor belt levels (wall-nut bowling and the mini-boss levels 1-10,
//! 2-10 and 3-10): new seed packets ride in on a timer.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Challenge.cpp`:
//! `UpdateConveyorBelt`. Only the plants ported so far are on the 1-10 belt.

use sexy::rand::MtRand;

use crate::plant::SeedType;

use super::Board;

/// The belt's plants and their weights on a mini-boss level. Plants that
/// aren't ported yet (kernel-pult, ice-shroom, ...) are left out.
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
        // TODO: the night and pool belts need plants from those areas.
        _ => &[(SeedType::Peashooter, 100)],
    }
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

        let mut picks: Vec<(SeedType, i32)> = if self.is_wallnut_bowling_level() {
            vec![(SeedType::Wallnut, 85), (SeedType::ExplodeONut, 15)]
        } else {
            mini_boss_seeds(self.level).to_vec()
        };
        // With more than two choices, a plant that's already plentiful is
        // less likely, and so is repeating the last one.
        if picks.len() > 2 {
            for (seed, weight) in &mut picks {
                let in_bank = self.seed_bank.count_of_type_on_conveyor_belt(*seed);
                if in_bank >= 4 {
                    *weight = 1;
                } else if in_bank >= 3 {
                    *weight = 5;
                } else if Some(*seed) == self.last_conveyor_seed {
                    *weight /= 2;
                }
            }
        }
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
