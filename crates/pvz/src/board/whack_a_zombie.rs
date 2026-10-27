//! Whack-a-zombie (adventure level 2-5): zombies climb out of graves and
//! the player knocks them down with a mallet. Three seed packets (potato
//! mine, grave buster, cherry bomb) help, paid for with the sun that
//! whacked zombies sometimes drop. Graves come back every wave, and the
//! last wave sends twenty.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Challenge.cpp`:
//! `WhackAZombiePlaceGraves`, `WhackAZombieSpawning`, `MouseDownWhackAZombie`,
//! `StartLevel` and `ClearCursor`; `Lawn/CursorObject.cpp` (the mallet
//! cursor) and `Board::DropLootPiece`'s sun; its packet tutorial is included.

use bevy::prelude::*;
use sexy::data_array::Id;
use sexy::rand::MtRand;
use sexy::reanim::LoopType;
use sexy::tod::{TodCurve, animate_curve};

use crate::audio::FoleyType;
use crate::graphics::Graphics;
use crate::particle::ParticleEffect;
use crate::plant::SeedType;
use crate::reanim::{EffectSystem, ReanimationType};
use crate::resources::GameResources;
use crate::zombie::ZombieType;

use super::cursor::CursorType;
use super::grid_item::GridItemType;
use super::zombie::{Fx, Zombie};
use super::{Board, GameMode, MAX_GRID_SIZE_X, MAX_GRID_SIZE_Y, TutorialState, circle_rect_overlap, render_layer};

/// What the board keeps for whack-a-zombie.
#[derive(Debug, Default)]
pub struct WhackState {
    /// The mallet the mouse carries (`mReanimCursorID`).
    pub hammer: Option<crate::reanim::ReanimationId>,
    /// Ticks to the next zombies out of the graves (`mChallengeStateCounter`).
    counter: i32,
}

impl Board {
    /// `LawnApp::IsWhackAZombieLevel`.
    pub fn is_whack_a_zombie_level(&self) -> bool {
        (self.mode == GameMode::Adventure && self.level == 15) || self.minigame() == Some(super::MiniGame::WhackAZombie)
    }

    /// The seed packets whack-a-zombie gives (`InitLevel`).
    pub(super) const WHACK_SEEDS: [SeedType; 3] = [SeedType::Potatomine, SeedType::Gravebuster, SeedType::Cherrybomb];

    /// `CursorObject::CursorObject`: the mallet, ready to swing.
    pub(super) fn add_hammer(&mut self, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>) {
        match effects.add_reanimation(-25.0, 16.0, ReanimationType::Hammer, res, images) {
            Ok((id, hammer)) => {
                hammer.is_attachment = true;
                hammer.play("anim_whack_zombie", LoopType::PlayOnceAndHold, 0, 24.0);
                hammer.anim_time = 1.0;
                self.whack.hammer = Some(id);
            }
            Err(e) => error!("{e}"),
        }
    }

    /// `Challenge::StartLevel`: the mallet in hand, the first zombies soon.
    pub(super) fn start_whack_a_zombie(&mut self) {
        self.cursor.cursor_type = CursorType::Hammer;
        self.waves.zombie_count_down = 200;
        self.waves.zombie_count_down_start = 200;
    }

    /// Every tick: `CursorObject::Update` swings the mallet, and
    /// `Challenge::ClearCursor` puts it back in hand after planting.
    pub(super) fn update_whack_a_zombie(&mut self, effects: &mut EffectSystem) {
        if let Some(hammer) = self.whack.hammer {
            effects.update_reanimation(hammer);
        }
        if self.scene == super::GameScene::Playing
            && self.cursor.cursor_type == CursorType::Normal
            && !self.has_level_award_dropped()
        {
            self.cursor.cursor_type = CursorType::Hammer;
        }
        // `Challenge::WhackAZombieUpdate`: once the first sun is in, the
        // packets flash so the player knows they can be used.
        if self.scene == super::GameScene::Playing {
            if self.sun_money > 0 && self.tutorial_state == TutorialState::Off {
                self.tutorial_state = TutorialState::WhackBeforePickSeed;
                self.tutorial_timer = 1500;
            }
            if self.tutorial_state == TutorialState::WhackBeforePickSeed && self.tutorial_timer == 0 {
                self.tutorial_state = TutorialState::WhackPickSeed;
                self.tutorial_timer = 400;
            }
            if self.tutorial_state == TutorialState::WhackPickSeed && self.tutorial_timer == 0 {
                self.tutorial_state = TutorialState::WhackCompleted;
            }
        }
    }

    /// `Challenge::WhackAZombiePlaceGraves`: graves on random squares from
    /// the fourth column on, preferring empty ones (a plant there is crushed).
    pub(super) fn whack_place_graves(&mut self, count: i32, mut effects: Option<&mut EffectSystem>, rng: &mut MtRand) {
        let mut picks: Vec<(i32, i32, i32)> = Vec::new();
        for col in 3..MAX_GRID_SIZE_X as i32 {
            for row in 0..MAX_GRID_SIZE_Y as i32 {
                if self.can_add_grave_stone_at(col, row) {
                    let taken = self.plants.iter().any(|(_, p)| p.col == col && p.row == row && !p.dead);
                    picks.push((col, row, if taken { 1 } else { 100_000 }));
                }
            }
        }
        for _ in 0..count.min(picks.len() as i32) {
            // TodPickFromWeightedGridArray.
            let total: i32 = picks.iter().map(|p| p.2).sum();
            if total <= 0 {
                break;
            }
            let mut hit = rng.next_range(total as u32) as i32;
            let Some(i) = picks.iter().position(|p| {
                hit -= p.2;
                hit < 0
            }) else {
                break;
            };
            let (col, row, _) = picks[i];
            picks[i].2 = 0;
            if let Some(effects) = effects.as_deref_mut() {
                for (_, plant) in self.plants.iter_mut() {
                    if plant.col == col && plant.row == row {
                        plant.die(effects);
                    }
                }
            }
            self.add_a_grave_stone(col, row, rng);
        }
    }

    /// `Challenge::WhackAZombieSpawning`: each wave brings the graves back
    /// up to five, then zombies climb out of them now and then, faster and
    /// tougher as the waves go on. The last wave sends twenty at once.
    pub(super) fn whack_spawning(&mut self, fx: &mut Fx) {
        let waves = &mut self.waves;
        if waves.current_wave == waves.num_waves && waves.zombie_count_down == 0 {
            return;
        }
        waves.zombie_count_down -= 1;
        if waves.zombie_count_down == 100 && waves.current_wave > 0 {
            let graves = self.grid_items.iter().filter(|(_, g)| g.item_type == GridItemType::GraveStone && !g.dead).count() as i32;
            self.whack_place_graves((5 - graves).max(1), Some(&mut *fx.effects), fx.sounds.rng());
        }
        if self.waves.zombie_count_down == 5 {
            self.next_wave_coming(fx.effects, fx.res, fx.images, fx.sounds);
        }
        if self.waves.zombie_count_down == 0 {
            self.waves.zombie_count_down = 2000;
            self.waves.zombie_count_down_start = 2000;
            self.waves.current_wave += 1;
            self.whack.counter = if self.waves.current_wave == self.waves.num_waves { 300 } else { 1 };
        } else if self.waves.zombie_count_down < 300 {
            return;
        }
        self.whack.counter -= 1;
        if self.whack.counter != 0 {
            return;
        }

        let wave = self.waves.current_wave;
        let phase = ((wave - 1) * 6 / 12).clamp(0, 5) as usize;
        const DOUBLE: [i32; 6] = [0, 30, 10, 10, 15, 18];
        const TRIPLE: [i32; 6] = [0, 0, 0, 0, 10, 13];
        const PAIL: [i32; 6] = [0, 0, 0, 10, 15, 15];
        const CONE: [i32; 6] = [0, 0, 30, 30, 30, 30];
        let rng = fx.sounds.rng();
        let (num_hit, type_hit) = (rng.next_range(100) as i32, rng.next_range(100) as i32);
        let final_wave = wave == self.waves.num_waves;
        let mut count = if final_wave {
            20
        } else if num_hit < TRIPLE[phase] {
            3
        } else if num_hit < TRIPLE[phase] + DOUBLE[phase] {
            2
        } else {
            1
        };
        let mut ty = if type_hit < PAIL[phase] && count < 3 {
            ZombieType::Pail
        } else if type_hit < PAIL[phase] + CONE[phase] {
            ZombieType::TrafficCone
        } else {
            ZombieType::Normal
        };

        // Graves a grave buster isn't eating.
        let mut graves: Vec<(i32, i32)> = self
            .grid_items
            .iter()
            .filter(|(_, g)| g.item_type == GridItemType::GraveStone && !g.dead)
            .filter(|(_, g)| self.normal_plant_at(g.grid_x, g.grid_y).is_none_or(|p| p.seed != SeedType::Gravebuster))
            .map(|(_, g)| (g.grid_x, g.grid_y))
            .collect();
        let mut max_speed = animate_curve(1, 12, wave, 1, 3, TodCurve::EaseIn) as f32;
        count = count.min(graves.len() as i32);
        for _ in 0..count {
            let pick = fx.sounds.rng().next_range(graves.len() as u32) as usize;
            let (col, row) = graves.swap_remove(pick);
            if final_wave {
                ty = if fx.sounds.rng().next_range(2) == 0 { ZombieType::TrafficCone } else { ZombieType::Pail };
                max_speed = 2.0;
            }
            let id = self.add_zombie(ty, wave, fx);
            let env = self.zombie_env();
            let x = self.grid_to_pixel_x(col);
            let speed = fx.sounds.rng().range_f32(0.5, max_speed);
            if let Some(zombie) = self.zombies.get_mut(id) {
                zombie.row = row;
                zombie.rise_out_of_whack_grave(x, speed, &env, fx);
            }
        }
        let min = animate_curve(1, 12, wave, 100, 30, TodCurve::Linear);
        let max = animate_curve(1, 12, wave, 200, 60, TodCurve::Linear);
        self.whack.counter = fx.sounds.rng().range_int(min, max);
        if final_wave {
            self.waves.zombie_count_down = 0;
            self.whack.counter = 0;
        }
    }

    /// `Challenge::MouseDownWhackAZombie`: the mallet comes down on the
    /// topmost zombie under it. A helmet takes the blow; otherwise the
    /// zombie's done for.
    pub(super) fn whack_mouse_down(&mut self, x: i32, y: i32, fx: &mut Fx) {
        if let Some(hammer) = self.whack.hammer.and_then(|h| fx.effects.get_mut(h)) {
            hammer.anim_time = 0.2;
        }
        fx.sounds.play_foley(FoleyType::Swing);
        let mut target: Option<(i32, Id<Zombie>)> = None;
        for (id, zombie) in self.zombies.iter() {
            if !zombie.is_dead_or_dying()
                && circle_rect_overlap(x, y - 20, 45, zombie.zombie_rect())
                && target.is_none_or(|(order, _)| zombie.render_order >= order)
            {
                target = Some((zombie.render_order, id));
            }
        }
        let Some((_, id)) = target else { return };
        let Some(zombie) = self.zombies.get_mut(id) else { return };
        if zombie.whack(fx) {
            self.add_particle((x - 3) as f32, (y + 9) as f32, render_layer::ABOVE_UI, ParticleEffect::Pow);
            self.cursor = super::cursor::CursorObject { cursor_type: CursorType::Hammer, ..Default::default() };
        }
    }

    /// `CursorObject::Draw` for the mallet, at the mouse.
    pub(super) fn draw_hammer(&self, effects: &EffectSystem, res: &GameResources, g: &mut Graphics) {
        let (Some(hammer), Some(p)) = (self.whack.hammer, self.mouse) else { return };
        if self.cursor.cursor_type != CursorType::Hammer || self.scene != super::GameScene::Playing {
            return;
        }
        g.push_state();
        g.translate(p.x.floor() - 25.0, p.y.floor() - 35.0);
        effects.draw(g, hammer, res);
        g.pop_state();
    }
}
