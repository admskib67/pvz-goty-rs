//! Zombie waves: which zombies come in each wave, when the waves start, and
//! which row each zombie walks down.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Board.cpp`: `InitZombieWaves`,
//! `InitZombieWavesForLevel`, `PickZombieWaves`, `PutZombieInWave`,
//! `PutInMissingZombies`, `PickZombieType`, `GetIntroducedZombieType`,
//! `IsFlagWave`, `GetNumWavesPerFlag`, `SpawnZombieWave`,
//! `UpdateZombieSpawning`, `NextWaveComing`, `TotalZombiesHealthInWave`,
//! `PickRowForNewZombie`, `RowCanHaveZombies`, `AddZombieInRow`; and
//! `Lawn/TodCommon.cpp` (`TodPickFromSmoothArray`).
//!
//! Only the zombies ported so far (see [`ZombieType::is_implemented`]) are
//! picked. Not yet: the huge wave banner and its siren, the final wave
//! banner, zombies rising from graves and the progress meter.

use bevy::prelude::*;
use sexy::rand::MtRand;

use crate::audio::Sounds;
use crate::reanim::EffectSystem;
use crate::resources::GameResources;
use crate::zombie::{NUM_ZOMBIE_TYPES, ZombieType, can_spawn_on_level, waves_for_level};

use super::tutorial::TutorialState;
use super::zombie::{Fx, Zombie, ZombieEnv};
use super::{Board, MAX_GRID_SIZE_Y, PlantRowType};

const ZOMBIE_COUNTDOWN_FIRST_WAVE: i32 = 1800;
const ZOMBIE_COUNTDOWN: i32 = 2500;
const ZOMBIE_COUNTDOWN_RANGE: i32 = 600;
const ZOMBIE_COUNTDOWN_BEFORE_FLAG: i32 = 4500;
const MAX_ZOMBIES_IN_WAVE: usize = 50;

/// `TodSmoothArray`: weights that favour whatever hasn't been picked for a
/// while, so zombies don't come down the same row over and over.
#[derive(Debug, Clone, Copy, Default)]
pub struct SmoothEntry {
    pub weight: f32,
    last_picked: f32,
    second_last_picked: f32,
}

/// `TodCalcSmoothWeight`.
fn calc_smooth_weight(weight: f32, last_picked: f32, second_last_picked: f32) -> f32 {
    if weight < 1e-6 {
        return 0.0;
    }
    let expected1 = 1.0 / weight;
    let expected2 = expected1 * 2.0;
    let advanced1 = last_picked + 1.0 - expected1;
    let advanced2 = second_last_picked + 1.0 - expected2;
    let factor1 = 1.0 + advanced1 / expected1 * 2.0;
    let factor2 = 1.0 + advanced2 / expected2 * 2.0;
    weight * (factor1 * 0.75 + factor2 * 0.25).clamp(0.01, 100.0)
}

/// `TodPickFromSmoothArray`: the index picked.
pub fn pick_from_smooth_array(array: &mut [SmoothEntry], rng: &mut MtRand) -> usize {
    let total: f32 = array.iter().map(|e| e.weight).sum();
    let normalize = 1.0 / total;
    let adjusted = |e: &SmoothEntry| calc_smooth_weight(e.weight * normalize, e.last_picked, e.second_last_picked);
    let total_adjusted: f32 = array.iter().map(adjusted).sum();
    let target = rng.next_f32(total_adjusted);
    let mut accumulated = 0.0;
    let mut picked = array.len() - 1;
    for (k, entry) in array.iter().enumerate().take(array.len() - 1) {
        accumulated += adjusted(entry);
        if target <= accumulated {
            picked = k;
            break;
        }
    }
    for e in array.iter_mut() {
        if e.weight > 0.0 {
            e.last_picked += 1.0;
            e.second_last_picked += 1.0;
        }
    }
    array[picked].second_last_picked = array[picked].last_picked;
    array[picked].last_picked = 0.0;
    picked
}

/// What the wave picker has put in a wave so far (`ZombiePicker`).
#[derive(Default)]
struct ZombiePicker {
    zombie_count: usize,
    zombie_points: i32,
}

/// Where the level is in its waves.
#[derive(Debug, Clone)]
pub struct WaveState {
    pub num_waves: i32,
    pub current_wave: i32,
    pub total_spawned_waves: i32,
    pub zombies_in_wave: Vec<Vec<ZombieType>>,
    pub zombie_allowed: [bool; NUM_ZOMBIE_TYPES],
    pub zombie_count_down: i32,
    pub zombie_count_down_start: i32,
    pub zombie_health_wave_start: i32,
    pub zombie_health_to_next_wave: i32,
    pub row_picking: [SmoothEntry; MAX_GRID_SIZE_Y],
    /// The wave in which each row's mower last ran (`mWaveRowGotLawnMowered`).
    pub wave_row_got_lawn_mowered: [i32; MAX_GRID_SIZE_Y],
}

impl Default for WaveState {
    fn default() -> Self {
        Self {
            num_waves: 0,
            current_wave: 0,
            total_spawned_waves: 0,
            zombies_in_wave: Vec::new(),
            zombie_allowed: [false; NUM_ZOMBIE_TYPES],
            zombie_count_down: 0,
            zombie_count_down_start: 0,
            zombie_health_wave_start: 0,
            zombie_health_to_next_wave: -1,
            row_picking: [SmoothEntry::default(); MAX_GRID_SIZE_Y],
            wave_row_got_lawn_mowered: [-100; MAX_GRID_SIZE_Y],
        }
    }
}

impl Board {
    /// `Board::GetNumWavesPerFlag`.
    fn num_waves_per_flag(&self) -> i32 {
        if self.first_time_adventure && self.waves.num_waves < 10 { self.waves.num_waves } else { 10 }
    }

    /// `Board::IsFlagWave`.
    pub fn is_flag_wave(&self, wave: i32) -> bool {
        if self.first_time_adventure && self.level == 1 {
            return false;
        }
        let per_flag = self.num_waves_per_flag();
        wave % per_flag == per_flag - 1
    }

    /// `Board::InitZombieWaves`.
    pub(super) fn init_zombie_waves(&mut self, rng: &mut MtRand) {
        self.waves = WaveState::default();
        for i in 0..NUM_ZOMBIE_TYPES {
            self.waves.zombie_allowed[i] = can_spawn_on_level(ZombieType::from_index(i), self.level);
        }
        self.pick_zombie_waves(rng);
        self.waves.zombie_count_down = if self.first_time_adventure && self.level == 2 {
            ZOMBIE_COUNTDOWN * 2
        } else {
            ZOMBIE_COUNTDOWN_FIRST_WAVE
        };
        self.waves.zombie_count_down_start = self.waves.zombie_count_down;
    }

    /// `Board::GetIntroducedZombieType`: the zombie that first appears on
    /// this level, if it's ported.
    fn introduced_zombie_type(&self) -> Option<ZombieType> {
        if self.level == 1 {
            return None;
        }
        (0..NUM_ZOMBIE_TYPES)
            .map(ZombieType::from_index)
            .find(|ty| *ty != ZombieType::Yeti && ty.definition().starting_level == self.level)
            .filter(|ty| ty.is_implemented())
    }

    /// `Board::PickZombieWaves`.
    fn pick_zombie_waves(&mut self, rng: &mut MtRand) {
        let mut num_waves = waves_for_level(self.level);
        if !self.first_time_adventure {
            num_waves = if num_waves < 10 { 20 } else { num_waves + 10 };
        }
        self.waves.num_waves = num_waves;
        self.waves.zombies_in_wave = vec![Vec::new(); num_waves as usize];
        let intro = self.introduced_zombie_type();

        for wave in 0..num_waves {
            let mut picker = ZombiePicker::default();
            let is_flag_wave = self.is_flag_wave(wave);
            let is_final_wave = wave == num_waves - 1;

            picker.zombie_points = if !self.first_time_adventure && self.level != 5 { wave * 2 / 5 + 1 } else { wave / 3 + 1 };
            if is_flag_wave {
                let plain = picker.zombie_points.min(8);
                picker.zombie_points = (picker.zombie_points as f32 * 2.5) as i32;
                for _ in 0..plain {
                    self.put_zombie_in_wave(ZombieType::Normal, wave, &mut picker);
                }
                self.put_zombie_in_wave(ZombieType::Flag, wave, &mut picker);
            }

            if let Some(intro) = intro
                && (wave == num_waves / 2 || is_final_wave)
            {
                self.put_zombie_in_wave(intro, wave, &mut picker);
            }
            if is_final_wave {
                // PutInMissingZombies: every kind this level has, at least once.
                for i in 0..NUM_ZOMBIE_TYPES {
                    let ty = ZombieType::from_index(i);
                    if ty != ZombieType::Yeti
                        && can_spawn_on_level(ty, self.level)
                        && !self.waves.zombies_in_wave[wave as usize].contains(&ty)
                    {
                        self.put_zombie_in_wave(ty, wave, &mut picker);
                    }
                }
            }

            while picker.zombie_points > 0 && picker.zombie_count < MAX_ZOMBIES_IN_WAVE {
                let ty = self.pick_zombie_type(picker.zombie_points, wave, rng);
                self.put_zombie_in_wave(ty, wave, &mut picker);
            }
        }
    }

    /// `Board::PutZombieInWave`.
    fn put_zombie_in_wave(&mut self, ty: ZombieType, wave: i32, picker: &mut ZombiePicker) {
        self.waves.zombies_in_wave[wave as usize].push(ty);
        picker.zombie_count += 1;
        picker.zombie_points -= ty.definition().zombie_value;
    }

    /// `Board::PickZombieType`: a weighted pick among the zombies this wave
    /// can afford.
    fn pick_zombie_type(&self, points: i32, wave: i32, rng: &mut MtRand) -> ZombieType {
        let choices: Vec<(ZombieType, i32)> = (0..NUM_ZOMBIE_TYPES)
            .map(ZombieType::from_index)
            .filter(|ty| self.waves.zombie_allowed[*ty as usize])
            .filter(|ty| {
                let d = ty.definition();
                wave + 1 >= d.first_allowed_wave && points >= d.zombie_value
            })
            .map(|ty| (ty, ty.definition().pick_weight))
            .collect();
        let total: i32 = choices.iter().map(|(_, w)| w).sum();
        let mut roll = rng.next_range(total.max(1) as u32) as i32;
        for (ty, weight) in &choices {
            roll -= weight;
            if roll < 0 {
                return *ty;
            }
        }
        ZombieType::Normal
    }

    /// `Board::RowCanHaveZombies`.
    pub fn row_can_have_zombies(&self, row: i32) -> bool {
        (0..MAX_GRID_SIZE_Y as i32).contains(&row) && self.plant_row[row as usize] != PlantRowType::Dirt
    }

    /// `Board::PickRowForNewZombie`.
    fn pick_row_for_new_zombie(&mut self, rng: &mut MtRand) -> i32 {
        for row in 0..MAX_GRID_SIZE_Y {
            self.waves.row_picking[row].weight = if !self.row_can_have_zombies(row as i32) {
                0.0
            } else {
                let mowered = self.waves.current_wave - self.waves.wave_row_got_lawn_mowered[row];
                match mowered {
                    ..=1 => 0.01,
                    2 => 0.5,
                    _ => 1.0,
                }
            };
        }
        pick_from_smooth_array(&mut self.waves.row_picking, rng) as i32
    }

    /// `Board::AddZombie`.
    pub(super) fn add_zombie(&mut self, ty: ZombieType, from_wave: i32, fx: &mut Fx) {
        let row = self.pick_row_for_new_zombie(fx.sounds.rng());
        self.add_zombie_in_row(ty, row, from_wave, fx);
    }

    /// `Board::AddZombieInRow`.
    pub(super) fn add_zombie_in_row(&mut self, ty: ZombieType, row: i32, from_wave: i32, fx: &mut Fx) {
        let variant = fx.sounds.rng().next_range(5) == 0;
        let env = self.zombie_env();
        let is_flag_wave = self.is_flag_wave(from_wave);
        let zombie = Zombie::new(row, ty, variant, from_wave, is_flag_wave, &env, fx);
        self.zombies.alloc(zombie);
    }

    /// What zombies need to know about the board.
    pub(super) fn zombie_env(&self) -> ZombieEnv {
        ZombieEnv {
            background: self.background,
            level: self.level,
            finished_adventure: !self.first_time_adventure,
            zombie_count: self.zombies.len(),
            zombies_on_screen: self.zombies.iter().filter(|(_, z)| !z.is_dead_or_dying() && z.x < super::BOARD_WIDTH).count(),
        }
    }

    /// `Board::TotalZombiesHealthInWave`.
    fn total_zombies_health_in_wave(&self, wave: i32) -> i32 {
        self.zombies
            .iter()
            .filter(|(_, z)| z.from_wave == wave && !z.is_dead_or_dying())
            .map(|(_, z)| z.total_health())
            .sum()
    }

    /// `Board::SpawnZombieWave`.
    fn spawn_zombie_wave(&mut self, fx: &mut Fx) {
        let wave = self.waves.current_wave;
        let types = self.waves.zombies_in_wave[wave as usize].clone();
        for ty in types {
            self.add_zombie(ty, wave, fx);
        }
        // TODO: the last wave brings zombies out of the graves (mRiseFromGraveCounter).
        // TODO: flag waves raise the progress meter's flag.
        self.waves.current_wave += 1;
        self.waves.total_spawned_waves += 1;
    }

    /// `Board::NextWaveComing`.
    fn next_wave_coming(&self, sounds: &mut Sounds) {
        // TODO: the final wave's banner.
        if self.waves.current_wave == 0 {
            sounds.play_sample("SOUND_AWOOGA");
        } else if self.is_flag_wave(self.waves.current_wave) {
            sounds.play_sample("SOUND_SIREN");
        }
    }

    /// `Board::UpdateZombieSpawning`.
    pub(super) fn update_zombie_spawning(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        if matches!(
            self.tutorial_state,
            TutorialState::Level1PickUpPeashooter | TutorialState::Level1PlantPeashooter | TutorialState::Level1RefreshPeashooter
        ) {
            return;
        }
        // TODO: HasLevelAwardDropped, the huge wave countdown.
        if self.waves.current_wave == self.waves.num_waves {
            return;
        }
        self.waves.zombie_count_down -= 1;
        if self.waves.zombie_count_down > 200
            && self.waves.zombie_count_down_start - self.waves.zombie_count_down > 400
            && self.total_zombies_health_in_wave(self.waves.current_wave - 1) <= self.waves.zombie_health_to_next_wave
        {
            self.waves.zombie_count_down = 200;
        }
        if self.waves.zombie_count_down == 5 {
            // TODO: flag waves show the huge wave banner first.
            self.next_wave_coming(sounds);
        }
        if self.waves.zombie_count_down == 0 {
            let mut fx = Fx { effects, res, images, sounds: &mut *sounds };
            self.spawn_zombie_wave(&mut fx);
            let wave = self.waves.current_wave;
            self.waves.zombie_health_wave_start = self.total_zombies_health_in_wave(wave - 1);
            let rng = sounds.rng();
            if self.is_flag_wave(wave) {
                self.waves.zombie_health_to_next_wave = 0;
                self.waves.zombie_count_down = ZOMBIE_COUNTDOWN_BEFORE_FLAG;
            } else {
                self.waves.zombie_health_to_next_wave =
                    (rng.range_f32(0.5, 0.65) * self.waves.zombie_health_wave_start as f32) as i32;
                self.waves.zombie_count_down = ZOMBIE_COUNTDOWN + rng.next_range(ZOMBIE_COUNTDOWN_RANGE as u32) as i32;
            }
            self.waves.zombie_count_down_start = self.waves.zombie_count_down;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooth_array_never_picks_zero_weight_items() {
        let mut rng = MtRand::new(1);
        let mut array = [SmoothEntry::default(); MAX_GRID_SIZE_Y];
        array[2].weight = 1.0;
        for _ in 0..50 {
            assert_eq!(pick_from_smooth_array(&mut array, &mut rng), 2);
        }
    }

    #[test]
    fn smooth_array_spreads_picks_out() {
        let mut rng = MtRand::new(7);
        let mut array = [SmoothEntry::default(); MAX_GRID_SIZE_Y];
        array.iter_mut().for_each(|e| e.weight = 1.0);
        let mut counts = [0; MAX_GRID_SIZE_Y];
        for _ in 0..600 {
            counts[pick_from_smooth_array(&mut array, &mut rng)] += 1;
        }
        // Every row gets about its share, no row more than a third over.
        assert!(counts.iter().all(|&c| (70..=130).contains(&c)), "{counts:?}");
    }
}
