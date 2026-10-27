//! Zombies: spawning, walking, eating, taking damage, and the special ones.

use crate::plant::SeedType;
use crate::zombie::{ZombieType, can_spawn_on_level, waves_for_level};

use super::super::GameScene;
use super::super::zombie::ZombiePhase;
use super::test_board;

#[test]
fn each_zombie_starts_with_its_armour() {
    let mut t = test_board!("2-8");
    // (type, body, helmet, shield)
    let table = [
        (ZombieType::Normal, 270, 0, 0),
        (ZombieType::Flag, 270, 0, 0),
        (ZombieType::TrafficCone, 270, 370, 0),
        (ZombieType::Pail, 270, 1100, 0),
        (ZombieType::Polevaulter, 500, 0, 0),
        (ZombieType::Newspaper, 270, 0, 150),
        (ZombieType::Door, 270, 0, 1100),
        (ZombieType::Football, 270, 1400, 0),
        (ZombieType::Dancer, 500, 0, 0),
    ];
    for (row, &(ty, body, helm, shield)) in table.iter().enumerate() {
        let row = row as i32 % 5;
        let id = t.with(|board, effects, res, images, sounds| {
            let mut fx = super::super::zombie::Fx { effects, res, images, sounds };
            board.add_zombie_in_row(ty, row, 0, &mut fx)
        });
        let z = t.zombie_ref(id).unwrap();
        assert_eq!(z.row, row, "{ty:?} row");
        assert_eq!((z.body_health, z.helm_health(), z.shield_health()), (body, helm, shield), "{ty:?} armour");
        assert!(z.pos_x >= 780.0, "{ty:?} starts off the right of the lawn, at {}", z.pos_x);
        assert_eq!(z.pos_y, t.board.pos_y_based_on_row(z.pos_x + 40.0, row) - 30.0, "{ty:?} stands in its row");
    }
}

#[test]
fn levels_only_send_their_own_zombies() {
    for level in 1..=30 {
        let stage = format!("{}-{}", (level - 1) / 10 + 1, (level - 1) % 10 + 1);
        let t = test_board!(&stage);
        // 2-5 is whack-a-zombie, with eight waves of its own.
        let expected = if level == 15 { 8 } else { waves_for_level(level) };
        assert_eq!(t.board.waves.num_waves, expected, "{stage} wave count");
        for wave in &t.board.waves.zombies_in_wave {
            for &ty in wave {
                assert!(ty == ZombieType::Flag || can_spawn_on_level(ty, level), "{stage} sends {ty:?}");
            }
        }
    }
}

#[test]
fn waves_come_on_their_own() {
    let mut t = test_board!("1-6").with_waves();
    assert!(t.run_until(3000, |b| !b.zombies.is_empty()), "the first wave comes within 30 s");
}

#[test]
fn zombies_walk_towards_the_house() {
    let mut t = test_board!("2-8");
    let normal = t.zombie(ZombieType::Normal, 1, 700.0);
    let football = t.zombie(ZombieType::Football, 3, 700.0);
    t.ticks(1000);
    let walked = 700.0 - t.zombie_ref(normal).unwrap().pos_x;
    let ran = 700.0 - t.zombie_ref(football).unwrap().pos_x;
    assert!((100.0..450.0).contains(&walked), "a zombie walks about 2-4 px a second... it walked {walked}");
    assert!(ran > walked * 1.5, "the football zombie is much faster ({ran} vs {walked})");
}

#[test]
fn a_zombie_eats_a_plant_then_walks_on() {
    let mut t = test_board!("1-6");
    let sunflower = t.plant(SeedType::Sunflower, 3, 2);
    let zombie = t.zombie(ZombieType::Normal, 2, 330.0);
    assert!(t.run_until(400, |b| b.zombies.get(zombie).is_some_and(|z| z.is_eating)), "it stops to eat");
    let health = t.plant_ref(sunflower).unwrap().health;
    t.ticks(100);
    assert!(t.plant_ref(sunflower).is_some_and(|p| p.health < health), "the plant loses health");
    assert!(t.run_until(1000, |b| b.plants.get(sunflower).is_none_or(|p| p.dead)), "until it's eaten");
    let x = t.zombie_ref(zombie).unwrap().pos_x;
    t.ticks(300);
    let z = t.zombie_ref(zombie).unwrap();
    assert!(!z.is_eating && z.pos_x < x, "then it walks on");
}

#[test]
fn armour_takes_the_damage_first() {
    let mut t = test_board!("2-3");
    let cone = t.zombie(ZombieType::TrafficCone, 1, 600.0);
    let door = t.zombie(ZombieType::Door, 3, 600.0);
    let hit = |t: &mut super::TestBoard, id, damage| {
        let env = t.board.zombie_env();
        t.with(|board, effects, res, images, sounds| {
            let mut fx = super::super::zombie::Fx { effects, res, images, sounds };
            board.zombies.get_mut(id).unwrap().take_damage(damage, 0, &env, &mut fx);
        });
    };
    hit(&mut t, cone, 300);
    let z = t.zombie_ref(cone).unwrap();
    assert_eq!((z.helm_health(), z.body_health), (70, 270), "the cone takes it");
    hit(&mut t, cone, 100);
    let z = t.zombie_ref(cone).unwrap();
    assert_eq!((z.helm_health(), z.body_health), (0, 240), "what the cone can't take goes to the body");

    hit(&mut t, door, 1000);
    assert_eq!(t.zombie_ref(door).unwrap().body_health, 270, "the door takes it");
    hit(&mut t, door, 200);
    let z = t.zombie_ref(door).unwrap();
    assert_eq!((z.shield_health(), z.body_health), (0, 170), "then the door's gone");
}

#[test]
fn zombies_lose_their_arm_then_their_head() {
    let mut t = test_board!("1-6");
    let id = t.zombie(ZombieType::Normal, 2, 600.0);
    let env = t.board.zombie_env();
    let hit = |t: &mut super::TestBoard, damage| {
        t.with(|board, effects, res, images, sounds| {
            let mut fx = super::super::zombie::Fx { effects, res, images, sounds };
            board.zombies.get_mut(id).unwrap().take_damage(damage, 0, &env, &mut fx);
        });
    };
    hit(&mut t, 90);
    let z = t.zombie_ref(id).unwrap();
    assert!(z.has_arm() && z.has_head, "180 left (two-thirds): still whole");
    hit(&mut t, 1);
    assert!(!t.zombie_ref(id).unwrap().has_arm(), "below two-thirds the arm drops");
    hit(&mut t, 89);
    assert!(t.zombie_ref(id).unwrap().has_head, "90 left (a third): head still on");
    hit(&mut t, 1);
    assert!(!t.zombie_ref(id).unwrap().has_head, "below a third the head goes");
    hit(&mut t, 100);
    assert!(t.zombie_ref(id).is_none_or(|z| z.is_dead_or_dying()), "then it dies");
}

#[test]
fn a_lawn_mower_clears_its_row() {
    let mut t = test_board!("1-6");
    let first = t.zombie(ZombieType::Normal, 2, 0.0);
    let second = t.zombie(ZombieType::Normal, 2, 300.0);
    assert!(t.run_until(1500, |b| b.zombies.get(second).is_none_or(|z| z.is_dead_or_dying())), "the mower runs them over");
    assert!(t.zombie_ref(first).is_none_or(|z| z.is_dead_or_dying()));
    assert!(t.board.scene == GameScene::Playing, "the level goes on");
}

#[test]
fn a_zombie_in_the_house_ends_the_level() {
    let mut t = test_board!("1-6");
    t.board.lawn_mowers.retain(|m| m.row != 2);
    t.zombie(ZombieType::Normal, 2, -60.0);
    assert!(t.run_until(1000, |b| b.scene == GameScene::ZombiesWon), "the zombies win");
}

#[test]
fn pole_vaulter_jumps_the_first_plant() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Wallnut, 4, 2);
    let vaulter = t.zombie(ZombieType::Polevaulter, 2, 700.0);
    assert!(
        t.run_until(1500, |b| b.zombies.get(vaulter).is_some_and(|z| z.phase == ZombiePhase::PoleInVault)),
        "it vaults when it reaches the plant"
    );
    assert!(
        t.run_until(300, |b| b.zombies.get(vaulter).is_some_and(|z| z.phase == ZombiePhase::Normal)),
        "and lands"
    );
    let nut_x = t.plant_ref(nut).unwrap().x;
    assert!(t.zombie_ref(vaulter).unwrap().x < nut_x, "on the other side of the plant");
    assert_eq!(t.plant_ref(nut).unwrap().health, 4000, "without biting it");
}

#[test]
fn pole_vaulter_cannot_jump_a_tall_nut() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Tallnut, 4, 2);
    let nut_x = t.plant_ref(nut).unwrap().x;
    let vaulter = t.zombie(ZombieType::Polevaulter, 2, 700.0);
    assert!(
        t.run_until(1500, |b| b.zombies.get(vaulter).is_some_and(|z| z.phase == ZombiePhase::PoleInVault)),
        "it starts the jump at the plant"
    );
    assert!(
        t.run_until(300, |b| b.zombies.get(vaulter).is_some_and(|z| z.phase == ZombiePhase::Normal)),
        "the jump ends"
    );
    assert!(t.zombie_ref(vaulter).unwrap().x >= nut_x - 10, "but it's stopped by the tall-nut, not over it");
    assert!(
        t.run_until(600, |b| b.zombies.get(vaulter).is_some_and(|z| z.is_eating)),
        "and it has to eat the nut"
    );
    assert!(t.run_until(600, |b| b.plants.get(nut).is_some_and(|p| p.health < 8000)), "which gets bitten");
}

#[test]
fn newspaper_zombie_speeds_up_without_its_paper() {
    let mut t = test_board!("2-1");
    let id = t.zombie(ZombieType::Newspaper, 2, 700.0);
    t.ticks(500);
    let x = t.zombie_ref(id).unwrap().pos_x;
    t.ticks(500);
    let reading = x - t.zombie_ref(id).unwrap().pos_x;
    let env = t.board.zombie_env();
    t.with(|board, effects, res, images, sounds| {
        let mut fx = super::super::zombie::Fx { effects, res, images, sounds };
        board.zombies.get_mut(id).unwrap().take_damage(150, 0, &env, &mut fx);
    });
    assert_eq!(t.zombie_ref(id).unwrap().phase, ZombiePhase::NewspaperMaddening, "it gasps");
    assert!(t.run_until(500, |b| b.zombies.get(id).is_some_and(|z| z.phase == ZombiePhase::NewspaperMad)), "then gets angry");
    let x = t.zombie_ref(id).unwrap().pos_x;
    t.ticks(500);
    let angry = x - t.zombie_ref(id).unwrap().pos_x;
    assert!(angry > reading * 2.0, "and storms on ({angry} vs {reading})");
}

#[test]
fn dancer_calls_up_four_backup_dancers() {
    let mut t = test_board!("2-8");
    let dancer = t.zombie(ZombieType::Dancer, 2, 650.0);
    let backups = |b: &super::super::Board| b.zombies.iter().filter(|(_, z)| z.zombie_type == ZombieType::BackupDancer).count();
    assert!(t.run_until(1000, |b| backups(b) == 4), "four backup dancers rise");
    let leader = t.zombie_ref(dancer).unwrap();
    assert!(leader.followers.iter().all(Option::is_some), "and follow the dancer");
    let rows: Vec<_> = t.board.zombies.iter().filter(|(_, z)| z.zombie_type == ZombieType::BackupDancer).map(|(_, z)| z.row).collect();
    for row in [1, 2, 3] {
        assert!(rows.contains(&row), "around the dancer: above, beside and below (rows {rows:?})");
    }
}

#[test]
fn the_dancers_wait_while_one_eats() {
    let mut t = test_board!("2-8");
    let dancer = t.zombie(ZombieType::Dancer, 2, 650.0);
    let backups = |b: &super::super::Board| b.zombies.iter().filter(|(_, z)| z.zombie_type == ZombieType::BackupDancer).count();
    assert!(t.run_until(1000, |b| backups(b) == 4));
    t.ticks(200);
    // A plant in front of the backup dancer above.
    let above = t.board.zombies.iter().find(|(_, z)| z.zombie_type == ZombieType::BackupDancer && z.row == 1).map(|(id, _)| id).unwrap();
    let col = t.board.pixel_to_grid_x(t.zombie_ref(above).unwrap().x + 20);
    t.plant(SeedType::Wallnut, col, 1);
    assert!(t.run_until(800, |b| b.zombies.get(above).is_some_and(|z| z.is_eating)), "it stops to eat");
    let x = t.zombie_ref(dancer).unwrap().pos_x;
    t.ticks(300);
    assert_eq!(t.zombie_ref(dancer).unwrap().pos_x, x, "the whole troupe waits");
}

#[test]
fn frozen_zombies_stand_still() {
    let mut t = test_board!("1-6");
    let id = t.zombie(ZombieType::Normal, 2, 600.0);
    t.ticks(100);
    let env = t.board.zombie_env();
    t.with(|board, effects, res, images, sounds| {
        let mut fx = super::super::zombie::Fx { effects, res, images, sounds };
        board.zombies.get_mut(id).unwrap().hit_ice_trap(&env, &mut fx);
    });
    let x = t.zombie_ref(id).unwrap().pos_x;
    t.ticks(250);
    assert_eq!(t.zombie_ref(id).unwrap().pos_x, x, "frozen");
    t.ticks(500);
    assert!(t.zombie_ref(id).unwrap().pos_x < x, "and walks on once the ice melts");
}

#[test]
fn zombies_climb_out_of_the_graves_on_the_last_wave() {
    let mut t = test_board!("2-1");
    let graves = t.board.grid_items.len();
    assert!(graves > 0, "2-1 has graves");
    t.board.rise_from_grave_counter = 1;
    t.tick();
    let rising = t.board.zombies.iter().filter(|(_, z)| z.phase == ZombiePhase::RisingFromGrave).count();
    assert_eq!(rising, graves, "one zombie out of each grave");
    assert!(
        t.run_until(200, |b| b.zombies.iter().all(|(_, z)| z.phase != ZombiePhase::RisingFromGrave)),
        "they climb all the way out"
    );
}


#[test]
fn a_zombie_sinks_into_the_pool_and_climbs_out() {
    let mut t = test_board!("3-1");
    let id = t.zombie(ZombieType::Normal, 2, 700.0);
    assert!(!t.zombie_ref(id).unwrap().in_pool(), "dry at the right edge");
    assert!(t.run_until(600, |b| b.zombies.get(id).is_some_and(|z| z.in_pool())), "it wades in once it is over the water");
    assert!(t.run_until(100, |b| b.zombies.get(id).is_some_and(|z| z.altitude() <= -40.0)), "and sinks to its chest");
    // Leaving the water on the far side: it rises again (no mower to stop it).
    t.board.lawn_mowers.retain(|m| m.row != 2);
    t.board.zombies.get_mut(id).unwrap().place_x(-20.0);
    assert!(t.run_until(100, |b| b.zombies.get(id).is_some_and(|z| !z.in_pool() && z.altitude() == 0.0)), "it climbs out");
}

#[test]
fn land_only_zombies_do_not_swim() {
    assert!(ZombieType::Normal.can_go_in_pool() && ZombieType::Pail.can_go_in_pool());
    assert!(!ZombieType::Newspaper.can_go_in_pool() && !ZombieType::Football.can_go_in_pool());
    let mut t = test_board!("3-1");
    let id = t.zombie(ZombieType::Newspaper, 2, 500.0);
    t.ticks(100);
    assert!(!t.zombie_ref(id).unwrap().in_pool());
}

#[test]
fn zombies_in_the_grass_rows_stay_dry() {
    let mut t = test_board!("3-1");
    let id = t.zombie(ZombieType::Normal, 1, 500.0);
    t.ticks(300);
    assert!(!t.zombie_ref(id).unwrap().in_pool());
    assert_eq!(t.zombie_ref(id).unwrap().altitude(), 0.0);
}

#[test]
fn water_rows_only_take_swimmers_and_only_late_for_land_swimmers() {
    let mut t = test_board!("3-1");
    t.board.waves.current_wave = 0;
    assert!(t.board.row_can_have_zombie_type(1, ZombieType::Normal), "grass");
    assert!(!t.board.row_can_have_zombie_type(2, ZombieType::Normal), "no wading zombies in the first waves");
    assert!(!t.board.row_can_have_zombie_type(1, ZombieType::Snorkel), "pool-only types stay in the water");
    t.board.waves.current_wave = 5;
    assert!(t.board.row_can_have_zombie_type(2, ZombieType::Normal), "from the fifth wave they wade in");
    assert!(t.board.row_can_have_zombie_type(3, ZombieType::Pail));
    assert!(!t.board.row_can_have_zombie_type(2, ZombieType::Newspaper), "but newspaper zombies can't swim");
    assert!(!t.board.row_can_have_zombie_type(2, ZombieType::Football));
    assert!(t.board.row_can_have_zombie_type(2, ZombieType::Snorkel));
}

#[test]
fn early_waves_never_send_land_zombies_into_the_pool() {
    let mut t = test_board!("3-1").with_waves();
    let mut seen = 0;
    for _ in 0..6000 {
        t.tick();
        for (_, z) in t.board.zombies.iter() {
            assert!(z.row != 2 && z.row != 3, "row {} is water", z.row);
            seen += 1;
        }
    }
    assert!(seen > 0, "zombies came");
}

#[test]
fn snorkel_dives_in_and_cannot_be_shot_while_swimming() {
    use crate::board::zombie::ZombiePhase::*;
    let mut t = test_board!("3-1");
    t.plant(SeedType::Lilypad, 0, 2);
    t.plant(SeedType::Peashooter, 0, 2);
    let snorkel = t.zombie(ZombieType::Snorkel, 2, 760.0);
    assert!(t.run_until(500, |b| b.zombies.get(snorkel).is_some_and(|z| z.phase == SnorkelWalkingInPool)), "it dives in");
    assert!(t.zombie_ref(snorkel).unwrap().in_pool());
    // Peas fired as it dived are still in the air for a moment.
    t.ticks(150);
    let health = t.zombie_ref(snorkel).unwrap().body_health;
    t.ticks(300);
    assert_eq!(t.zombie_ref(snorkel).unwrap().body_health, health, "peas can't reach it under the water");
    assert!(t.board.projectiles.is_empty(), "so the peashooter doesn't even fire");
}

#[test]
fn snorkel_surfaces_to_eat_the_plant_on_the_pad() {
    use crate::board::zombie::ZombiePhase::*;
    let mut t = test_board!("3-1");
    t.plant(SeedType::Lilypad, 0, 2);
    t.plant(SeedType::Peashooter, 0, 2);
    let pad = t.plant(SeedType::Lilypad, 4, 2);
    let nut = t.plant(SeedType::Wallnut, 4, 2);
    let snorkel = t.zombie(ZombieType::Snorkel, 2, 760.0);
    assert!(t.run_until(2500, |b| b.zombies.get(snorkel).is_some_and(|z| z.phase == SnorkelEatingInPool)), "it comes up to bite");
    t.ticks(100);
    assert!(t.plant_ref(nut).is_some_and(|p| p.health < 4000), "the wall-nut on the pad is bitten first");
    assert_eq!(t.plant_ref(pad).unwrap().health, 300, "the pad is spared");
    assert!(t.run_until(600, |b| b.zombies.get(snorkel).is_none_or(|z| z.body_health < 270)), "and now the peas can hit it");
}

/// The plant of this kind on a square (a lily pad shares its square).
fn plant_of(t: &super::TestBoard, seed: SeedType, col: i32, row: i32) -> sexy::data_array::Id<super::super::plant::Plant> {
    t.board.plants.iter().find(|(_, p)| p.col == col && p.row == row && p.seed == seed && !p.dead).map(|(id, _)| id).unwrap()
}

#[test]
fn dolphin_rider_rides_the_pool_and_jumps_the_first_plant() {
    use crate::board::zombie::ZombiePhase::*;
    let mut t = test_board!("3-1");
    t.plant(SeedType::Lilypad, 3, 2);
    t.plant(SeedType::Wallnut, 3, 2);
    let nut = plant_of(&t, SeedType::Wallnut, 3, 2);
    let nut_x = t.plant_ref(nut).unwrap().x;
    let dolphin = t.zombie(ZombieType::DolphinRider, 2, 760.0);
    assert!(t.run_until(600, |b| b.zombies.get(dolphin).is_some_and(|z| z.phase == DolphinRiding)), "it dives in and rides");
    assert!(t.run_until(1500, |b| b.zombies.get(dolphin).is_some_and(|z| z.phase == DolphinInJump)), "and leaps at the plant");
    assert!(t.run_until(300, |b| b.zombies.get(dolphin).is_some_and(|z| z.phase == DolphinWalkingInPool)), "then lands");
    assert!(t.zombie_ref(dolphin).unwrap().x < nut_x, "on the other side");
    assert_eq!(t.plant_ref(nut).unwrap().health, 4000, "without biting it");
}

#[test]
fn dolphin_rider_is_stopped_by_a_tall_nut() {
    use crate::board::zombie::ZombiePhase::*;
    let mut t = test_board!("3-1");
    t.plant(SeedType::Lilypad, 3, 2);
    t.plant(SeedType::Tallnut, 3, 2);
    let nut = plant_of(&t, SeedType::Tallnut, 3, 2);
    let nut_x = t.plant_ref(nut).unwrap().x;
    let dolphin = t.zombie(ZombieType::DolphinRider, 2, 760.0);
    assert!(t.run_until(2000, |b| b.zombies.get(dolphin).is_some_and(|z| z.phase == DolphinWalkingInPool)), "the jump ends");
    assert!(t.zombie_ref(dolphin).unwrap().x >= nut_x, "against the tall-nut, not over it");
    assert!(t.run_until(600, |b| b.plants.get(nut).is_some_and(|p| p.health < 8000)), "so it has to bite through");
}

#[test]
fn zamboni_drives_over_plants_and_leaves_ice() {
    let mut t = test_board!("3-6");
    let nut = t.plant(SeedType::Wallnut, 5, 1);
    let zamboni = t.zombie(ZombieType::Zamboni, 1, 760.0);
    assert!(t.run_until(4000, |b| b.plants.get(nut).is_none_or(|p| p.dead)), "the wall-nut is flattened, not eaten");
    assert!(!t.zombie_ref(zamboni).unwrap().is_eating);
    assert!(t.board.ice_timer[1] > 0, "it leaves a trail of ice in its row");
    assert!(t.board.ice_min_x[1] < 800);
    assert_eq!(t.board.ice_timer[0], 0, "and only in its row");
}

#[test]
fn zamboni_pops_its_tires_on_spikeweed() {
    use crate::board::zombie::ZombiePhase::*;
    let mut t = test_board!("3-6");
    t.plant(SeedType::Spikeweed, 5, 1);
    let zamboni = t.zombie(ZombieType::Zamboni, 1, 760.0);
    assert!(t.run_until(4000, |b| b.zombies.get(zamboni).is_some_and(|z| z.flat_tires())), "the spikes pop its tires");
    assert_eq!(t.zombie_ref(zamboni).unwrap().phase, Dying);
}

#[test]
fn a_cherry_bomb_blows_up_a_zamboni() {
    let mut t = test_board!("3-6");
    let zamboni = t.zombie(ZombieType::Zamboni, 1, 400.0);
    t.plant(SeedType::Cherrybomb, 4, 1);
    t.ticks(150);
    assert!(t.zombie_ref(zamboni).is_none_or(|z| z.is_dead_or_dying()), "it goes up with the bomb");
}

#[test]
fn zambonis_cannot_be_chilled_or_frozen() {
    let mut t = test_board!("3-6");
    t.plant(SeedType::Snowpea, 0, 1);
    let zamboni = t.zombie(ZombieType::Zamboni, 1, 700.0);
    t.ticks(600);
    assert!(!t.zombie_ref(zamboni).unwrap().is_chilled(), "snow peas don't slow it");
}

fn bobsleds(t: &super::TestBoard) -> Vec<sexy::data_array::Id<super::super::zombie::Zombie>> {
    t.board.zombies.iter().filter(|(_, z)| z.zombie_type == ZombieType::Bobsled).map(|(id, _)| id).collect()
}

#[test]
fn a_bobsled_team_needs_ice_to_ride() {
    let mut t = test_board!("3-6");
    t.board.waves.current_wave = 5;
    assert!(!t.board.row_can_have_zombie_type(1, ZombieType::Bobsled), "no ice, no sled");
    t.board.ice_timer[1] = 3000;
    t.board.ice_min_x[1] = 300;
    assert!(t.board.row_can_have_zombie_type(1, ZombieType::Bobsled));
    assert!(!t.board.row_can_have_zombie_type(0, ZombieType::Bobsled), "and only in the iced row");
}

#[test]
fn a_bobsled_team_is_a_leader_and_three_riders() {
    let mut t = test_board!("3-6");
    t.board.ice_timer[1] = 100_000;
    t.board.ice_min_x[1] = 0;
    let leader = t.zombie(ZombieType::Bobsled, 1, 880.0);
    let team = bobsleds(&t);
    assert_eq!(team.len(), 4, "four zombies on the sled");
    let mut positions: Vec<_> = team.iter().map(|&id| t.zombie_ref(id).unwrap().bobsled_position).collect();
    positions.sort_unstable();
    assert_eq!(positions, [0, 1, 2, 3]);
    assert_eq!(t.zombie_ref(leader).unwrap().bobsled_position, 0);
    // Only the leader can be hurt while they ride, and the sled takes the blows.
    let follower = *team.iter().find(|&&id| t.zombie_ref(id).unwrap().bobsled_position == 2).unwrap();
    assert!(!t.zombie_ref(follower).unwrap().effected_by_damage(1), "the riders behind are out of reach");
    assert!(t.zombie_ref(leader).unwrap().effected_by_damage(1));
}

#[test]
fn a_broken_sled_tips_the_team_off_and_they_walk() {
    use crate::board::zombie::ZombiePhase::*;
    let mut t = test_board!("3-6");
    t.board.ice_timer[1] = 100_000;
    t.board.ice_min_x[1] = 0;
    let leader = t.zombie(ZombieType::Bobsled, 1, 880.0);
    t.ticks(10);
    let env = t.board.zombie_env();
    t.with(|board, effects, res, images, sounds| {
        let mut fx = super::super::zombie::Fx { effects, res, images, sounds };
        board.zombies.get_mut(leader).unwrap().take_damage(300, 0, &env, &mut fx);
    });
    t.ticks(2);
    for id in bobsleds(&t) {
        assert_eq!(t.zombie_ref(id).unwrap().phase, BobsledCrashing, "the whole team crashes");
    }
    t.ticks(200);
    for id in bobsleds(&t) {
        let z = t.zombie_ref(id).unwrap();
        assert_eq!(z.phase, Normal, "then they get up");
        assert_eq!(z.bobsled_position, -1);
        assert_eq!(z.body_health, 270);
    }
}

#[test]
fn a_sled_that_runs_off_the_ice_falls_apart() {
    use crate::board::zombie::ZombiePhase::*;
    let mut t = test_board!("3-6");
    t.board.ice_timer[1] = 100_000;
    t.board.ice_min_x[1] = 600;
    let leader = t.zombie(ZombieType::Bobsled, 1, 880.0);
    assert!(
        t.run_until(3000, |b| b.zombies.get(leader).is_some_and(|z| z.phase == BobsledCrashing)),
        "past the end of the ice the sled breaks up"
    );
}

#[test]
fn a_cherry_bomb_takes_a_whole_sled_team() {
    let mut t = test_board!("3-6");
    t.board.ice_timer[1] = 100_000;
    t.board.ice_min_x[1] = 0;
    t.zombie(ZombieType::Bobsled, 1, 880.0);
    t.ticks(5);
    for id in bobsleds(&t) {
        t.board.zombies.get_mut(id).unwrap().place_x(400.0);
    }
    t.plant(SeedType::Cherrybomb, 4, 1);
    t.ticks(200);
    assert!(bobsleds(&t).iter().all(|&id| t.zombie_ref(id).is_none_or(|z| z.is_dead_or_dying())), "none of them survive");
}

#[test]
fn the_pool_levels_run_their_waves_without_incident() {
    // The stage, and the zombie that level brings in for the first time.
    for (stage, new) in [("3-1", None), ("3-3", Some(ZombieType::Snorkel)), ("3-6", Some(ZombieType::Zamboni)), ("3-8", Some(ZombieType::DolphinRider))] {
        let mut t = test_board!(stage).with_waves();
        t.give_sun(9000);
        // A wall of defences so the zombies have something to meet.
        for row in [0, 1, 4, 5] {
            t.plant(SeedType::Peashooter, 0, row);
        }
        let mut kinds = std::collections::HashSet::new();
        for _ in 0..25_000 {
            t.tick();
            for (_, z) in t.board.zombies.iter() {
                kinds.insert(z.zombie_type);
                if z.zombie_type.is_pool_only() {
                    assert!(z.row == 2 || z.row == 3, "{stage}: {:?} on a grass row", z.zombie_type);
                }
            }
        }
        if let Some(new) = new {
            assert!(kinds.contains(&new), "{stage} should send {new:?}, sent {kinds:?}");
        }
    }
}


#[test]
fn typing_dance_toggles_the_dance_animation() {
    let mut t = test_board!("2-1");
    let normal = t.zombie(ZombieType::Normal, 2, 600.0);
    let cone = t.zombie(ZombieType::TrafficCone, 3, 630.0);
    t.ticks(5);
    t.with(|board, effects, res, images, sounds| {
        for letter in "dance".chars() {
            board.type_cheat_letter(letter, effects, res, images, sounds);
        }
    });
    assert!(t.board.dance_mode(), "the word toggles dance mode");
    assert!(t.zombie_ref(normal).is_some_and(|z| z.body_playing(&t.effects, "anim_dance")));
    assert!(t.zombie_ref(cone).is_some_and(|z| z.body_playing(&t.effects, "anim_dance")));
    t.with(|board, effects, res, images, sounds| {
        for letter in "dance".chars() {
            board.type_cheat_letter(letter, effects, res, images, sounds);
        }
    });
    assert!(!t.board.dance_mode(), "the same word turns it off");
    assert!(t.zombie_ref(normal).is_some_and(|z| !z.body_playing(&t.effects, "anim_dance")));
}
