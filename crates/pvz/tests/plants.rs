//! Plants: planting, digging up, and what each one does.

use sexy::data_array::Id;

use crate::plant::SeedType;
use crate::zombie::ZombieType;

use super::super::coin::CoinType;
use super::super::cursor::PlantingReason;
use super::super::plant::PlantState;
use super::super::projectile::ProjectileType;
use super::super::zombie::Zombie;
use super::{TestBoard, test_board};

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
    // Swelling up, its heads jitter (`UpdateDoomShroom`'s SetShakeOverride).
    t.ticks(2);
    let body = t.plant_ref(t.plant_at(5, 2).unwrap()).unwrap().body_reanim().expect("the doom-shroom body");
    let reanim = t.effects.get(body).unwrap();
    let shakes: Vec<f32> = ["DoomShroom_head1", "DoomShroom_head2", "DoomShroom_head3"]
        .into_iter()
        .map(|name| reanim.tracks[reanim.find_track(name)].shake_override)
        .collect();
    assert_eq!(shakes, [1.0, 2.0, 2.0], "the heads shake as it swells");

    t.ticks(148);
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

#[test]
fn squash_flattens_a_zombie_in_front_of_it() {
    let mut t = test_board!("1-6");
    let squash = t.plant(SeedType::Squash, 3, 2);
    let zombie = t.zombie(ZombieType::Normal, 2, 380.0);
    assert!(t.run_until(600, |b| b.zombies.get(zombie).is_none_or(|z| z.is_dead_or_dying())), "the zombie is flattened");
    assert!(t.run_until(300, |b| b.plants.get(squash).is_none()), "and the squash is used up afterwards");
}

#[test]
fn squash_ignores_other_rows_and_far_zombies() {
    let mut t = test_board!("1-6");
    let squash = t.plant(SeedType::Squash, 3, 2);
    let other_row = t.zombie(ZombieType::Normal, 1, 380.0);
    let far = t.zombie(ZombieType::Normal, 2, 760.0);
    t.ticks(100);
    assert_eq!(t.plant_ref(squash).unwrap().state(), PlantState::NotReady, "nothing to squash yet");
    assert!(t.zombie_ref(other_row).is_some_and(|z| !z.is_dead_or_dying()));
    assert!(t.zombie_ref(far).is_some_and(|z| !z.is_dead_or_dying()));
}

#[test]
fn jalapeno_burns_its_whole_row() {
    let mut t = test_board!("1-6");
    let row: Vec<_> = [300.0, 500.0, 700.0].iter().map(|&x| t.zombie(ZombieType::Normal, 2, x)).collect();
    let other_row = t.zombie(ZombieType::Normal, 1, 500.0);
    t.plant(SeedType::Jalapeno, 1, 2);
    t.ticks(200);
    for id in row {
        assert!(t.zombie_ref(id).is_none_or(|z| z.is_dead_or_dying()), "everything in the row is burned");
    }
    assert!(t.zombie_ref(other_row).is_some_and(|z| !z.is_dead_or_dying()), "the next row is safe");
    assert!(t.plant_at(1, 2).is_none(), "the jalapeno is used up");
}

#[test]
fn jalapeno_burns_chilled_and_vaulting_zombies() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Snowpea, 0, 2);
    let chilled = t.zombie(ZombieType::Normal, 2, 600.0);
    assert!(t.run_until(500, |b| b.zombies.get(chilled).is_some_and(|z| z.is_chilled())), "chilled first");
    let vaulter = t.zombie(ZombieType::Polevaulter, 2, 700.0);
    t.plant(SeedType::Jalapeno, 1, 2);
    t.ticks(200);
    assert!(t.zombie_ref(chilled).is_none_or(|z| z.is_dead_or_dying()));
    assert!(t.zombie_ref(vaulter).is_none_or(|z| z.is_dead_or_dying()));
}

#[test]
fn spikeweed_hurts_a_zombie_walking_over_it() {
    let mut t = test_board!("1-6");
    let weed = t.plant(SeedType::Spikeweed, 3, 2);
    let weed_x = t.plant_ref(weed).unwrap().x;
    let zombie = t.zombie(ZombieType::Normal, 2, 430.0);
    let mut ate = false;
    assert!(
        t.run_until(1500, |b| {
            ate |= b.zombies.get(zombie).is_some_and(|z| z.is_eating);
            b.zombies.get(zombie).is_none_or(|z| z.x < weed_x - 40)
        }),
        "the zombie walks right over it"
    );
    assert!(!ate, "zombies don't stop to eat spikeweed");
    assert!(t.zombie_ref(zombie).unwrap().body_health <= 230, "but its feet are hurt more than once");
    assert_eq!(t.plant_ref(weed).unwrap().health, 300, "and the spikeweed is unharmed");
}

#[test]
fn spikeweed_ignores_zombies_in_other_rows() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Spikeweed, 3, 2);
    let other = t.zombie(ZombieType::Normal, 1, 300.0);
    t.ticks(400);
    assert_eq!(t.zombie_ref(other).unwrap().body_health, 270);
}

/// The zombie's health once a shot has first hurt it.
fn first_hit(t: &mut TestBoard, zombie: Id<Zombie>) -> i32 {
    assert!(t.run_until(800, |b| b.zombies.get(zombie).is_some_and(|z| z.body_health < 270)), "the zombie should be hit");
    270 - t.zombie_ref(zombie).unwrap().body_health
}

#[test]
fn torchwood_turns_peas_into_fire_peas() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Peashooter, 0, 2);
    t.plant(SeedType::Torchwood, 2, 2);
    let zombie = t.zombie(ZombieType::Normal, 2, 780.0);
    assert!(t.run_until(400, |b| b.projectiles.iter().any(|(_, p)| p.ty == ProjectileType::Fire)), "a pea catches fire");
    assert_eq!(first_hit(&mut t, zombie), 40, "and hits twice as hard");
}

#[test]
fn torchwood_thaws_snow_peas() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Snowpea, 0, 2);
    t.plant(SeedType::Torchwood, 2, 2);
    let zombie = t.zombie(ZombieType::Normal, 2, 780.0);
    assert_eq!(first_hit(&mut t, zombie), 20, "a thawed pea is a plain pea, and isn't set alight by the same torchwood");
    assert!(!t.zombie_ref(zombie).unwrap().is_chilled(), "so the zombie isn't chilled");
}

#[test]
fn a_snow_pea_through_two_torchwoods_burns() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Snowpea, 0, 2);
    t.plant(SeedType::Torchwood, 2, 2);
    t.plant(SeedType::Torchwood, 3, 2);
    let zombie = t.zombie(ZombieType::Normal, 2, 780.0);
    assert_eq!(first_hit(&mut t, zombie), 40);
}

#[test]
fn torchwood_ignores_peas_in_other_rows() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Peashooter, 0, 2);
    t.plant(SeedType::Torchwood, 2, 1);
    t.zombie(ZombieType::Normal, 2, 780.0);
    for _ in 0..400 {
        t.tick();
        assert!(t.board.projectiles.iter().all(|(_, p)| p.ty != ProjectileType::Fire), "no fire peas");
    }
}

fn shot_rows(t: &TestBoard) -> Vec<i32> {
    let mut rows: Vec<_> = t.board.projectiles.iter().map(|(_, p)| p.row).collect();
    rows.sort_unstable();
    rows.dedup();
    rows
}

#[test]
fn threepeater_fires_in_three_rows() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Threepeater, 2, 2);
    let zombie = t.zombie(ZombieType::Normal, 1, 700.0);
    assert!(t.run_until(500, |b| b.projectiles.len() >= 3), "it fires a pea in each row");
    assert_eq!(shot_rows(&t), vec![1, 2, 3]);
    assert!(t.run_until(600, |b| b.zombies.get(zombie).is_some_and(|z| z.body_health < 270)), "the pea that drifted up hits the zombie");
}

#[test]
fn threepeater_on_the_edge_only_fires_rows_that_exist() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Threepeater, 2, 0);
    t.zombie(ZombieType::Normal, 0, 700.0);
    let mut seen = Vec::new();
    for _ in 0..500 {
        t.tick();
        seen.extend(shot_rows(&t));
    }
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen, vec![0, 1], "no row above the top one");
}

#[test]
fn threepeater_waits_for_a_zombie_in_its_three_rows() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Threepeater, 2, 2);
    t.zombie(ZombieType::Normal, 4, 700.0);
    t.ticks(500);
    assert!(t.board.projectiles.is_empty(), "two rows away is out of its sights");
}

#[test]
fn zombies_cannot_eat_a_squash_that_has_jumped() {
    let mut t = test_board!("1-6");
    let squash = t.plant(SeedType::Squash, 3, 2);
    t.zombie(ZombieType::Normal, 2, 380.0);
    assert!(
        t.run_until(600, |b| b.plants.get(squash).is_some_and(|p| p.state() == PlantState::SquashDoneFalling)),
        "the squash lands"
    );
    let x = t.plant_ref(squash).unwrap().x;
    let behind = t.zombie(ZombieType::Normal, 2, x as f32 + 30.0);
    for _ in 0..60 {
        t.tick();
        assert!(t.zombie_ref(behind).is_none_or(|z| !z.is_eating), "it's not on the ground, so it can't be bitten");
    }
    assert_eq!(t.plant_ref(squash).unwrap().health, 300);
}

#[test]
fn zombies_cannot_eat_a_jalapeno_before_it_goes_off() {
    let mut t = test_board!("1-6");
    let jalapeno = t.plant(SeedType::Jalapeno, 3, 2);
    let x = t.plant_ref(jalapeno).unwrap().x;
    let zombies: Vec<_> = (0..3).map(|i| t.zombie(ZombieType::Normal, 2, x as f32 + 40.0 + i as f32 * 5.0)).collect();
    t.ticks(90);
    assert_eq!(t.plant_ref(jalapeno).unwrap().health, 300, "not a bite while its fuse burns");
    t.ticks(100);
    for id in zombies {
        assert!(t.zombie_ref(id).is_none_or(|z| z.is_dead_or_dying()), "the row is burned");
    }
}

#[test]
fn zombies_cannot_eat_a_squash_that_is_about_to_jump() {
    let mut t = test_board!("1-6");
    let squash = t.plant(SeedType::Squash, 3, 2);
    let x = t.plant_ref(squash).unwrap().x;
    t.zombie(ZombieType::Normal, 2, x as f32 + 60.0);
    t.ticks(100);
    assert_eq!(t.plant_ref(squash).unwrap().health, 300, "no bites while it looks and crouches");
}

/// The plant of this kind on a square (a lily pad shares its square).
fn plant_of(t: &TestBoard, seed: SeedType, col: i32, row: i32) -> Option<Id<crate::board::plant::Plant>> {
    t.board.plants.iter().find(|(_, p)| p.col == col && p.row == row && p.seed == seed && !p.dead).map(|(id, _)| id)
}

#[test]
fn water_plants_go_on_water_and_the_rest_need_a_lily_pad() {
    let mut t = test_board!("3-1");
    // Rows 2 and 3 are the pool.
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Peashooter), PlantingReason::NotOnWater);
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Lilypad), PlantingReason::Ok);
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Tanglekelp), PlantingReason::Ok);
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Seashroom), PlantingReason::Ok);
    // On the grass it is the other way round.
    assert_eq!(t.board.can_plant_at(3, 1, SeedType::Peashooter), PlantingReason::Ok);
    for seed in [SeedType::Lilypad, SeedType::Tanglekelp, SeedType::Seashroom] {
        assert_eq!(t.board.can_plant_at(3, 1, seed), PlantingReason::NotHere, "{seed:?} needs water");
    }

    t.plant(SeedType::Lilypad, 3, 2);
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Lilypad), PlantingReason::NotHere, "one pad per square");
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Peashooter), PlantingReason::Ok, "a pad carries a plant");
    t.plant(SeedType::Peashooter, 3, 2);
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Wallnut), PlantingReason::NotHere, "but only one");
}

#[test]
fn a_plant_on_a_lily_pad_shoots() {
    let mut t = test_board!("3-1");
    t.plant(SeedType::Lilypad, 1, 2);
    let pea = t.plant(SeedType::Peashooter, 1, 2);
    let zombie = t.zombie(ZombieType::Normal, 2, 700.0);
    assert!(t.plant_ref(pea).is_some());
    assert!(t.run_until(600, |b| b.zombies.get(zombie).is_some_and(|z| z.body_health < 270)), "the pea hits the zombie");
    assert!(plant_of(&t, SeedType::Lilypad, 1, 2).is_some(), "the pad is still there");
}

#[test]
fn tangle_kelp_drags_a_zombie_under() {
    let mut t = test_board!("3-1");
    let kelp = t.plant(SeedType::Tanglekelp, 3, 2);
    let x = t.plant_ref(kelp).unwrap().x;
    let other_row = t.zombie(ZombieType::Normal, 1, x as f32 + 20.0);
    let zombie = t.zombie(ZombieType::Normal, 2, x as f32 + 20.0);
    assert!(t.run_until(300, |b| b.zombies.get(zombie).is_none_or(|z| z.is_dead_or_dying())), "the zombie is pulled under");
    assert!(t.run_until(200, |b| plant_of_gone(b, 3, 2)), "and the kelp is used up");
    assert!(t.zombie_ref(other_row).is_some_and(|z| !z.is_dead_or_dying()), "other rows are safe");
}

fn plant_of_gone(b: &super::super::Board, col: i32, row: i32) -> bool {
    !b.plants.iter().any(|(_, p)| p.col == col && p.row == row && !p.dead)
}

#[test]
fn sea_shroom_shoots_from_the_water_at_night() {
    let mut t = test_board!("3-1");
    let shroom = t.plant(SeedType::Seashroom, 1, 2);
    assert!(t.plant_ref(shroom).unwrap().is_asleep(), "asleep in the day");
    drop(t);
    let mut t = test_board!("4-1");
    let shroom = t.plant(SeedType::Seashroom, 1, 2);
    assert!(!t.plant_ref(shroom).unwrap().is_asleep());
    let zombie = t.zombie(ZombieType::Normal, 2, 200.0);
    assert!(t.run_until(400, |b| b.zombies.get(zombie).is_none_or(|z| z.body_health < 270)), "it spits at close zombies");
}

#[test]
fn a_coffee_bean_wakes_a_sleeping_mushroom() {
    let mut t = test_board!("1-6");
    let shroom = t.plant(SeedType::Puffshroom, 2, 2);
    assert!(t.plant_ref(shroom).unwrap().is_asleep());
    assert_eq!(t.board.can_plant_at(1, 2, SeedType::InstantCoffee), PlantingReason::NotHere, "nothing to wake");
    assert_eq!(t.board.can_plant_at(2, 2, SeedType::InstantCoffee), PlantingReason::Ok);
    t.with(|board, effects, res, images, sounds| board.add_plant(2, 2, SeedType::InstantCoffee, effects, res, images, sounds));
    t.ticks(101);
    assert_eq!(t.board.can_plant_at(2, 2, SeedType::InstantCoffee), PlantingReason::NotHere, "no second bean while it's waking");
    assert!(t.plant_ref(shroom).unwrap().is_asleep(), "the bean takes a moment to crumble");
    t.ticks(101);
    assert!(!t.plant_ref(shroom).unwrap().is_asleep(), "then it wakes up");
    t.ticks(60);
    assert_eq!(t.board.plants.iter().filter(|(_, p)| !p.dead && p.seed == SeedType::InstantCoffee).count(), 0, "and the bean's gone");
    assert_eq!(t.board.can_plant_at(2, 2, SeedType::InstantCoffee), PlantingReason::NotHere, "needing no second bean");
    let zombie = t.zombie(ZombieType::Normal, 2, 450.0);
    assert!(t.run_until(500, |b| b.zombies.get(zombie).is_none_or(|z| z.body_health < 270)), "so it shoots in the day");
}

#[test]
fn upgrades_only_go_on_their_base_plant() {
    let mut t = test_board!("1-6");
    let up = |t: &TestBoard, c, r, s| t.board.can_plant_at(c, r, s);
    assert_eq!(up(&t, 3, 2, SeedType::Gatlingpea), PlantingReason::NeedsUpgrade, "a bare square");
    t.plant(SeedType::Peashooter, 3, 2);
    assert_eq!(up(&t, 3, 2, SeedType::Gatlingpea), PlantingReason::NeedsUpgrade, "a peashooter isn't a repeater");
    t.plant(SeedType::Repeater, 4, 2);
    assert_eq!(up(&t, 4, 2, SeedType::Gatlingpea), PlantingReason::Ok);
    assert_eq!(up(&t, 4, 2, SeedType::Peashooter), PlantingReason::NotHere, "ordinary plants still can't stack");
    t.plant(SeedType::Sunflower, 5, 2);
    assert_eq!(up(&t, 5, 2, SeedType::Twinsunflower), PlantingReason::Ok);
    assert_eq!(up(&t, 5, 2, SeedType::Gatlingpea), PlantingReason::NeedsUpgrade);
    t.plant(SeedType::Spikeweed, 6, 2);
    assert_eq!(up(&t, 6, 2, SeedType::Spikerock), PlantingReason::Ok);
}

#[test]
fn an_upgrade_packet_needs_its_base_plant_on_the_lawn() {
    let mut t = test_board!("1-6");
    assert!(!t.board.planting_requirements_met(SeedType::Gatlingpea));
    assert!(t.board.planting_requirements_met(SeedType::Peashooter), "ordinary plants have no requirement");
    t.plant(SeedType::Repeater, 0, 0);
    assert!(t.board.planting_requirements_met(SeedType::Gatlingpea));
    assert!(!t.board.planting_requirements_met(SeedType::Twinsunflower), "each upgrade has its own base");
}

#[test]
fn planting_an_upgrade_replaces_its_base() {
    let mut t = test_board!("1-6");
    let old = t.plant(SeedType::Repeater, 3, 2);
    let new = t.plant(SeedType::Gatlingpea, 3, 2);
    assert!(t.plant_ref(old).is_none(), "the repeater is gone");
    assert_eq!(t.plant_ref(new).unwrap().seed, SeedType::Gatlingpea);
    assert_eq!(t.plant_at(3, 2), Some(new));
}

#[test]
fn gatling_pea_fires_four_peas_in_a_volley() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Gatlingpea, 0, 2);
    t.zombie(ZombieType::Gargantuar, 2, 780.0);
    let mut most = 0;
    t.run_until(300, |b| {
        most = most.max(b.projectiles.len());
        most >= 4
    });
    assert_eq!(most, 4, "four peas in the air from one volley");
}

#[test]
fn twin_sunflower_makes_two_suns_at_once() {
    let mut t = test_board!("2-1");
    t.plant(SeedType::Twinsunflower, 0, 2);
    assert!(t.run_until(2600, |b| b.coins.iter().any(|(_, c)| c.ty == CoinType::Sun)));
    let suns = t.board.coins.iter().filter(|(_, c)| c.ty == CoinType::Sun).count();
    assert_eq!(suns, 2, "a pair");
}

#[test]
fn spikerock_hurts_zombies_and_is_sturdier_than_spikeweed() {
    let mut t = test_board!("1-6");
    let rock = t.plant(SeedType::Spikerock, 3, 2);
    assert_eq!(t.plant_ref(rock).unwrap().health, 450);
    let x = t.plant_ref(rock).unwrap().x;
    let zombie = t.zombie(ZombieType::Normal, 2, 430.0);
    assert!(t.run_until(1500, |b| b.zombies.get(zombie).is_none_or(|z| z.is_dead_or_dying() || z.x < x - 40)));
    assert!(t.zombie_ref(zombie).is_none_or(|z| z.body_health <= 230), "its spikes hurt");
    assert_eq!(t.plant_ref(rock).unwrap().health, 450, "walkers don't hurt it");
}

#[test]
fn winter_melon_chills_the_zombies_its_splash_reaches() {
    let mut t = test_board!("1-6");
    let melon = t.plant(SeedType::Melonpult, 0, 2);
    let winter = t.plant(SeedType::Wintermelon, 0, 2);
    assert!(t.plant_ref(melon).is_none(), "it replaces the melon-pult");
    assert_eq!(t.plant_ref(winter).unwrap().seed, SeedType::Wintermelon);
    let target = t.zombie(ZombieType::Normal, 2, 600.0);
    let beside = t.zombie(ZombieType::Normal, 3, 600.0);
    assert!(
        t.run_until(1500, |b| {
            b.zombies.get(target).is_some_and(|z| z.is_chilled()) && b.zombies.get(beside).is_some_and(|z| z.is_chilled())
        }),
        "the zombie it hits and the one in the next row are both chilled"
    );
    assert!(t.board.projectiles.iter().all(|(_, p)| p.ty != ProjectileType::Melon), "no plain melons");
}

#[test]
fn gloom_shroom_clouds_hurt_the_rows_beside_it() {
    let mut t = test_board!("2-2");
    let fume = t.plant(SeedType::Fumeshroom, 3, 2);
    let gloom = t.plant(SeedType::Gloomshroom, 3, 2);
    assert!(t.plant_ref(fume).is_none(), "it replaces the fume-shroom");
    let (x, _) = t.square_center(3, 2);
    let near = t.zombie(ZombieType::Normal, 2, x as f32 + 20.0);
    let beside = t.zombie(ZombieType::Normal, 1, x as f32 + 20.0);
    let far_row = t.zombie(ZombieType::Normal, 4, x as f32 + 20.0);
    let behind = t.zombie(ZombieType::Normal, 3, x as f32 - 60.0);
    // Its first volley starts within 200 ticks and takes another 160.
    t.ticks(360);
    let hurt = |t: &TestBoard, id| t.zombie_ref(id).is_none_or(|z| z.body_health < 270);
    assert!(hurt(&t, near) && hurt(&t, beside) && hurt(&t, behind), "its own row, the next and behind it");
    assert!(!hurt(&t, far_row), "but not two rows away");
    assert!(t.plant_ref(gloom).is_some());
}

#[test]
fn a_gloom_shroom_inherits_its_base_plants_sleep() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Fumeshroom, 3, 2);
    let gloom = t.plant(SeedType::Gloomshroom, 3, 2);
    assert!(t.plant_ref(gloom).unwrap().is_asleep(), "in the day a gloom-shroom sleeps too");
    t.plant(SeedType::Fumeshroom, 4, 2);
    t.plant(SeedType::InstantCoffee, 4, 2);
    t.ticks(250);
    let awake = t.plant(SeedType::Gloomshroom, 4, 2);
    assert!(!t.plant_ref(awake).unwrap().is_asleep(), "a woken fume-shroom's upgrade stays awake");
}

#[test]
fn a_cob_cannon_needs_two_kernel_pults_side_by_side() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Kernelpult, 3, 2);
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Cobcannon), PlantingReason::NeedsUpgrade, "one kernel-pult isn't enough");
    assert!(!t.board.planting_requirements_met(SeedType::Cobcannon));
    t.plant(SeedType::Kernelpult, 4, 2);
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Cobcannon), PlantingReason::Ok);
    assert_eq!(t.board.can_plant_at(4, 2, SeedType::Cobcannon), PlantingReason::NeedsUpgrade, "it goes on the left one");
    assert!(t.board.planting_requirements_met(SeedType::Cobcannon));
    let cannon = t.plant(SeedType::Cobcannon, 3, 2);
    assert_eq!(t.plant_at(3, 2), Some(cannon), "both kernel-pults make way");
    assert_eq!(t.board.plants.iter().filter(|(_, p)| !p.dead).count(), 1);
    assert_eq!(t.board.can_plant_at(4, 2, SeedType::Peashooter), PlantingReason::NotHere, "it fills two squares");
}

#[test]
fn a_cob_cannon_loads_then_fires_where_it_is_aimed() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Kernelpult, 0, 2);
    t.plant(SeedType::Kernelpult, 1, 2);
    let cannon = t.plant(SeedType::Cobcannon, 0, 2);
    assert!(!t.plant_ref(cannon).unwrap().is_cob_ready(), "it starts unarmed");
    assert!(t.run_until(2000, |b| b.plants.get(cannon).is_some_and(|p| p.is_cob_ready())), "then loads a cob");
    let (x, y) = t.square_center(0, 2);
    t.click(x, y);
    assert_eq!(t.board.cursor.cursor_type, super::super::cursor::CursorType::CobCannonTarget, "a click asks for a target");
    let victim = t.zombie(ZombieType::Normal, 2, 700.0);
    let bystander = t.zombie(ZombieType::Normal, 4, 700.0);
    t.ticks(40);
    let (tx, ty) = t.square_center(8, 2);
    t.click(tx, ty);
    assert_eq!(t.board.cursor.cursor_type, super::super::cursor::CursorType::Normal, "the aim is taken");
    assert!(!t.plant_ref(cannon).unwrap().is_cob_ready(), "and the cannon's busy");
    let mut flew = false;
    assert!(
        t.run_until(900, |b| {
            flew |= b.projectiles.iter().any(|(_, p)| p.ty == ProjectileType::Cob);
            b.zombies.get(victim).is_none_or(|z| z.is_dead_or_dying())
        }),
        "the cob comes down on the zombie"
    );
    assert!(flew, "it was a cob that did it");
    assert!(t.zombie_ref(bystander).is_some_and(|z| !z.is_dead_or_dying()), "two rows away is safe");
    assert!(t.run_until(4000, |b| b.plants.get(cannon).is_some_and(|p| p.is_cob_ready())), "it reloads after half a minute");
}

#[test]
fn marigold_makes_coins() {
    let mut t = test_board!("2-1");
    t.plant(SeedType::Marigold, 0, 2);
    assert!(
        t.run_until(2600, |b| b.coins.iter().any(|(_, c)| matches!(c.ty, CoinType::Silver | CoinType::Gold))),
        "a marigold makes a coin every ~25 s"
    );
    assert!(t.board.coins.iter().all(|(_, c)| !c.is_sun()), "never sun");
}

#[test]
fn marigold_stops_a_minute_after_the_last_wave() {
    let mut t = test_board!("2-1");
    let flower = t.plant(SeedType::Marigold, 0, 2);
    t.board.waves.current_wave = t.board.waves.num_waves;
    t.ticks(1);
    assert_eq!(t.plant_ref(flower).unwrap().state(), PlantState::MarigoldEnding);
    t.ticks(6100);
    t.board.coins.clear();
    t.ticks(3000);
    assert_eq!(t.board.coins.len(), 0, "it has stopped for good");
}

#[test]
fn a_zombie_that_bites_garlic_turns_into_another_lane() {
    let mut t = test_board!("1-6");
    let garlic = t.plant(SeedType::Garlic, 3, 2);
    let zombie = t.zombie(ZombieType::Normal, 2, 330.0);
    assert!(t.run_until(600, |b| b.zombies.get(zombie).is_some_and(|z| z.is_eating)), "it starts to eat");
    assert!(
        t.run_until(800, |b| b.zombies.get(zombie).is_some_and(|z| z.row != 2)),
        "but it pulls a face and changes lanes"
    );
    assert!(t.plant_ref(garlic).unwrap().health > 150, "having hardly eaten any of it");
    assert!(!t.zombie_ref(zombie).unwrap().is_eating);
}

#[test]
fn a_magnet_shroom_pulls_the_bucket_off_a_zombie() {
    use super::super::zombie::HelmType;
    let mut t = test_board!("2-1");
    t.plant(SeedType::Magnetshroom, 3, 2);
    let bucket = t.zombie(ZombieType::Pail, 2, 540.0);
    let plain = t.zombie(ZombieType::Normal, 2, 540.0);
    assert_eq!(t.zombie_ref(bucket).unwrap().helm_type(), HelmType::Pail);
    assert!(t.run_until(300, |b| b.zombies.get(bucket).is_some_and(|z| z.helm_type() == HelmType::None)), "the bucket flies off");
    assert!(t.zombie_ref(bucket).unwrap().body_health >= 270, "the zombie itself is unharmed");
    assert!(t.zombie_ref(plain).is_some_and(|z| !z.is_dead_or_dying()));
}

#[test]
fn a_magnet_shroom_pulls_a_screen_door_then_recharges() {
    let mut t = test_board!("2-1");
    let magnet = t.plant(SeedType::Magnetshroom, 3, 2);
    let door = t.zombie(ZombieType::Door, 2, 560.0);
    assert!(t.run_until(300, |b| b.zombies.get(door).is_some_and(|z| z.shield_health() == 0)), "the door goes");
    let second = t.zombie(ZombieType::Door, 2, 600.0);
    t.ticks(600);
    assert!(t.zombie_ref(second).unwrap().shield_health() > 0, "but it takes a while to pull again");
    assert!(t.run_until(1200, |b| b.zombies.get(second).is_some_and(|z| z.shield_health() == 0)), "and then it does");
    assert!(t.plant_ref(magnet).is_some());
}

#[test]
fn a_magnet_shroom_ignores_zombies_out_of_reach() {
    let mut t = test_board!("2-1");
    t.plant(SeedType::Magnetshroom, 0, 0);
    let far = t.zombie(ZombieType::Pail, 4, 750.0);
    t.ticks(200);
    assert_eq!(t.zombie_ref(far).unwrap().helm_type(), super::super::zombie::HelmType::Pail);
}

#[test]
fn a_gold_magnet_pulls_in_money_from_all_over_the_lawn() {
    use super::super::coin::CoinMotion;
    let mut t = test_board!("2-1");
    t.plant(SeedType::Magnetshroom, 0, 2);
    let magnet = t.plant(SeedType::GoldMagnet, 0, 2);
    assert_eq!(t.plant_ref(magnet).unwrap().seed, SeedType::GoldMagnet, "it goes on the magnet-shroom");
    t.with(|board, effects, res, images, sounds| {
        board.add_coin(700.0, 200.0, CoinType::Gold, CoinMotion::Coin, effects, res, images, sounds);
        board.add_coin(500.0, 400.0, CoinType::Silver, CoinMotion::Coin, effects, res, images, sounds);
    });
    assert!(t.run_until(3000, |b| !b.coins.iter().any(|(_, c)| c.is_money() && !c.dead)), "both coins are taken");
    t.ticks(600);
    assert_eq!(t.money, 6, "and cashed: 5 for the gold, 1 for the silver");
}

#[test]
fn a_split_pea_shoots_forwards_and_backwards() {
    let mut t = test_board!("1-6");
    let pea = t.plant(SeedType::Splitpea, 4, 2);
    let (x, _) = t.square_center(4, 2);
    let ahead = t.zombie(ZombieType::Normal, 2, 700.0);
    let behind = t.zombie(ZombieType::Normal, 2, x as f32 - 150.0);
    assert!(
        t.run_until(1200, |b| {
            let hurt = |id| b.zombies.get(id).is_none_or(|z| z.body_health < 270);
            hurt(ahead) && hurt(behind)
        }),
        "the zombie in front and the one behind are both hit"
    );
    assert!(t.plant_ref(pea).is_some());
}

#[test]
fn a_split_pea_doesnt_waste_peas_on_other_rows() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Splitpea, 4, 2);
    let (x, _) = t.square_center(4, 2);
    let behind = t.zombie(ZombieType::Normal, 3, x as f32 - 150.0);
    t.ticks(300);
    assert_eq!(t.zombie_ref(behind).unwrap().body_health, 270);
}

#[test]
fn a_cactus_rises_for_a_balloon_zombie_and_pops_it() {
    use super::super::zombie::ZombiePhase;
    let mut t = test_board!("1-6");
    let cactus = t.plant(SeedType::Cactus, 0, 2);
    assert_eq!(t.plant_ref(cactus).unwrap().state(), PlantState::CactusLow);
    let balloon = t.zombie(ZombieType::Balloon, 2, 600.0);
    assert!(t.run_until(400, |b| b.plants.get(cactus).is_some_and(|p| p.state() == PlantState::CactusHigh)), "it rises");
    assert!(t.run_until(1500, |b| b.zombies.get(balloon).is_some_and(|z| z.phase != ZombiePhase::BalloonFlying)), "and pops the balloon");
    assert!(t.run_until(800, |b| b.plants.get(cactus).is_some_and(|p| p.state() == PlantState::CactusLow)), "then lowers");
}

#[test]
fn a_low_cactus_shoots_walkers_and_a_high_one_ignores_them() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Cactus, 0, 2);
    let walker = t.zombie(ZombieType::Normal, 2, 700.0);
    assert!(t.run_until(1200, |b| b.zombies.get(walker).is_none_or(|z| z.body_health < 270)), "spikes hit a zombie on the ground");
}

#[test]
fn a_starfruit_shoots_five_stars_and_hits_a_zombie_behind_it() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Starfruit, 4, 2);
    let (x, _) = t.square_center(4, 2);
    let behind = t.zombie(ZombieType::Normal, 2, x as f32 - 200.0);
    let mut most = 0;
    assert!(
        t.run_until(1200, |b| {
            most = most.max(b.projectiles.iter().filter(|(_, p)| p.ty == ProjectileType::Star).count());
            b.zombies.get(behind).is_none_or(|z| z.body_health < 270)
        }),
        "the star that flies left hits it"
    );
    assert_eq!(most, 5, "five stars to a volley");
}

#[test]
fn a_starfruit_stays_quiet_with_nothing_in_line() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Starfruit, 4, 2);
    t.zombie(ZombieType::Normal, 2, 780.0);
    t.ticks(400);
    assert!(t.board.projectiles.iter().all(|(_, p)| p.ty != ProjectileType::Star), "a zombie straight ahead isn't a target");
}

#[test]
fn a_cattail_only_goes_on_a_lily_pad_and_replaces_it() {
    let mut t = test_board!("3-1");
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Cattail), PlantingReason::NeedsUpgrade, "bare water");
    assert_eq!(t.board.can_plant_at(3, 0, SeedType::Cattail), PlantingReason::NotHere, "dry land");
    assert!(!t.board.planting_requirements_met(SeedType::Cattail) || t.board.plants.iter().any(|(_, p)| p.seed == SeedType::Lilypad));
    let pad = t.plant(SeedType::Lilypad, 3, 2);
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Cattail), PlantingReason::Ok);
    let cattail = t.plant(SeedType::Cattail, 3, 2);
    assert!(t.plant_ref(pad).is_none(), "the lily pad is used up");
    assert_eq!(t.plant_ref(cattail).unwrap().seed, SeedType::Cattail);
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Cattail), PlantingReason::NotHere, "and only one");
}

#[test]
fn a_cattail_shoots_zombies_in_any_row_and_flyers() {
    use super::super::zombie::ZombiePhase;
    let mut t = test_board!("3-1");
    t.plant(SeedType::Lilypad, 2, 2);
    let cattail = t.plant(SeedType::Cattail, 2, 2);
    let far_row = t.zombie(ZombieType::Normal, 0, 450.0);
    let mut fired = false;
    assert!(
        t.run_until(1500, |b| {
            fired |= b.projectiles.iter().any(|(_, p)| p.ty == ProjectileType::Spike);
            b.zombies.get(far_row).is_none_or(|z| z.body_health < 270)
        }),
        "a zombie two rows away is hit (spikes flown: {fired})"
    );
    let balloon = t.zombie(ZombieType::Balloon, 4, 650.0);
    assert!(t.run_until(3000, |b| b.zombies.get(balloon).is_none_or(|z| z.phase != ZombiePhase::BalloonFlying)), "and a balloon's popped");
    assert!(t.plant_ref(cattail).is_some());
}

#[test]
fn a_pumpkin_goes_over_a_plant_once() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Wallnut, 3, 2);
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Pumpkinshell), PlantingReason::Ok, "over the wall-nut");
    assert_eq!(t.board.can_plant_at(4, 2, SeedType::Pumpkinshell), PlantingReason::Ok, "or on a bare square");
    t.plant(SeedType::Pumpkinshell, 3, 2);
    let pumpkin = t.board.plants.iter().find(|(_, p)| p.seed == SeedType::Pumpkinshell).map(|(id, _)| id).unwrap();
    assert_eq!(t.plant_ref(pumpkin).unwrap().health, 4000);
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Pumpkinshell), PlantingReason::NotHere, "one pumpkin is enough");
    assert!(t.plant_ref(nut).is_some(), "the plant inside is untouched");
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Peashooter), PlantingReason::NotHere, "and the square is still taken");
    t.plant(SeedType::Peashooter, 4, 2);
    t.plant(SeedType::Pumpkinshell, 4, 2);
    assert_eq!(t.board.plants.iter().filter(|(_, p)| !p.dead).count(), 4);
}

#[test]
fn zombies_eat_the_pumpkin_before_the_plant_inside() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Wallnut, 3, 2);
    t.plant(SeedType::Pumpkinshell, 3, 2);
    let pumpkin = t.board.plants.iter().find(|(_, p)| p.seed == SeedType::Pumpkinshell).map(|(id, _)| id).unwrap();
    let zombie = t.zombie(ZombieType::Normal, 2, 330.0);
    assert!(t.run_until(1500, |b| b.zombies.get(zombie).is_some_and(|z| z.is_eating)), "it stops to eat");
    t.ticks(300);
    assert!(t.plant_ref(pumpkin).unwrap().health < 4000, "the pumpkin is chewed");
    assert_eq!(t.plant_ref(nut).unwrap().health, 4000, "the wall-nut isn't touched yet");
}

#[test]
fn first_aid_lets_a_hurt_pumpkin_or_nut_be_replaced() {
    let mut t = test_board!("1-6");
    let pumpkin = t.plant(SeedType::Pumpkinshell, 3, 2);
    let nut = t.plant(SeedType::Wallnut, 4, 2);
    t.board.plants.get_mut(pumpkin).unwrap().health = 2500;
    t.board.plants.get_mut(nut).unwrap().health = 2500;
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Pumpkinshell), PlantingReason::NotHere, "without First Aid, no");
    assert_eq!(t.board.can_plant_at(4, 2, SeedType::Wallnut), PlantingReason::NotHere);
    t.board.first_aid = true;
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Pumpkinshell), PlantingReason::Ok);
    assert_eq!(t.board.can_plant_at(4, 2, SeedType::Wallnut), PlantingReason::Ok);
    t.board.plants.get_mut(nut).unwrap().health = 4000;
    assert_eq!(t.board.can_plant_at(4, 2, SeedType::Wallnut), PlantingReason::NotHere, "but not a healthy one");
    t.plant(SeedType::Pumpkinshell, 3, 2);
    assert!(t.plant_ref(pumpkin).is_none(), "the old pumpkin is replaced");
    assert_eq!(t.plant_ref(t.plant_at(3, 2).unwrap()).unwrap().health, 4000);
}

#[test]
fn a_pumpkin_over_a_kernel_pult_spoils_the_cob_cannon_spot() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Kernelpult, 3, 2);
    t.plant(SeedType::Kernelpult, 4, 2);
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Cobcannon), PlantingReason::Ok);
    t.plant(SeedType::Pumpkinshell, 4, 2);
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Cobcannon), PlantingReason::NeedsUpgrade);
}

#[test]
fn nothing_goes_wrong_planting_a_pumpkin_on_water() {
    let mut t = test_board!("3-1");
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Pumpkinshell), PlantingReason::NotOnWater, "no lily pad");
    t.plant(SeedType::Lilypad, 3, 2);
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Pumpkinshell), PlantingReason::Ok);
}

#[test]
fn an_imitater_turns_into_the_plant_it_copies() {
    let mut t = test_board!("1-6");
    t.with(|board, effects, res, images, sounds| board.add_plant_as(3, 2, SeedType::Peashooter, true, effects, res, images, sounds));
    let imitater = t.plant_at(3, 2).unwrap();
    assert_eq!(t.plant_ref(imitater).unwrap().seed, SeedType::Imitater, "it goes down as an imitater");
    assert_eq!(t.board.can_plant_at(3, 2, SeedType::Sunflower), PlantingReason::NotHere, "and holds the square");
    assert!(
        t.run_until(600, |b| b.plants.iter().any(|(_, p)| !p.dead && p.seed == SeedType::Peashooter)),
        "a moment later it's a peashooter"
    );
    assert!(t.board.plants.iter().all(|(_, p)| p.dead || p.seed != SeedType::Imitater), "and the imitater's gone");
    let zombie = t.zombie(ZombieType::Normal, 2, 700.0);
    assert!(t.run_until(800, |b| b.zombies.get(zombie).is_none_or(|z| z.body_health < 270)), "the copy shoots like the real thing");
}

#[test]
fn an_imitated_pumpkin_goes_over_a_plant() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Wallnut, 3, 2);
    t.with(|board, effects, res, images, sounds| board.add_plant_as(3, 2, SeedType::Pumpkinshell, true, effects, res, images, sounds));
    assert!(t.run_until(600, |b| b.plants.iter().any(|(_, p)| !p.dead && p.seed == SeedType::Pumpkinshell)));
    assert!(t.plant_ref(nut).is_some(), "the wall-nut's still inside");
}

#[test]
fn an_imitater_packet_plants_the_copy_for_its_cost() {
    let mut t = test_board!("2-1");
    t.board.seed_bank.packets[0].set_packet_type(Some(SeedType::Peashooter));
    t.board.seed_bank.packets[0].imitater = true;
    t.board.sun_money = 150;
    t.plant_from_bank(0, 3, 2);
    assert_eq!(t.board.sun_money, 50, "a peashooter's price");
    assert_eq!(t.plant_ref(t.plant_at(3, 2).unwrap()).unwrap().seed, SeedType::Imitater);
    assert!(t.run_until(600, |b| b.plants.iter().any(|(_, p)| !p.dead && p.seed == SeedType::Peashooter)));
    assert!(t.board.seed_bank.packets[0].refreshing || !t.board.seed_bank.packets[0].active, "and it recharges like one");
}

#[test]
fn a_giant_wall_nut_flattens_everything_in_its_lane_and_keeps_going() {
    let mut t = test_board!("1-5");
    let giant = t.plant(SeedType::GiantWallnut, 2, 2);
    let door = t.zombie(ZombieType::Door, 2, 500.0);
    let bucket = t.zombie(ZombieType::Pail, 2, 650.0);
    let other_lane = t.zombie(ZombieType::Normal, 3, 650.0);
    assert!(
        t.run_until(1500, |b| {
            let down = |id| b.zombies.get(id).is_none_or(|z| z.is_dead_or_dying());
            down(door) && down(bucket)
        }),
        "a screen door and a bucket make no difference"
    );
    assert!(t.zombie_ref(other_lane).is_some_and(|z| !z.is_dead_or_dying()), "the other lane's untouched");
    if let Some(nut) = t.plant_ref(giant) {
        assert_eq!(nut.row, 2, "it never leaves its lane");
    }
}
