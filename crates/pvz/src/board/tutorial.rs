//! The adventure tutorials (`TutorialState`) and the board's one-off hints
//! (`AdviceType`). So far: level 1's "pick up the peashooter, plant it,
//! collect sun, plant another" walkthrough, level 2's sunflower lesson and
//! the later reminder to plant more sunflowers.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Board.cpp`: `SetTutorialState`,
//! `UpdateTutorial`, `DisplayAdvice`, `ClearAdvice`; and
//! `SeedPacket::FlashIfReady`.

use crate::resources::GameResources;

use crate::particle::ParticleEffect;

use super::{Board, ParticleRequest, make_render_order, render_layer};
use super::message::MessageStyle;

/// `TutorialState` (the level 1 part so far).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TutorialState {
    Off,
    Level1PickUpPeashooter,
    Level1PlantPeashooter,
    Level1RefreshPeashooter,
    Level1Completed,
    Level2PickUpSunflower,
    Level2PlantSunflower,
    Level2RefreshSunflower,
    Level2Completed,
    MoreSunPickUpSunflower,
    MoreSunPlantSunflower,
    MoreSunRefreshSunflower,
    MoreSunCompleted,
}

impl TutorialState {
    /// The state where the sunflower packet should be picked up.
    pub fn is_sunflower_pick_up(self) -> bool {
        matches!(self, Self::Level2PickUpSunflower | Self::MoreSunPickUpSunflower)
    }

    fn is_level2(self) -> bool {
        matches!(self, Self::Level2PickUpSunflower | Self::Level2PlantSunflower | Self::Level2RefreshSunflower)
    }

    fn is_more_sun(self) -> bool {
        matches!(self, Self::MoreSunPickUpSunflower | Self::MoreSunPlantSunflower | Self::MoreSunRefreshSunflower)
    }

    /// The same lesson, with the sunflower in hand.
    fn plant_state(self) -> Self {
        if self.is_level2() { Self::Level2PlantSunflower } else { Self::MoreSunPlantSunflower }
    }

    fn refresh_state(self) -> Self {
        if self.is_level2() { Self::Level2RefreshSunflower } else { Self::MoreSunRefreshSunflower }
    }

    fn pick_up_state(self) -> Self {
        if self.is_level2() { Self::Level2PickUpSunflower } else { Self::MoreSunPickUpSunflower }
    }
}

/// `AdviceType`: hints that are only shown once per level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdviceType {
    ClickOnSun,
    ClickedOnSun,
    SeedRefresh,
    CantAffordPlant,
    CantPlantThere,
    PlantNotOnWater,
    PlantNeedPot,
    PlantSunflower5,
    PlantNotPassedLine,
    HugeWave,
}

impl Board {
    /// `Board::DisplayAdvice`: shows `text` (a `[KEY]` or plain text), once
    /// per level if `help` is given.
    pub(super) fn display_advice(&mut self, res: &GameResources, text: &str, style: MessageStyle, help: Option<AdviceType>) {
        if let Some(help) = help
            && !self.help_displayed.insert(help)
        {
            return;
        }
        self.advice.set_label(res.translate(text), style);
        self.help_index = help;
    }

    /// `Board::DisplayAdviceAgain`: like `display_advice`, even if the hint was shown before.
    pub(super) fn display_advice_again(&mut self, res: &GameResources, text: &str, style: MessageStyle, help: AdviceType) {
        self.help_displayed.remove(&help);
        self.display_advice(res, text, style, Some(help));
    }

    /// `Board::ClearAdvice`: clears the banner if it's showing `help` (or
    /// whatever it's showing, for `None`).
    pub(super) fn clear_advice(&mut self, help: Option<AdviceType>) {
        if help.is_none() || help == self.help_index {
            self.advice.clear_label();
            self.help_index = None;
        }
    }

    /// `Board::TutorialArrowShow`: a bouncing arrow over the seed packet at
    /// `index` (positioned by its slot even if it's empty).
    fn tutorial_arrow_show(&mut self, index: usize) {
        let bank = &self.seed_bank;
        let (x, y) = (bank.x + bank.packets[index].x, bank.y + bank.packets[index].y);
        self.tutorial_arrow_show_at(x, y);
    }

    /// `Board::TutorialArrowShow`.
    fn tutorial_arrow_show_at(&mut self, x: i32, y: i32) {
        self.tutorial_arrow_remove();
        let render_order = make_render_order(render_layer::TOP, 0, 0);
        self.particle_requests.push(ParticleRequest::Add {
            x: x as f32,
            y: y as f32,
            render_order,
            effect: ParticleEffect::SeedPacketPick,
            tutorial_arrow: true,
        });
    }

    /// `Board::TutorialArrowRemove`.
    pub(super) fn tutorial_arrow_remove(&mut self) {
        self.particle_requests.push(ParticleRequest::RemoveTutorialArrow);
    }

    /// `Board::SetTutorialState`.
    pub(super) fn set_tutorial_state(&mut self, res: &GameResources, state: TutorialState) {
        use MessageStyle::TutorialLevel1Stay;
        match state {
            TutorialState::Level1PickUpPeashooter => {
                if self.plants.is_empty() {
                    self.tutorial_arrow_show(0);
                    self.display_advice(res, "[ADVICE_CLICK_SEED_PACKET]", TutorialLevel1Stay, None);
                } else {
                    self.display_advice(res, "[ADVICE_ENOUGH_SUN]", TutorialLevel1Stay, None);
                    self.tutorial_timer = 400;
                }
            }
            TutorialState::Level1PlantPeashooter => {
                self.tutorial_timer = -1;
                self.tutorial_arrow_remove();
                if self.plants.is_empty() {
                    self.display_advice(res, "[ADVICE_CLICK_ON_GRASS]", TutorialLevel1Stay, None);
                } else {
                    self.clear_advice(None);
                }
            }
            TutorialState::Level1RefreshPeashooter => {
                self.display_advice(res, "[ADVICE_PLANTED_PEASHOOTER]", TutorialLevel1Stay, None);
                self.sun_count_down = 400;
            }
            TutorialState::Level1Completed => {
                self.display_advice(res, "[ADVICE_ZOMBIE_ONSLAUGHT]", MessageStyle::TutorialLevel1, None);
                self.waves.zombie_count_down = 99;
                self.waves.zombie_count_down_start = 99;
            }
            TutorialState::Level2PickUpSunflower | TutorialState::MoreSunPickUpSunflower => {
                // The original passes the bank's x for both coordinates here.
                let x = self.seed_bank.x + self.seed_bank.packets[1].x;
                self.tutorial_arrow_show_at(x, x);
            }
            TutorialState::Level2PlantSunflower
            | TutorialState::Level2RefreshSunflower
            | TutorialState::MoreSunPlantSunflower
            | TutorialState::MoreSunRefreshSunflower => self.tutorial_arrow_remove(),
            TutorialState::Level2Completed => {
                if self.waves.current_wave == 0 {
                    self.waves.zombie_count_down = 999;
                    self.waves.zombie_count_down_start = 999;
                }
            }
            TutorialState::MoreSunCompleted | TutorialState::Off => {}
        }
        self.tutorial_state = state;
    }

    /// `Board::UpdateTutorial`: after "you have enough sun", point at the
    /// peashooter again.
    pub(super) fn update_tutorial(&mut self, res: &GameResources) {
        if self.tutorial_timer > 0 {
            self.tutorial_timer -= 1;
        }
        if self.tutorial_state == TutorialState::Level1PickUpPeashooter && self.tutorial_timer == 0 {
            self.display_advice(res, "[ADVICE_CLICK_PEASHOOTER]", MessageStyle::TutorialLevel1Stay, None);
            // Behaviour per ruslan831 decomp, Board::UpdateTutorial: this points at packet slot 1.
            self.tutorial_arrow_show(1);
            self.tutorial_timer = -1;
        } else if self.tutorial_state.is_level2() {
            if self.tutorial_timer == 0 {
                self.display_advice(res, "[ADVICE_PLANT_SUNFLOWER2]", MessageStyle::TutorialLevel2, None);
                self.tutorial_timer = -1;
            } else if self.waves.zombie_count_down == 750 && self.waves.current_wave == 0 {
                self.display_advice(res, "[ADVICE_PLANT_SUNFLOWER3]", MessageStyle::TutorialLevel2, None);
            }
        } else if self.tutorial_state.is_more_sun() && self.tutorial_timer == 0 {
            let help = Some(AdviceType::PlantSunflower5);
            self.display_advice(res, "[ADVICE_PLANT_SUNFLOWER5]", MessageStyle::TutorialLater, help);
            self.tutorial_timer = -1;
        }

        // Later levels remind the player that sunflowers pay off.
        let more_sun_level = self.first_time_adventure && self.level >= 3 && self.level != 5 && self.level <= 7;
        if more_sun_level
            && self.tutorial_state == TutorialState::Off
            && self.waves.current_wave >= 5
            && !self.shown_more_sun_tutorial
            && self.packet_can_pick_up(1)
            && self.count_sunflowers() < 3
        {
            self.display_advice(res, "[ADVICE_PLANT_SUNFLOWER4]", MessageStyle::TutorialLaterStay, None);
            self.shown_more_sun_tutorial = true;
            self.set_tutorial_state(res, TutorialState::MoreSunPickUpSunflower);
            self.tutorial_timer = 500;
        }
    }

    /// `Board::CountSunFlowers`.
    pub(super) fn count_sunflowers(&self) -> usize {
        self.plants.iter().filter(|(_, p)| p.seed == crate::plant::SeedType::Sunflower && !p.dead).count()
    }

    /// The lesson that follows picking up the sunflower (`SeedPacket::MouseDown`).
    pub(super) fn tutorial_sunflower_picked_up(&mut self, res: &GameResources, seed: crate::plant::SeedType) {
        let state = self.tutorial_state;
        if state.is_sunflower_pick_up() {
            let next = if seed == crate::plant::SeedType::Sunflower { state.plant_state() } else { state.refresh_state() };
            self.set_tutorial_state(res, next);
        }
    }

    /// What follows the carried sunflower going back or being planted
    /// (`Board::ClearCursor` and `MouseDownWithPlant`).
    pub(super) fn tutorial_sunflower_done_carrying(&mut self, res: &GameResources, planted: Option<crate::plant::SeedType>) {
        let state = self.tutorial_state;
        if state.is_level2() && state == TutorialState::Level2PlantSunflower && planted.is_some() {
            let count = self.count_sunflowers();
            if planted == Some(crate::plant::SeedType::Sunflower) && count == 2 {
                self.display_advice(res, "[ADVICE_MORE_SUNFLOWERS]", MessageStyle::TutorialLevel2, None);
                let next = if self.packet_can_pick_up(1) { state.pick_up_state() } else { state.refresh_state() };
                self.set_tutorial_state(res, next);
            } else if count >= 3 {
                self.set_tutorial_state(res, TutorialState::Level2Completed);
            } else {
                let next = if self.packet_can_pick_up(1) { state.pick_up_state() } else { state.refresh_state() };
                self.set_tutorial_state(res, next);
            }
        } else if state == TutorialState::MoreSunPlantSunflower && planted.is_some() {
            if self.count_sunflowers() >= 3 {
                self.set_tutorial_state(res, TutorialState::MoreSunCompleted);
                let help = Some(AdviceType::PlantSunflower5);
                self.display_advice(res, "[ADVICE_PLANT_SUNFLOWER5]", MessageStyle::TutorialLater, help);
                self.tutorial_timer = -1;
            } else {
                let next = if self.packet_can_pick_up(1) { state.pick_up_state() } else { state.refresh_state() };
                self.set_tutorial_state(res, next);
            }
        } else if (state.is_level2() || state.is_more_sun()) && planted.is_none() && state != state.pick_up_state() {
            // The carried sunflower went back.
            let next = if self.packet_can_pick_up(1) { state.pick_up_state() } else { state.refresh_state() };
            self.set_tutorial_state(res, next);
        }
    }

    /// `SeedPacket::FlashIfReady`: a packet becoming usable sparkles, and
    /// moves the level 1 tutorial on.
    pub(super) fn flash_if_ready(&mut self, res: &GameResources, index: usize) {
        if !self.packet_can_pick_up(index) {
            return;
        }
        let bank = &self.seed_bank;
        let (x, y) = (bank.x + bank.packets[index].x, bank.y + bank.packets[index].y);
        let order = make_render_order(render_layer::UI_BOTTOM, 0, 2);
        self.add_particle(x as f32, y as f32, order, ParticleEffect::SeedPacketFlash);
        if self.tutorial_state == TutorialState::Level1RefreshPeashooter {
            self.set_tutorial_state(res, TutorialState::Level1PickUpPeashooter);
        } else if index == 1
            && matches!(self.tutorial_state, TutorialState::Level2RefreshSunflower | TutorialState::MoreSunRefreshSunflower)
        {
            let next = self.tutorial_state.pick_up_state();
            self.set_tutorial_state(res, next);
        }
    }
}
