//! The bungee zombie's side of the board: where it drops, and what becomes
//! of the plant it had hold of once it's gone.

use sexy::data_array::Id;

use super::plant::{OnBungee, top_plant_any};
use super::zombie::{Fx, Zombie};
use super::{Board, GridSquareType, MAX_GRID_SIZE_X, MAX_GRID_SIZE_Y};
use crate::plant::SeedType;
use crate::reanim::EffectSystem;
use crate::zombie::ZombieType;

impl Board {
    /// `Zombie::PickBungeeZombieTarget`: a bungee zombie drops on a random
    /// square (in `column` if given), almost always one with a plant on it.
    /// With nowhere to go, it's gone.
    pub(super) fn pick_bungee_target(&mut self, id: Id<Zombie>, column: Option<i32>, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::PickBungeeZombieTarget.
        let Some(zombie) = self.zombies.get_mut(id) else { return };
        zombie.target_col = -1;
        // One sun maker is always left alone.
        let allow_sun_makers = self.count_bungees_targeting_sun_makers() != self.count_sun_makers() - 1;
        let mut picks: Vec<(i32, i32, i32)> = Vec::new();
        for x in (0..MAX_GRID_SIZE_X as i32).filter(|&x| column.is_none_or(|c| c == x)) {
            for y in 0..MAX_GRID_SIZE_Y as i32 {
                if self.grave_stone_at(x, y).is_some() || self.grid_square_type[x as usize][y as usize] == GridSquareType::Dirt {
                    continue;
                }
                let mut weight = 1;
                if let Some(plant) = top_plant_any(&self.plants, x, y).and_then(|p| self.plants.get(p)) {
                    if (!allow_sun_makers && plant.seed.makes_sun()) || matches!(plant.seed, SeedType::Gravebuster | SeedType::Cobcannon) {
                        continue;
                    }
                    weight = 10_000;
                }
                if !self.bungee_is_targeting_cell(x, y) {
                    picks.push((x, y, weight));
                }
            }
        }
        // TodPickFromWeightedGridArray.
        let total: i32 = picks.iter().map(|p| p.2).sum();
        let pick = (total > 0)
            .then(|| {
                let mut hit = fx.sounds.rng().next_range(total as u32) as i32;
                picks.iter().find(|p| {
                    hit -= p.2;
                    hit < 0
                })
            })
            .flatten()
            .copied();
        let Some((col, row, _)) = pick else {
            if let Some(zombie) = self.zombies.get_mut(id) {
                zombie.die_no_loot(fx.effects);
            }
            return;
        };
        let env = self.zombie_env();
        let x = self.grid_to_pixel_x(col) as f32;
        if let Some(zombie) = self.zombies.get_mut(id) {
            zombie.set_bungee_target(col, row, x, &env);
        }
    }

    /// `Board::CountSunFlowers`: every plant that makes sun.
    fn count_sun_makers(&self) -> i32 {
        self.plants.iter().filter(|(_, p)| !p.dead && p.seed.makes_sun()).count() as i32
    }

    /// `Zombie::CountBungeesTargetingSunFlowers`.
    fn count_bungees_targeting_sun_makers(&self) -> i32 {
        self.live_bungees()
            .filter(|z| z.target_col != -1)
            .filter(|z| top_plant_any(&self.plants, z.target_col, z.row).and_then(|p| self.plants.get(p)).is_some_and(|p| p.seed.makes_sun()))
            .count() as i32
    }

    /// `Board::BungeeIsTargetingCell`.
    fn bungee_is_targeting_cell(&self, x: i32, y: i32) -> bool {
        self.live_bungees().any(|z| z.row == y && z.target_col == x)
    }

    fn live_bungees(&self) -> impl Iterator<Item = &Zombie> {
        self.zombies.iter().map(|(_, z)| z).filter(|z| z.zombie_type == ZombieType::Bungee && !z.is_dead_or_dying())
    }

    /// `Zombie::BungeeDropPlant` and `BungeeDie`, done by the board since a
    /// zombie can die where it can't reach the plants: once no bungee zombie
    /// has hold of a plant any more, one it was grabbing is let go, and one
    /// it was carrying off is gone.
    pub(super) fn update_bungee_plants(&mut self, effects: &mut EffectSystem) {
        let held: Vec<_> = self.live_bungees().filter_map(|z| z.target_plant).collect();
        for (id, plant) in self.plants.iter_mut() {
            if plant.on_bungee == OnBungee::No || held.contains(&id) {
                continue;
            }
            match plant.on_bungee {
                OnBungee::Grabbed => plant.on_bungee = OnBungee::No,
                OnBungee::Rising => {
                    self.plants_eaten += 1;
                    plant.die(effects);
                }
                OnBungee::No => {}
            }
        }
    }
}
