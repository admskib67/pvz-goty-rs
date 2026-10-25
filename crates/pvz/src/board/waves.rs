//! Zombie waves: which zombies come in each wave, when the waves start, and
//! which row each zombie walks down.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Board.cpp`: `InitZombieWaves`,
//! `InitZombieWavesForLevel`, `PickZombieWaves`, `PutZombieInWave`,
//! `PutInMissingZombies`, `PickZombieType`, `GetIntroducedZombieType`,
//! `IsFlagWave`, `GetNumWavesPerFlag`, `SpawnZombieWave`,
//! `UpdateZombieSpawning`, `NextWaveComing`, `TotalZombiesHealthInWave`,
//! `PickRowForNewZombie`, `RowCanHaveZombies`, `AddZombieInRow`,
//! `UpdateProgressMeter`, `HasProgressMeter`, `ProgressMeterHasFlags` and
//! `DrawProgressMeter`; and
//! `Lawn/TodCommon.cpp` (`TodPickFromSmoothArray`).
//!
//! Only the zombies ported so far (see [`ZombieType::is_implemented`]) are
//! picked. Not yet: the huge wave banner's animated letters, the music's
//! drum burst and zombies rising from graves.

use bevy::prelude::*;
use sexy::data_array::Id;
use sexy::rand::MtRand;

use crate::audio::Sounds;
use crate::reanim::EffectSystem;
use crate::resources::GameResources;
use crate::zombie::{NUM_ZOMBIE_TYPES, ZombieType, can_spawn_on_level, waves_for_level};

use super::message::MessageStyle;
use super::tutorial::{AdviceType, TutorialState};
use super::zombie::{Fx, NUM_BACKUP_DANCERS, Zombie, ZombieEnv};
use crate::graphics::{Graphics, SexyRect};

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
    /// How far the level progress meter has filled, 0..=150.
    pub progress_meter_width: i32,
    /// Counts down from 100 after a flag wave starts, raising the meter's flag.
    pub flag_raise_counter: i32,
    /// Counts down from the "huge wave" warning to the wave itself.
    pub huge_wave_count_down: i32,
    /// Counts down from the "final wave" banner to its sound.
    pub final_wave_sound_counter: i32,
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
            progress_meter_width: 0,
            flag_raise_counter: 0,
            huge_wave_count_down: 0,
            final_wave_sound_counter: 0,
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
        if self.is_whack_a_zombie_level() {
            // Challenge::InitZombieWaves: eight waves out of the graves.
            self.waves.zombie_allowed = [false; NUM_ZOMBIE_TYPES];
            for ty in [ZombieType::Normal, ZombieType::TrafficCone, ZombieType::Pail] {
                self.waves.zombie_allowed[ty as usize] = true;
            }
            self.waves.num_waves = 8;
            self.waves.zombies_in_wave = vec![Vec::new(); 8];
            return;
        }
        self.pick_zombie_waves(rng);
        self.waves.zombie_count_down = if self.is_wallnut_bowling_level() {
            200
        } else if self.is_mini_boss_level() {
            100
        } else if self.first_time_adventure && self.level == 2 {
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
            .find(|ty| *ty != ZombieType::Yeti && *ty != ZombieType::DuckyTube && ty.definition().starting_level == self.level)
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
            if self.is_wallnut_bowling_level() {
                picker.zombie_points *= 4;
            } else if self.is_mini_boss_level() {
                picker.zombie_points *= 3;
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

    /// `Board::RowCanHaveZombieType`: water rows take only swimmers (and
    /// only the pool-only kinds in the first five waves), grass rows never
    /// take those.
    pub fn row_can_have_zombie_type(&self, row: i32, ty: ZombieType) -> bool {
        // Behaviour per ruslan831 decomp, Board::RowCanHaveZombieType.
        if !self.row_can_have_zombies(row) {
            return false;
        }
        if self.plant_row[row as usize] == PlantRowType::Pool {
            if !ty.can_go_in_pool() || (self.waves.current_wave < 5 && !ty.is_pool_only()) {
                return false;
            }
        } else if ty.is_pool_only() {
            return false;
        }
        // The bobsled team only rides on ice.
        if ty == ZombieType::Bobsled && self.ice_timer[row as usize] == 0 {
            return false;
        }
        // Dancers need rows either side for their backup, unless the lawn has a pool.
        ty != ZombieType::Dancer || self.stage_has_pool() || (self.row_can_have_zombies(row - 1) && self.row_can_have_zombies(row + 1))
    }

    /// `Board::PickRowForNewZombie`.
    fn pick_row_for_new_zombie(&mut self, ty: ZombieType, rng: &mut MtRand) -> i32 {
        for row in 0..MAX_GRID_SIZE_Y {
            self.waves.row_picking[row].weight = if !self.row_can_have_zombie_type(row as i32, ty) {
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
    pub(super) fn add_zombie(&mut self, ty: ZombieType, from_wave: i32, fx: &mut Fx) -> Id<Zombie> {
        let row = self.pick_row_for_new_zombie(ty, fx.sounds.rng());
        self.add_zombie_in_row(ty, row, from_wave, fx)
    }

    /// `Board::AddZombieInRow`.
    pub(super) fn add_zombie_in_row(&mut self, ty: ZombieType, row: i32, from_wave: i32, fx: &mut Fx) -> Id<Zombie> {
        let variant = fx.sounds.rng().next_range(5) == 0;
        let env = self.zombie_env();
        let is_flag_wave = self.is_flag_wave(from_wave);
        let zombie = Zombie::new(row, ty, variant, from_wave, is_flag_wave, &env, fx);
        let on_board = zombie.is_on_board();
        let leader = self.zombies.alloc(zombie);
        // A bobsled team is a leader and three zombies behind it on the sled.
        if ty == ZombieType::Bobsled && on_board {
            let leader_x = self.zombies.get(leader).map_or(0.0, |z| z.pos_x);
            let ids: [Id<Zombie>; super::zombie::NUM_BOBSLED_FOLLOWERS] = std::array::from_fn(|position| {
                let mut follower = Zombie::new(row, ty, false, from_wave, is_flag_wave, &env, fx);
                follower.join_bobsled(leader, leader_x, position);
                self.zombies.alloc(follower)
            });
            if let Some(leader) = self.zombies.get_mut(leader) {
                leader.lead_bobsled(ids);
            }
        }
        leader
    }

    /// `Board::CanAddBobSled`: some row has ice for it to ride.
    fn can_add_bobsled(&self) -> bool {
        (0..MAX_GRID_SIZE_Y).any(|row| self.ice_timer[row] > 0 && self.ice_min_x[row] < 700)
    }

    /// Keeps the bobsled teams together: followers see their leader's sled, a
    /// crash tumbles the whole team off, and losing one rider loses the lot.
    pub(super) fn update_bobsleds(&mut self, fx: &mut Fx) {
        let env = self.zombie_env();
        // The leaders' sleds, and who crashed.
        let leaders: Vec<_> = self.zombies.iter().filter(|(_, z)| z.bobsled_position == 0).map(|(id, z)| (id, z.sled_info())).collect();
        for (leader_id, info) in leaders {
            let (crashed, vel_x, rate) = match self.zombies.get_mut(leader_id) {
                Some(leader) => {
                    let crashed = std::mem::take(&mut leader.bobsled_crashed);
                    let rate = leader.body_anim_rate(fx.effects);
                    (crashed, leader.velocity(), rate)
                }
                None => continue,
            };
            let followers: Vec<_> = self.zombies.iter().filter(|(_, z)| z.leader == Some(leader_id) && z.bobsled_position > 0).map(|(id, _)| id).collect();
            for id in followers {
                if let Some(follower) = self.zombies.get_mut(id) {
                    follower.sled = info;
                    if crashed {
                        follower.bobsled_crash_follower(vel_x, rate, &env, fx);
                    }
                }
            }
        }
        // A follower whose leader is gone has lost its sled (BobsledDie).
        let orphans: Vec<_> = self
            .zombies
            .iter()
            .filter(|(_, z)| z.bobsled_position > 0 && !z.dead && z.leader.is_none_or(|l| self.zombies.get(l).is_none_or(|leader| leader.dead)))
            .map(|(id, _)| id)
            .collect();
        for id in orphans {
            if let Some(zombie) = self.zombies.get_mut(id) {
                zombie.die_no_loot(fx.effects);
            }
        }
        // One rider gone takes the whole team (BobsledDie).
        let gone: Vec<_> = self
            .zombies
            .iter()
            .filter(|(_, z)| z.dead && z.bobsled_position >= 0)
            .map(|(id, z)| if z.bobsled_position == 0 { id } else { z.leader.unwrap_or(id) })
            .collect();
        for leader_id in gone {
            for (id, zombie) in self.zombies.iter_mut() {
                let on_team = id == leader_id || zombie.leader == Some(leader_id);
                if on_team && !zombie.dead && zombie.bobsled_position >= 0 {
                    zombie.die_no_loot(fx.effects);
                }
            }
        }
    }

    /// `CutScene::PlaceStreetZombies`: a few zombies shuffling about on the
    /// street, seen as the level intro pans right. The reference doesn't
    /// have this routine, so how many and where is a guess.
    pub(super) fn place_street_zombies(&mut self, fx: &mut Fx) {
        let count = (2 + self.level / 3).min(8);
        for _ in 0..count {
            let ty = self.pick_zombie_type(10, self.waves.num_waves - 1, fx.sounds.rng());
            let mut row = fx.sounds.rng().next_range(MAX_GRID_SIZE_Y as u32) as i32;
            for _ in 0..MAX_GRID_SIZE_Y {
                if self.row_can_have_zombie_type(row, ty) {
                    break;
                }
                row = (row + 1) % MAX_GRID_SIZE_Y as i32;
            }
            self.add_zombie_in_row(ty, row, super::zombie::ZOMBIE_WAVE_CUTSCENE, fx);
            let rng = fx.sounds.rng();
            let (x, jitter) = (rng.range_f32(900.0, 1050.0), rng.range_f32(-15.0, 15.0));
            let y = self.grid_to_pixel_y(0, row) as f32 - 30.0 + jitter;
            if let Some((_, z)) = self.zombies.iter_mut().last() {
                z.place(x, y);
            }
        }
    }

    /// What a dancer's troupe needs to know about each other before they
    /// move (`ZombieNotWalking`, `IsMovingAtChilledSpeed`,
    /// `NeedsMoreBackupDancers`): the board works it out, since a zombie
    /// only sees itself.
    pub(super) fn update_dancer_troupes(&mut self) {
        fn alive(board: &Board, id: Option<Id<Zombie>>) -> Option<&Zombie> {
            id.and_then(|id| board.zombies.get(id)).filter(|z| !z.dead)
        }
        let ids: Vec<_> = self.zombies.iter().map(|(id, _)| id).collect();
        for id in ids {
            let Some(zombie) = self.zombies.get(id) else { continue };
            let leader_id = match zombie.zombie_type {
                ZombieType::Dancer => Some(id),
                ZombieType::BackupDancer => zombie.leader,
                _ => continue,
            };
            let (mut stalled, mut chilled, mut needs_more) = (false, false, false);
            if let Some(leader) = alive(self, leader_id) {
                let members = std::iter::once(leader).chain(leader.followers.iter().filter_map(|&f| alive(self, f)));
                for member in members {
                    stalled |= member.is_eating || member.is_immobilized();
                    chilled |= member.is_chilled();
                }
                if zombie.zombie_type == ZombieType::Dancer {
                    needs_more = (0..NUM_BACKUP_DANCERS).any(|i| {
                        alive(self, leader.followers[i]).is_none() && self.backup_dancer_spot(leader, i).is_some()
                    });
                }
            }
            if let Some(zombie) = self.zombies.get_mut(id) {
                zombie.troupe_stalled = stalled;
                zombie.troupe_chilled = chilled;
                zombie.needs_more_backup = needs_more;
            }
        }
    }

    /// Before zombies move: who each zombie could bite on the other side
    /// (`FindZombieTarget`), since a zombie only sees itself.
    pub(super) fn find_zombie_targets(&mut self) {
        let ids: Vec<_> = self.zombies.iter().map(|(id, _)| id).collect();
        let any_hypnotized = self.zombies.iter().any(|(_, z)| z.mind_controlled && !z.dead);
        for &id in &ids {
            let target = any_hypnotized
                .then(|| {
                    let zombie = self.zombies.get(id)?;
                    self.zombies.iter().find(|&(other, o)| other != id && zombie.can_bite_zombie(o)).map(|(other, _)| other)
                })
                .flatten();
            if let Some(zombie) = self.zombies.get_mut(id) {
                zombie.zombie_target = target;
            }
        }
    }

    /// After zombies move: bites between zombies (`EatZombie`), newly
    /// hypnotized zombies leaving their troupe (`StartMindControlled`), and
    /// hypnotized zombies walking off the lawn (`CheckForBoardEdge`).
    pub(super) fn update_hypnotized_zombies(&mut self, fx: &mut Fx) {
        let env = self.zombie_env();
        let ids: Vec<_> = self.zombies.iter().map(|(id, _)| id).collect();
        for id in ids {
            let Some(zombie) = self.zombies.get_mut(id) else { continue };
            let bitten = zombie.bitten_zombie.take();
            let hypnotized = std::mem::take(&mut zombie.just_hypnotized);
            if zombie.walked_off_lawn() {
                zombie.die_no_loot(fx.effects);
            }
            if hypnotized {
                let (followers, leader) = (std::mem::take(&mut zombie.followers), zombie.leader.take());
                for follower in followers.into_iter().flatten() {
                    if let Some(f) = self.zombies.get_mut(follower) {
                        f.leader = None;
                    }
                }
                if let Some(leader) = leader.and_then(|l| self.zombies.get_mut(l)) {
                    for slot in leader.followers.iter_mut().filter(|f| **f == Some(id)) {
                        *slot = None;
                    }
                }
                // TrySpawnLevelAward: it may have been the last one.
                if let Some(zombie) = self.zombies.get_mut(id) {
                    zombie.award_check = true;
                }
            }
            if let Some(target) = bitten.and_then(|t| self.zombies.get_mut(t)) {
                target.take_damage(super::zombie::DAMAGE_PER_EAT, super::zombie::damage::HEADLESS, &env, fx);
                if target.body_health <= 0 {
                    fx.sounds.play_sample("SOUND_GULP");
                }
            }
        }
    }

    /// Where backup dancer `i` goes: above, below, behind or in front of
    /// its dancer (`SummonBackupDancers`), if that row can have one.
    fn backup_dancer_spot(&self, dancer: &Zombie, i: usize) -> Option<(i32, f32)> {
        let (row, x) = match i {
            0 => (dancer.row - 1, dancer.pos_x),
            1 => (dancer.row + 1, dancer.pos_x),
            2 => (dancer.row, dancer.pos_x - 100.0),
            _ => (dancer.row, dancer.pos_x + 100.0),
        };
        // RowCanHaveZombieType: backup dancers can't swim.
        let ok = self.row_can_have_zombies(row) && self.plant_row[row as usize] != PlantRowType::Pool;
        ok.then_some((row, x))
    }

    /// `Zombie::SummonBackupDancers`, for the dancers that snapped their
    /// fingers this tick: the missing backup dancers rise around them.
    pub(super) fn summon_backup_dancers(&mut self, fx: &mut Fx) {
        let leaders: Vec<_> = self.zombies.iter().filter(|(_, z)| z.summon_requested).map(|(id, _)| id).collect();
        for leader_id in leaders {
            let Some(leader) = self.zombies.get_mut(leader_id) else { continue };
            leader.summon_requested = false;
            for i in 0..NUM_BACKUP_DANCERS {
                let Some(leader) = self.zombies.get(leader_id) else { break };
                if leader.followers[i].and_then(|f| self.zombies.get(f)).is_some_and(|z| !z.dead) {
                    continue;
                }
                let (from_wave, hypnotized) = (leader.from_wave, leader.mind_controlled);
                let Some((row, x)) = self.backup_dancer_spot(leader, i) else {
                    if let Some(leader) = self.zombies.get_mut(leader_id) {
                        leader.followers[i] = None;
                    }
                    continue;
                };
                let id = self.add_zombie(ZombieType::BackupDancer, from_wave, fx);
                let env = self.zombie_env();
                if let Some(dancer) = self.zombies.get_mut(id) {
                    dancer.rise_as_backup_dancer(row, x, leader_id, &env, fx);
                    dancer.mind_controlled = hypnotized;
                }
                if let Some(leader) = self.zombies.get_mut(leader_id) {
                    leader.followers[i] = Some(id);
                }
            }
        }
        // StopZombieSound: the music stops with the last dancer.
        let dancing = self.zombies.iter().any(|(_, z)| {
            matches!(z.zombie_type, ZombieType::Dancer | ZombieType::BackupDancer)
                && z.has_head
                && !z.is_dead_or_dying()
                && z.is_on_board()
        });
        if !dancing {
            fx.sounds.stop_foley(crate::audio::FoleyType::Dancer);
        }
    }

    /// What zombies need to know about the board.
    pub(super) fn zombie_env(&self) -> ZombieEnv {
        ZombieEnv {
            background: self.background,
            level: self.level,
            finished_adventure: !self.first_time_adventure,
            zombie_count: self.zombies.len(),
            bowling: self.is_wallnut_bowling_level(),
            main_counter: self.main_counter,
            award_dropped: self.has_level_award_dropped(),
            pool_rows: self.plant_row.map(|r| r == PlantRowType::Pool),
            ice_min_x: self.ice_min_x,
            zombies_on_screen: self
                .zombies
                .iter()
                .filter(|(_, z)| z.has_head && !z.is_dead_or_dying() && !z.mind_controlled && z.is_on_board())
                .count(),
        }
    }

    /// `Board::TotalZombiesHealthInWave`.
    fn total_zombies_health_in_wave(&self, wave: i32) -> i32 {
        self.zombies
            .iter()
            .filter(|(_, z)| z.from_wave == wave && !z.is_dead_or_dying() && !z.mind_controlled)
            .map(|(_, z)| z.total_health())
            .sum()
    }

    /// `Board::SpawnZombieWave`.
    fn spawn_zombie_wave(&mut self, fx: &mut Fx) {
        let wave = self.waves.current_wave;
        let types = self.waves.zombies_in_wave[wave as usize].clone();
        for ty in types {
            if ty == ZombieType::Bobsled && !self.can_add_bobsled() {
                // No ice to ride: four ordinary zombies come instead of the team.
                for _ in 0..4 {
                    self.add_zombie(ZombieType::Normal, wave, fx);
                }
            } else {
                self.add_zombie(ty, wave, fx);
            }
        }
        // The last wave brings zombies out of the graves.
        if wave == self.waves.num_waves - 1 {
            self.rise_from_grave_counter = 200;
        }
        if self.is_flag_wave(wave) {
            self.waves.flag_raise_counter = 100;
        }
        self.waves.current_wave += 1;
        self.waves.total_spawned_waves += 1;
    }

    /// `Board::NextWaveComing`.
    pub(super) fn next_wave_coming(&mut self, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>, sounds: &mut Sounds) {
        if self.waves.current_wave + 1 == self.waves.num_waves {
            let order = super::make_render_order(super::render_layer::ABOVE_UI, 0, 0);
            match effects.add_reanimation(0.0, 30.0, crate::reanim::ReanimationType::FinalWave, res, images) {
                Ok((_, banner)) => banner.render_order = order,
                Err(e) => error!("{e}"),
            }
            self.waves.final_wave_sound_counter = 60;
        }
        if self.waves.current_wave == 0 {
            sounds.play_sample("SOUND_AWOOGA");
        } else if self.is_siren_wave(self.waves.current_wave) {
            sounds.play_sample("SOUND_SIREN");
        }
    }

    /// Whether the siren sounds before `wave`: a flag wave, or whack-a-zombie's last.
    fn is_siren_wave(&self, wave: i32) -> bool {
        if self.is_whack_a_zombie_level() { wave == self.waves.num_waves - 1 } else { self.is_flag_wave(wave) }
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
        if self.waves.final_wave_sound_counter > 0 {
            self.waves.final_wave_sound_counter -= 1;
            if self.waves.final_wave_sound_counter == 0 {
                sounds.play_sample("SOUND_FINALWAVE");
            }
        }
        if self.has_level_award_dropped() {
            return;
        }
        if self.is_whack_a_zombie_level() {
            let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
            self.whack_spawning(&mut fx);
            return;
        }
        if self.rise_from_grave_counter > 0 {
            self.rise_from_grave_counter -= 1;
            if self.rise_from_grave_counter == 0 {
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                self.spawn_zombies_from_graves(&mut fx);
            }
        }
        if self.waves.huge_wave_count_down > 0 {
            self.waves.huge_wave_count_down -= 1;
            if self.waves.huge_wave_count_down == 0 {
                self.clear_advice(Some(AdviceType::HugeWave));
                self.next_wave_coming(effects, res, images, sounds);
                self.waves.zombie_count_down = 1;
            } else {
                if self.waves.huge_wave_count_down == 725 {
                    sounds.play_sample("SOUND_HUGE_WAVE");
                }
                // TODO: the music's drum burst before the wave.
                return;
            }
        }
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
            if self.is_flag_wave(self.waves.current_wave) {
                self.clear_advice(None);
                self.display_advice_again(res, "[ADVICE_HUGE_WAVE]", MessageStyle::HugeWave, AdviceType::HugeWave);
                self.waves.huge_wave_count_down = 750;
                return;
            }
            self.next_wave_coming(effects, res, images, sounds);
        }
        if self.waves.zombie_count_down == 0 {
            let mut fx = Fx { effects, res, images, sounds: &mut *sounds };
            self.spawn_zombie_wave(&mut fx);
            let wave = self.waves.current_wave;
            self.waves.zombie_health_wave_start = self.total_zombies_health_in_wave(wave - 1);
            let rng = sounds.rng();
            if self.is_flag_wave(wave) && !self.is_wallnut_bowling_level() {
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

impl Board {
    /// `Board::HasProgressMeter`.
    pub fn has_progress_meter(&self) -> bool {
        self.waves.progress_meter_width != 0
    }

    /// `Board::ProgressMeterHasFlags`.
    fn progress_meter_has_flags(&self) -> bool {
        !(self.first_time_adventure && self.level == 1)
    }

    /// `Board::UpdateProgressMeter`: creeps towards where the level is.
    pub(super) fn update_progress_meter(&mut self) {
        let w = &mut self.waves;
        if w.current_wave == 0 || w.num_waves < 2 {
            return;
        }
        if w.flag_raise_counter > 0 {
            w.flag_raise_counter -= 1;
        }
        let mut total_width = 150;
        let per_flag = self.num_waves_per_flag();
        let has_flags = self.progress_meter_has_flags();
        let w = &self.waves;
        if has_flags {
            total_width -= 12 * w.num_waves / per_flag;
        }
        let wave_length = total_width / (w.num_waves - 1);
        let mut current_wave_length = (w.current_wave - 1) * total_width / (w.num_waves - 1);
        let mut next_wave_length = w.current_wave * total_width / (w.num_waves - 1);
        if has_flags {
            let extra = w.current_wave / per_flag * 12;
            current_wave_length += extra;
            next_wave_length += extra;
        }
        // How far through the wave the countdown is, or how much of the
        // wave's zombies have been hurt, whichever is further.
        let mut fraction = (w.zombie_count_down_start - w.zombie_count_down) as f32 / w.zombie_count_down_start as f32;
        if w.zombie_health_to_next_wave != -1 {
            let current = self.total_zombies_health_in_wave(w.current_wave - 1);
            let target = (w.zombie_health_wave_start - w.zombie_health_to_next_wave).max(1);
            let health_fraction = (target - current + w.zombie_health_to_next_wave) as f32 / target as f32;
            fraction = fraction.max(health_fraction);
        }
        let length = (current_wave_length
            + sexy::tod::float_round_to_int((next_wave_length - current_wave_length) as f32 * fraction))
        .clamp(1, 150);
        let delta = length - self.waves.progress_meter_width;
        if (delta > wave_length && self.main_counter % 5 == 0) || (delta > 0 && self.main_counter % 20 == 0) {
            self.waves.progress_meter_width += 1;
        }
    }

    /// `Board::DrawProgressMeter`, for adventure levels.
    pub(super) fn draw_progress_meter(&self, res: &GameResources, g: &mut Graphics) {
        if !self.has_progress_meter() {
            return;
        }
        let (Some(meter), Some(parts)) = (res.image("IMAGE_FLAGMETER"), res.image("IMAGE_FLAGMETERPARTS")) else {
            return;
        };
        g.draw_image_cel(meter, 600.0, 575.0, 0, 0);
        let (cel_w, cel_h) = (meter.cel_width() as i32, meter.cel_height() as i32);
        let clip = sexy::tod::animate_curve(0, 150, self.waves.progress_meter_width, 0, 143, sexy::tod::TodCurve::Linear);
        let src = SexyRect::new((cel_w - clip - 7) as f32, cel_h as f32, clip as f32, cel_h as f32);
        g.draw_image_src(meter, (cel_w - clip + 593) as f32, 575.0, src);

        if self.progress_meter_has_flags() {
            let per_flag = self.num_waves_per_flag();
            let flag_waves = self.waves.num_waves / per_flag;
            let flags_end = 590 + cel_w;
            for flag_wave in 1..=flag_waves {
                let total_waves_at_flag = flag_wave * per_flag;
                let height = if total_waves_at_flag < self.waves.current_wave {
                    14
                } else if total_waves_at_flag == self.waves.current_wave {
                    sexy::tod::animate_curve(100, 0, self.waves.flag_raise_counter, 0, 14, sexy::tod::TodCurve::Linear)
                } else {
                    0
                };
                let x = sexy::tod::animate_curve(0, self.waves.num_waves, total_waves_at_flag, flags_end, 606, sexy::tod::TodCurve::Linear);
                g.draw_image_cel(parts, x as f32, 571.0, 1, 0);
                g.draw_image_cel(parts, x as f32, (572 - height) as f32, 2, 0);
            }
        }

        if let Some(label) = res.image("IMAGE_FLAGMETERLEVELPROGRESS") {
            g.draw_image(label, 638.0, 589.0);
        }
        // The zombie head marks how far along it is.
        let head = sexy::tod::animate_curve(0, 150, self.waves.progress_meter_width, 0, 135, sexy::tod::TodCurve::Linear);
        g.draw_image_cel(parts, (cel_w - head + 580) as f32, 572.0, 0, 0);
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
