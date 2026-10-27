//! The board's side of the final boss (level 5-10, Dr. Zomboss): the zombie
//! asks for things it can't do alone, and the board does them.
//!
//! Behaviour per ruslan831 decomp: `Lawn/Challenge.cpp` (`PlayBossEnter`,
//! `CountEmptyPotsOrLilies`), `Lawn/Board.cpp` (`GetBossZombie`,
//! `UpdateProgressMeter`'s boss bar) and `Lawn/Zombie.cpp` (`BossSpawnContact`,
//! `BossBungeeSpawn`, `BossBungeeLeave`, `BossAreBungeesDone`, `BossDie`).

use sexy::data_array::Id;

use crate::plant::SeedType;
use crate::zombie::ZombieType;

use super::zombie::{BossRequest, Fx, Zombie};
use super::{Board, GameMode};

impl Board {
    /// `LawnApp::IsFinalBossLevel`.
    pub fn is_final_boss_level(&self) -> bool {
        (self.mode == GameMode::Adventure && self.level == super::FINAL_LEVEL) || self.minigame() == Some(super::MiniGame::FinalBoss)
    }

    /// `Board::GetBossZombie`.
    pub fn boss_zombie(&self) -> Option<Id<Zombie>> {
        self.zombies.iter().find(|(_, z)| z.is_boss()).map(|(id, _)| id)
    }

    /// `Challenge::PlayBossEnter`: Dr. Zomboss steps onto the lawn.
    pub(super) fn play_boss_enter(&mut self, fx: &mut Fx) {
        if self.boss_zombie().is_none() {
            self.add_zombie(ZombieType::Boss, 0, fx);
        }
    }

    /// `Zombie::BossAreBungeesDone`: the bungees he dropped are all gone, or
    /// one is already on its way up.
    pub(super) fn boss_bungees_done(&self) -> bool {
        let Some(boss) = self.boss_zombie().and_then(|id| self.zombies.get(id)) else { return false };
        Zombie::boss_bungees_done(boss.boss_followers().iter().map(|id| id.and_then(|id| self.zombies.get(id)).map(|z| z.phase)))
    }

    /// `Challenge::CountEmptyPotsOrLilies(SEED_FLOWERPOT)`: pots with nothing in them.
    pub(super) fn count_empty_pots(&self) -> i32 {
        self.plants
            .iter()
            .filter(|(_, pot)| !pot.dead && !pot.squished && pot.seed == SeedType::Flowerpot)
            .filter(|(_, pot)| !self.plants.iter().any(|(_, p)| !p.dead && p.seed != SeedType::Flowerpot && p.col == pot.col && p.row == pot.row))
            .count() as i32
    }

    /// The boss's health bar, as the progress meter shows it
    /// (`Board::UpdateProgressMeter`).
    pub(super) fn boss_progress(&self) -> i32 {
        match self.boss_zombie().and_then(|id| self.zombies.get(id)).filter(|z| !z.is_dead_or_dying()) {
            Some(boss) => boss.boss_progress(),
            None => 150,
        }
    }

    /// Carries out what the boss asked of the board this tick.
    pub(super) fn handle_boss_requests(&mut self, requests: Vec<(Id<Zombie>, BossRequest)>, fx: &mut Fx) {
        for (boss_id, request) in requests {
            match request {
                BossRequest::SpawnZombie { ty, row } => {
                    let id = self.add_zombie_in_row(ty, row, 0, fx);
                    if let Some(zombie) = self.zombies.get_mut(id) {
                        zombie.place_x(600.0);
                    }
                }
                BossRequest::SpawnBungees { target_col } => {
                    let mut followers = [None; super::zombie::NUM_BOSS_BUNGEES];
                    for (i, slot) in followers.iter_mut().enumerate() {
                        let id = self.add_zombie_in_row(ZombieType::Bungee, 0, 0, fx);
                        self.pick_bungee_target(id, Some(target_col + i as i32), fx);
                        if let Some(bungee) = self.zombies.get_mut(id) {
                            bungee.drop_from_boss();
                            *slot = Some(id);
                        }
                    }
                    if let Some(boss) = self.zombies.get_mut(boss_id) {
                        boss.set_boss_followers(followers);
                    }
                }
                BossRequest::KillButteredBungees => {
                    let followers = self.zombies.get(boss_id).map(|b| b.boss_followers()).unwrap_or_default();
                    for id in followers.into_iter().flatten() {
                        if let Some(bungee) = self.zombies.get_mut(id).filter(|z| z.is_buttered()) {
                            bungee.die_with_loot(fx);
                        }
                    }
                }
                BossRequest::SquishMowers { row, from_x, to_x } => {
                    for mower in &mut self.lawn_mowers {
                        if mower.row == row
                            && mower.state != super::lawn_mower::LawnMowerState::Squished
                            && mower.pos_x > from_x
                            && mower.pos_x < to_x
                        {
                            mower.squish(fx);
                        }
                    }
                }
                BossRequest::Died => {
                    // He takes everyone with him.
                    for (id, zombie) in self.zombies.iter_mut() {
                        if id != boss_id && !zombie.is_dead_or_dying() {
                            zombie.die_with_loot(fx);
                        }
                    }
                    self.boss_music_fade = true;
                }
            }
        }
    }

    /// On the boss level, the bungees' cords are cut off below his arm
    /// (`Zombie::DrawBungeeCord`).
    pub(super) fn update_boss_cord_clips(&mut self, effects: &crate::reanim::EffectSystem) {
        let boss = self.boss_zombie().and_then(|id| self.zombies.get(id));
        let clips: Vec<(Id<Zombie>, Option<f32>)> = self
            .zombies
            .iter()
            .filter(|(_, z)| z.zombie_type == ZombieType::Bungee)
            .map(|(id, z)| (id, boss.and_then(|b| b.boss_cord_clip(effects, z.target_col))))
            .collect();
        for (id, clip) in clips {
            if let Some(z) = self.zombies.get_mut(id) {
                z.set_cord_clip(clip);
            }
        }
    }
}
