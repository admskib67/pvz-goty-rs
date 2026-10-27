//! Things on the lawn's squares besides plants (`GridItem`). So far the
//! gravestones of the night levels (they rise out of the ground during the
//! level intro, can't be planted on except by grave busters, and on the
//! last wave zombies climb out of them) and the craters doom-shrooms leave.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Board.cpp` (`Board::Board`'s grid
//! cels, `GetLevelRandSeed`, `PickBackground`'s graves, `GetGraveStoneAt`,
//! `CanAddGraveStoneAt`, `AddAGraveStone`, `AddGraveStones`,
//! `PickSpecialGraveStone`, `UpdateGridItems`,
//! `PickGraveRisingZombieType`, `SpawnZombiesFromGraves`) and
//! `Lawn/GridItem.cpp` (`DrawGraveStone`, `AddGraveStoneParticles`,
//! `DrawCrater`), and `Board::AddACrater` and `GetCraterAt`.

use bevy::prelude::*;
use sexy::data_array::Id;
use sexy::rand::MtRand;
use sexy::tod::{TodCurve, animate_curve, animate_curve_float};

use crate::audio::FoleyType;
use crate::graphics::{Graphics, SexyRect};
use crate::particle::ParticleEffect;
use crate::reanim::ReanimationType;
use crate::resources::GameResources;
use crate::zombie::ZombieType;

use super::zombie::{Fx, IRect, Zombie, rect_overlap};
use super::{Board, GridSquareType, MAX_GRID_SIZE_X, MAX_GRID_SIZE_Y, PlantRowType, make_render_order, render_layer};

/// `GridItemType`, for what's ported so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridItemType {
    GraveStone,
    /// A doom-shroom's hole: nothing can be planted there for a while.
    Crater,
    /// A ladder zombie's ladder, leaning on a plant for zombies to climb over.
    Ladder,
    /// The store's rake: swings at the first zombie to reach it.
    Rake,
}

/// `GridItemState`, for what's ported so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridItemState {
    Normal,
    /// The one grave per level drawn with the special look (`GRIDITEM_STATE_GRAVESTONE_SPECIAL`).
    GravestoneSpecial,
    /// The rake waiting for a zombie (`GRIDITEM_STATE_RAKE_ATTRACTING`).
    RakeAttracting,
    /// The rake swinging; it goes once its countdown ends.
    RakeTriggered,
}

#[derive(Debug, Clone)]
pub struct GridItem {
    pub item_type: GridItemType,
    pub state: GridItemState,
    pub grid_x: i32,
    pub grid_y: i32,
    pub render_order: i32,
    /// `mGridItemCounter`: a grave rises from below 0 up to 100.
    counter: i32,
    /// The rake's animation (`mGridItemReanimID`).
    reanim: Option<crate::reanim::ReanimationId>,
    pub dead: bool,
}

/// `mGridCelLook` and `mGridCelOffset`: each square's random look (which
/// grave picture) and nudge, picked when the board is made.
#[derive(Debug, Clone)]
pub struct GridCels {
    look: [[i32; MAX_GRID_SIZE_Y]; MAX_GRID_SIZE_X],
    offset: [[(i32, i32); MAX_GRID_SIZE_Y]; MAX_GRID_SIZE_X],
}

impl GridCels {
    /// How a square's cel looks (`mGridCelLook`), 0 to 19.
    pub(super) fn look(&self, x: usize, y: usize) -> i32 {
        self.look[x][y]
    }

    /// `Board::Board`'s loop over the squares.
    pub fn new(rng: &mut MtRand) -> Self {
        let mut cels = Self { look: [[0; MAX_GRID_SIZE_Y]; MAX_GRID_SIZE_X], offset: [[(0, 0); MAX_GRID_SIZE_Y]; MAX_GRID_SIZE_X] };
        for x in 0..MAX_GRID_SIZE_X {
            for y in 0..MAX_GRID_SIZE_Y {
                cels.look[x][y] = rng.next_range(20) as i32;
                let dx = rng.next_range(10) as i32 - 5;
                let dy = rng.next_range(10) as i32 - 5;
                cels.offset[x][y] = (dx, dy);
            }
        }
        cels
    }
}

impl Board {
    /// `Board::GetLevelRandSeed`, for adventure mode. The original's
    /// `mBoardRandSeed` is the app's startup seed; here it's drawn when the
    /// board is made.
    pub(super) fn level_rand_seed(&self, player_id: u32, finished_adventure: i32) -> u32 {
        player_id
            .wrapping_add(self.board_rand_seed)
            .wrapping_add((finished_adventure * 101 + self.level) as u32)
    }

    /// `Board::StageHasGraveStones`.
    pub fn stage_has_grave_stones(&self) -> bool {
        self.background == super::BackgroundType::Night && !self.is_wallnut_bowling_level()
    }

    /// `PickBackground`'s graves: more of them, further left, as the night
    /// levels go on. Where they go comes from the level's own generator, so
    /// a level always has the same graves.
    pub(super) fn add_level_grave_stones(&mut self, level_rng: &mut MtRand, rng: &mut MtRand) {
        if self.is_whack_a_zombie_level() {
            // No plants yet, so nothing to crush.
            self.whack_place_graves(9, None, rng);
        } else if self.stage_has_grave_stones() {
            let columns: &[(i32, i32)] = match self.level {
                11..=13 => &[(6, 1), (7, 1), (8, 2)],
                14 | 16 => &[(5, 1), (6, 1), (7, 2), (8, 3)],
                17..=19 => &[(4, 1), (5, 2), (6, 2), (7, 3), (8, 3)],
                l if l >= 20 => &[(3, 1), (4, 2), (5, 2), (6, 2), (7, 3), (8, 3)],
                _ => &[],
            };
            for &(col, count) in columns {
                self.add_grave_stones(col, count, level_rng, rng);
            }
        }
        self.pick_special_grave_stone(rng);
    }

    /// `Board::GetGraveStoneAt`.
    pub fn grave_stone_at(&self, grid_x: i32, grid_y: i32) -> Option<Id<GridItem>> {
        self.grid_items
            .iter()
            .find(|(_, g)| g.item_type == GridItemType::GraveStone && g.grid_x == grid_x && g.grid_y == grid_y && !g.dead)
            .map(|(id, _)| id)
    }

    /// `Board::AddACrater`: the hole lasts `counter` ticks.
    pub(super) fn add_a_crater(&mut self, grid_x: i32, grid_y: i32, counter: i32) {
        self.grid_items.alloc(GridItem {
            item_type: GridItemType::Crater,
            state: GridItemState::Normal,
            grid_x,
            grid_y,
            render_order: make_render_order(render_layer::GROUND, grid_y, 1),
            counter,
            reanim: None,
            dead: false,
        });
    }

    /// `Board::AddALadder`.
    pub(super) fn add_a_ladder(&mut self, grid_x: i32, grid_y: i32) {
        self.grid_items.alloc(GridItem {
            item_type: GridItemType::Ladder,
            state: GridItemState::Normal,
            grid_x,
            grid_y,
            render_order: make_render_order(render_layer::PLANT, grid_y, 800),
            counter: 0,
            reanim: None,
            dead: false,
        });
    }
}

impl GridItem {
    /// A grid item put back where the save left it: graves stand risen,
    /// craters and ladders as they were. Used by the save loader.
    pub fn new_restored(item_type: GridItemType, grid_x: i32, grid_y: i32) -> Self {
        let (render_order, counter) = match item_type {
            GridItemType::GraveStone => {
                (make_render_order(render_layer::GRAVE_STONE, grid_y, 0), 100)
            }
            GridItemType::Crater => (make_render_order(render_layer::GROUND, grid_y, 1), 18000),
            GridItemType::Ladder => (make_render_order(render_layer::PLANT, grid_y, 800), 0),
            GridItemType::Rake => (make_render_order(render_layer::GRAVE_STONE, grid_y, 9), 0),
        };
        Self { item_type, state: GridItemState::Normal, grid_x, grid_y, render_order, counter, reanim: None, dead: false }
    }
}

impl Board {
    /// `Board::GetLadderAt`.
    pub(super) fn ladder_at(&self, grid_x: i32, grid_y: i32) -> Option<Id<GridItem>> {
        self.grid_items
            .iter()
            .find(|(_, g)| g.item_type == GridItemType::Ladder && g.grid_x == grid_x && g.grid_y == grid_y && !g.dead)
            .map(|(id, _)| id)
    }

    /// Every square's ladder, for the zombies.
    pub(super) fn ladder_grid(&self) -> super::zombie::LadderGrid {
        let mut grid = [[false; MAX_GRID_SIZE_Y]; MAX_GRID_SIZE_X];
        for (_, item) in self.grid_items.iter() {
            if item.item_type == GridItemType::Ladder
                && !item.dead
                && let Some(cell) = grid.get_mut(item.grid_x as usize).and_then(|c| c.get_mut(item.grid_y as usize))
            {
                *cell = true;
            }
        }
        grid
    }

    /// `Board::GetCraterAt`.
    pub fn crater_at(&self, grid_x: i32, grid_y: i32) -> bool {
        self.grid_items
            .iter()
            .any(|(_, g)| g.item_type == GridItemType::Crater && g.grid_x == grid_x && g.grid_y == grid_y && !g.dead)
    }

    /// `Board::CanAddGraveStoneAt`: plain grass with nothing else on it.
    pub(super) fn can_add_grave_stone_at(&self, grid_x: i32, grid_y: i32) -> bool {
        self.grid_square_type[grid_x as usize][grid_y as usize] == GridSquareType::Grass
            && !self.grid_items.iter().any(|(_, g)| g.grid_x == grid_x && g.grid_y == grid_y && !g.dead)
    }

    /// `Board::AddAGraveStone`.
    pub(super) fn add_a_grave_stone(&mut self, grid_x: i32, grid_y: i32, rng: &mut MtRand) {
        self.grid_items.alloc(GridItem {
            item_type: GridItemType::GraveStone,
            state: GridItemState::Normal,
            grid_x,
            grid_y,
            render_order: make_render_order(render_layer::GRAVE_STONE, grid_y, 3),
            counter: -(rng.next_range(50) as i32),
            reanim: None,
            dead: false,
        });
    }

    /// `Board::AddGraveStones`: `count` graves in random rows of column `grid_x`.
    fn add_grave_stones(&mut self, grid_x: i32, count: i32, level_rng: &mut MtRand, rng: &mut MtRand) {
        let free = (0..MAX_GRID_SIZE_Y as i32).filter(|&y| self.can_add_grave_stone_at(grid_x, y)).count() as i32;
        let mut added = 0;
        while added < count.min(free) {
            let grid_y = level_rng.next_range(MAX_GRID_SIZE_Y as u32) as i32;
            if self.can_add_grave_stone_at(grid_x, grid_y) {
                self.add_a_grave_stone(grid_x, grid_y, rng);
                added += 1;
            }
        }
    }

    /// `Board::PickSpecialGraveStone`.
    fn pick_special_grave_stone(&mut self, rng: &mut MtRand) {
        let graves: Vec<_> =
            self.grid_items.iter().filter(|(_, g)| g.item_type == GridItemType::GraveStone).map(|(id, _)| id).collect();
        if graves.is_empty() {
            return;
        }
        let pick = graves[rng.next_range(graves.len() as u32) as usize];
        if let Some(grave) = self.grid_items.get_mut(pick) {
            grave.state = GridItemState::GravestoneSpecial;
        }
    }

    /// `GridItem::GridItemDie`, for a grave a grave buster ate.
    pub(super) fn remove_grave_stone(&mut self, id: Id<GridItem>) {
        if let Some(grave) = self.grid_items.get_mut(id) {
            grave.dead = true;
        }
    }

    /// `Board::UpdateGridItems`: graves rise once the intro lets them, and
    /// craters fill in over time.
    pub(super) fn update_grid_items(&mut self) {
        let enabled = self.enable_grave_stones;
        let playing = self.scene == super::GameScene::Playing;
        for (_, item) in self.grid_items.iter_mut() {
            if enabled && item.item_type == GridItemType::GraveStone && item.counter < 100 {
                item.counter += 1;
            }
            if item.item_type == GridItemType::Crater && playing {
                item.counter = (item.counter - 1).max(0);
                if item.counter == 0 {
                    item.dead = true;
                }
            }
        }
    }

    /// `CutScene::AddGraveStoneParticles` + `GridItem::AddGraveStoneParticles`:
    /// dirt flies as the graves come up.
    pub(super) fn add_grave_stone_particles(&mut self, sounds: &mut crate::audio::Sounds) {
        let mut spots = Vec::new();
        for (_, grave) in self.grid_items.iter().filter(|(_, g)| g.item_type == GridItemType::GraveStone) {
            let (dx, dy) = self.grid_cels.offset[grave.grid_x as usize][grave.grid_y as usize];
            let x = self.grid_to_pixel_x(grave.grid_x) + 14 + dx;
            let y = self.grid_to_pixel_y(grave.grid_x, grave.grid_y) + 78 + dy;
            spots.push((x, y, grave.render_order + 1));
        }
        for (x, y, order) in spots {
            self.add_particle(x as f32, y as f32, order, ParticleEffect::GraveStoneRise);
            sounds.play_foley(FoleyType::DirtRise);
        }
    }

    /// `Board::PickGraveRisingZombieType`.
    fn pick_grave_rising_zombie_type(&self, rng: &mut MtRand) -> ZombieType {
        let mut candidates = vec![ZombieType::Normal, ZombieType::TrafficCone];
        if !self.stage_has_grave_stones() {
            candidates.push(ZombieType::Pail);
        }
        let weights: Vec<i32> = candidates
            .iter()
            .map(|&ty| {
                let def = ty.definition();
                let too_early = self.first_time_adventure && self.level < def.starting_level;
                let not_allowed = !self.waves.zombie_allowed[ty as usize] && ty != ZombieType::Normal;
                if too_early || not_allowed { 0 } else { def.pick_weight }
            })
            .collect();
        // TodPickFromWeightedArray.
        let total: i32 = weights.iter().sum();
        if total <= 0 {
            return ZombieType::Normal;
        }
        let mut hit = rng.next_range(total as u32) as i32;
        for (ty, weight) in candidates.iter().zip(&weights) {
            if hit < *weight {
                return *ty;
            }
            hit -= weight;
        }
        ZombieType::Normal
    }

    /// `Board::SpawnZombiesFromGraves`: one zombie out of each grave that's
    /// fully up. TODO: the pool and roof levels' versions.
    pub(super) fn spawn_zombies_from_graves(&mut self, fx: &mut Fx) {
        let graves: Vec<_> = self
            .grid_items
            .iter()
            .filter(|(_, g)| g.item_type == GridItemType::GraveStone && !g.dead && g.counter >= 100)
            .map(|(_, g)| (g.grid_x, g.grid_y))
            .collect();
        for (grid_x, grid_y) in graves {
            let ty = self.pick_grave_rising_zombie_type(fx.sounds.rng());
            let wave = self.waves.current_wave;
            let id = self.add_zombie_in_row(ty, grid_y, wave, fx);
            let env = self.zombie_env();
            let x = self.grid_to_pixel_x(grid_x);
            if let Some(zombie) = self.zombies.get_mut(id) {
                zombie.rise_from_grave(x, &env, fx);
            }
        }
    }

    /// `Board::PlaceRake`: the store's rake waits in column 7 of a random
    /// walkable row, and swings at the first zombie that reaches it.
    pub(super) fn place_rake(&mut self, fx: &mut Fx) {
        if self.rake_count <= 0 || self.is_whack_a_zombie_level() || self.is_final_boss_level() {
            return;
        }
        let rows: Vec<i32> = (0..MAX_GRID_SIZE_Y as i32)
            .filter(|&row| row != 5 && self.plant_row[row as usize] == PlantRowType::Normal)
            .collect();
        let Some(&grid_y) = rows.get(fx.sounds.rng().next_range(rows.len() as u32) as usize) else { return };
        self.rake_count -= 1;
        let grid_x = 7;
        let (x, y) = (self.grid_to_pixel_x(grid_x) as f32, self.grid_to_pixel_y(grid_x, grid_y) as f32);
        // `CreateRakeReanim`: parked on its first frame until a zombie comes.
        let reanim = fx
            .effects
            .add_reanimation(x + 20.0, y, ReanimationType::Rake, fx.res, fx.images)
            .ok()
            .map(|(id, reanim)| {
                reanim.anim_rate = 0.0;
                reanim.loop_type = sexy::reanim::LoopType::PlayOnceAndHold;
                reanim.is_attachment = true;
                id
            });
        self.grid_items.alloc(GridItem {
            item_type: GridItemType::Rake,
            state: GridItemState::RakeAttracting,
            grid_x,
            grid_y,
            render_order: make_render_order(render_layer::GRAVE_STONE, grid_y, 9),
            counter: 0,
            reanim,
            dead: false,
        });
    }

    /// `GridItem::UpdateRake`: the rake swings at a zombie in its square and
    /// is spent once the swing ends.
    pub(super) fn update_rakes(&mut self, fx: &mut Fx) {
        struct RakeAction {
            id: Id<GridItem>,
            target: Option<Id<Zombie>>,
            state: GridItemState,
            counter: i32,
            reanim: Option<crate::reanim::ReanimationId>,
        }
        let actions: Vec<RakeAction> = self
            .grid_items
            .iter()
            .filter(|(_, item)| item.item_type == GridItemType::Rake)
            .map(|(id, item)| {
                let foot = IRect::new(self.grid_to_pixel_x(item.grid_x), self.grid_to_pixel_y(item.grid_x, item.grid_y), 63, 80);
                let target = self
                    .zombies
                    .iter()
                    .find(|(_, z)| {
                        !z.is_dead_or_dying()
                            && !z.is_bobsled_team_with_sled()
                            && z.row == item.grid_y
                            && z.effected_by_damage(1)
                            && rect_overlap(foot, z.zombie_rect()) >= 0
                    })
                    .map(|(id, _)| id);
                RakeAction { id, target, state: item.state, counter: item.counter, reanim: item.reanim }
            })
            .collect();
        let env = self.zombie_env();
        // The rake is an attachment with no parent, so it's updated here
        // (`GridItem::UpdateGridItem` calls its `Update`).
        for action in &actions {
            if let Some(id) = action.reanim {
                fx.effects.update_reanimation(id);
            }
        }
        for action in actions {
            match action.state {
                GridItemState::RakeAttracting => {
                    if action.target.is_some() {
                        if let Some(item) = self.grid_items.get_mut(action.id) {
                            item.state = GridItemState::RakeTriggered;
                            item.counter = 200;
                        }
                        if let Some(reanim) = action.reanim.and_then(|id| fx.effects.get_mut(id)) {
                            reanim.anim_rate = 20.0;
                        }
                        fx.sounds.play_foley(FoleyType::Swing);
                    }
                }
                GridItemState::RakeTriggered => {
                    if let Some(target) = action.target
                        && action.reanim.and_then(|id| fx.effects.get(id)).is_some_and(|r| r.should_trigger_timed_event(0.8))
                        && let Some(zombie) = self.zombies.get_mut(target)
                    {
                        zombie.take_damage(1800, 0, &env, fx);
                        fx.sounds.play_foley(FoleyType::Bonk);
                    }
                    let counter = action.counter - 1;
                    if counter <= 0 {
                        if let Some(item) = self.grid_items.get_mut(action.id) {
                            item.dead = true;
                        }
                        if let Some(reanim) = action.reanim {
                            fx.effects.kill(reanim);
                        }
                    } else if let Some(item) = self.grid_items.get_mut(action.id) {
                        item.counter = counter;
                    }
                }
                _ => {}
            }
        }
    }

    /// `GridItem::DrawGridItem`.
    pub(super) fn draw_grid_item(&self, item: &GridItem, effects: &crate::reanim::EffectSystem, res: &GameResources, g: &mut Graphics) {
        match item.item_type {
            GridItemType::GraveStone => self.draw_grave_stone(item, res, g),
            GridItemType::Crater => self.draw_crater(item, res, g),
            GridItemType::Ladder => self.draw_ladder(item, res, g),
            GridItemType::Rake => {
                if let Some(reanim) = item.reanim.and_then(|id| effects.get(id)) {
                    crate::reanim::draw_reanimation(g, reanim, res);
                }
            }
        }
    }

    /// `GridItem::DrawLadder`: the ladder at 80% size, leaning on the plant.
    fn draw_ladder(&self, item: &GridItem, res: &GameResources, g: &mut Graphics) {
        let Some(ladder) = res.image("IMAGE_REANIM_ZOMBIE_LADDER_5") else { return };
        let x = self.grid_to_pixel_x(item.grid_x) as f32 + 25.0;
        let y = self.grid_to_pixel_y(item.grid_x, item.grid_y) as f32 - 4.0;
        g.draw_image_scaled(ladder, x, y, 0.8, 0.8);
    }

    /// `GridItem::DrawCrater`: a scorched hole that fades as it fills in.
    fn draw_crater(&self, item: &GridItem, res: &GameResources, g: &mut Graphics) {
        let mut x = self.grid_to_pixel_x(item.grid_x) as f32 - 8.0;
        let mut y = self.grid_to_pixel_y(item.grid_x, item.grid_y) as f32 + 40.0;
        let fading = item.counter < 9000;
        let mut cel_col = 0;
        let mut id = "IMAGE_CRATER";
        let pool = item.grid_x >= 0
            && item.grid_y >= 0
            && self.grid_square_type.get(item.grid_x as usize).and_then(|c| c.get(item.grid_y as usize)) == Some(&GridSquareType::Pool);
        if pool {
            // A hole in the water, bobbing with it.
            id = if self.stage_is_night() { "IMAGE_CRATER_WATER_NIGHT" } else { "IMAGE_CRATER_WATER_DAY" };
            if fading {
                cel_col = 1;
            }
            let pos = item.grid_y as f32 * std::f32::consts::PI + item.grid_x as f32 * std::f32::consts::PI * 0.25;
            let time = self.main_counter as f32 * std::f32::consts::PI * 2.0 / 200.0;
            y += (pos + time).sin() * 2.0;
        } else if self.stage_has_roof() {
            if item.grid_x < 5 {
                id = "IMAGE_CRATER_ROOF_LEFT";
                x += 16.0;
                y -= 16.0;
            } else {
                id = "IMAGE_CRATER_ROOF_CENTER";
                x += 18.0;
                y -= 9.0;
            }
            if fading {
                cel_col = 1;
            }
        } else if self.stage_is_night() {
            cel_col = 1;
            if fading {
                id = "IMAGE_CRATER_FADING";
            }
        } else if fading {
            id = "IMAGE_CRATER_FADING";
        }
        let Some(image) = res.image(id) else { return };
        g.push_state();
        if item.counter < 25 {
            let alpha = animate_curve(25, 0, item.counter, 255, 0, TodCurve::Linear).clamp(0, 255) as u8;
            g.set_colorize_images(true);
            g.set_color(crate::graphics::SexyColor::rgba(255, 255, 255, alpha));
        }
        g.draw_image_cel(image, x, y, cel_col, 0);
        g.pop_state();
    }

    /// `GridItem::DrawGraveStone`: the grave and its mound of dirt, both
    /// rising out of the ground; a grave being eaten sinks from the top.
    fn draw_grave_stone(&self, item: &GridItem, res: &GameResources, g: &mut Graphics) {
        if item.counter <= 0 {
            return;
        }
        let (Some(stones), Some(mounds)) = (res.image("IMAGE_TOMBSTONES"), res.image("IMAGE_TOMBSTONE_MOUNDS")) else {
            return;
        };
        let (gx, gy) = (item.grid_x as usize, item.grid_y as usize);
        let height_position = animate_curve(0, 100, item.counter, 1000, 0, TodCurve::EaseInOut);
        let look = self.grid_cels.look[gx][gy];
        let (offset_x, offset_y) = self.grid_cels.offset[gx][gy];
        let (cel_w, cel_h) = (stones.cel_width() as i32, stones.cel_height() as i32);
        let col = look % 5;
        let row = if item.grid_y == 0 {
            1
        } else if item.state == GridItemState::GravestoneSpecial {
            0
        } else {
            2 + look % 2
        };
        let visible_height = animate_curve(0, 1000, height_position, cel_h, 0, TodCurve::EaseInOut);
        let extra_bottom_clip = animate_curve(0, 50, height_position, 0, 14, TodCurve::EaseInOut);
        let visible_height_dirt = animate_curve(500, 1000, height_position, cel_h, 0, TodCurve::EaseInOut);
        let extra_top_clip = self
            .normal_plant_at(item.grid_x, item.grid_y)
            .and_then(|p| p.grave_buster_eaten_countdown())
            .map_or(0, |countdown| animate_curve_float(400, 0, countdown, 10.0, 40.0, TodCurve::Linear) as i32);

        let src = SexyRect::new(
            (cel_w * col) as f32,
            (cel_h * row + extra_top_clip) as f32,
            cel_w as f32,
            (visible_height - extra_bottom_clip - extra_top_clip) as f32,
        );
        let src_dirt = SexyRect::new((cel_w * col) as f32, (cel_h * row) as f32, cel_w as f32, visible_height_dirt as f32);
        let x = self.grid_to_pixel_x(item.grid_x) + offset_x - 4;
        let y = self.grid_to_pixel_y(item.grid_x, item.grid_y) + cel_h + offset_y - 9;
        if src.h > 0.0 {
            g.draw_image_src(stones, x as f32, (y - visible_height + extra_top_clip) as f32, src);
        }
        if src_dirt.h > 0.0 {
            g.draw_image_src(mounds, x as f32, (y - visible_height_dirt) as f32, src_dirt);
        }
    }
}
