//! The roof: flower pots, the slope, and the lobbing plants.

use crate::plant::SeedType;
use crate::zombie::ZombieType;

use super::super::cursor::PlantingReason;
use super::test_board;

fn plant_of(t: &super::TestBoard, seed: SeedType, col: i32, row: i32) -> sexy::data_array::Id<super::super::plant::Plant> {
    t.board.plants.iter().find(|(_, p)| p.col == col && p.row == row && p.seed == seed && !p.dead).map(|(id, _)| id).unwrap()
}

#[test]
fn nothing_goes_on_the_roof_without_a_flower_pot() {
    let mut t = test_board!("5-1");
    assert!(t.board.stage_has_roof());
    assert_eq!(t.board.can_plant_at(2, 2, SeedType::Peashooter), PlantingReason::NeedsPot);
    assert_eq!(t.board.can_plant_at(2, 2, SeedType::Flowerpot), PlantingReason::Ok);
    t.plant(SeedType::Flowerpot, 2, 2);
    assert_eq!(t.board.can_plant_at(2, 2, SeedType::Flowerpot), PlantingReason::NotHere, "one pot a square");
    assert_eq!(t.board.can_plant_at(2, 2, SeedType::Peashooter), PlantingReason::Ok, "a pot takes a plant");
    assert_eq!(t.board.can_plant_at(2, 2, SeedType::Spikeweed), PlantingReason::NotHere, "but never spikeweed");
    t.plant(SeedType::Peashooter, 2, 2);
    assert_eq!(t.board.can_plant_at(2, 2, SeedType::Wallnut), PlantingReason::NotHere, "and one plant");
    assert_eq!(t.board.can_plant_at(2, 2, SeedType::Flowerpot), PlantingReason::NotHere, "the pot has a plant in it");
}

#[test]
fn the_roof_has_no_lawn_mowers() {
    let t = test_board!("5-1");
    assert!(t.board.lawn_mowers.is_empty());
}

#[test]
fn zombies_eat_the_plant_then_the_pot() {
    let mut t = test_board!("5-1");
    t.plant(SeedType::Flowerpot, 3, 2);
    t.plant(SeedType::Sunflower, 3, 2);
    let nut = plant_of(&t, SeedType::Sunflower, 3, 2);
    let pot = plant_of(&t, SeedType::Flowerpot, 3, 2);
    t.zombie(ZombieType::Normal, 2, 330.0);
    assert!(t.run_until(3000, |b| b.plants.get(nut).is_none_or(|p| p.dead)), "the sunflower is eaten first");
    assert!(t.plant_ref(pot).is_some_and(|p| p.health == 300), "with the pot untouched");
    assert!(t.run_until(2000, |b| b.plants.get(pot).is_none_or(|p| p.dead)), "then the pot");
}

#[test]
fn potted_plants_stand_and_shoot_higher() {
    let mut t = test_board!("5-1");
    t.plant(SeedType::Flowerpot, 0, 2);
    t.plant(SeedType::Peashooter, 0, 2);
    let pea = plant_of(&t, SeedType::Peashooter, 0, 2);
    assert!(t.plant_ref(pea).unwrap().in_pot);
    t.zombie(ZombieType::Normal, 2, 600.0);
    assert!(t.run_until(600, |b| !b.projectiles.is_empty()));
}

#[test]
fn a_cabbage_pult_lobs_cabbages_at_zombies() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Cabbagepult, 0, 2);
    let far = t.zombie(ZombieType::Normal, 2, 700.0);
    let other = t.zombie(ZombieType::Normal, 0, 700.0);
    assert!(t.run_until(900, |b| b.zombies.get(far).is_none_or(|z| z.body_health < 270)), "the cabbage lands on it");
    assert_eq!(t.zombie_ref(other).unwrap().body_health, 270, "but not on another row");
    assert!(t.zombie_ref(far).is_none_or(|z| z.body_health == 230), "for 40 damage");
}

#[test]
fn a_cabbage_goes_over_the_plant_and_zombie_in_front() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Cabbagepult, 0, 2);
    let near = t.zombie(ZombieType::Normal, 2, 450.0);
    let far = t.zombie(ZombieType::Normal, 2, 700.0);
    // It throws at the nearest one: the lob doesn't stop for anything it passes over.
    assert!(t.run_until(900, |b| b.zombies.get(near).is_none_or(|z| z.body_health < 270)));
    assert_eq!(t.zombie_ref(far).unwrap().body_health, 270);
}

#[test]
fn a_melon_splashes_the_rows_beside_it() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Melonpult, 0, 2);
    let hit = t.zombie(ZombieType::Normal, 2, 600.0);
    let above = t.zombie(ZombieType::Normal, 1, 600.0);
    let below = t.zombie(ZombieType::Normal, 3, 600.0);
    let far = t.zombie(ZombieType::Normal, 0, 600.0);
    assert!(t.run_until(900, |b| b.zombies.get(hit).is_none_or(|z| z.body_health < 270)));
    t.ticks(5);
    assert!(t.zombie_ref(hit).is_none_or(|z| z.is_dead_or_dying() || z.body_health <= 190), "80 on the target");
    assert!(t.zombie_ref(above).is_some_and(|z| z.body_health < 270), "a third on the row above");
    assert!(t.zombie_ref(below).is_some_and(|z| z.body_health < 270), "and the row below");
    assert_eq!(t.zombie_ref(far).unwrap().body_health, 270, "two rows away is safe");
}

#[test]
fn a_kernel_pult_sometimes_throws_butter_that_stops_a_zombie() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Kernelpult, 0, 2);
    let zombie = t.zombie(ZombieType::Pail, 2, 700.0);
    assert!(t.run_until(12_000, |b| b.zombies.get(zombie).is_none_or(|z| z.is_buttered())), "a pat of butter lands");
    let x = t.zombie_ref(zombie).unwrap().pos_x;
    t.ticks(200);
    assert_eq!(t.zombie_ref(zombie).unwrap().pos_x, x, "and it stands stuck");
    assert!(t.run_until(3000, |b| b.zombies.get(zombie).is_none_or(|z| z.pos_x < x)), "until it wears off");
}

#[test]
fn lobbers_work_on_the_sloped_roof() {
    let mut t = test_board!("5-1");
    t.plant(SeedType::Flowerpot, 1, 2);
    t.plant(SeedType::Cabbagepult, 1, 2);
    let near = t.zombie(ZombieType::Normal, 2, 560.0);
    assert!(t.run_until(900, |b| b.zombies.get(near).is_none_or(|z| z.body_health < 270)), "the cabbage lands on the roof too");
    // And on the low left edge of it.
    drop(t);
    let mut t = test_board!("5-1");
    t.plant(SeedType::Flowerpot, 0, 3);
    t.plant(SeedType::Melonpult, 0, 3);
    let far = t.zombie(ZombieType::Normal, 3, 300.0);
    assert!(t.run_until(900, |b| b.zombies.get(far).is_none_or(|z| z.body_health < 270)));
}

#[test]
fn lobbed_shots_reach_a_snorkel_under_the_water() {
    use crate::board::zombie::ZombiePhase::SnorkelWalkingInPool;
    let mut t = test_board!("3-1");
    t.plant(SeedType::Lilypad, 0, 2);
    t.plant(SeedType::Cabbagepult, 0, 2);
    t.plant(SeedType::Lilypad, 0, 3);
    t.plant(SeedType::Peashooter, 0, 3);
    let swimmer = t.zombie(ZombieType::Snorkel, 2, 760.0);
    let swimmer2 = t.zombie(ZombieType::Snorkel, 3, 760.0);
    assert!(t.run_until(900, |b| b.zombies.get(swimmer).is_some_and(|z| z.phase == SnorkelWalkingInPool)));
    assert!(t.run_until(2500, |b| b.zombies.get(swimmer).is_none_or(|z| z.body_health < 270)), "a cabbage comes down on it");
    t.ticks(1000);
    if t.zombie_ref(swimmer2).is_some_and(|z| z.phase == SnorkelWalkingInPool) {
        assert_eq!(t.zombie_ref(swimmer2).unwrap().body_health, 270, "peas still can't");
    }
}

#[test]
fn the_roof_levels_run_their_waves_without_incident() {
    for stage in ["5-1", "5-3", "5-6", "5-9"] {
        let mut t = test_board!(stage).with_waves();
        t.give_sun(9000);
        for row in 0..5 {
            t.plant(SeedType::Flowerpot, 0, row);
            t.plant(SeedType::Cabbagepult, 0, row);
        }
        let mut kinds = std::collections::HashSet::new();
        for _ in 0..25_000 {
            t.tick();
            for (_, z) in t.board.zombies.iter() {
                kinds.insert(z.zombie_type);
            }
        }
        assert!(!kinds.is_empty(), "{stage}: zombies came");
        drop(t);
    }
}
