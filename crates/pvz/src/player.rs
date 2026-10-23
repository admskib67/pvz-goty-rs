//! The current player's profile (`PlayerInfo`) and the unlock checks built
//! on it (`LawnApp::CanShowAlmanac` and friends).
//!
//! TODO: profiles aren't saved yet, and the original asks for a name on first
//! run (`DoCreateUserDialog`). Until then everyone is a new player.

use bevy::prelude::*;
use sexy::rand::MtRand;

#[derive(Resource, Debug, Clone)]
pub struct PlayerInfo {
    pub name: String,
    /// Next adventure level, 1-based (level 1-1 is 1, 2-1 is 11, ...).
    pub level: i32,
    /// Times adventure mode has been completed.
    pub finished_adventure: i32,
    pub has_seen_upsell: bool,
    pub has_unlocked_minigames: bool,
    pub has_unlocked_puzzle_mode: bool,
    pub has_unlocked_survival_mode: bool,
}

impl Default for PlayerInfo {
    fn default() -> Self {
        Self {
            name: "Player".to_owned(),
            level: 1,
            finished_adventure: 0,
            has_seen_upsell: false,
            has_unlocked_minigames: false,
            has_unlocked_puzzle_mode: false,
            has_unlocked_survival_mode: false,
        }
    }
}

impl PlayerInfo {
    pub fn has_finished_adventure(&self) -> bool {
        self.finished_adventure > 0
    }

    pub fn can_show_almanac(&self) -> bool {
        self.has_finished_adventure() || self.level >= 15
    }

    pub fn can_show_store(&self) -> bool {
        self.has_finished_adventure() || self.has_seen_upsell || self.level >= 25
    }

    pub fn can_show_zen_garden(&self) -> bool {
        self.has_finished_adventure() || self.level >= 45
    }
}

/// The game's random number generator (the original's global `MTRand`).
#[derive(Resource, Debug)]
pub struct GameRng(pub MtRand);

impl Default for GameRng {
    fn default() -> Self {
        // The original seeds from the tick count at startup.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(4357, |d| d.as_millis() as u32);
        Self(MtRand::new(seed))
    }
}
