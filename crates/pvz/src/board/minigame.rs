//! Mini-games: the challenge screen's `Challenge` page, one entry per
//! `gChallengeDefs` row, and the board rules each one changes.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Widget/ChallengeScreen.cpp`
//! (`gChallengeDefs`), `Lawn/Challenge.cpp` (`InitLevel`, `StartLevel`,
//! `InitZombieWaves`) and `Lawn/Board.cpp` (`PickBackground`, the
//! `mNumWaves` table, `PickZombieWaves`' per-mode points and fixed waves).
//!
//! Ported so far (`MiniGame::is_ported`): War and Peas 1 and 2, Wall-nut
//! Bowling 1 and 2, Big Trouble Little Zombie, Bobsled Bonanza,
//! Whack a Zombie, Pogo Party, Column Like You See 'Em, Invisighoul and
//! Dr. Zomboss's Revenge. The rest of the page shows as Coming Soon.
//!
//! Not yet: the per-game extras for the rest of the page (Slot Machine,
//! Raining Seeds, Beghouled 1 and 2, Seeing Stars, Zombiquarium, Portal
//! Combat, Zombies on Speed, Last Stand) and War and Peas' debug-only head
//! spawns.

use sexy::rand::MtRand;

use crate::zombie::ZombieType;

use super::waves::{WaveState, ZOMBIE_COUNTDOWN_FIRST_WAVE};
use super::{BackgroundType, Board, MAX_GRID_SIZE_Y, PlantRowType};

/// `gChallengeDefs`' entries on `CHALLENGE_PAGE_CHALLENGE`, in page order
/// (`mRow * 5 + mCol`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MiniGame {
    WarAndPeas,
    WallnutBowling,
    SlotMachine,
    RainingSeeds,
    Beghouled,
    Invisighoul,
    SeeingStars,
    Zombiquarium,
    BeghouledTwist,
    LittleTrouble,
    PortalCombat,
    Column,
    BobsledBonanza,
    Speed,
    WhackAZombie,
    LastStand,
    WarAndPeas2,
    WallnutBowling2,
    PogoParty,
    FinalBoss,
}

impl MiniGame {
    pub const ALL: [Self; 20] = [
        Self::WarAndPeas,
        Self::WallnutBowling,
        Self::SlotMachine,
        Self::RainingSeeds,
        Self::Beghouled,
        Self::Invisighoul,
        Self::SeeingStars,
        Self::Zombiquarium,
        Self::BeghouledTwist,
        Self::LittleTrouble,
        Self::PortalCombat,
        Self::Column,
        Self::BobsledBonanza,
        Self::Speed,
        Self::WhackAZombie,
        Self::LastStand,
        Self::WarAndPeas2,
        Self::WallnutBowling2,
        Self::PogoParty,
        Self::FinalBoss,
    ];

    /// `mChallengeMode` on the challenge page: the tile's order.
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&m| m == self).unwrap_or(0)
    }

    /// `mRow`.
    pub fn row(self) -> i32 {
        (self.index() / 5) as i32
    }

    /// `mCol`.
    pub fn col(self) -> i32 {
        (self.index() % 5) as i32
    }

    /// `mChallengeIconIndex`: the cel in `Challenge_Thumbnails`.
    pub fn icon(self) -> u32 {
        match self {
            Self::WarAndPeas | Self::WarAndPeas2 => 0,
            Self::WallnutBowling | Self::WallnutBowling2 => 6,
            Self::SlotMachine => 2,
            Self::RainingSeeds => 3,
            Self::Beghouled => 1,
            Self::Invisighoul => 8,
            Self::SeeingStars => 5,
            Self::Zombiquarium => 7,
            Self::BeghouledTwist => 20,
            Self::LittleTrouble => 12,
            Self::PortalCombat => 15,
            Self::Column => 4,
            Self::BobsledBonanza => 17,
            Self::Speed => 18,
            Self::WhackAZombie => 16,
            Self::LastStand => 21,
            Self::PogoParty => 14,
            Self::FinalBoss => 19,
        }
    }

    /// `mChallengeName`, the `LawnStrings` key.
    pub fn name(self) -> &'static str {
        match self {
            Self::WarAndPeas => "[WAR_AND_PEAS]",
            Self::WallnutBowling => "[WALL_NUT_BOWLING]",
            Self::SlotMachine => "[SLOT_MACHINE]",
            Self::RainingSeeds => "[ITS_RAINING_SEEDS]",
            Self::Beghouled => "[BEGHOULED]",
            Self::Invisighoul => "[INVISIGHOUL]",
            Self::SeeingStars => "[SEEING_STARS]",
            Self::Zombiquarium => "[ZOMBIQUARIUM]",
            Self::BeghouledTwist => "[BEGHOULED_TWIST]",
            Self::LittleTrouble => "[LITTLE_TROUBLE]",
            Self::PortalCombat => "[PORTAL_COMBAT]",
            Self::Column => "[COLUMN_AS_YOU_SEE_EM]",
            Self::BobsledBonanza => "[BOBSLED_BONANZA]",
            Self::Speed => "[ZOMBIES_ON_SPEED]",
            Self::WhackAZombie => "[WHACK_A_ZOMBIE]",
            Self::LastStand => "[LAST_STAND]",
            Self::WarAndPeas2 => "[WAR_AND_PEAS_2]",
            Self::WallnutBowling2 => "[WALL_NUT_BOWLING_EXTREME]",
            Self::PogoParty => "[POGO_PARTY]",
            Self::FinalBoss => "[FINAL_BOSS]",
        }
    }

    /// Whether this port plays the game; the rest of the page says so.
    pub fn is_ported(self) -> bool {
        matches!(
            self,
            Self::WarAndPeas
                | Self::WallnutBowling
                | Self::LittleTrouble
                | Self::BobsledBonanza
                | Self::WhackAZombie
                | Self::WarAndPeas2
                | Self::WallnutBowling2
                | Self::PogoParty
                | Self::Column
                | Self::Invisighoul
                | Self::FinalBoss
        )
    }

    /// `PickBackground`'s per-mode lawn.
    pub fn background(self) -> BackgroundType {
        match self {
            Self::WarAndPeas | Self::WallnutBowling | Self::SeeingStars | Self::WallnutBowling2 => BackgroundType::Day,
            Self::SlotMachine => BackgroundType::Day,
            Self::Beghouled | Self::BeghouledTwist | Self::PortalCombat | Self::WhackAZombie => BackgroundType::Night,
            Self::LittleTrouble | Self::BobsledBonanza | Self::Speed | Self::LastStand | Self::WarAndPeas2 => BackgroundType::Pool,
            Self::RainingSeeds | Self::Invisighoul => BackgroundType::Fog,
            Self::Column | Self::PogoParty => BackgroundType::Roof,
            Self::Zombiquarium => BackgroundType::Day,
            Self::FinalBoss => BackgroundType::Boss,
        }
    }

    /// `Board`'s `mNumWaves` table for non-adventure modes.
    pub fn num_waves(self) -> i32 {
        match self {
            Self::WhackAZombie => 12,
            Self::WallnutBowling
            | Self::WarAndPeas
            | Self::Invisighoul
            | Self::PortalCombat => 20,
            Self::LittleTrouble
            | Self::Column
            | Self::WarAndPeas2
            | Self::WallnutBowling2
            | Self::PogoParty => 30,
            _ => 40,
        }
    }

    /// `Challenge::InitZombieWaves`'s list for the mode.
    pub fn allowed(self) -> &'static [ZombieType] {
        use ZombieType as Z;
        match self {
            Self::WarAndPeas => &[Z::PeaHead, Z::WallnutHead],
            Self::WarAndPeas2 => &[Z::PeaHead, Z::WallnutHead, Z::JalapenoHead, Z::GatlingHead, Z::SquashHead, Z::TallnutHead],
            Self::WallnutBowling => &[Z::Normal, Z::TrafficCone, Z::Pail, Z::Polevaulter, Z::Newspaper],
            Self::WallnutBowling2 => &[Z::Normal, Z::TrafficCone, Z::Pail, Z::Polevaulter, Z::Newspaper, Z::Dancer, Z::Door],
            Self::LittleTrouble => &[Z::Normal, Z::TrafficCone, Z::Football, Z::Snorkel],
            Self::BobsledBonanza => &[Z::Bobsled, Z::Zamboni],
            Self::WhackAZombie => &[Z::Normal, Z::TrafficCone, Z::Pail],
            Self::PogoParty => &[Z::Pogo],
            Self::Column => &[Z::Normal, Z::TrafficCone, Z::Pail, Z::Football],
            Self::Invisighoul => &[Z::Normal, Z::TrafficCone, Z::Pail, Z::DolphinRider, Z::Zamboni, Z::JackInTheBox],
            // The boss level's own waves never run (`is_final_boss_level`);
            // the table is the reference's fallback.
            Self::FinalBoss => &[Z::Normal, Z::TrafficCone, Z::Pail],
            // Not ported yet: their tables are ignored until they are.
            _ => &[],
        }
    }

    /// `PickZombieWaves`' per-mode zombie-point multiplier.
    fn points_multiplier(self) -> i32 {
        match self {
            Self::LittleTrouble | Self::WallnutBowling | Self::WallnutBowling2 => 4,
            Self::Column => 6,
            Self::Invisighoul => 2,
            _ => 1,
        }
    }

    /// `Challenge::UpdateConveyorBelt`'s per-mode speed multiplier.
    pub fn conveyor_speed_multiplier(self) -> f32 {
        match self {
            Self::FinalBoss => 0.875,
            Self::Invisighoul => 2.0,
            Self::Column => 3.0,
            _ => 1.0,
        }
    }

    /// War and Peas' flag waves take no plain or flag zombies: there are no
    /// ordinary zombies in the mode.
    pub fn plain_flag_waves(self) -> bool {
        !matches!(self, Self::WarAndPeas | Self::WarAndPeas2)
    }

    /// `Challenge::StartLevel`'s first-wave delay.
    pub fn first_wave_countdown(self) -> i32 {
        match self {
            Self::WallnutBowling | Self::WallnutBowling2 | Self::LittleTrouble | Self::WhackAZombie | Self::Invisighoul => 200,
            Self::Column => 2400,
            Self::BobsledBonanza => 4500,
            Self::PogoParty => 5500,
            _ => ZOMBIE_COUNTDOWN_FIRST_WAVE,
        }
    }

    /// `PickZombieType` skips `mFirstAllowedWave` in these modes.
    pub fn ignores_first_allowed_wave(self) -> bool {
        matches!(self, Self::PogoParty | Self::BobsledBonanza)
    }
}

impl Board {
    /// The mini-game being played, if any.
    pub fn minigame(&self) -> Option<MiniGame> {
        match self.mode {
            super::GameMode::MiniGame(game) => Some(game),
            _ => None,
        }
    }

    pub fn is_minigame(&self) -> bool {
        self.minigame().is_some()
    }

    /// Challenge modes (survival and mini-games): level 50's adventure rules
    /// don't apply to them.
    pub fn is_challenge(&self) -> bool {
        self.is_survival() || self.is_minigame()
    }

    /// `Board::InitZombieWaves` for a mini-game.
    pub(super) fn init_minigame_waves(&mut self, rng: &mut MtRand) {
        let Some(game) = self.minigame() else { return };
        self.waves = WaveState::default();
        self.waves.num_waves = game.num_waves();
        self.waves.zombies_in_wave = vec![Vec::new(); self.waves.num_waves as usize];
        for &ty in game.allowed() {
            self.waves.zombie_allowed[ty as usize] = true;
        }
        self.pick_zombie_waves(rng);
        self.waves.zombie_count_down = game.first_wave_countdown();
        self.waves.zombie_count_down_start = self.waves.zombie_count_down;
        // `Challenge::StartLevel`: Column Like You See 'Em starts at wave 9.
        if game == MiniGame::Column {
            self.waves.current_wave = 9;
        }
        // `Challenge::StartLevel`: Bobsled Bonanza starts with the lawn frozen.
        if game == MiniGame::BobsledBonanza {
            for row in 0..MAX_GRID_SIZE_Y {
                if self.plant_row[row] != PlantRowType::Pool {
                    self.ice_min_x[row] = 400;
                    self.ice_timer[row] = i32::MAX;
                }
            }
        }
    }

    /// `PickZombieWaves`'s multiplier for the current mini-game.
    pub(super) fn minigame_points_multiplier(&self) -> i32 {
        self.minigame().map_or(1, MiniGame::points_multiplier)
    }

    /// `Challenge::UpdateConveyorBelt`'s speed for the current mode.
    pub(super) fn conveyor_speed_multiplier(&self) -> f32 {
        self.minigame().map_or(1.0, MiniGame::conveyor_speed_multiplier)
    }

    /// `Zombie::IsVisible`: Invisighoul's zombies aren't drawn (their bites
    /// still land). The level intro's street zombies are.
    pub fn zombie_hidden(&self, zombie: &super::zombie::Zombie) -> bool {
        self.minigame() == Some(MiniGame::Invisighoul) && zombie.from_wave != super::zombie::ZOMBIE_WAVE_CUTSCENE
    }
}
