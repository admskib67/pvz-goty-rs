//! Plants: planting, digging up, and what each one does.

use crate::plant::SeedType;
use crate::zombie::ZombieType;

use super::super::coin::CoinType;
use super::super::cursor::PlantingReason;
use super::super::plant::PlantState;
use super::test_board;

/// The plants that stand and do something (not the instant ones).
const STANDING: [SeedType; 13] = [
    SeedType::Peashooter,
    SeedType::Sunflower,
    SeedType::Wallnut,
    SeedType::Potatomine,
    SeedType::Snowpea,
    SeedType::Chomper,
    SeedType::Repeater,
    SeedType::Puffshroom,
    SeedType::Sunshroom,
    SeedType::Fumeshroom,
    SeedType::Hypnoshroom,
    SeedType::Scaredyshroom,
    SeedType::Doomshroom,
];

#[test]
fn every_plant_can_be_planted_and_dug_up() {
    let mut t = test_board!("2-1");
    for (i, &seed) in STANDING.iter().enumerate() {
        // The doom-shroom goes off at night before it can be dug up.
        if seed == SeedType::Doomshroom {
            continue;
        }
        let (col, row) = (i as i32 % 3, i as i32 / 3);
        let id = t.plant(seed, col, row);
        t.tick();
        assert!(t.plant_ref(id).is_some(), "{seed:?} should be on the lawn");
        t.dig(col, row);
        t.tick();
        assert!(t.plant_at(col, row).is_none(), "{seed:?} should be dug up");
        assert!(t.board.plants.get(id).is_none(), "{seed:?} should be gone from the board");
    }
}

#[test]
fn planting_from_the_bank_spends_sun_and_recharges_the_packet() {
    let mut t = test_board!("1-6");
    t.set_bank(&[SeedType::Peashooter]);
    t.give_sun(200);
    t.plant_from_bank(0, 2, 2);
    assert!(t.plant_at(2, 2).is_some(), "the peashooter should be planted");
    assert_eq!(t.board.sun_money, 100);
    assert!(!t.board.seed_bank.packets[0].active, "the packet should be recharging");

    // Recharging: picking it again does nothing.
    t.plant_from_bank(0, 3, 2);
    assert!(t.plant_at(3, 2).is_none());
    assert_eq!(t.board.sun_money, 100);

    // It recharges in 7.5 seconds.
    assert!(t.run_until(800, |b| b.seed_bank.packets[0].active), "the packet should recharge");
    t.plant_from_bank(0, 3, 2);
    assert!(t.plant_at(3, 2).is_some());
}

#[test]
fn planting_needs_enough_sun() {
    let mut t = test_board!("1-6");
    t.set_bank(&[SeedType::Peashooter]);
    t.give_sun(50);
    t.plant_from_bank(0, 2, 2);
    assert!(t.plant_at(2, 2).is_none());
    assert_eq!(t.board.sun_money, 50);
}

#[test]
fn plants_only_go_where_they_fit() {
    let mut t = test_board!("2-1");
    t.plant(SeedType::Peashooter, 0, 0);
    assert_eq!(t.board.can_plant_at(0, 0, SeedType::Wallnut), PlantingReason::NotHere, "the square is taken");
    assert_eq!(t.board.can_plant_at(1, 0, SeedType::Wallnut), PlantingReason::Ok);

    let grave = t.board.grid_items.iter().map(|(_, g)| (g.grid_x, g.grid_y)).next().expect("2-1 has graves");
    assert_eq!(t.board.can_plant_at(grave.0, grave.1, SeedType::Peashooter), PlantingReason::NotOnGrave);
    assert_eq!(t.board.can_plant_at(grave.0, grave.1, SeedType::Gravebuster), PlantingReason::Ok);
    assert_eq!(t.board.can_plant_at(1, 0, SeedType::Gravebuster), PlantingReason::OnlyOnGraves);

    t.board.add_a_crater(2, 0, 100);
    assert_eq!(t.board.can_plant_at(2, 0, SeedType::Peashooter), PlantingReason::NotOnCrater);
    t.ticks(101);
    assert_eq!(t.board.can_plant_at(2, 0, SeedType::Peashooter), PlantingReason::Ok, "the crater fills in");
}

#[test]
fn the_first_levels_have_unsodded_rows() {
    let t = test_board!("1-1");
    assert_eq!(t.board.can_plant_at(0, 0, SeedType::Peashooter), PlantingReason::NotHere);
    assert_eq!(t.board.can_plant_at(0, 2, SeedType::Peashooter), PlantingReason::Ok);
}

#[test]
fn mushrooms_sleep_in_the_day_only() {
    let mut day = test_board!("1-6");
    let id = day.plant(SeedType::Puffshroom, 2, 2);
    assert!(day.plant_ref(id).unwrap().is_asleep());
    let zombie = day.zombie(ZombieType::Normal, 2, 450.0);
    day.ticks(300);
    assert_eq!(day.zombie_ref(zombie).unwrap().body_health, 270, "a sleeping mushroom doesn't shoot");
    drop(day);

    let mut night = test_board!("2-1");
    let id = night.plant(SeedType::Puffshroom, 2, 2);
    assert!(!night.plant_ref(id).unwrap().is_asleep());
}

#[test]
fn peashooter_hits_zombies_in_its_row_only() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Peashooter, 0, 2);
    let target = t.zombie(ZombieType::Normal, 2, 600.0);
    let other = t.zombie(ZombieType::Normal, 0, 600.0);
    t.ticks(600);
    let hurt = t.zombie_ref(target).map_or(0, |z| z.body_health);
    assert!(hurt < 270, "the zombie in its row should be hit");
    assert_eq!(t.zombie_ref(other).unwrap().body_health, 270, "the zombie in another row isn't");
}

#[test]
fn snow_pea_chills() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Snowpea, 0, 2);
    let zombie = t.zombie(ZombieType::Normal, 2, 500.0);
    assert!(t.run_until(500, |b| b.zombies.get(zombie).is_some_and(|z| z.is_chilled())), "the zombie should be chilled");
}

#[test]
fn repeater_fires_two_peas_at_a_time() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Repeater, 0, 2);
    t.zombie(ZombieType::Pail, 2, 700.0);
    assert!(t.run_until(400, |b| !b.projectiles.is_empty()), "it should shoot");
    assert!(t.run_until(40, |b| b.projectiles.len() >= 2), "a second pea follows right after");
}

#[test]
fn sunflower_makes_sun() {
    let mut t = test_board!("2-1");
    t.plant(SeedType::Sunflower, 0, 2);
    assert!(t.run_until(2600, |b| b.coins.iter().any(|(_, c)| c.ty == CoinType::Sun)), "a sunflower makes sun every ~25 s");
}

#[test]
fn sun_shroom_makes_small_sun_then_grows() {
    let mut t = test_board!("2-1");
    let id = t.plant(SeedType::Sunshroom, 0, 2);
    assert!(t.run_until(2600, |b| b.coins.iter().any(|(_, c)| c.ty == CoinType::SmallSun)), "small sun first");
    assert!(
        t.run_until(12_500, |b| b.plants.get(id).is_some_and(|p| p.state() == PlantState::SunshroomBig)),
        "it grows up after two minutes"
    );
}

#[test]
fn wall_nut_holds_a_zombie_back() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Wallnut, 3, 2);
    let zombie = t.zombie(ZombieType::Normal, 2, 330.0);
    assert!(t.run_until(600, |b| b.zombies.get(zombie).is_some_and(|z| z.is_eating)), "the zombie should start eating");
    let x = t.zombie_ref(zombie).unwrap().x;
    t.ticks(1500);
    assert!(t.plant_ref(nut).is_some_and(|p| p.health < 4000 && p.health > 0), "the nut is bitten but holds");
    assert!((t.zombie_ref(zombie).unwrap().x - x).abs() <= 2, "the zombie stays put while eating");
}

#[test]
fn cherry_bomb_destroys_zombies_around_it() {
    let mut t = test_board!("1-6");
    let near: Vec<_> = (1..=3).map(|row| t.zombie(ZombieType::Normal, row, 370.0)).collect();
    let far = t.zombie(ZombieType::Normal, 2, 700.0);
    let away = t.zombie(ZombieType::Normal, 0, 370.0);
    t.plant(SeedType::Cherrybomb, 4, 2);
    t.ticks(150);
    for id in near {
        assert!(t.zombie_ref(id).is_none_or(|z| z.is_dead_or_dying()), "zombies next to it are burned");
    }
    assert!(t.zombie_ref(far).is_some_and(|z| !z.is_dead_or_dying()), "far away is safe");
    assert!(t.zombie_ref(away).is_some_and(|z| !z.is_dead_or_dying()), "two rows away is safe");
    assert!(t.plant_at(4, 2).is_none(), "the cherry bomb is used up");
}

#[test]
fn potato_mine_arms_then_blows_up() {
    let mut t = test_board!("1-6");
    let mine = t.plant(SeedType::Potatomine, 3, 2);
    t.ticks(100);
    assert_eq!(t.plant_ref(mine).unwrap().state(), PlantState::NotReady, "it takes a while to arm");
    assert!(t.run_until(1700, |b| b.plants.get(mine).is_some_and(|p| p.state() == PlantState::PotatoArmed)), "armed after ~15 s");
    let zombie = t.zombie(ZombieType::Normal, 2, 330.0);
    assert!(t.run_until(600, |b| b.zombies.get(zombie).is_none_or(|z| z.is_dead_or_dying())), "the zombie that steps on it dies");
    assert!(t.plant_at(3, 2).is_none(), "the mine is used up");
}

#[test]
fn chomper_eats_a_zombie_then_chews() {
    let mut t = test_board!("1-6");
    let chomper = t.plant(SeedType::Chomper, 3, 2);
    let zombie = t.zombie(ZombieType::Normal, 2, 380.0);
    assert!(t.run_until(800, |b| b.zombies.get(zombie).is_none_or(|z| z.dead)), "the zombie is swallowed");
    assert!(
        t.run_until(200, |b| b.plants.get(chomper).is_some_and(|p| p.state() == PlantState::ChomperDigesting)),
        "then it chews for a while"
    );
}

#[test]
fn puff_shroom_only_reaches_so_far() {
    let mut t = test_board!("2-1");
    t.plant(SeedType::Puffshroom, 0, 2);
    let far = t.zombie(ZombieType::Normal, 2, 700.0);
    t.ticks(300);
    assert_eq!(t.zombie_ref(far).unwrap().body_health, 270, "out of reach");
    let near = t.zombie(ZombieType::Normal, 2, 200.0);
    assert!(t.run_until(400, |b| b.zombies.get(near).is_none_or(|z| z.body_health < 270)), "close by, it's hit");
}

#[test]
fn scaredy_shroom_hides_from_close_zombies() {
    let mut t = test_board!("2-1");
    let scaredy = t.plant(SeedType::Scaredyshroom, 3, 2);
    let zombie = t.zombie(ZombieType::Normal, 2, 700.0);
    assert!(t.run_until(400, |b| !b.projectiles.is_empty()), "it shoots from far away");
    if let Some(z) = t.board.zombies.get_mut(zombie) {
        z.place_x(330.0);
    }
    assert!(
        t.run_until(200, |b| b.plants.get(scaredy).is_some_and(|p| p.state() == PlantState::ScaredyshroomScared)),
        "it ducks when a zombie comes close"
    );
    let shots = t.board.projectiles.len();
    t.ticks(300);
    assert!(t.board.projectiles.len() <= shots, "and stops shooting");
}

#[test]
fn fume_shroom_hits_through_a_screen_door() {
    let mut t = test_board!("2-1");
    t.plant(SeedType::Fumeshroom, 2, 2);
    let zombie = t.zombie(ZombieType::Door, 2, 400.0);
    assert!(
        t.run_until(400, |b| b.zombies.get(zombie).is_some_and(|z| z.body_health < 270)),
        "the fumes hurt the zombie behind its door"
    );
    assert!(t.zombie_ref(zombie).unwrap().shield_health() < 1100, "and the door");
}

#[test]
fn ice_shroom_freezes_every_zombie() {
    let mut t = test_board!("2-1");
    let zombies: Vec<_> = (0..5).map(|row| t.zombie(ZombieType::Normal, row, 600.0)).collect();
    t.plant(SeedType::Iceshroom, 0, 0);
    t.ticks(110);
    let xs: Vec<_> = zombies.iter().map(|&id| t.zombie_ref(id).unwrap().pos_x).collect();
    for &id in &zombies {
        assert!(t.zombie_ref(id).unwrap().is_immobilized(), "every zombie is frozen");
    }
    t.ticks(200);
    for (&id, x) in zombies.iter().zip(xs) {
        assert_eq!(t.zombie_ref(id).unwrap().pos_x, x, "frozen zombies don't move");
    }
    assert!(t.plant_at(0, 0).is_none(), "the ice-shroom is used up");
}

#[test]
fn doom_shroom_destroys_widely_and_leaves_a_crater() {
    let mut t = test_board!("2-1");
    let zombies: Vec<_> = (0..5).map(|row| t.zombie(ZombieType::Normal, row, 450.0)).collect();
    t.plant(SeedType::Doomshroom, 5, 2);
    t.ticks(150);
    for id in zombies {
        assert!(t.zombie_ref(id).is_none_or(|z| z.is_dead_or_dying()), "everything nearby is destroyed");
    }
    assert!(t.board.crater_at(5, 2), "it leaves a crater");
    t.ticks(18_000);
    assert!(!t.board.crater_at(5, 2), "the crater fills in after three minutes");
}

#[test]
fn hypno_shroom_turns_the_zombie_that_eats_it() {
    let mut t = test_board!("2-1");
    t.plant(SeedType::Hypnoshroom, 3, 2);
    let zombie = t.zombie(ZombieType::Normal, 2, 330.0);
    assert!(t.run_until(600, |b| b.zombies.get(zombie).is_some_and(|z| z.mind_controlled)), "the zombie is hypnotized");
    assert!(t.plant_at(3, 2).is_none(), "the hypno-shroom is eaten");
    let x = t.zombie_ref(zombie).unwrap().pos_x;
    t.ticks(300);
    assert!(t.zombie_ref(zombie).unwrap().pos_x > x, "it walks back the way it came");

    // It fights the next zombie in its row.
    let enemy = t.zombie(ZombieType::Normal, 2, t.zombie_ref(zombie).unwrap().pos_x + 60.0);
    assert!(t.run_until(300, |b| b.zombies.get(enemy).is_some_and(|z| z.body_health < 270)), "it bites other zombies");
}

#[test]
fn grave_buster_eats_a_grave() {
    let mut t = test_board!("2-1");
    let (col, row) = t.board.grid_items.iter().map(|(_, g)| (g.grid_x, g.grid_y)).next().expect("2-1 has graves");
    t.plant(SeedType::Gravebuster, col, row);
    assert!(t.run_until(800, |b| b.grave_stone_at(col, row).is_none()), "the grave is eaten");
    t.tick();
    assert!(t.plant_at(col, row).is_none(), "and the grave buster with it");
}
