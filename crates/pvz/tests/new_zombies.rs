//! The newly ported zombies: jack-in-the-box, digger, pogo, yeti,
//! the red-eyed gargantuar, and the six zombotany heads.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Zombie.cpp` (`ZombieInitialize`,
//! `UpdateZombieJackInTheBox`, `UpdateZombieDigger`, `UpdateZombiePogo`,
//! `UpdateYeti`, the head updates, `PogoBreak`, `DiggerLoseAxe`).

use crate::plant::SeedType;
use crate::zombie::{ZombieType, can_spawn_on_level};

use super::super::zombie::ZombiePhase;
use super::test_board;

#[test]
fn each_new_zombie_starts_with_its_armour() {
    let mut t = test_board!("2-8");
    // (type, body, helmet, shield)
    let table = [
        (ZombieType::JackInTheBox, 500, 0, 0),
        (ZombieType::Digger, 270, 100, 0),
        (ZombieType::Pogo, 500, 0, 0),
        (ZombieType::Yeti, 1350, 0, 0),
        (ZombieType::RedeyeGargantuar, 6000, 0, 0),
        (ZombieType::PeaHead, 270, 0, 0),
        (ZombieType::WallnutHead, 270, 1100, 0),
        (ZombieType::JalapenoHead, 500, 0, 0),
        (ZombieType::GatlingHead, 270, 0, 0),
        (ZombieType::SquashHead, 270, 0, 0),
        (ZombieType::TallnutHead, 270, 2200, 0),
    ];
    for (row, &(ty, body, helm, shield)) in table.iter().enumerate() {
        let row = row as i32 % 5;
        let id = t.with(|board, effects, res, images, sounds| {
            let mut fx = super::super::zombie::Fx { effects, res, images, sounds };
            board.add_zombie_in_row(ty, row, 0, &mut fx)
        });
        let z = t.zombie_ref(id).unwrap();
        assert_eq!((z.body_health, z.helm_health(), z.shield_health()), (body, helm, shield), "{ty:?} armour");
        assert!(z.pos_x >= 780.0 || ty == ZombieType::TallnutHead, "{ty:?} starts off the right of the lawn, at {}", z.pos_x);
    }
}

#[test]
fn jack_in_the_box_runs_then_blows_up() {
    let mut t = test_board!("1-6");
    let victim = t.zombie(ZombieType::Normal, 2, 420.0);
    let dinner = t.plant(SeedType::Wallnut, 4, 2);
    let jack = t.zombie(ZombieType::JackInTheBox, 2, 700.0);
    assert!(
        t.run_until(500, |b| b.zombies.get(jack).is_some_and(|z| z.phase == ZombiePhase::JackRunning)),
        "it runs at the house counting down"
    );
    assert!(
        t.run_until(3000, |b| b.zombies.get(jack).is_none()),
        "then it pops and is gone"
    );
    assert!(t.board.zombies.get(victim).is_none_or(|z| z.is_dead_or_dying()), "the blast kills zombies around it");
    assert!(t.plant_ref(dinner).is_none_or(|p| p.dead), "and the plants around it");
}

#[test]
fn jack_in_the_box_cannot_be_shot_while_popping() {
    let mut t = test_board!("1-6");
    let jack = t.zombie(ZombieType::JackInTheBox, 2, 700.0);
    assert!(t.run_until(3000, |b| b.zombies.get(jack).is_some_and(|z| z.phase == ZombiePhase::JackPopping)), "it pops");
    let before = t.zombie_ref(jack).unwrap().body_health;
    t.ticks(5);
    assert_eq!(t.zombie_ref(jack).map(|z| z.body_health).unwrap_or(before), before, "peas do nothing to it now");
}

#[test]
fn digger_tunnels_past_the_defences_then_comes_back() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Wallnut, 4, 2);
    let digger = t.zombie(ZombieType::Digger, 2, 700.0);
    assert!(
        t.run_until(200, |b| b.zombies.get(digger).is_some_and(|z| z.phase == ZombiePhase::DiggerTunneling)),
        "it goes underground"
    );
    t.ticks(200);
    assert_eq!(t.plant_ref(nut).unwrap().health, 4000, "tunnelling under a plant doesn't touch it");
    assert!(
        t.run_until(2000, |b| b.zombies.get(digger).is_some_and(|z| matches!(z.phase, ZombiePhase::DiggerWalking | ZombiePhase::DiggerWalkingWithoutAxe))),
        "it bursts out at the back and walks back"
    );
    let left = t.zombie_ref(digger).unwrap().pos_x;
    t.ticks(100);
    assert!(t.zombie_ref(digger).unwrap().pos_x > left, "back towards the plants it tunnelled under");
}

#[test]
fn peas_pass_over_a_tunnelling_digger_but_cherry_bombs_do_not() {
    use super::super::zombie::damage_range;
    let mut t = test_board!("1-6");
    let digger = t.zombie(ZombieType::Digger, 2, 500.0);
    let z = t.zombie_ref(digger).unwrap();
    assert_eq!(z.phase, ZombiePhase::DiggerTunneling);
    assert!(!z.effected_by_damage(damage_range::GROUND), "peas can't reach it underground");
    assert!(z.effected_by_damage(damage_range::UNDERGROUND), "but doom-shrooms and cherry bombs can");
}

#[test]
fn pogo_stick_bounces_over_the_first_plant() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Wallnut, 4, 2);
    let nut_x = t.plant_ref(nut).unwrap().x;
    let pogo = t.zombie(ZombieType::Pogo, 2, 700.0);
    assert!(
        t.run_until(2000, |b| b.zombies.get(pogo).is_some_and(|z| z.phase == ZombiePhase::PogoHighBounce)),
        "it bounces high at the plant"
    );
    assert!(
        t.run_until(500, |b| b.zombies.get(pogo).is_some_and(|z| z.phase == ZombiePhase::PogoBouncing && z.pos_x < nut_x as f32 - 40.0)),
        "and lands past it, bouncing on"
    );
    assert_eq!(t.plant_ref(nut).unwrap().health, 4000, "without biting it");
}

#[test]
fn a_tall_nut_breaks_the_pogo_stick() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Tallnut, 4, 2);
    let pogo = t.zombie(ZombieType::Pogo, 2, 700.0);
    assert!(
        t.run_until(2500, |b| b.zombies.get(pogo).is_some_and(|z| !z.has_object())),
        "the stick breaks on the tall-nut"
    );
    assert!(
        t.run_until(300, |b| b.zombies.get(pogo).is_some_and(|z| z.phase == ZombiePhase::Normal)),
        "and it walks on"
    );
}

#[test]
fn the_yeti_gives_up_and_runs() {
    let mut t = test_board!("4-10");
    let yeti = t.zombie(ZombieType::Yeti, 2, 700.0);
    assert!(
        t.run_until(500, |b| b.zombies.get(yeti).is_some_and(|z| z.phase == ZombiePhase::Normal)),
        "it lumbers at the house first"
    );
    assert!(
        t.run_until(2500, |b| b.zombies.get(yeti).is_some_and(|z| z.phase == ZombiePhase::YetiRunning)),
        "then it flees"
    );
    assert!(!t.zombie_ref(yeti).unwrap().has_object(), "dropping its lunchbox");
}

#[test]
fn red_eyed_gargantuar_takes_twice_the_beating() {
    let mut t = test_board!("5-10");
    let red = t.zombie(ZombieType::RedeyeGargantuar, 2, 700.0);
    let plain = t.zombie(ZombieType::Gargantuar, 0, 700.0);
    assert_eq!(t.zombie_ref(red).unwrap().body_health, 6000);
    assert_eq!(t.zombie_ref(plain).unwrap().body_health, 3000);
    let nut = t.plant(SeedType::Wallnut, 7, 2);
    assert!(t.run_until(3000, |b| b.plants.get(nut).is_none_or(|p| p.squished)), "and it still smashes plants");
}

#[test]
fn pea_head_shoots_backwards_at_the_plants() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Wallnut, 1, 2);
    t.zombie(ZombieType::PeaHead, 2, 500.0);
    assert!(
        t.run_until(600, |b| b.plants.get(nut).is_some_and(|p| p.health < 4000)),
        "a backwards pea hits the wall-nut"
    );
}

#[test]
fn gatling_head_fires_a_volley() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Wallnut, 1, 2);
    t.zombie(ZombieType::GatlingHead, 2, 500.0);
    assert!(
        t.run_until(600, |b| b.plants.get(nut).is_some_and(|p| p.health <= 4000 - 40)),
        "several peas hit in one volley"
    );
}

#[test]
fn jalapeno_head_burns_its_row() {
    let mut t = test_board!("1-6");
    let left = t.plant(SeedType::Sunflower, 0, 2);
    let right = t.plant(SeedType::Wallnut, 5, 2);
    let other = t.plant(SeedType::Sunflower, 0, 1);
    t.zombie(ZombieType::JalapenoHead, 2, 500.0);
    assert!(
        t.run_until(4500, |b| b.plants.get(left).is_none_or(|p| p.dead) && b.plants.get(right).is_none_or(|p| p.dead)),
        "everything in its row burns"
    );
    assert!(!t.plant_ref(other).unwrap().dead, "but the next row over is fine");
}

#[test]
fn squash_head_squashes_what_it_eats() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Wallnut, 4, 2);
    t.zombie(ZombieType::SquashHead, 2, 700.0);
    assert!(
        t.run_until(2500, |b| b.plants.get(nut).is_some_and(|p| p.squished)),
        "the squash jumps on the wall-nut"
    );
}

#[test]
fn magnet_shroom_breaks_a_pogo_stick() {
    // Night: the magnet-shroom is awake (`Plant::UpdateMagnetShroom`).
    let mut t = test_board!("2-1");
    t.plant(SeedType::Magnetshroom, 1, 2);
    let pogo = t.zombie(ZombieType::Pogo, 2, 300.0);
    assert!(
        t.run_until(2500, |b| b.zombies.get(pogo).is_some_and(|z| !z.has_object())),
        "the magnet takes the stick"
    );
}
#[test]
fn magnet_shroom_takes_a_jack_box() {
    // Night: the magnet-shroom is awake (`Plant::UpdateMagnetShroom`).
    let mut t = test_board!("2-1");
    t.plant(SeedType::Magnetshroom, 1, 2);
    let jack = t.zombie(ZombieType::JackInTheBox, 2, 300.0);
    assert!(
        t.run_until(2500, |b| b.zombies.get(jack).is_some_and(|z| z.phase == ZombiePhase::Normal)),
        "the magnet takes the jack's box"
    );
}

#[test]
fn magnet_shroom_takes_a_digger_pickaxe() {
    let mut t = test_board!("2-1");
    t.plant(SeedType::Magnetshroom, 1, 2);
    let digger = t.zombie(ZombieType::Digger, 2, 300.0);
    assert!(
        t.run_until(2500, |b| b.zombies.get(digger).is_some_and(|z| !z.has_object())),
        "and a digger's pickaxe"
    );
}

#[test]
fn new_zombies_join_the_waves_on_their_levels() {
    assert!(can_spawn_on_level(ZombieType::JackInTheBox, 31), "jack-in-the-box from 4-1");
    assert!(can_spawn_on_level(ZombieType::Digger, 36), "digger from 4-6");
    assert!(can_spawn_on_level(ZombieType::Pogo, 38), "pogo from 4-8");
    assert!(can_spawn_on_level(ZombieType::RedeyeGargantuar, 48), "red eyes on the final boss");
    // The yeti never marches in a wave, even on its own level.
    let t = test_board!("4-10");
    assert!(
        t.board.waves.zombies_in_wave.iter().all(|w| !w.contains(&ZombieType::Yeti)),
        "the yeti never marches in a wave"
    );
}
