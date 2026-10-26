//! Zombie types and their fixed data (`ZombieType`, `gZombieDefs`), and the
//! level tables that decide which zombies a level has
//! (`gZombieWaves`, `gZombieAllowedLevels`).
//!
//! Behaviour per ruslan831 decomp, `Lawn/Zombie.cpp` (`gZombieDefs`,
//! `GetZombieDefinition`) and `Lawn/Challenge.cpp` (`gZombieWaves`,
//! `gZombieAllowedLevels`).

use crate::reanim::ReanimationType;

/// `ZombieType`, in the original's order.
#[allow(dead_code, reason = "the full table; waves use more of it as zombies are ported")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ZombieType {
    Normal,
    Flag,
    TrafficCone,
    Polevaulter,
    Pail,
    Newspaper,
    Door,
    Football,
    Dancer,
    BackupDancer,
    DuckyTube,
    Snorkel,
    Zamboni,
    Bobsled,
    DolphinRider,
    JackInTheBox,
    Balloon,
    Digger,
    Pogo,
    Yeti,
    Bungee,
    Ladder,
    Catapult,
    Gargantuar,
    Imp,
    Boss,
    PeaHead,
    WallnutHead,
    JalapenoHead,
    GatlingHead,
    SquashHead,
    TallnutHead,
    RedeyeGargantuar,
}

pub const NUM_ZOMBIE_TYPES: usize = ZombieType::RedeyeGargantuar as usize + 1;

/// `ZombieDefinition`.
#[derive(Debug, Clone, Copy)]
pub struct ZombieDefinition {
    /// What a zombie of this type is worth when the wave picker spends its points.
    pub zombie_value: i32,
    /// The first adventure level it can appear on.
    pub starting_level: i32,
    /// The first wave (1-based) it can appear in.
    pub first_allowed_wave: i32,
    pub pick_weight: i32,
}

const fn def(zombie_value: i32, starting_level: i32, first_allowed_wave: i32, pick_weight: i32) -> ZombieDefinition {
    ZombieDefinition { zombie_value, starting_level, first_allowed_wave, pick_weight }
}

/// `gZombieDefs`.
static ZOMBIE_DEFS: [ZombieDefinition; NUM_ZOMBIE_TYPES] = [
    def(1, 1, 1, 4000),   // Normal
    def(1, 1, 1, 0),      // Flag
    def(2, 3, 1, 4000),   // TrafficCone
    def(2, 6, 5, 2000),   // Polevaulter
    def(4, 8, 1, 3000),   // Pail
    def(2, 11, 1, 1000),  // Newspaper
    def(4, 13, 5, 3500),  // Door
    def(7, 16, 5, 2000),  // Football
    def(5, 18, 5, 1000),  // Dancer
    def(1, 18, 1, 0),     // BackupDancer
    def(1, 21, 5, 0),     // DuckyTube
    def(3, 23, 10, 2000), // Snorkel
    def(7, 26, 10, 2000), // Zamboni
    def(3, 26, 10, 2000), // Bobsled
    def(3, 28, 10, 1500), // DolphinRider
    def(3, 31, 10, 1000), // JackInTheBox
    def(2, 33, 10, 2000), // Balloon
    def(4, 36, 10, 1000), // Digger
    def(4, 38, 10, 1000), // Pogo
    def(4, 40, 1, 1),     // Yeti
    def(3, 41, 10, 1000), // Bungee
    def(4, 43, 10, 1000), // Ladder
    def(5, 46, 10, 1500), // Catapult
    def(10, 48, 15, 1500), // Gargantuar
    def(10, 48, 1, 0),    // Imp
    def(10, 50, 1, 0),    // Boss
    def(1, 99, 1, 4000),  // PeaHead
    def(4, 99, 1, 3000),  // WallnutHead
    def(3, 99, 10, 1000), // JalapenoHead
    def(3, 99, 10, 2000), // GatlingHead
    def(3, 99, 10, 2000), // SquashHead
    def(4, 99, 10, 2000), // TallnutHead
    def(10, 48, 15, 6000), // RedeyeGargantuar
];

impl ZombieType {
    pub fn definition(self) -> &'static ZombieDefinition {
        &ZOMBIE_DEFS[self as usize]
    }

    pub fn from_index(i: usize) -> Self {
        ALL[i]
    }

    /// The reanimation a zombie of this type plays (`mReanimationType`), for
    /// the types ported so far.
    pub fn reanim(self) -> Option<ReanimationType> {
        match self {
            Self::Normal | Self::Flag | Self::TrafficCone | Self::Pail | Self::Door | Self::DuckyTube => Some(ReanimationType::Zombie),
            Self::Polevaulter => Some(ReanimationType::Polevaulter),
            Self::Snorkel => Some(ReanimationType::Snorkel),
            Self::DolphinRider => Some(ReanimationType::ZombieDolphinrider),
            Self::Zamboni => Some(ReanimationType::ZombieZamboni),
            Self::Bobsled => Some(ReanimationType::Bobsled),
            Self::Newspaper => Some(ReanimationType::ZombieNewspaper),
            Self::Football => Some(ReanimationType::ZombieFootball),
            Self::Dancer => Some(ReanimationType::Dancer),
            Self::BackupDancer => Some(ReanimationType::BackupDancer),
            Self::Bungee => Some(ReanimationType::Bungee),
            Self::Ladder => Some(ReanimationType::Ladder),
            Self::Catapult => Some(ReanimationType::Catapult),
            Self::Gargantuar => Some(ReanimationType::Gargantuar),
            Self::Imp => Some(ReanimationType::Imp),
            Self::Balloon => Some(ReanimationType::Balloon),
            Self::Boss => Some(ReanimationType::Boss),
            _ => None,
        }
    }

    /// The reanimation that shows a zombie of this type in the almanac,
    /// including the types not ported yet (`gZombieDefs`' `mReanimationType`).
    pub fn almanac_reanim(self) -> Option<ReanimationType> {
        self.reanim().or(match self {
            Self::JackInTheBox => Some(ReanimationType::Jackinthebox),
            Self::Digger => Some(ReanimationType::Digger),
            Self::Pogo => Some(ReanimationType::Pogo),
            Self::Yeti => Some(ReanimationType::Yeti),
            _ => None,
        })
    }

    /// The `[NAME]` in `LawnStrings.txt`, without brackets (`mZombieName`).
    pub fn name(self) -> &'static str {
        ZOMBIE_NAMES[self as usize]
    }

    /// `Board::IsZombieTypeSpawnedOnly`: zombies that only come with another.
    pub fn is_spawned_only(self) -> bool {
        matches!(self, Self::BackupDancer | Self::Bobsled | Self::Imp)
    }

    /// `Zombie::ZombieTypeCanGoInPool`: who can wade into the water.
    pub fn can_go_in_pool(self) -> bool {
        matches!(
            self,
            Self::Normal | Self::TrafficCone | Self::Pail | Self::Flag | Self::Balloon | Self::Snorkel | Self::DolphinRider
                | Self::PeaHead | Self::WallnutHead | Self::JalapenoHead | Self::GatlingHead | Self::TallnutHead
        )
    }

    /// `Board::IsZombieTypePoolOnly`.
    pub fn is_pool_only(self) -> bool {
        matches!(self, Self::Snorkel | Self::DolphinRider)
    }

    /// Whether zombies of this type are ported. Until the rest are, waves
    /// only pick from these.
    pub fn is_implemented(self) -> bool {
        self.reanim().is_some()
    }
}

const ALL: [ZombieType; NUM_ZOMBIE_TYPES] = {
    use ZombieType::*;
    [
        Normal, Flag, TrafficCone, Polevaulter, Pail, Newspaper, Door, Football, Dancer, BackupDancer, DuckyTube,
        Snorkel, Zamboni, Bobsled, DolphinRider, JackInTheBox, Balloon, Digger, Pogo, Yeti, Bungee, Ladder,
        Catapult, Gargantuar, Imp, Boss, PeaHead, WallnutHead, JalapenoHead, GatlingHead, SquashHead, TallnutHead,
        RedeyeGargantuar,
    ]
};

/// `gZombieDefs`' names.
const ZOMBIE_NAMES: [&str; NUM_ZOMBIE_TYPES] = [
    "ZOMBIE", "FLAG_ZOMBIE", "CONEHEAD_ZOMBIE", "POLE_VAULTING_ZOMBIE", "BUCKETHEAD_ZOMBIE", "NEWSPAPER_ZOMBIE",
    "SCREEN_DOOR_ZOMBIE", "FOOTBALL_ZOMBIE", "DANCING_ZOMBIE", "BACKUP_DANCER", "DUCKY_TUBE_ZOMBIE", "SNORKEL_ZOMBIE",
    "ZOMBONI", "ZOMBIE_BOBSLED_TEAM", "DOLPHIN_RIDER_ZOMBIE", "JACK_IN_THE_BOX_ZOMBIE", "BALLOON_ZOMBIE", "DIGGER_ZOMBIE",
    "POGO_ZOMBIE", "ZOMBIE_YETI", "BUNGEE_ZOMBIE", "LADDER_ZOMBIE", "CATAPULT_ZOMBIE", "GARGANTUAR", "IMP", "BOSS",
    "ZOMBIE", "ZOMBIE", "ZOMBIE", "ZOMBIE", "ZOMBIE", "ZOMBIE", "REDEYED_GARGANTUAR",
];

/// `gZombieWaves`: how many waves each adventure level has.
const ZOMBIE_WAVES: [i32; 50] = [
    4, 6, 8, 10, 8, 10, 20, 10, 20, 20, //
    10, 20, 10, 20, 10, 10, 20, 10, 20, 20, //
    10, 20, 20, 30, 20, 20, 30, 20, 30, 30, //
    10, 20, 10, 20, 20, 10, 20, 10, 20, 20, //
    10, 20, 20, 30, 20, 20, 30, 20, 30, 30,
];

pub fn waves_for_level(level: i32) -> i32 {
    ZOMBIE_WAVES[(level - 1).clamp(0, 49) as usize]
}

/// `gZombieAllowedLevels` for the zombies ported so far: the levels (1 to
/// 50) each can appear on.
const CONE_LEVELS: [bool; 50] = bits(&[
    0, 0, 1, 1, 1, 1, 1, 1, 1, 1, //
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, //
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, //
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, //
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
]);
/// `gZombieAllowedLevels` for the snorkel zombie.
const SNORKEL_LEVELS: [bool; 50] = bits(&[
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 1, 1, 1, 0, 1, 0, 0, 1, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
]);
/// `gZombieAllowedLevels` for the dolphin rider.
const DOLPHIN_LEVELS: [bool; 50] = bits(&[
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 1, 1, 1, //
    0, 0, 0, 1, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
]);
/// `gZombieAllowedLevels` for the Zamboni.
const ZAMBONI_LEVELS: [bool; 50] = bits(&[
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 1, 1, 0, 1, 1, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
]);
const PAIL_LEVELS: [bool; 50] = bits(&[
    0, 0, 0, 0, 0, 0, 0, 1, 1, 1, //
    0, 1, 0, 0, 1, 0, 0, 0, 0, 0, //
    0, 1, 0, 1, 0, 0, 1, 0, 1, 1, //
    0, 0, 0, 0, 0, 0, 1, 0, 1, 1, //
    0, 1, 0, 0, 1, 0, 0, 0, 1, 1,
]);

const POLEVAULTER_LEVELS: [bool; 50] = bits(&[
    0, 0, 0, 0, 0, 1, 1, 0, 1, 1, //
    0, 0, 0, 1, 1, 0, 0, 0, 0, 0, //
    0, 0, 0, 1, 0, 0, 0, 0, 1, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 1, 0, 0, 0, 0, 0, 0, 0, 0,
]);

const NEWSPAPER_LEVELS: [bool; 50] = bits(&[
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    1, 1, 0, 0, 1, 0, 0, 0, 0, 0, //
    0, 1, 0, 1, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
]);
const DOOR_LEVELS: [bool; 50] = bits(&[
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 1, 1, 0, 0, 1, 0, 1, 1, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
]);
const FOOTBALL_LEVELS: [bool; 50] = bits(&[
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 1, 1, 0, 0, 1, //
    0, 1, 0, 0, 1, 0, 0, 0, 0, 0, //
    0, 1, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 1, 0, 0, 0, 0, 0, 0,
]);
/// The dancer's backup dancers come with it, on the same levels.
const DANCER_LEVELS: [bool; 50] = bits(&[
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 1, 1, 1, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
]);

const fn bits(v: &[u8; 50]) -> [bool; 50] {
    let mut out = [false; 50];
    let mut i = 0;
    while i < 50 {
        out[i] = v[i] != 0;
        i += 1;
    }
    out
}

/// `Board::CanZombieSpawnOnLevel`. Zombies that aren't ported yet are
/// never allowed.
pub fn can_spawn_on_level(ty: ZombieType, level: i32) -> bool {
    let d = ty.definition();
    if !ty.is_implemented() || level < d.starting_level || d.pick_weight == 0 {
        return false;
    }
    let i = (level - 1).clamp(0, 49) as usize;
    match ty {
        ZombieType::TrafficCone => CONE_LEVELS[i],
        ZombieType::Pail => PAIL_LEVELS[i],
        ZombieType::Polevaulter => POLEVAULTER_LEVELS[i],
        ZombieType::Newspaper => NEWSPAPER_LEVELS[i],
        ZombieType::Door => DOOR_LEVELS[i],
        ZombieType::Football => FOOTBALL_LEVELS[i],
        ZombieType::Dancer | ZombieType::BackupDancer => DANCER_LEVELS[i],
        ZombieType::Snorkel => SNORKEL_LEVELS[i],
        ZombieType::DolphinRider => DOLPHIN_LEVELS[i],
        ZombieType::Zamboni | ZombieType::Bobsled => ZAMBONI_LEVELS[i],
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_levels_only_have_plain_zombies() {
        assert!(can_spawn_on_level(ZombieType::Normal, 1));
        assert!(!can_spawn_on_level(ZombieType::TrafficCone, 2));
        assert!(can_spawn_on_level(ZombieType::TrafficCone, 3));
        assert!(!can_spawn_on_level(ZombieType::Pail, 7));
        assert!(can_spawn_on_level(ZombieType::Pail, 8));
        assert!(!can_spawn_on_level(ZombieType::Flag, 1) || ZombieType::Flag.definition().pick_weight == 0);
    }

    #[test]
    fn night_zombies_start_on_their_levels() {
        assert!(can_spawn_on_level(ZombieType::Newspaper, 11));
        assert!(!can_spawn_on_level(ZombieType::Door, 12));
        assert!(can_spawn_on_level(ZombieType::Door, 13));
        assert!(can_spawn_on_level(ZombieType::Football, 16));
        assert!(can_spawn_on_level(ZombieType::Dancer, 18));
        // Backup dancers only come with a dancer, never in a wave of their own.
        assert!(!can_spawn_on_level(ZombieType::BackupDancer, 18));
    }

    #[test]
    fn table_matches_enum_order() {
        assert_eq!(ZombieType::from_index(ZombieType::Pail as usize), ZombieType::Pail);
        assert_eq!(NUM_ZOMBIE_TYPES, 33);
        assert_eq!(waves_for_level(1), 4);
    }
}
