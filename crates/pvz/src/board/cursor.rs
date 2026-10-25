//! The mouse on the board: picking up seed packets, carrying a plant
//! (`CursorObject`), its see-through preview where it would go
//! (`CursorPreview`), planting it, and clicking sun.
//!
//! Behaviour per ruslan831 decomp: `Board::MouseDown`, `MouseHitTest`,
//! `MouseDownWithPlant`, `CanPlantAt`, `AddPlant`, `DoPlantingEffects`,
//! `ClearCursor`, `RefreshSeedPacketFromCursor`, `TakeSunMoney`,
//! `CanTakeSunMoney`, `CountSunBeingCollected`, `PixelToGridX`/`Y` and
//! `PlantingPixelToGridX`/`Y` (`Lawn/Board.cpp`); `SeedPacket::CanPickUp`,
//! `MouseDown`, `WasPlanted` and `SeedBank::MouseHitTest`
//! (`Lawn/SeedPacket.cpp`); `CursorObject` and `CursorPreview`
//! (`Lawn/CursorObject.cpp`); `Coin::MouseDown`.

use bevy::prelude::*;

use crate::audio::{FoleyType, Sounds};
use crate::graphics::{Graphics, SexyColor};
use crate::particle::ParticleEffect;
use crate::plant::SeedType;
use crate::reanim::EffectSystem;
use crate::resources::GameResources;

use super::coin::Coin;
use super::message::MessageStyle;
use super::plant::{Plant, plant_draw_height_offset};
use super::seed_bank::draw_seed_type;
use super::tutorial::{AdviceType, TutorialState};
use super::{Board, GameScene, GridSquareType, LAWN_YMIN, MAX_GRID_SIZE_X, MAX_GRID_SIZE_Y, render_layer};

/// The leftmost lawn column's x (`LAWN_XMIN`).
pub const LAWN_XMIN: i32 = 40;

/// `CursorType` (the parts in use so far).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorType {
    Normal,
    PlantFromBank,
    Shovel,
    /// Whack-a-zombie's mallet.
    Hammer,
}

/// What the mouse is carrying (`CursorObject`).
#[derive(Debug, Clone)]
pub struct CursorObject {
    pub cursor_type: CursorType,
    pub seed: Option<SeedType>,
    pub seed_bank_index: Option<usize>,
}

impl Default for CursorObject {
    fn default() -> Self {
        Self { cursor_type: CursorType::Normal, seed: None, seed_bank_index: None }
    }
}

/// Where the carried plant would go (`CursorPreview`).
#[derive(Debug, Clone, Default)]
pub struct CursorPreview {
    pub visible: bool,
    pub x: i32,
    pub y: i32,
}

impl CursorPreview {
    /// `GameObject::mRenderOrder`'s default: above the plants.
    pub const RENDER_ORDER: i32 = render_layer::TOP;
}

/// `PlantingReason` (the ones checked so far).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlantingReason {
    Ok,
    NotHere,
    NotOnWater,
    NeedsPot,
    /// Wall-nut bowling: nuts go in the first three columns.
    NotPassedLine,
    /// A grave buster needs a grave to eat.
    OnlyOnGraves,
    /// Nothing else can go on a grave.
    NotOnGrave,
    /// A doom-shroom's crater hasn't filled in yet.
    NotOnCrater,
}

impl Board {
    /// `Board::PixelToGridX`.
    pub fn pixel_to_grid_x(&self, x: i32) -> i32 {
        if x < LAWN_XMIN {
            return -1;
        }
        ((x - LAWN_XMIN) / 80).clamp(0, MAX_GRID_SIZE_X as i32 - 1)
    }

    /// `Board::PixelToGridY`.
    pub fn pixel_to_grid_y(&self, x: i32, y: i32) -> i32 {
        let grid_x = self.pixel_to_grid_x(x);
        if grid_x == -1 || y < LAWN_YMIN {
            return -1;
        }
        if self.stage_has_roof() {
            let y = if grid_x < 5 { y - (4 - grid_x) * 20 } else { y };
            ((y - LAWN_YMIN) / 85).clamp(0, MAX_GRID_SIZE_Y as i32 - 2)
        } else if self.stage_has_pool() {
            ((y - LAWN_YMIN) / 85).clamp(0, MAX_GRID_SIZE_Y as i32 - 1)
        } else {
            ((y - LAWN_YMIN) / 100).clamp(0, MAX_GRID_SIZE_Y as i32 - 2)
        }
    }

    /// `Board::GridToPixelX`.
    pub fn grid_to_pixel_x(&self, grid_x: i32) -> i32 {
        grid_x * 80 + LAWN_XMIN
    }

    /// `OffsetYForPlanting` + `PlantingPixelToGridX`/`Y`.
    fn planting_pixel_to_grid(&self, p: IVec2, seed: SeedType) -> (i32, i32) {
        let mut y = p.y;
        if matches!(seed, SeedType::InstantCoffee | SeedType::Gravebuster) {
            y += 15;
        }
        if matches!(seed, SeedType::Spikeweed | SeedType::Spikerock) {
            y -= 15;
        }
        (self.pixel_to_grid_x(p.x), self.pixel_to_grid_y(p.x, y))
    }

    /// The plant in `(x, y)`, ignoring lily pads, pots and pumpkins.
    pub(super) fn normal_plant_at(&self, grid_x: i32, grid_y: i32) -> Option<&Plant> {
        use SeedType::*;
        if self.is_wallnut_bowling_level() {
            return None;
        }
        self.plants.iter().map(|(_, p)| p).find(|p| {
            p.col == grid_x && p.row == grid_y && !p.dead && !matches!(p.seed, Lilypad | Flowerpot | Pumpkinshell | InstantCoffee)
        })
    }

    /// Whether a lily pad floats on `(grid_x, grid_y)`.
    pub(super) fn has_lily_pad_at(&self, grid_x: i32, grid_y: i32) -> bool {
        self.plants.iter().any(|(_, p)| p.col == grid_x && p.row == grid_y && !p.dead && p.seed == SeedType::Lilypad)
    }

    /// `Board::CanPlantAt`, for ordinary plants on grass, dirt and water.
    /// TODO: pots, pumpkins and upgrades.
    pub fn can_plant_at(&self, grid_x: i32, grid_y: i32, seed: SeedType) -> PlantingReason {
        if !(0..MAX_GRID_SIZE_X as i32).contains(&grid_x) || !(0..MAX_GRID_SIZE_Y as i32).contains(&grid_y) {
            return PlantingReason::NotHere;
        }
        // Challenge::CanPlantAt: nuts only start behind the line.
        if self.is_wallnut_bowling_level() {
            return if grid_x > 2 { PlantingReason::NotPassedLine } else { PlantingReason::Ok };
        }
        // Grave busters only go on graves, and nothing else does.
        let has_grave = self.grave_stone_at(grid_x, grid_y).is_some();
        if seed == SeedType::Gravebuster {
            if self.normal_plant_at(grid_x, grid_y).is_some() {
                return PlantingReason::NotHere;
            }
            return if has_grave { PlantingReason::Ok } else { PlantingReason::OnlyOnGraves };
        }
        if has_grave {
            return PlantingReason::NotOnGrave;
        }
        if self.crater_at(grid_x, grid_y) {
            return PlantingReason::NotOnCrater;
        }
        let square = self.grid_square_type[grid_x as usize][grid_y as usize];
        if square == GridSquareType::Dirt {
            return PlantingReason::NotHere;
        }
        let on_water = square == GridSquareType::Pool;
        // Coffee beans only wake a mushroom that is asleep on the square.
        if seed == SeedType::InstantCoffee {
            let asleep = self.normal_plant_at(grid_x, grid_y).is_some_and(Plant::is_asleep);
            return if asleep { PlantingReason::Ok } else { PlantingReason::NotHere };
        }
        // Behaviour per ruslan831 decomp, Board::CanPlantAt: water plants only
        // go on water, and everything else on water needs a lily pad under it.
        if seed == SeedType::Lilypad {
            let free = on_water && !self.has_lily_pad_at(grid_x, grid_y) && self.normal_plant_at(grid_x, grid_y).is_none();
            return if free { PlantingReason::Ok } else { PlantingReason::NotHere };
        }
        if on_water {
            if self.normal_plant_at(grid_x, grid_y).is_some() {
                return PlantingReason::NotHere;
            }
            if !seed.is_aquatic() && !self.has_lily_pad_at(grid_x, grid_y) {
                return PlantingReason::NotOnWater;
            }
            return PlantingReason::Ok;
        }
        if seed.is_aquatic() {
            return PlantingReason::NotHere;
        }
        if self.stage_has_roof() {
            return PlantingReason::NeedsPot;
        }
        if self.normal_plant_at(grid_x, grid_y).is_some() {
            return PlantingReason::NotHere;
        }
        if seed.is_upgrade() {
            return PlantingReason::NotHere;
        }
        PlantingReason::Ok
    }

    /// `Board::CountSunBeingCollected`.
    fn count_sun_being_collected(&self) -> i32 {
        self.coins.iter().filter(|(_, c)| c.is_being_collected && !c.dead).map(|(_, c)| c.sun_value()).sum()
    }

    /// `Board::CanTakeSunMoney`: sun on its way to the counter counts.
    pub fn can_take_sun_money(&self, amount: i32) -> bool {
        amount <= self.sun_money + self.count_sun_being_collected()
    }

    /// `Board::TakeSunMoney`.
    fn take_sun_money(&mut self, amount: i32, sounds: &mut Sounds) -> bool {
        if self.can_take_sun_money(amount) {
            self.sun_money -= amount;
            return true;
        }
        sounds.play_sample("SOUND_BUZZER");
        self.out_of_money_counter = 70;
        false
    }

    /// `Board::AddSunMoney`.
    pub fn add_sun_money(&mut self, amount: i32) {
        self.sun_money = (self.sun_money + amount).min(9990);
    }

    /// `SeedPacket::CanPickUp`.
    pub fn packet_can_pick_up(&self, index: usize) -> bool {
        let packet = &self.seed_bank.packets[index];
        let Some(seed) = packet.packet_type else { return false };
        // TODO: upgrades also need their base plant on the lawn.
        !self.paused
            && self.scene == GameScene::Playing
            && packet.active
            && (self.can_take_sun_money(seed.cost()) || self.has_conveyor_belt_seed_bank())
    }

    /// `SeedBank::MouseHitTest`: the packet under the mouse.
    fn seed_packet_at(&self, p: IVec2) -> Option<usize> {
        let bank = &self.seed_bank;
        let local = p - IVec2::new(bank.x, bank.y);
        bank.packets[..bank.num_packets].iter().position(|packet| {
            packet.packet_type.is_some()
                && local.x >= packet.x + packet.offset_x
                && local.x < packet.x + packet.offset_x + 50
                && local.y >= packet.y
                && local.y < packet.y + 70
        })
    }

    /// The topmost sun under the mouse.
    fn coin_at(&self, p: Vec2) -> Option<sexy::data_array::Id<Coin>> {
        self.coins
            .iter()
            .filter(|(_, c)| c.hit_test(p))
            .max_by_key(|(_, c)| c.render_order)
            .map(|(id, _)| id)
    }

    fn is_plant_in_cursor(&self) -> bool {
        self.cursor.cursor_type == CursorType::PlantFromBank
    }

    /// `Board::ClearCursor`.
    pub(super) fn clear_cursor(&mut self, res: &GameResources) {
        self.cursor = CursorObject::default();
        if self.tutorial_state == TutorialState::Level1PlantPeashooter {
            self.set_tutorial_state(res, TutorialState::Level1PickUpPeashooter);
        } else {
            self.tutorial_sunflower_done_carrying(res, None);
        }
    }

    /// `Board::RefreshSeedPacketFromCursor`: puts a carried plant back.
    pub(super) fn refresh_seed_packet_from_cursor(&mut self, res: &GameResources) {
        if let Some(i) = self.cursor.seed_bank_index {
            self.seed_bank.packets[i].active = true;
        }
        self.clear_cursor(res);
    }

    /// `Board::GetShovelButtonRect`: (x, y, width, height).
    pub(super) fn shovel_button_rect(&self, res: &GameResources) -> (i32, i32, i32, i32) {
        let width = res.image("IMAGE_SHOVELBANK").map_or(70, |i| i.width as i32);
        let height = res.image("IMAGE_SEEDBANK").map_or(87, |i| i.height as i32);
        (self.seed_bank.extra_width() + 456, 0, width, height)
    }

    /// `Board::MouseDownWithTool`, for the shovel.
    fn mouse_down_with_shovel(&mut self, p: IVec2, right: bool, effects: &mut EffectSystem, res: &GameResources, sounds: &mut Sounds) {
        self.cursor = CursorObject::default();
        sounds.play_foley(FoleyType::Drop);
        if right {
            return;
        }
        let (grid_x, grid_y) = (self.pixel_to_grid_x(p.x), self.pixel_to_grid_y(p.x, p.y));
        // ToolHitTest: the plant on the square under the mouse.
        let target = self.plants.iter().find(|(_, plant)| plant.col == grid_x && plant.row == grid_y && !plant.dead).map(|(id, _)| id);
        if let Some(plant) = target.and_then(|id| self.plants.get_mut(id)) {
            sounds.play_foley(FoleyType::UseShovel);
            plant.die(effects);
            if matches!(self.tutorial_state, TutorialState::ShovelDig | TutorialState::ShovelKeepDigging) {
                let peashooters = self.plants.iter().filter(|(_, p)| !p.dead && p.seed == SeedType::Peashooter).count();
                let next = if peashooters == 0 { TutorialState::ShovelCompleted } else { TutorialState::ShovelKeepDigging };
                self.set_tutorial_state(res, next);
            }
        }
        // ClearCursor: a click that missed puts the lesson back at the shovel.
        if self.tutorial_state == TutorialState::ShovelDig {
            self.set_tutorial_state(res, TutorialState::ShovelPickup);
        }
    }

    /// `Board::MouseDown`, at a point in board coordinates. `right` is the
    /// right button (the original's negative click count).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn mouse_down(
        &mut self,
        p: Vec2,
        right: bool,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        let ip = p.floor().as_ivec2();
        if self.cursor.cursor_type == CursorType::Shovel {
            self.mouse_down_with_shovel(ip, right, effects, res, sounds);
            return;
        }
        if !right && self.cursor.cursor_type == CursorType::Normal && self.show_shovel && self.can_interact_with_board_buttons() {
            let (x, y, w, h) = self.shovel_button_rect(res);
            if ip.x >= x && ip.x < x + w && ip.y >= y && ip.y < y + h {
                if !self.paused && (self.scene == GameScene::Playing || self.is_in_shovel_tutorial()) {
                    if self.tutorial_state == TutorialState::ShovelPickup {
                        self.set_tutorial_state(res, TutorialState::ShovelDig);
                    }
                    self.cursor.cursor_type = CursorType::Shovel;
                    sounds.play_foley(FoleyType::Shovel);
                }
                return;
            }
        }
        // MouseHitTest: the seed bank and sun only count with an empty hand.
        if !right
            && !self.is_plant_in_cursor()
            && let Some(id) = self.coin_at(p)
        {
            self.coin_mouse_down(id, res, sounds);
            return;
        }
        if self.is_plant_in_cursor() {
            self.mouse_down_with_plant(ip, right, effects, res, images, sounds);
        } else if !right && let Some(index) = self.seed_packet_at(ip) {
            self.packet_mouse_down(index, res, sounds);
        } else if !right && self.cursor.cursor_type == CursorType::Hammer && self.scene == GameScene::Playing && !self.paused {
            let mut fx = super::zombie::Fx { effects, res, images, sounds };
            self.whack_mouse_down(ip.x, ip.y, &mut fx);
        }
    }

    /// `Coin::MouseDown` + `Coin::Collect`'s sun handling.
    fn coin_mouse_down(&mut self, id: sexy::data_array::Id<Coin>, res: &GameResources, sounds: &mut Sounds) {
        if self.paused || self.scene != GameScene::Playing {
            return;
        }
        let Some(coin) = self.coins.get_mut(id) else { return };
        if coin.dead || coin.is_being_collected {
            return;
        }
        if coin.is_level_award() {
            // The level's award: it flies to the middle of the screen and the
            // level fades out (the board does that on its next tick).
            for sound in coin.pickup_sound() {
                sounds.play_sample(sound);
            }
            coin.collect();
            self.pending_fade_out = true;
            // TODO: the starburst and pickup particles around the packet.
            return;
        }
        // PlayCollectSound.
        if coin.is_money() {
            match coin.ty {
                super::coin::CoinType::Diamond => sounds.play_sample("SOUND_DIAMOND"),
                _ => sounds.play_foley(FoleyType::Coin),
            }
            coin.needs_bouncy_arrow = false;
            coin.collect();
            self.show_coin_bank();
            return;
        }
        sounds.play_foley(FoleyType::Sun);
        coin.collect();
        let value = coin.sun_value();
        // A packet that this sun makes affordable flashes.
        for i in 0..self.seed_bank.num_packets {
            let Some(seed) = self.seed_bank.packets[i].packet_type else { continue };
            let profit = self.sun_money + self.count_sun_being_collected() - seed.cost();
            if profit >= 0 && profit < value {
                self.flash_if_ready(res, i);
            }
        }
        if self.first_time_adventure && self.level == 1 {
            let style = MessageStyle::TutorialLevel1Stay;
            self.display_advice(res, "[ADVICE_CLICKED_ON_SUN]", style, Some(AdviceType::ClickedOnSun));
        }
    }

    /// `SeedPacket::MouseDown`: picks the plant up if it's ready and affordable.
    fn packet_mouse_down(&mut self, index: usize, res: &GameResources, sounds: &mut Sounds) {
        let packet = &self.seed_bank.packets[index];
        let Some(seed) = packet.packet_type else { return };
        if self.paused || self.scene != GameScene::Playing {
            return;
        }
        let level1 = self.first_time_adventure && self.level == 1 && self.help_displayed.contains(&AdviceType::ClickOnSun);
        if !packet.active {
            sounds.play_sample("SOUND_BUZZER");
            if level1 {
                self.display_advice(res, "[ADVICE_SEED_REFRESH]", MessageStyle::TutorialLevel1, Some(AdviceType::SeedRefresh));
            }
            return;
        }
        if !self.can_take_sun_money(seed.cost()) && !self.has_conveyor_belt_seed_bank() {
            sounds.play_sample("SOUND_BUZZER");
            self.out_of_money_counter = 70;
            if level1 {
                let advice = Some(AdviceType::CantAffordPlant);
                self.display_advice(res, "[ADVICE_CANT_AFFORD_PLANT]", MessageStyle::TutorialLevel1, advice);
            }
            return;
        }
        // TODO: upgrades refuse to be picked up without their base plant.
        self.clear_advice(Some(AdviceType::CantAffordPlant));
        self.cursor = CursorObject { cursor_type: CursorType::PlantFromBank, seed: Some(seed), seed_bank_index: Some(index) };
        sounds.play_sample("SOUND_SEEDLIFT");
        if self.tutorial_state == TutorialState::Level1PickUpPeashooter {
            self.set_tutorial_state(res, TutorialState::Level1PlantPeashooter);
        } else {
            self.tutorial_sunflower_picked_up(res, seed);
        }
        // Deactivate.
        let packet = &mut self.seed_bank.packets[index];
        packet.active = false;
        packet.refresh_counter = 0;
        packet.refresh_time = 0;
        packet.refreshing = false;
    }

    /// `Board::MouseDownWithPlant`: plants what the mouse carries, or drops it.
    #[allow(clippy::too_many_arguments)]
    fn mouse_down_with_plant(
        &mut self,
        p: IVec2,
        right: bool,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        let drop = |board: &mut Self, sounds: &mut Sounds, res: &GameResources| {
            board.refresh_seed_packet_from_cursor(res);
            sounds.play_foley(FoleyType::Drop);
        };
        let Some(seed) = self.cursor.seed else { return };
        if right {
            drop(self, sounds, res);
            return;
        }
        let (grid_x, grid_y) = self.planting_pixel_to_grid(p, seed);
        if grid_x < 0 || grid_x >= MAX_GRID_SIZE_X as i32 || grid_y < 0 || grid_y > MAX_GRID_SIZE_Y as i32 {
            drop(self, sounds, res);
            return;
        }
        let reason = self.can_plant_at(grid_x, grid_y, seed);
        // Whack-a-zombie puts the plant back rather than waiting for a good spot.
        if reason != PlantingReason::Ok && self.is_whack_a_zombie_level() {
            drop(self, sounds, res);
            return;
        }
        match reason {
            PlantingReason::Ok => {}
            // Keep holding the plant; some reasons explain themselves.
            PlantingReason::NotOnWater => {
                let advice = Some(AdviceType::PlantNotOnWater);
                self.display_advice(res, "[ADVICE_PLANT_NOT_ON_WATER]", MessageStyle::HintFast, advice);
                return;
            }
            PlantingReason::NeedsPot => {
                let advice = Some(AdviceType::PlantNeedPot);
                self.display_advice(res, "[ADVICE_PLANT_NEED_POT2]", MessageStyle::HintFast, advice);
                return;
            }
            PlantingReason::NotPassedLine => {
                let advice = Some(AdviceType::PlantNotPassedLine);
                self.display_advice(res, "[ADVICE_NOT_PASSED_LINE]", MessageStyle::HintFast, advice);
                return;
            }
            PlantingReason::OnlyOnGraves => {
                let advice = Some(AdviceType::PlantGravebustersOnGraves);
                self.display_advice(res, "[ADVICE_GRAVEBUSTERS_ON_GRAVES]", MessageStyle::HintFast, advice);
                return;
            }
            PlantingReason::NotOnGrave => {
                let advice = Some(AdviceType::PlantNotOnGrave);
                self.display_advice(res, "[ADVICE_PLANT_NOT_ON_GRAVE]", MessageStyle::HintFast, advice);
                return;
            }
            PlantingReason::NotOnCrater => {
                let advice = Some(AdviceType::PlantNotOnCrater);
                self.display_advice(res, "[ADVICE_PLANT_NOT_ON_CRATER]", MessageStyle::HintFast, advice);
                return;
            }
            PlantingReason::NotHere => return,
        }
        for advice in [
            AdviceType::CantPlantThere,
            AdviceType::PlantNotOnWater,
            AdviceType::PlantNotOnGrave,
            AdviceType::PlantNotOnCrater,
            AdviceType::PlantNeedPot,
            AdviceType::PlantNotPassedLine,
            AdviceType::PlantGravebustersOnGraves,
        ] {
            self.clear_advice(Some(advice));
        }
        let conveyor = self.has_conveyor_belt_seed_bank();
        if !conveyor && !self.take_sun_money(seed.cost(), sounds) {
            return;
        }
        self.add_plant(grid_x, grid_y, seed, effects, res, images, sounds);
        if let Some(i) = self.cursor.seed_bank_index {
            // SeedPacket::WasPlanted.
            if conveyor {
                self.seed_bank.remove_seed(i);
            } else {
                let packet = &mut self.seed_bank.packets[i];
                packet.refreshing = true;
                packet.refresh_time = seed.definition().refresh_time;
            }
        }
        if self.is_wallnut_bowling_level() {
            sounds.play_sample("SOUND_BOWLING");
        }
        if self.tutorial_state == TutorialState::Level1PlantPeashooter {
            let next = if self.plants.len() >= 2 {
                TutorialState::Level1Completed
            } else {
                TutorialState::Level1RefreshPeashooter
            };
            self.set_tutorial_state(res, next);
        } else {
            self.tutorial_sunflower_done_carrying(res, Some(seed));
        }
        self.cursor = CursorObject::default();
    }

    /// `Board::AddPlant` + `DoPlantingEffects`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn add_plant(
        &mut self,
        grid_x: i32,
        grid_y: i32,
        seed: SeedType,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        let (x, y) = (self.grid_to_pixel_x(grid_x), self.grid_to_pixel_y(grid_x, grid_y));
        // Behaviour per ruslan831 decomp, Plant::DoSpecial (coffee bean): it
        // wakes the mushroom on its square. TODO: the bean's own animation.
        if seed == SeedType::InstantCoffee {
            let sleeper = self.plants.iter().find(|(_, p)| p.col == grid_x && p.row == grid_y && !p.dead && p.is_asleep()).map(|(id, _)| id);
            if let Some(plant) = sleeper.and_then(|id| self.plants.get_mut(id)) {
                plant.set_sleeping(false, effects, res, images, sounds.rng());
            }
            sounds.play_foley(FoleyType::Plant);
            return;
        }
        let bowling = self.is_wallnut_bowling_level();
        let plant = Plant::new(grid_x, grid_y, x, y, seed, bowling, effects, res, images, sounds.rng());
        let id = self.plants.alloc(plant);
        // TODO: water sounds and splashes in the pool.
        sounds.play_foley(FoleyType::Plant);
        if matches!(seed, SeedType::Cherrybomb | SeedType::Jalapeno) {
            sounds.play_foley(FoleyType::ReverseExplosion);
        }
        if seed == SeedType::Gravebuster {
            sounds.play_foley(FoleyType::GravebusterChomp);
        }
        // Mushrooms sleep through the day.
        if seed.is_nocturnal() && !self.stage_is_night()
            && let Some(plant) = self.plants.get_mut(id)
        {
            plant.set_sleeping(true, effects, res, images, sounds.rng());
        }
        let (px, py) = ((x + 41) as f32, (y + 74) as f32);
        let py = match seed {
            SeedType::Lilypad => py + 15.0,
            SeedType::Flowerpot => py + 30.0,
            _ => py,
        };
        // The original passes the bare layer as the render order here.
        effects.add_particle(px, py, render_layer::TOP, ParticleEffect::Planting, res, images, sounds.rng());
    }

    /// `CursorPreview::Update`: shows where the carried plant would go.
    pub(super) fn update_cursor_preview(&mut self) {
        self.cursor_preview.visible = false;
        if self.scene != GameScene::Playing || !self.is_plant_in_cursor() {
            return;
        }
        let (Some(p), Some(seed)) = (self.mouse, self.cursor.seed) else { return };
        let (grid_x, grid_y) = self.planting_pixel_to_grid(p.floor().as_ivec2(), seed);
        if self.can_plant_at(grid_x, grid_y, seed) == PlantingReason::Ok {
            self.cursor_preview = CursorPreview {
                visible: true,
                x: self.grid_to_pixel_x(grid_x),
                y: self.grid_to_pixel_y(grid_x, grid_y),
            };
        }
    }

    /// `CursorPreview::Draw`: the plant, faint, in the square it would go.
    pub(super) fn draw_cursor_preview(&self, res: &GameResources, g: &mut Graphics) {
        let preview = &self.cursor_preview;
        let Some(seed) = self.cursor.seed.filter(|_| preview.visible) else { return };
        g.push_state();
        g.translate(preview.x as f32, preview.y as f32);
        g.set_colorize_images(true);
        g.set_color(SexyColor::rgba(255, 255, 255, 100));
        draw_seed_type(g, res, seed, 0.0, plant_draw_height_offset(seed), 1.0);
        g.pop_state();
    }

    /// `CursorObject::Draw`: the plant held by the mouse.
    pub(super) fn draw_cursor_object(&self, res: &GameResources, g: &mut Graphics) {
        if self.scene == GameScene::Playing
            && self.cursor.cursor_type == CursorType::Shovel
            && let (Some(p), Some(shovel)) = (self.mouse, res.image("IMAGE_SHOVEL"))
        {
            g.draw_image(shovel, p.x.floor() - 15.0, p.y.floor() - 65.0);
            return;
        }
        if self.scene != GameScene::Playing || !self.is_plant_in_cursor() {
            return;
        }
        let (Some(p), Some(seed)) = (self.mouse, self.cursor.seed) else { return };
        let mut offset_y = plant_draw_height_offset(seed) - 10.0;
        if matches!(seed, SeedType::InstantCoffee | SeedType::Gravebuster) {
            offset_y += 30.0;
        }
        offset_y -= 15.0;
        let (x, y) = (p.x.floor() - 25.0, p.y.floor() - 35.0);
        draw_seed_type(g, res, seed, x - 10.0, y + offset_y, 1.0);
    }
}
