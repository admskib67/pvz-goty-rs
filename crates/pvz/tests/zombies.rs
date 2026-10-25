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

