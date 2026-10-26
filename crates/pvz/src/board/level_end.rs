//! How a level ends: the last zombie drops a seed packet that fades the
//! level out (won), or a zombie gets into the house (lost).
//!
//! Behaviour per ruslan831 decomp, `Lawn/Board.cpp`: `HasLevelAwardDropped`,
//! `AreEnemyZombiesOnScreen`, `FadeOutLevel`, `UpdateLevelEndSequence`,
//! `DrawFadeOut`, `ZombiesWon` and `RemoveAllZombies`; `Lawn/Zombie.cpp`:
//! `TrySpawnLevelAward`; `LawnApp.cpp`: `UpdatePlayerProfileForFinishingLevel`
//! and `CheckForGameEnd`.
//!
//! Not yet: the other awards (shovel, almanac, trophies, money bags), the
//! money coins, the award screen between levels, and the roof and boss
//! lawns' endings (the pool lawn's house position is in `Zombie::walk_into_house`).

use bevy::prelude::*;
use sexy::tod::{TodCurve, animate_curve};

use crate::audio::{FoleyType, Music, Sounds};
use crate::graphics::{Graphics, SexyColor};
use crate::plant::SeedType;
use crate::player::ProfileMgr;
use crate::reanim::EffectSystem;
use crate::resources::GameResources;

use super::coin::{CoinMotion, CoinType};
use super::{BOARD_HEIGHT, BOARD_WIDTH, Board, GameScene};

impl Board {
    /// `Board::HasLevelAwardDropped`.
    pub fn has_level_award_dropped(&self) -> bool {
        self.level_award_spawned || self.board_fade_out_counter >= 0
    }

    /// `Board::AreEnemyZombiesOnScreen`.
    fn are_enemy_zombies_on_screen(&self) -> bool {
        self.zombies.iter().any(|(_, z)| z.has_head && !z.is_dead_or_dying() && !z.mind_controlled)
    }

    /// Zombies that just died ask whether they were the last one
    /// (`Zombie::DropLoot` → `TrySpawnLevelAward`).
    pub(super) fn collect_zombie_loot(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        // A zombie that's been hypnotized may have been the last one against you.
        let turned: Vec<_> = self.zombies.iter().filter(|(_, z)| z.award_check).map(|(id, _)| id).collect();
        for id in turned {
            let Some(zombie) = self.zombies.get_mut(id) else { continue };
            zombie.award_check = false;
            let rect = zombie.zombie_rect();
            let center = (rect.x + rect.w / 2, rect.y + rect.h / 2);
            if self.try_spawn_level_award(id, center, effects, res, images, sounds) {
                return;
            }
        }
        let asking: Vec<_> = self.zombies.iter().filter(|(_, z)| z.loot_pending).map(|(id, _)| id).collect();
        for id in asking {
            let Some(zombie) = self.zombies.get_mut(id) else { continue };
            zombie.loot_pending = false;
            // Zombie::DropLoot: the almanac learns about it.
            self.defeated_zombies[zombie.zombie_type as usize] = true;
            let Some(zombie) = self.zombies.get(id) else { continue };
            let rect = zombie.zombie_rect();
            let center = (rect.x + rect.w / 2, rect.y + rect.h / 2);
            if self.try_spawn_level_award(id, center, effects, res, images, sounds) {
                break;
            }
            // Zombie::DropLoot.
            let blocked = self.level_award_spawned || self.board_fade_out_counter >= 0 || !self.can_drop_loot();
            // Behaviour per ruslan831 decomp, Zombie::DropLoot: on Little
            // Trouble only one zombie in four drops anything.
            let little_trouble = self.is_little_trouble_level();
            let Some(zombie) = self.zombies.get_mut(id) else { continue };
            if zombie.dropped_loot() || blocked {
                continue;
            }
            zombie.set_dropped_loot();
            if little_trouble && sounds.rng().next_range(4) != 0 {
                continue;
            }
            let value = zombie.zombie_type.definition().zombie_value;
            let (x, y) = (center.0, zombie.zombie_rect().y + zombie.zombie_rect().h / 4);
            self.drop_loot_piece(x, y, value, effects, res, images, sounds);
        }
    }

    /// `Zombie::TrySpawnLevelAward`.
    fn try_spawn_level_award(
        &mut self,
        zombie: sexy::data_array::Id<super::zombie::Zombie>,
        center: (i32, i32),
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) -> bool {
        let dropped = self.zombies.get(zombie).is_some_and(|z| z.dropped_loot());
        if self.has_level_award_dropped()
            || self.level_complete
            || dropped
            || self.waves.current_wave < self.waves.num_waves
            || self.are_enemy_zombies_on_screen()
            || (self.is_whack_a_zombie_level() && self.waves.zombie_count_down > 0)
        {
            return false;
        }
        self.level_award_spawned = true;
        // RemoveAllZombies.
        for (_, z) in self.zombies.iter_mut() {
            if !z.is_dead_or_dying() {
                z.die_no_loot(effects);
            }
        }
        if let Some(z) = self.zombies.get_mut(zombie) {
            z.set_dropped_loot();
        }
        if self.first_time_adventure {
            // TODO: levels 9, 14, ... and 50 award a note, an almanac and so
            // on instead; finishing adventure gives a money bag.
            sounds.play_foley(FoleyType::SpawnSun);
            let (x, y) = (center.0 as f32, center.1 as f32);
            let ty = match self.level {
                9 | 19 | 29 | 39 | 49 => CoinType::Note,
                4 => CoinType::Shovel,
                14 => CoinType::Almanac,
                24 => CoinType::CarKeys,
                34 => CoinType::Taco,
                44 => CoinType::WateringCan,
                _ => CoinType::FinalSeedPacket,
            };
            self.add_coin(x, y, ty, CoinMotion::Coin, effects, res, images, sounds);
            let seed = SeedType::from_index(crate::plant::award_seed_for_level(self.level));
            if let Some(seed) = seed
                && let Err(e) = res.reanim_definition(seed.definition().reanim.file(), images)
            {
                error!("{e}");
            }
            if let Some((_, coin)) = self.coins.iter_mut().find(|(_, c)| c.ty == CoinType::FinalSeedPacket) {
                coin.award_seed = seed;
            }
        } else {
            // TODO: a money bag to click.
            self.pending_fade_out = true;
        }
        true
    }

    /// `Board::FadeOutLevel`: the level is won.
    pub(super) fn fade_out_level(&mut self, music: &mut Music, sounds: &mut Sounds, res: &GameResources) {
        self.pending_fade_out = false;
        music.stop_all_music();
        sounds.play_foley(FoleyType::WinMusic);
        self.refresh_seed_packet_from_cursor(res);
        self.board_fade_out_counter = 600;
        // TryAutoCollectAfterLevelAward: sun on the lawn flies to the bank.
        for (_, coin) in self.coins.iter_mut() {
            if !coin.is_level_award() && !coin.dead && !coin.is_being_collected {
                sounds.play_foley(FoleyType::Sun);
                coin.collect();
            }
        }
        // TODO: the mowers that never ran turn into money (CanDropLoot).
    }

    /// `Board::UpdateLevelEndSequence`.
    pub(super) fn update_level_end_sequence(&mut self, sounds: &mut Sounds) {
        if self.board_fade_out_counter < 0 {
            return;
        }
        self.board_fade_out_counter -= 1;
        if self.board_fade_out_counter == 0 {
            self.level_complete = true;
            return;
        }
        if self.board_fade_out_counter == 300 {
            sounds.play_sample("SOUND_LIGHTFILL");
        }
    }

    /// `Board::DrawFadeOut`: the screen goes white as the level ends.
    pub(super) fn draw_fade_out(&self, g: &mut Graphics) {
        if self.board_fade_out_counter < 0 {
            return;
        }
        let alpha = animate_curve(200, 0, self.board_fade_out_counter, 0, 255, TodCurve::Linear);
        g.push_state();
        g.set_color(SexyColor::rgba(255, 255, 255, alpha.clamp(0, 255) as u8));
        g.fill_rect(0.0, 0.0, BOARD_WIDTH as f32, BOARD_HEIGHT as f32);
        g.pop_state();
    }

    /// `Board::ZombiesWon`: a zombie got into the house.
    pub(super) fn zombies_won(
        &mut self,
        zombie: sexy::data_array::Id<super::zombie::Zombie>,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        music: &mut Music,
        sounds: &mut Sounds,
    ) {
        if self.scene == GameScene::ZombiesWon {
            return;
        }
        self.clear_advice(None);
        self.scene = GameScene::ZombiesWon;
        let env = self.zombie_env();
        if self.zombies.get(zombie).is_some() {
            let background = self.background;
            let mut fx = super::zombie::Fx { effects, res, images, sounds };
            if let Some(z) = self.zombies.get_mut(zombie) {
                z.walk_into_house(background, &env, &mut fx);
            }
        }
        // CutScene::StartZombiesWon.
        self.cut_scene.start_zombies_won();
        self.menu_button.no_draw = true;
        self.show_shovel = false;
        music.stop_all_music();
        sounds.play_sample("SOUND_LOSEMUSIC");
        self.tutorial_arrow_remove();
        self.refresh_seed_packet_from_cursor(res);
    }

    /// `LawnApp::UpdatePlayerProfileForFinishingLevel` + `CheckForGameEnd`
    /// for adventure mode: the player moves on to the next level.
    pub(super) fn finish_level(&mut self, profiles: &mut ProfileMgr) {
        self.level_complete = false;
        if self.level_overridden {
            return;
        }
        if let Some(player) = profiles.player_mut() {
            if self.level == super::FINAL_LEVEL {
                player.level = 1;
                player.finished_adventure += 1;
            } else {
                player.level = self.level + 1;
            }
        }
        profiles.save_current_details();
    }
}
