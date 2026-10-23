//! The adventure tutorials (`TutorialState`) and the board's one-off hints
//! (`AdviceType`). So far: level 1's "pick up the peashooter, plant it,
//! collect sun, plant another" walkthrough.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Board.cpp`: `SetTutorialState`,
//! `UpdateTutorial`, `DisplayAdvice`, `ClearAdvice`; and
//! `SeedPacket::FlashIfReady`.

use crate::resources::GameResources;

use super::Board;
use super::message::MessageStyle;

/// `TutorialState` (the level 1 part so far).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TutorialState {
    Off,
    Level1PickUpPeashooter,
    Level1PlantPeashooter,
    Level1RefreshPeashooter,
    Level1Completed,
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

    /// `Board::ClearAdvice`: clears the banner if it's showing `help` (or
    /// whatever it's showing, for `None`).
    pub(super) fn clear_advice(&mut self, help: Option<AdviceType>) {
        if help.is_none() || help == self.help_index {
            self.advice.clear_label();
            self.help_index = None;
        }
    }

    /// `Board::SetTutorialState`. TODO: the bouncing arrow over the seed
    /// packet (`TutorialArrowShow`, a particle).
    pub(super) fn set_tutorial_state(&mut self, res: &GameResources, state: TutorialState) {
        use MessageStyle::TutorialLevel1Stay;
        match state {
            TutorialState::Level1PickUpPeashooter => {
                if self.plants.is_empty() {
                    self.display_advice(res, "[ADVICE_CLICK_SEED_PACKET]", TutorialLevel1Stay, None);
                } else {
                    self.display_advice(res, "[ADVICE_ENOUGH_SUN]", TutorialLevel1Stay, None);
                    self.tutorial_timer = 400;
                }
            }
            TutorialState::Level1PlantPeashooter => {
                self.tutorial_timer = -1;
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
                // TODO: the first zombie comes right away (mZombieCountDown = 99).
            }
            TutorialState::Off => {}
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
            self.tutorial_timer = -1;
        }
    }

    /// `SeedPacket::FlashIfReady`: a packet becoming usable moves the level 1
    /// tutorial on. TODO: the packet's sparkle (a particle).
    pub(super) fn flash_if_ready(&mut self, res: &GameResources, index: usize) {
        if !self.packet_can_pick_up(index) {
            return;
        }
        if self.tutorial_state == TutorialState::Level1RefreshPeashooter {
            self.set_tutorial_state(res, TutorialState::Level1PickUpPeashooter);
        }
    }
}
