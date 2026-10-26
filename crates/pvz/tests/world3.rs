//! World 3 (pool levels 3-1 to 3-10): which zombies each level sends and
//! which rows they can enter. Tables from the decomp's `gZombieAllowedLevels`
//! (`Lawn/Challenge.cpp`) and `Board::RowCanHaveZombieType`.

use std::sync::Arc;

use crate::audio::MusicTune;
use crate::award::AwardKind;
use crate::board::coin::CoinType;
use crate::board::cutscene;
use crate::board::lawn_mower::{LawnMower, LawnMowerType, MowerHeight};
use crate::board::zombie::{Fx, ZOMBIE_WAVE_CUTSCENE, ZombiePhase};
use crate::board::{BackgroundType, GameScene, PlantRowType};
use crate::particle::ParticleEffect;
use crate::plant::{SeedType, award_seed_for_level};
use crate::player::ProfileMgr;
use crate::store::StoreItem;
use crate::zombie::{NUM_ZOMBIE_TYPES, ZombieType, can_spawn_on_level};

use super::{TestBoard, test_board};

/// The levels (21 to 30) each zombie type can appear on, from the decomp's
/// `gZombieAllowedLevels`.
#[test]
fn pool_levels_allow_their_zombies() {
    let expected = [
        (ZombieType::DuckyTube, vec![]),
        (ZombieType::Snorkel, vec![23, 24, 25, 27, 30]),
        (ZombieType::DolphinRider, vec![28, 29, 30]),
        (ZombieType::Zamboni, vec![26, 27, 29, 30]),
        (ZombieType::Bobsled, vec![26, 27, 29, 30]),
        (ZombieType::Dancer, vec![]),
        (ZombieType::BackupDancer, vec![]),
        (ZombieType::Newspaper, vec![22, 24]),
    ];
    for (ty, levels) in expected {
        let allowed: Vec<i32> = (21..=30).filter(|&level| can_spawn_on_level(ty, level)).collect();
        assert_eq!(allowed, levels, "{ty:?} on levels 21-30");
    }
}

#[test]
fn pool_rows_take_only_swimmers_in_every_wave() {
    let mut t = test_board!("3-1");
    let pool_rows: Vec<i32> = (0..6).filter(|&row| t.board.plant_row[row as usize] == PlantRowType::Pool).collect();
    assert_eq!(pool_rows, [2, 3], "3-1 has its pool in the middle rows");
    for wave in 0..10 {
        t.board.waves.current_wave = wave;
        for i in 0..NUM_ZOMBIE_TYPES {
            let ty = ZombieType::from_index(i);
            for row in 0..6 {
                let allowed = t.board.row_can_have_zombie_type(row, ty);
                if pool_rows.contains(&row) {
                    let swimmer_wave = wave >= 5 || ty.is_pool_only();
                    assert_eq!(allowed, ty.can_go_in_pool() && swimmer_wave, "{ty:?} in pool row {row}, wave {wave}");
                } else {
                    assert!(!(allowed && ty.is_pool_only()), "{ty:?} never on grass (row {row}, wave {wave})");
                }
            }
        }
    }
}

/// A whole level of waves with no plants: zombies that reach the house are
/// taken away, so the level isn't lost before its last wave.
#[test]
fn swimmers_only_spawn_in_water_rows() {
    let mut t = test_board!("3-1").with_waves();
    let pool_rows = [2, 3];
    let num_waves = t.board.waves.num_waves;
    let mut seen = std::collections::HashSet::new();
    for _ in 0..200_000 {
        t.tick();
        let mut gone = Vec::new();
        for (id, z) in t.board.zombies.iter() {
            seen.insert((z.zombie_type, z.row));
            if pool_rows.contains(&z.row) {
                assert!(z.zombie_type.can_go_in_pool(), "{:?} in water row {}", z.zombie_type, z.row);
            } else {
                assert!(!z.zombie_type.is_pool_only(), "{:?} on grass row {}", z.zombie_type, z.row);
            }
            if z.pos_x < 200.0 {
                gone.push(id);
            }
        }
        for id in gone {
            t.board.zombies.free(id);
        }
        if t.board.waves.current_wave >= num_waves {
            break;
        }
    }
    assert!(t.board.waves.current_wave >= num_waves, "the level's waves all came");
    assert!(seen.iter().any(|(ty, _)| *ty == ZombieType::Normal), "zombies came at all");
    assert!(seen.iter().any(|(_, row)| pool_rows.contains(row)), "some zombies came into the water");
}

/// Puts a zombie of `ty` in row 2 at x = 300 (in `phase` if given), calls
/// `walk_into_house` on it the way `Board::zombies_won` does, and returns the
/// lawn's background, the zombie's x before and after, and its phase after.
fn walk_in(mut t: TestBoard, ty: ZombieType, phase: Option<ZombiePhase>) -> (BackgroundType, f32, f32, ZombiePhase) {
    let background = t.board.background;
    let id = t.zombie(ty, 2, 300.0);
    if let Some(phase) = phase {
        t.board.zombies.get_mut(id).expect("placed").phase = phase;
    }
    let before = t.board.zombies.get(id).expect("placed").pos_x;
    t.with(|board, effects, res, images, sounds| {
        let env = board.zombie_env();
        let mut fx = Fx { effects, res, images, sounds };
        board.zombies.get_mut(id).expect("placed").walk_into_house(background, &env, &mut fx);
    });
    let zombie = t.board.zombies.get(id).expect("still on the lawn");
    (background, before, zombie.pos_x, zombie.phase)
}

/// The house offset per lawn: the day lawn leaves the zombie where it was
/// (apart from its y), the pool lawn moves it 80 px left, a football player
/// 10 px left (`Zombie::WalkIntoHouse`).
#[test]
fn walking_into_the_house_shifts_by_lawn() {
    let (day, before, after, _) = walk_in(test_board!("1-1"), ZombieType::Normal, None);
    assert_eq!(day, BackgroundType::Day);
    assert_eq!(after - before, 0.0, "day lawn: no x shift");

    let (pool, before, after, _) = walk_in(test_board!("3-1"), ZombieType::Normal, None);
    assert_eq!(pool, BackgroundType::Pool);
    assert_eq!(after - before, -80.0, "pool lawn: 80 px left");

    let (_, before, after, _) = walk_in(test_board!("3-1"), ZombieType::Football, None);
    assert_eq!(after - before, -10.0, "football on the pool lawn: 10 px left");
}

/// A pole vaulter still running up to a plant is walking on by the time it's
/// in the house, so it gets the ordinary walk and not the pre-vault +35 x.
#[test]
fn pole_vaulter_running_up_walks_into_the_house() {
    let (_, before, after, phase) = walk_in(test_board!("3-1"), ZombieType::Polevaulter, Some(ZombiePhase::PolePreVault));
    assert_eq!(phase, ZombiePhase::Normal, "the vault is over, the zombie walks");
    assert_eq!(after - before, -80.0, "no +35 x for a vaulter; only the pool's -80");
}

/// A normal zombie in a pool row that gets into the house loses the level,
/// and the zombie stands at the pool lawn's house place.
#[test]
fn zombie_in_pool_row_enters_the_house_at_pool_y() {
    let mut t = test_board!("3-1");
    assert_eq!(t.board.background, BackgroundType::Pool, "3-1 is a pool lawn");
    // The row's mower (lawn or cleaner) would reach the zombie first and mow
    // it down, so take it out of the row for this walk to the house.
    t.board.lawn_mowers.retain(|mower| mower.row != 2);
    let id = t.zombie(ZombieType::Normal, 2, -40.0);
    let reached = t.run_until(2_000, |board| board.scene == GameScene::ZombiesWon);
    assert!(reached, "the zombie got into the house and the level was lost");
    let zombie = t.board.zombies.get(id).expect("the winning zombie stays on the lawn");
    assert!(zombie.is_winner());
    assert_eq!(zombie.pos_y, 290.0, "the house's y, the same on the pool lawn");
}

/// The pool's bob (`PlantDrawHeightOffset`): 2 px either way, one cycle per
/// 200 ticks, and each square's wave out of step with its neighbours.
#[test]
fn pool_bob_is_small_periodic_and_differs_by_square() {
    use crate::board::plant::pool_bob_offset;
    for row in 0..5 {
        for col in 0..9 {
            for counter in 0..400 {
                let offset = pool_bob_offset(counter, col, row);
                assert!(offset.abs() <= 2.0 + 1e-4, "{offset} at ({col}, {row}) tick {counter}");
                let again = pool_bob_offset(counter + 200, col, row);
                assert!((offset - again).abs() < 1e-3, "one cycle is 200 ticks");
            }
        }
    }
    // The squares' waves aren't in step: at the same tick they differ.
    assert!((0..20).any(|c| (pool_bob_offset(c, 2, 2) - pool_bob_offset(c, 3, 2)).abs() > 0.5));
    assert!((0..20).any(|c| (pool_bob_offset(c, 2, 2) - pool_bob_offset(c, 2, 3)).abs() > 0.5));
}

/// A plant on water bobs as the board runs; one on the lawn doesn't move.
#[test]
fn plants_on_water_bob_and_plants_on_grass_stay_put() {
    use crate::board::plant::pool_bob_offset;
    let mut t = test_board!("3-1");
    let on_water = t.plant(SeedType::Lilypad, 3, 2);
    let on_grass = t.plant(SeedType::Peashooter, 3, 0);
    let mut seen = Vec::new();
    for _ in 0..60 {
        t.tick();
        let water = t.plant_ref(on_water).expect("the lily pad lives");
        let counter = t.board.main_counter;
        // The plant's bob is worked out in its update, so it may be a tick behind.
        let (col, row) = (water.col, water.row);
        let expected = [counter, counter - 1].map(|c| pool_bob_offset(c, col, row));
        assert!(expected.iter().any(|e| (e - water.bob).abs() < 1e-3), "bob {} vs {expected:?}", water.bob);
        assert_eq!(t.plant_ref(on_grass).expect("the peashooter lives").bob, 0.0);
        seen.push(water.bob);
    }
    let (low, high) = seen.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &b| (lo.min(b), hi.max(b)));
    assert!(high - low > 1.0, "the lily pad moves up and down ({low}..{high})");
}

/// Whether a particle effect of this kind is running on the board.
fn running(t: &mut TestBoard, effect: ParticleEffect) -> bool {
    let definition = t.with(|_, _, res, images, _| res.particle_definition(effect.file(), images).expect("the effect loads"));
    t.effects.particle_ids().any(|(_, system)| Arc::ptr_eq(&system.definition, &definition))
}

/// Planting on water starts the pool splash (`PlantingPool`), and on the
/// lawn the dirt puff (`Planting`).
#[test]
fn planting_on_water_splashes() {
    let mut t = test_board!("3-1");
    assert!(!running(&mut t, ParticleEffect::PlantingPool), "no splash before planting");
    t.plant(SeedType::Lilypad, 3, 2);
    assert!(running(&mut t, ParticleEffect::PlantingPool), "planting on water splashes");
    assert!(!running(&mut t, ParticleEffect::Planting), "and no dirt puff");
}

#[test]
fn planting_on_the_lawn_kicks_up_dirt() {
    let mut t = test_board!("3-1");
    t.plant(SeedType::Peashooter, 3, 0);
    assert!(running(&mut t, ParticleEffect::Planting), "planting on the lawn kicks up dirt");
    assert!(!running(&mut t, ParticleEffect::PlantingPool), "but no splash");
}

/// The water moves on every tick of a pool level, and its caustic light
/// changes with it. Day levels have no water.
#[test]
fn day_lawns_have_no_pool_water() {
    let t = test_board!("1-1");
    assert!(t.board.pool_water.is_none(), "day lawns have no pool");
}

#[test]
fn pool_water_moves_each_tick() {
    let mut t = test_board!("3-1");
    let water = t.board.pool_water.as_ref().expect("3-1 has a pool");
    let start = water.counter();
    let caustic = water.caustic().clone();
    let alpha = |t: &mut TestBoard| {
        t.with(|_, _, _, images, _| {
            images.get(&caustic).and_then(|image| image.data.clone()).expect("the caustic picture has data")
        })
    };
    let before = alpha(&mut t);
    t.ticks(3);
    let water = t.board.pool_water.as_ref().expect("still a pool");
    assert_eq!(water.counter(), start + 3);
    let after = alpha(&mut t);
    assert_ne!(before, after, "the caustic light moves");
}

/// The lawn mower in `row`, if there is one.
fn mower_in_row(t: &TestBoard, row: i32) -> Option<&LawnMower> {
    t.board.lawn_mowers.iter().find(|m| m.row == row)
}

/// With the pool cleaner bought, the pool rows get one and the lawn rows
/// keep their lawn mowers.
#[test]
fn pool_rows_get_a_pool_cleaner() {
    let t = test_board!("3-1", |p| p.purchases[StoreItem::PoolCleaner.slot()] = 1);
    assert_eq!(t.board.lawn_mowers.len(), 6, "one mower for each of the six rows");
    for mower in &t.board.lawn_mowers {
        let pool_row = t.board.plant_row[mower.row as usize] == crate::board::PlantRowType::Pool;
        let expected = if pool_row { LawnMowerType::Pool } else { LawnMowerType::Lawn };
        assert_eq!(mower.kind, expected, "the mower in row {}", mower.row);
    }
}

/// Runs the mower in `row` (triggered by a zombie at x = 0) until it's gone,
/// and returns the last x it was seen at before it went. The mower is removed
/// on the tick it dies, so that's the x it died from.
fn last_x_before_gone(t: &mut TestBoard, row: i32) -> f32 {
    t.zombie(ZombieType::Normal, row, 0.0);
    let mut last_x = None;
    for _ in 0..2000 {
        let Some(mower) = mower_in_row(t, row) else { break };
        last_x = Some(mower.pos_x);
        t.tick();
    }
    assert!(mower_in_row(t, row).is_none(), "the mower in row {row} went");
    last_x.expect("the mower was there to start with")
}

/// Without the cleaner, a pool row still gets a lawn mower (the decomp's
/// `LawnMowerInitialize` only swaps in the cleaner when it's bought). That
/// mower drives into the water and dies there: it's gone at x 50, not at the
/// far end of the board.
#[test]
fn pool_row_without_cleaner_gets_a_lawn_mower() {
    let mut t = test_board!("3-1");
    let mower = mower_in_row(&t, 2).expect("row 2 has a mower");
    assert_eq!(mower.kind, LawnMowerType::Lawn);
    let last_x = last_x_before_gone(&mut t, 2);
    // The sink check is `pos_x > 50` after each move, so the last x before it
    // dies is just under 50 (a step is at most 3.33 px).
    assert!(last_x > 46.0 && last_x <= 50.0, "the mower sank at x {last_x}, not at the far end");
}

/// The day lawn's mower in the same position runs on across the board: it
/// dies at the far edge, well past where the pool sinks a mower.
#[test]
fn day_lawn_mower_runs_past_the_pool() {
    let mut t = test_board!("1-6");
    let last_x = last_x_before_gone(&mut t, 0);
    assert!(last_x > 700.0 && last_x <= 800.0, "the day mower ran to the far edge, last seen at {last_x}");
}

/// Ticks a mower in `row` after its chomp is over and returns how many px it
/// moves per tick.
fn cruise_speed(mut t: TestBoard, row: i32) -> f32 {
    t.zombie(ZombieType::Normal, row, 0.0);
    let triggered = crate::board::lawn_mower::LawnMowerState::Triggered;
    assert!(t.run_until(100, |b| b.lawn_mowers.iter().any(|m| m.row == row && m.state == triggered)), "a zombie triggers it");
    // The chomp slows the start of the run; 60 ticks is well past it.
    t.ticks(60);
    let before = mower_in_row(&t, row).expect("still running").pos_x;
    t.ticks(100);
    let after = mower_in_row(&t, row).expect("still running").pos_x;
    (after - before) / 100.0
}

/// The pool cleaner moves 2.5 px a tick and a lawn mower 3.33.
#[test]
fn pool_cleaner_is_slower_than_a_lawn_mower() {
    let cleaner = cruise_speed(test_board!("3-1", |p| p.purchases[StoreItem::PoolCleaner.slot()] = 1), 2);
    assert!((cleaner - 2.5).abs() < 0.01, "the cleaner moves {cleaner} px a tick");
    let lawn = cruise_speed(test_board!("3-1"), 0);
    assert!((lawn - 3.33).abs() < 0.01, "the lawn mower moves {lawn} px a tick");
}

/// The pool cleaner eats the first zombie in its row (even in the water),
/// sinks into the pool, comes out past it and runs off the far end. It
/// doesn't come back for a zombie placed later in the row.
#[test]
fn pool_cleaner_clears_the_row_once() {
    let mut t = test_board!("3-1", |p| p.purchases[StoreItem::PoolCleaner.slot()] = 1);
    let first = t.zombie(ZombieType::Normal, 2, 0.0);
    assert!(t.run_until(100, |b| b.zombies.get(first).is_none_or(|z| z.is_dead_or_dying())), "the cleaner eats the zombie");
    assert!(t.zombie_ref(first).is_none(), "and the zombie is gone");
    assert!(mower_in_row(&t, 2).is_some_and(|m| m.state == crate::board::lawn_mower::LawnMowerState::Triggered), "it's running");

    // Its height goes down into the water, then up again past the pool, and
    // it isn't lost in the water on the way.
    let mut heights = Vec::new();
    let mut ticks = 0;
    while let Some(mower) = mower_in_row(&t, 2) {
        if heights.last() != Some(&mower.height) {
            heights.push(mower.height);
        }
        assert!(!mower.dead);
        assert!(ticks < 2000, "the cleaner runs off the far end");
        t.tick();
        ticks += 1;
    }
    assert_eq!(heights, [MowerHeight::Land, MowerHeight::DownToPool, MowerHeight::InPool, MowerHeight::UpToLand, MowerHeight::Land]);

    // It's gone, so the next zombie in the row is not mowed down.
    let later = t.zombie(ZombieType::Normal, 2, 300.0);
    t.ticks(50);
    assert!(t.zombie_ref(later).is_some(), "the cleaner runs once per level");
}

/// The pool cleaner passes over a zombie that a tangle kelp is dragging under
/// (`LawnMower::Update`, `IsTangleKelpTarget`): it's the first zombie in the
/// row, but it isn't mowed; without the kelp's hold it is.
#[test]
fn pool_cleaner_passes_over_a_zombie_the_kelp_holds() {
    let mut t = test_board!("3-1", |p| p.purchases[StoreItem::PoolCleaner.slot()] = 1);
    // Its rectangle (x 6 to 48) overlaps the cleaner's (x -21 to 29).
    let zombie = t.zombie(ZombieType::Normal, 2, -30.0);
    let run_mower = |t: &mut TestBoard, held: &[sexy::data_array::Id<crate::board::zombie::Zombie>]| {
        t.with(|board, effects, res, images, sounds| {
            let mut fx = Fx { effects, res, images, sounds };
            let mower = board.lawn_mowers.iter_mut().find(|m| m.row == 2).expect("row 2 has a cleaner");
            mower.update(true, |_, _| 0.0, &mut board.zombies, held, board.waves.current_wave, &mut board.waves.wave_row_got_lawn_mowered, &mut fx);
        });
    };
    run_mower(&mut t, &[zombie]);
    assert!(t.zombie_ref(zombie).is_some_and(|z| !z.is_dead_or_dying()), "the held zombie is left alone");
    assert!(mower_in_row(&t, 2).is_some_and(|m| m.state == crate::board::lawn_mower::LawnMowerState::Ready), "and the cleaner isn't triggered");

    run_mower(&mut t, &[]);
    assert!(t.zombie_ref(zombie).is_none_or(|z| z.is_dead_or_dying()), "without the kelp's hold the cleaner takes it");
}

/// Crazy Dave's talks at the start of the pool. Level 21 (3-1) talks on a
/// first visit only; 3-5 (level 25, Little Trouble) talks on every visit.
/// Behaviour per ruslan831 decomp, CutScene::StartLevelIntro.
#[test]
fn crazy_dave_talks_at_the_start_of_the_pool() {
    let mut t = test_board!("3-1");
    assert_eq!(cutscene::crazy_dave_dialog_start(&t.board), 501, "3-1: the zombies attack the backyard");
    t.board.first_time_adventure = false;
    assert_eq!(cutscene::crazy_dave_dialog_start(&t.board), -1, "3-1 replayed: no talk");
    t.board.first_time_adventure = true;
    t.board.player_coins = 0;
    for level in (22..=24).chain(26..=30) {
        t.board.level = level;
        assert_eq!(cutscene::crazy_dave_dialog_start(&t.board), -1, "level {level}: no talk on a first visit");
    }
    t.board.level = 25;
    assert_eq!(cutscene::crazy_dave_dialog_start(&t.board), 701, "3-5 on a first visit");
    t.board.first_time_adventure = false;
    assert_eq!(cutscene::crazy_dave_dialog_start(&t.board), 701, "3-5 replayed");
}

/// Big Trouble Little Zombie is 3-5 (adventure level 25): a pool lawn that
/// runs a conveyor belt, not a seed chooser, and plays the loonboon tune.
#[test]
fn little_trouble_is_3_5_on_a_conveyor_belt() {
    let t = test_board!("3-5");
    assert!(t.board.is_little_trouble_level());
    assert!(!t.board.is_mini_boss_level());
    assert_eq!(t.board.level, 25);
    assert_eq!(t.board.background, BackgroundType::Pool);
    assert!(t.board.has_conveyor_belt_seed_bank());
    assert!(t.board.seed_bank.conveyor);
    assert!(!t.board.choose_seeds_on_current_level());
    assert_eq!(t.board.game_music(), MusicTune::MinigameLoonboon);
    drop(t);
    let plain = TestBoard::new("3-1").expect("the game is installed");
    assert!(!plain.board.is_little_trouble_level());
    assert!(!plain.board.has_conveyor_belt_seed_bank());
}

/// The belt brings only its four plants (`Challenge::UpdateConveyorBelt`):
/// lily pad, wall-nut, peashooter and cherry bomb.
#[test]
fn little_trouble_belt_brings_its_four_plants() {
    let mut t = test_board!("3-5");
    for _ in 0..60 {
        t.board.conveyor_belt_counter = 0;
        t.with(|board, _, _, _, sounds| board.update_conveyor_belt(sounds.rng()));
    }
    let bank = &t.board.seed_bank;
    let on_belt = bank.num_seeds_on_conveyor_belt();
    let four = [SeedType::Lilypad, SeedType::Wallnut, SeedType::Peashooter, SeedType::Cherrybomb];
    let counted: usize = four.iter().map(|&seed| bank.count_of_type_on_conveyor_belt(seed)).sum();
    assert!(on_belt > 0, "packets ride in");
    assert_eq!(counted, on_belt, "every packet on the belt is one of the four");
    assert_eq!(four.map(|s| bank.count_of_type_on_conveyor_belt(s)).iter().filter(|&&n| n > 0).count(), 4,
        "all four come up over 60 packets");
}

/// The wave timing and size on Little Trouble (`Challenge::StartLevel`,
/// `Board::PickZombieWaves`, `Board::UpdateZombieSpawning`): the first wave
/// comes after 200 ticks, the first wave is worth four points of zombies,
/// and each later wave after 750 ticks.
#[test]
fn little_trouble_waves_come_quickly_and_are_worth_four_times_more() {
    let mut t = test_board!("3-5").with_waves();
    assert_eq!(t.board.waves.zombie_count_down, 200, "the first wave in 200 ticks");
    let points: i32 = t.board.waves.zombies_in_wave[0].iter().map(|ty| ty.definition().zombie_value).sum();
    assert_eq!(points, 4, "wave 1: one point, times four");
    assert!(t.run_until(1000, |b| b.waves.current_wave >= 1), "the first wave comes");
    assert_eq!(t.board.waves.zombie_count_down, 750, "the next wave is 750 ticks off");
}

/// Little Trouble's waves only send the zombies its table allows, and the
/// final wave has at least one of each (`PutInMissingZombies`).
#[test]
fn little_trouble_sends_only_its_zombies() {
    let t = test_board!("3-5");
    let allowed: Vec<ZombieType> = (0..NUM_ZOMBIE_TYPES)
        .map(ZombieType::from_index)
        .filter(|ty| t.board.waves.zombie_allowed[*ty as usize])
        .collect();
    // The levels table for 3-5 is exactly these four. Flags aren't in the
    // table: flag waves add them (`PutZombieInWave`), so they're the only
    // zombies outside the table that may come.
    let expected = [ZombieType::Normal, ZombieType::TrafficCone, ZombieType::Football, ZombieType::Snorkel];
    assert_eq!(allowed, expected, "the zombies 3-5's table allows");
    let flags: Vec<i32> = t.board.waves.zombies_in_wave.iter().enumerate()
        .filter(|(_, wave)| wave.contains(&ZombieType::Flag)).map(|(i, _)| i as i32).collect();
    assert!(!flags.is_empty(), "flag waves come on 3-5");
    for (i, wave) in t.board.waves.zombies_in_wave.iter().enumerate() {
        assert_eq!(wave.contains(&ZombieType::Flag), t.board.is_flag_wave(i as i32), "flag zombies only on flag waves (wave {i})");
    }
    assert!(!allowed.contains(&ZombieType::Pail), "no buckets on 3-5");
    assert!(!allowed.contains(&ZombieType::DolphinRider), "no dolphin riders on 3-5");
    // Flag waves also bring the flag zombie, which isn't in the table.
    for wave in &t.board.waves.zombies_in_wave {
        for ty in wave.iter().filter(|ty| **ty != ZombieType::Flag) {
            assert!(allowed.contains(ty), "{ty:?} is not allowed on 3-5");
        }
    }
    let last = t.board.waves.zombies_in_wave.last().expect("waves");
    for ty in &allowed {
        assert!(last.contains(ty), "the final wave has a {ty:?}");
    }
}

/// Little Trouble's zombies are half size with a quarter of their health
/// (`Zombie::ZombieInitialize`); the zombies on a normal level keep theirs.
#[test]
fn little_trouble_zombies_are_half_size_with_a_quarter_health() {
    let mut t = test_board!("3-5");
    let normal = t.zombie(ZombieType::Normal, 0, 500.0);
    let cone = t.zombie(ZombieType::TrafficCone, 1, 500.0);
    let z = t.zombie_ref(normal).expect("placed");
    assert_eq!(z.scale, 0.5);
    assert_eq!(z.body_health, 270 / 4);
    let c = t.zombie_ref(cone).expect("placed");
    assert_eq!(c.scale, 0.5);
    assert_eq!(c.body_health, 270 / 4);
    assert_eq!(c.helm_health(), 370 / 4, "the cone's helmet too");
    // One level at a time: a board holds the game's resources for its lifetime.
    drop(t);

    let mut plain = test_board!("3-1");
    let id = plain.zombie(ZombieType::Normal, 0, 500.0);
    let z = plain.zombie_ref(id).expect("placed");
    assert_eq!(z.scale, 1.0);
    assert_eq!(z.body_health, 270);
}

/// Tiny zombies walk at the normal pace (`Zombie::ZombieInitialize` sets
/// `mScaleZombie` before `UpdateAnimSpeed`): the animation runs twice as fast
/// as on a full-size zombie (`mVelX * ... / mScaleZombie`), and the walk's
/// speed, which reads the animation's rate, is scaled back down by the same
/// half (`Zombie::UpdateZombieWalking`). Over 400 ticks, the window spans
/// several walk cycles so the phase of the cycle doesn't matter.
#[test]
fn little_trouble_zombies_walk_at_normal_pace() {
    fn walked(mut t: TestBoard) -> (f32, f32) {
        let id = t.zombie(ZombieType::Normal, 0, 600.0);
        t.ticks(400);
        let z = t.zombie_ref(id).expect("still on the lawn");
        (600.0 - z.pos_x, z.body_anim_rate(&t.effects))
    }
    let (full, full_rate) = walked(test_board!("3-1"));
    let (tiny, tiny_rate) = walked(test_board!("3-5"));
    assert!((tiny_rate / full_rate - 2.0).abs() < 1e-3, "the tiny zombie's animation rate is doubled ({tiny_rate} vs {full_rate})");
    assert!((full / tiny - 1.0).abs() < 0.05, "3-1 walked {full} px, 3-5 walked {tiny} px in 400 ticks");
}

/// Money from Little Trouble's zombies drops from only one in four
/// (`Zombie::DropLoot`): the same zombies on 3-1 drop about four times as
/// much.
#[test]
fn little_trouble_drops_money_from_one_zombie_in_four() {
    fn coins_from(mut t: TestBoard, zombies: usize) -> usize {
        for i in 0..zombies {
            let id = t.zombie(ZombieType::Normal, (i % 5) as i32, 300.0);
            t.board.zombies.get_mut(id).expect("placed").loot_pending = true;
        }
        let before = t.board.coins.len();
        t.with(|board, effects, res, images, sounds| board.collect_zombie_loot(effects, res, images, sounds));
        t.board.coins.len() - before
    }
    let plain = coins_from(test_board!("3-1"), 200);
    let tiny = coins_from(test_board!("3-5"), 200);
    assert!(plain >= 8, "the normal level drops money ({plain})");
    assert!(tiny * 2 < plain, "3-5 drops {tiny} pieces against {plain} on 3-1");
}

/// The pool street's one ducky tube: exactly one, placed as the reference's
/// `PlaceAZombie` places it. Behaviour per ruslan831 decomp,
/// CutScene::PlaceStreetZombies and CutScene::PlaceAZombie.
#[test]
fn pool_street_has_one_ducky_tube_at_the_decomp_spot() {
    let mut t = test_board!("3-1");
    t.with(|board, effects, res, images, sounds| {
        let mut fx = Fx { effects, res, images, sounds };
        board.place_street_zombies(&mut fx);
    });
    let ducks: Vec<(i32, f32, f32)> = t
        .board
        .zombies
        .iter()
        .filter(|(_, z)| z.from_wave == ZOMBIE_WAVE_CUTSCENE && z.zombie_type == ZombieType::DuckyTube)
        .map(|(_, z)| (z.row, z.pos_x, z.pos_y))
        .collect();
    assert_eq!(ducks.len(), 1, "one ducky tube on the street");
    let (row, x, y) = ducks[0];
    assert!((0..5).contains(&row), "in grid row {row}");
    // Column from x: gridX * 56 + 830 plus 0-14 of jitter.
    let grid_x = ((x - 830.0) / 56.0).floor() as i32;
    assert!((1..=4).contains(&grid_x), "in grid column {grid_x}");
    let x_jitter = x - (grid_x * 56 + 830) as f32;
    assert!((0.0..15.0).contains(&x_jitter), "x jitter {x_jitter}");
    // Row y: gridY * 90 + 70, 30 lower on odd columns, plus 0-14 of jitter.
    let base_y = (row * 90 + 70) as f32 + if grid_x % 2 == 1 { 30.0 } else { 0.0 };
    let y_jitter = y - base_y;
    assert!((0.0..15.0).contains(&y_jitter), "y jitter {y_jitter}");
}

/// The lily pad's weight on Little Trouble's belt (`Challenge::UpdateConveyorBelt`):
/// its table value of 25 falls to 1 over 18 planted or on the belt, and the
/// belt's other rules apply with more than two choices.
#[test]
fn little_trouble_belt_weights_follow_the_decomp() {
    use crate::board::conveyor::{belt_seeds, belt_weights};
    let t = test_board!("3-5");
    let seeds = belt_seeds(&t.board);
    let weight_of = |picks: &[(SeedType, i32)], seed: SeedType| picks.iter().find(|(s, _)| *s == seed).map(|&(_, w)| w).expect("on the belt");
    let weights = |planted: i32, in_bank: i32, last: Option<SeedType>| {
        belt_weights(seeds, |seed| if seed == SeedType::Lilypad { planted } else { 0 }, |seed| if seed == SeedType::Lilypad { in_bank } else { 0 }, 0, last)
    };
    // TodAnimateCurve(0, 18, n, 25, 1, linear): 25 at none, 13 at nine, 1 from 18 on.
    assert_eq!(weight_of(&weights(0, 0, None), SeedType::Lilypad), 25);
    assert_eq!(weight_of(&weights(9, 0, None), SeedType::Lilypad), 13);
    assert_eq!(weight_of(&weights(4, 2, None), SeedType::Lilypad), 17, "planted and on the belt count together (6 of 18)");
    assert_eq!(weight_of(&weights(18, 0, None), SeedType::Lilypad), 1);
    assert_eq!(weight_of(&weights(30, 0, None), SeedType::Lilypad), 1, "it never goes below 1");

    // With more than two choices: four on the belt is weight 1, three is 5, and
    // otherwise repeating the last pick halves its weight.
    let peashooter = |planted: i32, in_bank: i32, last| {
        belt_weights(
            seeds,
            |_| planted,
            |seed| if seed == SeedType::Peashooter { in_bank } else { 0 },
            0,
            last,
        )
    };
    assert_eq!(weight_of(&peashooter(0, 4, None), SeedType::Peashooter), 1);
    assert_eq!(weight_of(&peashooter(0, 3, None), SeedType::Peashooter), 5);
    assert_eq!(weight_of(&peashooter(0, 2, Some(SeedType::Peashooter)), SeedType::Peashooter), 12, "25 halved for repeating");
    assert_eq!(weight_of(&peashooter(0, 2, None), SeedType::Peashooter), 25);
    // Wall-nut bowling's belt has two choices, so no rules: a repeat isn't halved.
    let two = [(SeedType::Wallnut, 85), (SeedType::ExplodeONut, 15)];
    let picks = belt_weights(&two, |_| 0, |_| 4, 0, Some(SeedType::Wallnut));
    assert_eq!(picks, two.to_vec());
}


/// 3-10 (adventure level 30), the pool's conveyor mini boss: its belt brings
/// the pool's plants and no others, with the weights of
/// `Challenge::UpdateConveyorBelt`, and the belt runs at once.
/// Behaviour per ruslan831 decomp, Challenge::UpdateConveyorBelt and StartLevel.
#[test]
fn third_mini_boss_belt_has_its_pool_plants() {
    use crate::board::conveyor::belt_seeds;
    let mut t = test_board!("3-10");
    assert!(t.board.is_mini_boss_level());
    assert!(t.board.has_conveyor_belt_seed_bank());
    assert!(t.board.seed_bank.conveyor);
    let seeds = belt_seeds(&t.board);
    let listed: Vec<_> = seeds.iter().map(|&(seed, _)| seed).collect();
    assert_eq!(
        listed,
        [
            SeedType::Lilypad,
            SeedType::Squash,
            SeedType::Threepeater,
            SeedType::Tanglekelp,
            SeedType::Jalapeno,
            SeedType::Spikeweed,
            SeedType::Torchwood,
            SeedType::Tallnut,
        ]
    );
    assert_eq!(seeds.iter().map(|&(_, w)| w).sum::<i32>(), 100, "the table's weights");
    for _ in 0..200 {
        t.board.conveyor_belt_counter = 0;
        t.with(|board, _, _, _, sounds| board.update_conveyor_belt(sounds.rng()));
    }
    let bank = &t.board.seed_bank;
    let on_belt = bank.num_seeds_on_conveyor_belt();
    let counted: usize = listed.iter().map(|&seed| bank.count_of_type_on_conveyor_belt(seed)).sum();
    assert!(on_belt > 0, "packets ride in");
    assert_eq!(counted, on_belt, "every packet on the belt is one of the pool's plants");
    assert!(listed.iter().filter(|&&seed| bank.count_of_type_on_conveyor_belt(seed) > 0).count() >= 6, "most come up over 200 packets");
}

/// 3-10 has no Crazy Dave talk, on a first visit or a replay.
#[test]
fn third_mini_boss_has_no_crazy_dave_talk() {
    let mut t = test_board!("3-10");
    assert_eq!(cutscene::crazy_dave_dialog_start(&t.board), -1, "first visit");
    t.board.first_time_adventure = false;
    assert_eq!(cutscene::crazy_dave_dialog_start(&t.board), -1, "replayed");
}

/// Mini-boss waves (`Board::PickZombieWaves`): the first wave comes after a
/// second and is worth three times the usual points, and a replayed mini-boss
/// level keeps its own thirty waves, where a replayed pool level gets twenty.
#[test]
fn third_mini_boss_waves_come_fast_and_keep_their_count_on_a_replay() {
    let mut t = test_board!("3-10").with_waves();
    assert_eq!(t.board.waves.zombie_count_down, 100, "the first wave in a second");
    assert_eq!(t.board.waves.num_waves, 30, "first visit: the table's thirty");
    let points: i32 = t.board.waves.zombies_in_wave[0].iter().map(|ty| ty.definition().zombie_value).sum();
    assert_eq!(points, 3, "wave 1: one point, times three");

    t.board.first_time_adventure = false;
    t.with(|board, _, _, _, sounds| board.init_zombie_waves(sounds.rng()));
    assert_eq!(t.board.waves.num_waves, 30, "replayed: still thirty");

    // One board at a time: a TestBoard holds the shared resources.
    drop(t);
    let mut pool = test_board!("3-1");
    assert_eq!(pool.board.waves.num_waves, 10, "3-1's table has ten waves");
    pool.board.first_time_adventure = false;
    pool.with(|board, _, _, _, sounds| board.init_zombie_waves(sounds.rng()));
    assert_eq!(pool.board.waves.num_waves, 20, "replayed pool levels get twenty");
}

/// Finishing 3-10 gives an award screen: its award is the level's seed
/// packet (`award_seed_for_level(30)`), not a note (the notes come at 3-9,
/// board level 29, when the player reaches level 30).
#[test]
fn finishing_3_10_has_an_award() {
    use crate::award::AwardScreen;
    use crate::plant::{SeedType, award_seed_for_level};
    assert!(AwardScreen::new(31).is_some(), "the award for finishing 3-10");
    assert_eq!(SeedType::from_index(award_seed_for_level(30)), Some(SeedType::Seashroom));
    assert!(AwardScreen::new(30).is_some(), "3-9's note");
}

/// A gravebuster is only on the mini-boss belt while there are more grave
/// stones than gravebusters (`Challenge::UpdateConveyorBelt`). 2-10 has graves.
#[test]
fn gravebusters_need_graves_to_come_up() {
    use crate::board::conveyor::{belt_weights, mini_boss_seeds};
    let seeds = mini_boss_seeds(20);
    let weight_of = |graves: i32, planted: i32| {
        let picks = belt_weights(seeds, |seed| if seed == SeedType::Gravebuster { planted } else { 0 }, |_| 0, graves, None);
        picks.iter().find(|(seed, _)| *seed == SeedType::Gravebuster).map(|&(_, w)| w).expect("on the belt")
    };
    assert_eq!(weight_of(2, 1), 20, "two graves, one gravebuster");
    assert_eq!(weight_of(2, 2), 0, "as many gravebusters as graves");
    assert_eq!(weight_of(0, 0), 0, "no graves");
}

/// Each pool level, in order (3-1 is adventure level 21).
const POOL_STAGES: [&str; 10] = ["3-1", "3-2", "3-3", "3-4", "3-5", "3-6", "3-7", "3-8", "3-9", "3-10"];

/// Ticks a whole pool level may take (3-10 has thirty waves).
const LEVEL_TICK_CAP: u32 = 400_000;

/// The defence the audit plants: on every row a wall-nut at the front
/// (column 7) and repeaters behind it; on the pool rows each square from
/// column 1 has a lily pad under it, so the repeaters stand on the pads. Planted straight onto the lawn, so the seed
/// bank and sun don't matter: the audit tests the level, not the economy.
fn audit_defence(t: &mut TestBoard) {
    // Pool lawns have six lanes (`Board::InitPlantRows`): the sixth is grass.
    for row in 0..6 {
        if t.board.plant_row[row as usize] == PlantRowType::Pool {
            for col in 1..=7 {
                t.plant(SeedType::Lilypad, col, row);
            }
        }
        for col in 1..=6 {
            t.plant(SeedType::Repeater, col, row);
        }
        t.plant(SeedType::Wallnut, 7, row);
    }
}

/// The award coin each pool level drops (`Zombie::TrySpawnLevelAward`):
/// keys on level 24, a note on 29, and the plant packet otherwise.
fn expected_award_coin(level: i32) -> CoinType {
    match level {
        24 => CoinType::CarKeys,
        29 => CoinType::Note,
        _ => CoinType::FinalSeedPacket,
    }
}

/// Plays each pool level with the audit defence until its last wave is
/// cleared, collects the award and checks the profile moves on. Every level
/// is one `TestBoard`, dropped before the next.
#[test]
fn every_pool_level_can_be_won() {
    for stage in POOL_STAGES {
        let mut t = test_board!(stage).with_waves();
        audit_defence(&mut t);
        let level = t.board.level;
        let num_waves = t.board.waves.num_waves;

        let dropped = t.run_until(LEVEL_TICK_CAP, |b| b.has_level_award_dropped() || b.scene == GameScene::ZombiesWon);
        assert!(t.board.scene != GameScene::ZombiesWon, "{stage}: a zombie got into the house");
        assert!(dropped, "{stage}: the last wave ({num_waves} waves) was not cleared in time");
        assert!(t.board.waves.current_wave >= num_waves, "{stage}: every wave came");

        let (ty, award_seed, x, y) = t
            .board
            .coins
            .iter()
            .find(|(_, c)| c.is_level_award() && !c.dead)
            .map(|(_, c)| (c.ty, c.award_seed, c.pos_x, c.pos_y))
            .unwrap_or_else(|| panic!("{stage}: the award coin is on the lawn"));
        assert_eq!(ty, expected_award_coin(level), "{stage}: the award coin");
        if ty == CoinType::FinalSeedPacket {
            assert_eq!(award_seed, SeedType::from_index(award_seed_for_level(level)), "{stage}: the packet's plant");
        }

        // The player clicks the award: the level fades out and is won.
        t.click(x as i32 + 20, y as i32 + 20);
        assert!(t.run_until(2_000, |b| b.level_complete), "{stage}: the level is won once the award is collected");

        let mut profiles = ProfileMgr::new(None);
        profiles.add_profile("Auditor");
        profiles.set_current(Some("Auditor"));
        profiles.player_mut().expect("the player").level = level;
        t.board.finish_level(&mut profiles);
        assert_eq!(profiles.player().map(|p| p.level), Some(level + 1), "{stage}: the player moves on");
        assert!(AwardKind::for_level(level + 1).is_some(), "{stage}: there is an award screen");
    }
}

/// Finishing each pool level shows the prize `AwardKind::for_level` gives for
/// the player's next level (`LawnApp::ShowAwardScreen`, `AwardScreen`):
/// plants from 3-1 to 3-8 except 3-4, which gives car keys, a zombie's note
/// on 3-9 and the seashroom packet on 3-10.
/// Behaviour per ruslan831 decomp, Zombie::TrySpawnLevelAward and LawnApp::GetAwardSeedForLevel.
#[test]
fn finishing_a_pool_level_awards_the_right_prize() {
    let expected = [
        (21, AwardKind::Seed(SeedType::Squash)),
        (22, AwardKind::Seed(SeedType::Threepeater)),
        (23, AwardKind::Seed(SeedType::Tanglekelp)),
        (24, AwardKind::Keys),
        (25, AwardKind::Seed(SeedType::Jalapeno)),
        (26, AwardKind::Seed(SeedType::Spikeweed)),
        (27, AwardKind::Seed(SeedType::Torchwood)),
        (28, AwardKind::Seed(SeedType::Tallnut)),
        (29, AwardKind::Note(3)),
        (30, AwardKind::Seed(SeedType::Seashroom)),
    ];
    for (level, prize) in expected {
        // The award is for the level the player moves on to.
        assert_eq!(AwardKind::for_level(level + 1), Some(prize), "finishing level {level}");
    }
    // The packet on the lawn is the same plant the award screen shows.
    for (level, prize) in expected {
        if let AwardKind::Seed(seed) = prize {
            assert_eq!(SeedType::from_index(award_seed_for_level(level)), Some(seed), "level {level}'s packet");
        }
    }
}

/// A pool square is planted by clicking from the seed bank as a player
/// does: a lily pad goes on the water, a repeater then goes on the pad, and
/// a repeater on bare water is refused (the packet stays on the cursor).
#[test]
fn pool_squares_are_planted_from_the_seed_bank() {
    let mut t = test_board!("3-1");
    t.give_sun(9990);
    t.set_bank(&[SeedType::Lilypad, SeedType::Repeater, SeedType::Repeater]);
    t.plant_from_bank(0, 3, 2);
    let pad = t.plant_at(3, 2).and_then(|id| t.plant_ref(id)).map(|p| p.seed);
    assert_eq!(pad, Some(SeedType::Lilypad), "the lily pad is on the water");

    t.plant_from_bank(1, 3, 2);
    assert!(t.plant_ref(t.plant_at(3, 2).expect("a plant on the square")).is_some());
    let on_pad: Vec<SeedType> = t.board.plants.iter().map(|(_, p)| p).filter(|p| p.col == 3 && p.row == 2 && !p.dead).map(|p| p.seed).collect();
    assert!(on_pad.contains(&SeedType::Repeater), "a repeater on the lily pad");

    t.plant_from_bank(2, 3, 3);
    let on_bare_water = t.board.plants.iter().any(|(_, p)| p.col == 3 && p.row == 3 && !p.dead);
    assert!(!on_bare_water, "nothing is planted on bare water");
}
