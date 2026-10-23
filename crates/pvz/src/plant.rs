//! Plant types and their fixed data (`SeedType`, `gPlantDefs`), and which
//! plants the player has.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Plant.cpp` (`gPlantDefs`,
//! `GetPlantDefinition`, `Plant::GetCost`, `IsUpgrade`) and `LawnApp.cpp`
//! (`GetAwardSeedForLevel`, `GetSeedsAvailable`, `HasSeedType`).

use crate::player::PlayerInfo;
use crate::reanim::ReanimationType;

/// `SeedType`: what a seed packet plants, in the original's order (which
/// is also the order adventure mode awards them).
#[allow(dead_code, reason = "the full table; levels use more of it as they're ported")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SeedType {
    Peashooter,
    Sunflower,
    Cherrybomb,
    Wallnut,
    Potatomine,
    Snowpea,
    Chomper,
    Repeater,
    Puffshroom,
    Sunshroom,
    Fumeshroom,
    Gravebuster,
    Hypnoshroom,
    Scaredyshroom,
    Iceshroom,
    Doomshroom,
    Lilypad,
    Squash,
    Threepeater,
    Tanglekelp,
    Jalapeno,
    Spikeweed,
    Torchwood,
    Tallnut,
    Seashroom,
    Plantern,
    Cactus,
    Blover,
    Splitpea,
    Starfruit,
    Pumpkinshell,
    Magnetshroom,
    Cabbagepult,
    Flowerpot,
    Kernelpult,
    InstantCoffee,
    Garlic,
    Umbrella,
    Marigold,
    Melonpult,
    Gatlingpea,
    Twinsunflower,
    Gloomshroom,
    Cattail,
    Wintermelon,
    GoldMagnet,
    Spikerock,
    Cobcannon,
    Imitater,
    ExplodeONut,
    GiantWallnut,
    Sprout,
    Leftpeater,
}

/// `PlantSubClass`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlantSubClass {
    Normal,
    Shooter,
}

/// `PlantDefinition`.
#[allow(dead_code, reason = "fields are used as plants are ported")]
#[derive(Debug, Clone, Copy)]
pub struct PlantDefinition {
    pub reanim: ReanimationType,
    /// Cel of `IMAGE_PACKET_PLANTS` and similar strips.
    pub packet_index: i32,
    /// Sun cost.
    pub cost: i32,
    /// Seed packet recharge time, in ticks.
    pub refresh_time: i32,
    pub sub_class: PlantSubClass,
    /// Ticks between shots (or sun) for plants that launch things.
    pub launch_rate: i32,
    /// The `[NAME]` in `LawnStrings.txt`, without brackets.
    pub name: &'static str,
}

const PLANT_DEFS: [PlantDefinition; 53] = [
    PlantDefinition { reanim: ReanimationType::Peashooter, packet_index: 0, cost: 100, refresh_time: 750, sub_class: PlantSubClass::Shooter, launch_rate: 150, name: "PEASHOOTER" },
    PlantDefinition { reanim: ReanimationType::Sunflower, packet_index: 1, cost: 50, refresh_time: 750, sub_class: PlantSubClass::Normal, launch_rate: 2500, name: "SUNFLOWER" },
    PlantDefinition { reanim: ReanimationType::Cherrybomb, packet_index: 3, cost: 150, refresh_time: 5000, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "CHERRY_BOMB" },
    PlantDefinition { reanim: ReanimationType::Wallnut, packet_index: 2, cost: 50, refresh_time: 3000, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "WALL_NUT" },
    PlantDefinition { reanim: ReanimationType::Potatomine, packet_index: 37, cost: 25, refresh_time: 3000, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "POTATO_MINE" },
    PlantDefinition { reanim: ReanimationType::Snowpea, packet_index: 4, cost: 175, refresh_time: 750, sub_class: PlantSubClass::Shooter, launch_rate: 150, name: "SNOW_PEA" },
    PlantDefinition { reanim: ReanimationType::Chomper, packet_index: 31, cost: 150, refresh_time: 750, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "CHOMPER" },
    PlantDefinition { reanim: ReanimationType::Repeater, packet_index: 5, cost: 200, refresh_time: 750, sub_class: PlantSubClass::Shooter, launch_rate: 150, name: "REPEATER" },
    PlantDefinition { reanim: ReanimationType::Puffshroom, packet_index: 6, cost: 0, refresh_time: 750, sub_class: PlantSubClass::Shooter, launch_rate: 150, name: "PUFF_SHROOM" },
    PlantDefinition { reanim: ReanimationType::Sunshroom, packet_index: 7, cost: 25, refresh_time: 750, sub_class: PlantSubClass::Normal, launch_rate: 2500, name: "SUN_SHROOM" },
    PlantDefinition { reanim: ReanimationType::Fumeshroom, packet_index: 9, cost: 75, refresh_time: 750, sub_class: PlantSubClass::Shooter, launch_rate: 150, name: "FUME_SHROOM" },
    PlantDefinition { reanim: ReanimationType::GraveBuster, packet_index: 40, cost: 75, refresh_time: 750, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "GRAVE_BUSTER" },
    PlantDefinition { reanim: ReanimationType::Hypnoshroom, packet_index: 10, cost: 75, refresh_time: 3000, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "HYPNO_SHROOM" },
    PlantDefinition { reanim: ReanimationType::Scrareyshroom, packet_index: 33, cost: 25, refresh_time: 750, sub_class: PlantSubClass::Shooter, launch_rate: 150, name: "SCAREDY_SHROOM" },
    PlantDefinition { reanim: ReanimationType::Iceshroom, packet_index: 36, cost: 75, refresh_time: 5000, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "ICE_SHROOM" },
    PlantDefinition { reanim: ReanimationType::Doomshroom, packet_index: 20, cost: 125, refresh_time: 5000, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "DOOM_SHROOM" },
    PlantDefinition { reanim: ReanimationType::Lilypad, packet_index: 19, cost: 25, refresh_time: 750, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "LILY_PAD" },
    PlantDefinition { reanim: ReanimationType::Squash, packet_index: 21, cost: 50, refresh_time: 3000, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "SQUASH" },
    PlantDefinition { reanim: ReanimationType::Threepeater, packet_index: 12, cost: 325, refresh_time: 750, sub_class: PlantSubClass::Shooter, launch_rate: 150, name: "THREEPEATER" },
    PlantDefinition { reanim: ReanimationType::Tanglekelp, packet_index: 17, cost: 25, refresh_time: 3000, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "TANGLE_KELP" },
    PlantDefinition { reanim: ReanimationType::Jalapeno, packet_index: 11, cost: 125, refresh_time: 5000, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "JALAPENO" },
    PlantDefinition { reanim: ReanimationType::Spikeweed, packet_index: 22, cost: 100, refresh_time: 750, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "SPIKEWEED" },
    PlantDefinition { reanim: ReanimationType::Torchwood, packet_index: 29, cost: 175, refresh_time: 750, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "TORCHWOOD" },
    PlantDefinition { reanim: ReanimationType::Tallnut, packet_index: 28, cost: 125, refresh_time: 3000, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "TALL_NUT" },
    PlantDefinition { reanim: ReanimationType::Seashroom, packet_index: 39, cost: 0, refresh_time: 3000, sub_class: PlantSubClass::Shooter, launch_rate: 150, name: "SEA_SHROOM" },
    PlantDefinition { reanim: ReanimationType::Plantern, packet_index: 38, cost: 25, refresh_time: 3000, sub_class: PlantSubClass::Normal, launch_rate: 2500, name: "PLANTERN" },
    PlantDefinition { reanim: ReanimationType::Cactus, packet_index: 15, cost: 125, refresh_time: 750, sub_class: PlantSubClass::Shooter, launch_rate: 150, name: "CACTUS" },
    PlantDefinition { reanim: ReanimationType::Blover, packet_index: 18, cost: 100, refresh_time: 750, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "BLOVER" },
    PlantDefinition { reanim: ReanimationType::Splitpea, packet_index: 32, cost: 125, refresh_time: 750, sub_class: PlantSubClass::Shooter, launch_rate: 150, name: "SPLIT_PEA" },
    PlantDefinition { reanim: ReanimationType::Starfruit, packet_index: 30, cost: 125, refresh_time: 750, sub_class: PlantSubClass::Shooter, launch_rate: 150, name: "STARFRUIT" },
    PlantDefinition { reanim: ReanimationType::Pumpkin, packet_index: 25, cost: 125, refresh_time: 3000, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "PUMPKIN" },
    PlantDefinition { reanim: ReanimationType::Magnetshroom, packet_index: 35, cost: 100, refresh_time: 750, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "MAGNET_SHROOM" },
    PlantDefinition { reanim: ReanimationType::Cabbagepult, packet_index: 13, cost: 100, refresh_time: 750, sub_class: PlantSubClass::Shooter, launch_rate: 300, name: "CABBAGE_PULT" },
    PlantDefinition { reanim: ReanimationType::FlowerPot, packet_index: 33, cost: 25, refresh_time: 750, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "FLOWER_POT" },
    PlantDefinition { reanim: ReanimationType::Kernelpult, packet_index: 13, cost: 100, refresh_time: 750, sub_class: PlantSubClass::Shooter, launch_rate: 300, name: "KERNEL_PULT" },
    PlantDefinition { reanim: ReanimationType::Coffeebean, packet_index: 33, cost: 75, refresh_time: 750, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "COFFEE_BEAN" },
    PlantDefinition { reanim: ReanimationType::Garlic, packet_index: 8, cost: 50, refresh_time: 750, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "GARLIC" },
    PlantDefinition { reanim: ReanimationType::Umbrellaleaf, packet_index: 23, cost: 100, refresh_time: 750, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "UMBRELLA_LEAF" },
    PlantDefinition { reanim: ReanimationType::Marigold, packet_index: 24, cost: 50, refresh_time: 3000, sub_class: PlantSubClass::Normal, launch_rate: 2500, name: "MARIGOLD" },
    PlantDefinition { reanim: ReanimationType::Melonpult, packet_index: 14, cost: 300, refresh_time: 750, sub_class: PlantSubClass::Shooter, launch_rate: 300, name: "MELON_PULT" },
    PlantDefinition { reanim: ReanimationType::Gatlingpea, packet_index: 5, cost: 250, refresh_time: 5000, sub_class: PlantSubClass::Shooter, launch_rate: 150, name: "GATLING_PEA" },
    PlantDefinition { reanim: ReanimationType::TwinSunflower, packet_index: 1, cost: 150, refresh_time: 5000, sub_class: PlantSubClass::Normal, launch_rate: 2500, name: "TWIN_SUNFLOWER" },
    PlantDefinition { reanim: ReanimationType::Gloomshroom, packet_index: 27, cost: 150, refresh_time: 5000, sub_class: PlantSubClass::Shooter, launch_rate: 200, name: "GLOOM_SHROOM" },
    PlantDefinition { reanim: ReanimationType::Cattail, packet_index: 27, cost: 225, refresh_time: 5000, sub_class: PlantSubClass::Shooter, launch_rate: 150, name: "CATTAIL" },
    PlantDefinition { reanim: ReanimationType::WinterMelon, packet_index: 27, cost: 200, refresh_time: 5000, sub_class: PlantSubClass::Shooter, launch_rate: 300, name: "WINTER_MELON" },
    PlantDefinition { reanim: ReanimationType::GoldMagnet, packet_index: 27, cost: 50, refresh_time: 5000, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "GOLD_MAGNET" },
    PlantDefinition { reanim: ReanimationType::Spikerock, packet_index: 27, cost: 125, refresh_time: 5000, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "SPIKEROCK" },
    PlantDefinition { reanim: ReanimationType::Cobcannon, packet_index: 16, cost: 500, refresh_time: 5000, sub_class: PlantSubClass::Normal, launch_rate: 600, name: "COB_CANNON" },
    PlantDefinition { reanim: ReanimationType::Imitater, packet_index: 33, cost: 0, refresh_time: 750, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "IMITATER" },
    PlantDefinition { reanim: ReanimationType::Wallnut, packet_index: 2, cost: 0, refresh_time: 3000, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "EXPLODE_O_NUT" },
    PlantDefinition { reanim: ReanimationType::Wallnut, packet_index: 2, cost: 0, refresh_time: 3000, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "GIANT_WALLNUT" },
    PlantDefinition { reanim: ReanimationType::ZengardenSprout, packet_index: 33, cost: 0, refresh_time: 3000, sub_class: PlantSubClass::Normal, launch_rate: 0, name: "SPROUT" },
    PlantDefinition { reanim: ReanimationType::Repeater, packet_index: 5, cost: 200, refresh_time: 750, sub_class: PlantSubClass::Shooter, launch_rate: 150, name: "REPEATER" },
];

/// Plants adventure mode can award (`SEED_IMITATER` is the last seed type
/// counted by `GetSeedsAvailable`).
const NUM_AWARDABLE_SEEDS: i32 = 49;

impl SeedType {
    /// All seed types, in order.
    pub fn from_index(i: i32) -> Option<Self> {
        ALL.get(usize::try_from(i).ok()?).copied()
    }

    /// `GetPlantDefinition`.
    pub fn definition(self) -> &'static PlantDefinition {
        &PLANT_DEFS[self as usize]
    }

    /// `Plant::IsUpgrade`: plants placed on top of another plant.
    pub fn is_upgrade(self) -> bool {
        use SeedType::*;
        matches!(self, Gatlingpea | Wintermelon | Twinsunflower | Spikerock | Cobcannon | GoldMagnet | Gloomshroom | Cattail)
    }

    /// `Plant::GetCost`, outside the challenge modes that change prices.
    pub fn cost(self) -> i32 {
        self.definition().cost
    }
}

const ALL: [SeedType; 53] = {
    use SeedType::*;
    [Peashooter, Sunflower, Cherrybomb, Wallnut, Potatomine, Snowpea, Chomper, Repeater, Puffshroom, Sunshroom, Fumeshroom, Gravebuster, Hypnoshroom, Scaredyshroom, Iceshroom, Doomshroom, Lilypad, Squash, Threepeater, Tanglekelp, Jalapeno, Spikeweed, Torchwood, Tallnut, Seashroom, Plantern, Cactus, Blover, Splitpea, Starfruit, Pumpkinshell, Magnetshroom, Cabbagepult, Flowerpot, Kernelpult, InstantCoffee, Garlic, Umbrella, Marigold, Melonpult, Gatlingpea, Twinsunflower, Gloomshroom, Cattail, Wintermelon, GoldMagnet, Spikerock, Cobcannon, Imitater, ExplodeONut, GiantWallnut, Sprout, Leftpeater]
};

/// `LawnApp::GetAwardSeedForLevel`: how many plants the player has by
/// adventure level `level` (also the next plant to award).
pub fn award_seed_for_level(level: i32) -> i32 {
    let area = (level - 1) / 10 + 1;
    let sub = (level - 1) % 10 + 1;
    let mut seeds = (area - 1) * 8 + sub;
    // Levels 5 and 10 of each area award something other than a plant.
    if sub >= 10 {
        seeds -= 2;
    } else if sub >= 5 {
        seeds -= 1;
    }
    seeds.min(40)
}

/// `LawnApp::GetSeedsAvailable`.
pub fn seeds_available(player: &PlayerInfo) -> i32 {
    if player.has_finished_adventure() || player.level > 50 {
        return NUM_AWARDABLE_SEEDS;
    }
    award_seed_for_level(player.level).min(NUM_AWARDABLE_SEEDS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_lines_up_with_seed_types() {
        assert_eq!(SeedType::Peashooter.definition().cost, 100);
        assert_eq!(SeedType::Leftpeater.definition().reanim, ReanimationType::Repeater);
        assert_eq!(SeedType::from_index(48), Some(SeedType::Imitater));
        assert!(SeedType::Cattail.is_upgrade());
    }

    #[test]
    fn awards_follow_the_adventure() {
        // 1-1 starts with only the peashooter; 1-5 and 1-10 give no new plant.
        assert_eq!(award_seed_for_level(1), 1);
        assert_eq!(award_seed_for_level(5), 4);
        assert_eq!(award_seed_for_level(10), 8);
        assert_eq!(award_seed_for_level(11), 9);
        assert_eq!(award_seed_for_level(50), 40);
    }
}
