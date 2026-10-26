//! The balloon zombie: it floats over the plants, only a blover (or a
//! cactus, later) can reach it, and once its balloon pops it walks.

use crate::plant::SeedType;
use crate::zombie::ZombieType;

use super::super::zombie::ZombiePhase;
use super::test_board;

#[test]
fn a_balloon_zombie_floats_over_plants_without_eating_them() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Wallnut, 4, 2);
    let balloon = t.zombie(ZombieType::Balloon, 2, 700.0);
    assert_eq!(t.zombie_ref(balloon).unwrap().phase, ZombiePhase::BalloonFlying);
    let nut_x = t.plant_ref(nut).unwrap().x as f32;
    assert!(t.run_until(3000, |b| b.zombies.get(balloon).is_some_and(|z| z.pos_x < nut_x - 100.0)), "it sails past the wall-nut");
    assert_eq!(t.plant_ref(nut).unwrap().health, 4000, "without a bite");
}

#[test]
fn peas_pass_through_a_balloon_zombie() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Repeater, 0, 2);
    let balloon = t.zombie(ZombieType::Balloon, 2, 600.0);
    t.ticks(300);
    let z = t.zombie_ref(balloon).unwrap();
    assert_eq!(z.body_health, 270, "peas don't hurt it");
    assert_eq!(z.phase, ZombiePhase::BalloonFlying);
}

#[test]
fn a_blover_blows_a_balloon_zombie_off_the_lawn() {
    let mut t = test_board!("1-6");
    let balloon = t.zombie(ZombieType::Balloon, 2, 600.0);
    t.plant(SeedType::Blover, 0, 0);
    assert!(t.run_until(600, |b| b.zombies.get(balloon).is_none_or(|z| z.dead)), "it's gone");
}

#[test]
fn a_popped_balloon_zombie_lands_and_walks_and_eats() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Wallnut, 1, 2);
    let balloon = t.zombie(ZombieType::Balloon, 2, 300.0);
    let env = t.board.zombie_env();
    t.with(|board, effects, res, images, sounds| {
        let mut fx = super::super::zombie::Fx { effects, res, images, sounds };
        board.zombies.get_mut(balloon).unwrap().take_damage(20, 0, &env, &mut fx);
    });
    assert_eq!(t.zombie_ref(balloon).unwrap().phase, ZombiePhase::BalloonPopping);
    assert!(t.run_until(300, |b| b.zombies.get(balloon).is_some_and(|z| z.phase == ZombiePhase::BalloonWalking && z.altitude() == 0.0)), "it comes down on its feet");
    assert!(t.run_until(3000, |b| b.plants.get(nut).is_none_or(|p| p.health < 4000)), "and goes for the wall-nut");
}
