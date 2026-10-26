//! The last level, 5-10: Dr. Zomboss.

use sexy::data_array::Id;

use crate::plant::SeedType;
use crate::zombie::ZombieType;

use super::super::coin::CoinType;
use super::super::cursor::PlantingReason;
use super::super::plant::Plant;
use super::super::zombie::{Fx, Zombie, ZombiePhase};
use super::{TestBoard, test_board};

fn boss_of(t: &TestBoard) -> Id<Zombie> {
    t.board.boss_zombie().expect("the boss is on the lawn")
}

fn boss_ref(t: &TestBoard) -> &Zombie {
    t.board.zombies.get(boss_of(t)).unwrap()
}

/// The boss steps onto the lawn, and gets through his entrance.
fn with_boss(t: &mut TestBoard) -> Id<Zombie> {
    t.with(|board, effects, res, images, sounds| {
        let mut fx = Fx { effects, res, images, sounds };
        board.play_boss_enter(&mut fx);
    });
    let id = boss_of(t);
    assert!(t.run_until(1000, |b| b.zombies.get(id).is_some_and(|z| z.phase == ZombiePhase::BossIdle)), "he settles down");
    // Nothing is due for a while.
    t.board.zombies.get_mut(id).unwrap().boss_set_counters(Some(100_000), Some(100_000), Some(100_000), Some(100_000));
    id
}

fn set_counters(t: &mut TestBoard, stomp: Option<i32>, bungee: Option<i32>, head: Option<i32>, summon: Option<i32>) {
    let id = boss_of(t);
    t.board.zombies.get_mut(id).unwrap().boss_set_counters(stomp, bungee, head, summon);
}

/// A plant in a pot, since on the roof nothing stands without one.
fn potted(t: &mut TestBoard, seed: SeedType, col: i32, row: i32) -> Id<Plant> {
    if t.board.can_plant_at(col, row, SeedType::Flowerpot) == PlantingReason::Ok {
        t.plant(SeedType::Flowerpot, col, row);
    }
    t.plant(seed, col, row)
}

fn squished(t: &TestBoard, col: i32, row: i32) -> bool {
    t.board.plants.iter().any(|(_, p)| p.col == col && p.row == row && p.seed == SeedType::Wallnut && p.squished)
}

fn health_of(t: &TestBoard, id: Id<Zombie>) -> i32 {
    t.board.zombies.get(id).unwrap().body_health
}

#[test]
fn the_boss_level_has_a_belt_and_no_waves() {
    let t = test_board!("5-10");
    assert!(t.board.is_final_boss_level() && t.board.has_conveyor_belt_seed_bank());
    assert_eq!(t.board.seed_bank.num_seeds_on_conveyor_belt(), 4, "four packets are on the belt from the start");
    assert_eq!(t.board.can_plant_at(8, 2, SeedType::Flowerpot), PlantingReason::NotHere, "the last column is his");
    assert_eq!(t.board.can_plant_at(7, 2, SeedType::Flowerpot), PlantingReason::Ok);
    let mut t = t.with_waves();
    t.ticks(3000);
    assert!(t.board.zombies.is_empty(), "no zombies come by themselves");
    assert!(t.board.seed_bank.num_seeds_on_conveyor_belt() > 4, "but the belt keeps bringing plants");
}

#[test]
fn the_boss_steps_in_and_stands_about() {
    let mut t = test_board!("5-10");
    t.with(|board, effects, res, images, sounds| {
        let mut fx = Fx { effects, res, images, sounds };
        board.play_boss_enter(&mut fx);
    });
    let id = boss_of(&t);
    assert_eq!(t.board.zombies.get(id).unwrap().phase, ZombiePhase::BossEnter);
    assert_eq!(health_of(&t, id), 40000);
    assert!(t.run_until(1000, |b| b.zombies.get(id).is_some_and(|z| z.phase == ZombiePhase::BossIdle)));
    assert_eq!(t.board.zombies.len(), 1);
    assert_eq!(t.board.waves.progress_meter_width, 0, "the bar starts empty");
}

#[test]
fn plants_cannot_hurt_him_while_his_head_is_up() {
    let mut t = test_board!("5-10");
    let id = with_boss(&mut t);
    potted(&mut t, SeedType::Repeater, 5, 2);
    t.ticks(600);
    assert_eq!(health_of(&t, id), 40000, "the peas bounce off");
    assert!(t.board.projectiles.iter().all(|(_, p)| p.dead || p.rect().x < 700), "and none reach him");
}

#[test]
fn he_lowers_his_head_and_takes_damage() {
    let mut t = test_board!("5-10");
    let id = with_boss(&mut t);
    // A row of repeaters; a different row each time he spits, so all five.
    for row in 0..5 {
        potted(&mut t, SeedType::Repeater, 4, row);
    }
    set_counters(&mut t, None, None, Some(0), None);
    assert!(t.run_until(2500, |b| b.zombies.get(id).is_some_and(|z| z.phase == ZombiePhase::BossHeadIdleBeforeSpit)), "his head comes down");
    assert!(t.run_until(2500, |b| b.zombies.get(id).is_some_and(|z| z.body_health < 40000)), "and the plants hit it");
    // His health is the bar.
    t.board.zombies.get_mut(id).unwrap().body_health = 30000;
    t.tick();
    assert_eq!(t.board.waves.progress_meter_width, 37, "a quarter of his health gone");
}

#[test]
fn the_vehicle_flattens_a_block_of_plants() {
    let mut t = test_board!("5-10");
    let id = with_boss(&mut t);
    for col in 0..8 {
        for row in 0..5 {
            potted(&mut t, SeedType::Wallnut, col, row);
        }
    }
    t.with(|board, effects, res, images, sounds| {
        let mut fx = Fx { effects, res, images, sounds };
        board.zombies.get_mut(id).unwrap().boss_rv_attack(&mut fx);
    });
    let (row, col) = t.board.zombies.get(id).unwrap().boss_target();
    assert!(t.run_until(2500, |b| b.zombies.get(id).is_some_and(|z| z.phase == ZombiePhase::BossIdle)), "he drops it and straightens up");
    for c in 0..8 {
        for r in 0..5 {
            let inside = (row..=row + 1).contains(&r) && (col..=col + 2).contains(&c);
            assert_eq!(squished(&t, c, r), inside, "square {c},{r} with the vehicle at {col},{row}");
        }
    }
    let boss = boss_ref(&t);
    assert!(boss.boss_mode() == 0);
}

#[test]
fn he_stomps_the_back_rows() {
    let mut t = test_board!("5-10");
    let id = with_boss(&mut t);
    for (col, row) in [(6, 0), (6, 1), (6, 2), (7, 3), (7, 4), (3, 1), (4, 3)] {
        potted(&mut t, SeedType::Wallnut, col, row);
    }
    set_counters(&mut t, Some(0), None, None, None);
    assert!(t.run_until(1000, |b| b.zombies.get(id).is_some_and(|z| z.phase == ZombiePhase::BossStomping)));
    let (row, _) = t.board.zombies.get(id).unwrap().boss_target();
    assert!(t.run_until(2500, |b| b.zombies.get(id).is_some_and(|z| z.phase == ZombiePhase::BossIdle)));
    for (col, r) in [(6, 0), (6, 1), (6, 2), (7, 3), (7, 4)] {
        assert_eq!(squished(&t, col, r), (row..=row + 1).contains(&r), "square {col},{r} with the stomp in rows {row} and {}", row + 1);
    }
    assert!(!squished(&t, 3, 1) && !squished(&t, 4, 3), "the front of the lawn is safe");
}

#[test]
fn he_sends_zombies_down_a_row() {
    let mut t = test_board!("5-10");
    let id = with_boss(&mut t);
    set_counters(&mut t, None, None, None, Some(0));
    assert!(t.run_until(2500, |b| b.zombies.len() > 1), "a zombie arrives");
    let (row, _) = t.board.zombies.get(id).unwrap().boss_target();
    let (_, zombie) = t.board.zombies.iter().find(|(zid, _)| *zid != id).unwrap();
    assert_eq!(zombie.zombie_type, ZombieType::Normal, "an ordinary one, to begin with");
    assert_eq!(zombie.row, row);
    assert!(zombie.pos_x > 500.0);
}

#[test]
fn three_bungees_drop_on_three_columns() {
    let mut t = test_board!("5-10");
    let id = with_boss(&mut t);
    for col in 0..8 {
        potted(&mut t, SeedType::Wallnut, col, 2);
    }
    t.with(|board, effects, res, images, sounds| {
        let mut fx = Fx { effects, res, images, sounds };
        board.zombies.get_mut(id).unwrap().boss_bungee_attack(&mut fx);
    });
    assert!(t.run_until(2500, |b| b.zombies.iter().filter(|(_, z)| z.zombie_type == ZombieType::Bungee).count() == 3), "three bungees drop");
    let (_, col) = t.board.zombies.get(id).unwrap().boss_target();
    let mut targets: Vec<_> = t.board.zombies.iter().filter(|(_, z)| z.zombie_type == ZombieType::Bungee).map(|(_, z)| z.target_col).collect();
    targets.sort();
    assert_eq!(targets, vec![col, col + 1, col + 2], "in neighbouring columns");
    assert!(t.run_until(3000, |b| b.zombies.get(id).is_some_and(|z| z.phase == ZombiePhase::BossIdle)), "when they are done he straightens up");
}

/// Gets him to spit and returns the row and kind of the ball.
fn spit(t: &mut TestBoard, id: Id<Zombie>) -> (i32, bool) {
    set_counters(t, None, None, Some(0), None);
    assert!(t.run_until(2500, |b| b.zombies.get(id).is_some_and(|z| z.phase == ZombiePhase::BossHeadIdleBeforeSpit)));
    // He spits when his head has been down for a moment.
    assert!(t.run_until(2500, |b| b.zombies.get(id).is_some_and(|z| z.boss_ball().is_some())), "a ball forms");
    t.board.zombies.get(id).unwrap().boss_ball().unwrap()
}

#[test]
fn a_fireball_flattens_what_it_rolls_over() {
    let mut t = test_board!("5-10");
    let id = with_boss(&mut t);
    for row in 0..5 {
        potted(&mut t, SeedType::Wallnut, 3, row);
        potted(&mut t, SeedType::Wallnut, 6, row);
    }
    let (row, _) = spit(&mut t, id);
    // A flattened plant goes after five seconds, so note them as they happen.
    let mut flattened = std::collections::HashSet::new();
    for _ in 0..4000 {
        for c in [3, 6] {
            for r in 0..5 {
                if squished(&t, c, r) {
                    flattened.insert((c, r));
                }
            }
        }
        if t.board.zombies.get(id).is_some_and(|z| z.boss_ball().is_none()) {
            break;
        }
        t.tick();
    }
    assert!(t.board.zombies.get(id).is_some_and(|z| z.boss_ball().is_none()), "it rolls off the left side");
    let expected: std::collections::HashSet<_> = [(3, row), (6, row)].into_iter().collect();
    assert_eq!(flattened, expected, "only the ball's row, with the ball in row {row}");
}

/// A board where he has just spat a ball of the wanted kind (what he spits
/// is rolled, so boards that roll the other kind are passed over).
fn board_with_ball(want_fire: bool) -> Option<(TestBoard, Id<Zombie>, i32)> {
    for skip in 0..60 {
        let mut t = TestBoard::new("5-10")?;
        t.with(|_, _, _, _, sounds| {
            for _ in 0..skip {
                sounds.rng().next_range(7);
            }
        });
        let id = with_boss(&mut t);
        for row in 0..5 {
            potted(&mut t, SeedType::Wallnut, 2, row);
        }
        let (row, fire) = spit(&mut t, id);
        if fire == want_fire {
            return Some((t, id, row));
        }
    }
    None
}

#[test]
fn an_ice_shroom_puts_out_a_fireball() {
    let mut t_none = None;
    if let Some(found) = board_with_ball(true) {
        t_none = Some(found);
    } else if TestBoard::new("5-10").is_none() {
        eprintln!("skipped: no main.pak");
        return;
    }
    let Some((mut t, id, row)) = t_none else { panic!("never rolled a fireball") };
    potted(&mut t, SeedType::Iceshroom, 0, 0);
    assert!(t.run_until(500, |b| b.zombies.get(id).is_some_and(|z| z.boss_ball().is_none())), "the fireball is out");
    assert!(!squished(&t, 2, row), "before it reached the wall-nut");
}

#[test]
fn a_jalapeno_melts_an_iceball_in_its_row() {
    let mut t_none = None;
    if let Some(found) = board_with_ball(false) {
        t_none = Some(found);
    } else if TestBoard::new("5-10").is_none() {
        eprintln!("skipped: no main.pak");
        return;
    }
    let Some((mut t, id, row)) = t_none else { panic!("never rolled an iceball") };
    // One in another row does nothing to it.
    potted(&mut t, SeedType::Jalapeno, 0, (row + 1) % 5);
    t.ticks(300);
    assert!(t.board.zombies.get(id).is_some_and(|z| z.boss_ball().is_some()), "the iceball rolls on");
    potted(&mut t, SeedType::Jalapeno, 0, row);
    assert!(t.run_until(500, |b| b.zombies.get(id).is_some_and(|z| z.boss_ball().is_none())), "the iceball is gone");
}

#[test]
fn a_roof_cleaner_is_flattened_by_a_fireball() {
    use super::super::lawn_mower::LawnMowerState;
    let mut t = test_board!("5-10", |p| p.purchases[crate::store::StoreItem::RoofCleaner.slot()] = 1);
    assert_eq!(t.board.lawn_mowers.len(), 5);
    let id = with_boss(&mut t);
    let (row, _) = spit(&mut t, id);
    assert!(t.run_until(2000, |b| b.zombies.get(id).is_some_and(|z| z.boss_ball().is_none())));
    for mower in &t.board.lawn_mowers {
        assert_eq!(mower.state == LawnMowerState::Squished, mower.row == row, "the mower in row {}, ball in row {row}", mower.row);
    }
}

#[test]
fn he_goes_down_when_beaten_and_a_trophy_drops() {
    let mut t = test_board!("5-10");
    let id = with_boss(&mut t);
    let other = t.zombie(ZombieType::Normal, 1, 700.0);
    set_counters(&mut t, None, None, Some(0), None);
    assert!(t.run_until(2500, |b| b.zombies.get(id).is_some_and(|z| z.phase == ZombiePhase::BossHeadIdleBeforeSpit)));
    let env = t.board.zombie_env();
    t.with(|board, effects, res, images, sounds| {
        let mut fx = Fx { effects, res, images, sounds };
        board.zombies.get_mut(id).unwrap().take_damage(100_000, 0, &env, &mut fx);
    });
    assert_eq!(health_of(&t, id), 1, "he can't be killed outright");
    assert!(t.run_until(10, |b| b.zombies.get(id).is_some_and(|z| z.phase == ZombiePhase::BossHeadLeave)), "he starts to go");
    assert!(t.run_until(10, |b| b.zombies.get(other).is_none_or(|z| z.is_dead_or_dying())), "and takes the others with him");
    assert!(t.run_until(3000, |b| b.coins.iter().any(|(_, c)| c.ty == CoinType::SilverSunflower)), "the trophy falls out");
    assert!(t.board.has_level_award_dropped());
}

#[test]
fn he_goes_down_when_beaten_while_standing_about() {
    let mut t = test_board!("5-10");
    let id = with_boss(&mut t);
    // Beaten while his head was up (a jalapeno's burn, say).
    let env = t.board.zombie_env();
    t.board.zombies.get_mut(id).unwrap().body_health = 2;
    t.with(|board, effects, res, images, sounds| {
        let mut fx = Fx { effects, res, images, sounds };
        board.zombies.get_mut(id).unwrap().take_damage(5, 0, &env, &mut fx);
    });
    assert_eq!(health_of(&t, id), 1);
    assert!(t.run_until(2500, |b| b.zombies.get(id).is_some_and(|z| z.phase == ZombiePhase::Dying)));
    assert!(t.run_until(3000, |b| b.coins.iter().any(|(_, c)| c.ty == CoinType::SilverSunflower)), "the trophy falls out");
}

#[test]
fn the_last_levels_intro_has_crazy_dave_warn_about_him_and_the_boss_step_in() {
    let Some(mut t) = TestBoard::at_intro("5-10") else {
        eprintln!("skipped: no main.pak");
        return;
    };
    use super::super::cutscene;
    assert_eq!(t.board.scene, super::super::GameScene::LevelIntro);
    assert!(t.board.zombies.is_empty());
    // Dave comes in and talks; the intro waits for the clicks.
    assert!(t.run_until(2000, |b| b.dave.message_index == 2300), "he starts with 'Uh oh, he's here'");
    let mut clicks = 0;
    while t.board.dave.message_index != -1 && clicks < 30 {
        t.with(cutscene::advance_crazy_dave_dialog);
        clicks += 1;
        t.ticks(2);
    }
    assert!((8..30).contains(&clicks), "a talk of about ten lines ({clicks} clicks)");
    // Then the boss stamps in, the lawn gets its pots and play starts.
    assert!(t.run_until(8000, |b| b.boss_zombie().is_some()), "he steps onto the lawn");
    assert_eq!(boss_ref(&t).phase, ZombiePhase::BossEnter);
    assert!(t.run_until(8000, |b| b.scene == super::super::GameScene::Playing), "and play starts");
    let pots = t.board.plants.iter().filter(|(_, p)| p.seed == SeedType::Flowerpot).count();
    assert_eq!(pots, 15, "three columns of five pots");
    assert!(t.board.lawn_mowers.is_empty(), "no mowers on the roof without a cleaner");
    assert!(t.run_until(500, |b| b.boss_zombie().and_then(|id| b.zombies.get(id)).is_some_and(|z| z.phase == ZombiePhase::BossIdle)));
}
