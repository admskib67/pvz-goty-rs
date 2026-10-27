//! Zen Garden loot: potted-plant presents and chocolate dropped by zombies.

use crate::board::coin::CoinType;
use crate::plant::SeedType;

use super::TestBoard;

#[test]
fn a_potted_plant_present_lands_in_the_garden() {
    let Some(mut t) = TestBoard::new("2-1") else { return };
    t.collect_zen_loot(CoinType::PresentPlant, Some(SeedType::Marigold));
    t.ticks(2);
    assert_eq!(t.potted_plant, Some(SeedType::Marigold));
}

#[test]
fn chocolate_is_counted() {
    let Some(mut t) = TestBoard::new("2-1") else { return };
    t.collect_zen_loot(CoinType::Chocolate, None);
    t.ticks(2);
    assert!(t.chocolate);
}
