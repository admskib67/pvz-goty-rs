//! The roof: flower pots, the slope, and the lobbing plants.

use crate::plant::SeedType;
use crate::zombie::ZombieType;

use super::super::cursor::PlantingReason;
use super::super::plant::OnBungee;
use super::super::zombie::ZombiePhase;
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
fn a_bought_roof_cleaner_gives_every_row_a_mower() {
    use super::super::lawn_mower::{LawnMowerState, LawnMowerType};
    let mut t = test_board!("5-1", |p| p.purchases[crate::store::StoreItem::RoofCleaner.slot()] = 1);
    assert_eq!(t.board.lawn_mowers.len(), 5, "one for each of the five rows");
    assert!(t.board.lawn_mowers.iter().all(|m| m.kind == LawnMowerType::Roof));
    let zombie = t.zombie(ZombieType::Normal, 2, 0.0);
    assert!(t.run_until(100, |b| b.zombies.get(zombie).is_none_or(|z| z.is_dead_or_dying())), "it mows the zombie down");
    let mower = t.board.lawn_mowers.iter().find(|m| m.row == 2).unwrap();
    assert_eq!(mower.state, LawnMowerState::Triggered);
    let (x, y) = (mower.pos_x, mower.pos_y);
    t.ticks(60);
    let mower = t.board.lawn_mowers.iter().find(|m| m.row == 2).unwrap();
    assert!(mower.pos_x > x + 100.0 && mower.pos_y != y, "it runs along the slope");
}

#[test]
fn a_squished_mower_stays_flat_then_goes() {
    use super::super::lawn_mower::LawnMowerState;
    let mut t = test_board!("5-1", |p| p.purchases[crate::store::StoreItem::RoofCleaner.slot()] = 1);
    t.with(|board, effects, res, images, sounds| {
        let mut fx = super::super::zombie::Fx { effects, res, images, sounds };
        board.lawn_mowers.iter_mut().find(|m| m.row == 1).unwrap().squish(&mut fx);
    });
    let mower = t.board.lawn_mowers.iter().find(|m| m.row == 1).unwrap();
    assert_eq!(mower.state, LawnMowerState::Squished);
    let x = mower.pos_x;
    t.ticks(499);
    let mower = t.board.lawn_mowers.iter().find(|m| m.row == 1).unwrap();
    assert_eq!(mower.pos_x, x, "it doesn't move");
    t.ticks(2);
    assert!(t.board.lawn_mowers.iter().all(|m| m.row != 1), "and is gone after five seconds");
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


#[test]
fn peas_climb_the_roofs_slope_with_the_zombies() {
    let mut t = test_board!("5-2");
    t.plant(SeedType::Flowerpot, 0, 2);
    t.plant(SeedType::Peashooter, 0, 2);
    t.zombie(ZombieType::Newspaper, 2, 780.0);
    assert!(t.run_until(600, |b| !b.projectiles.is_empty()));
    let (x0, y0) = t.board.projectiles.iter().next().unwrap().1.position();
    let mut rose = false;
    for _ in 0..30 {
        t.tick();
        if let Some((_, p)) = t.board.projectiles.iter().next() {
            let (x, y) = p.position();
            if x > x0 + 40.0 && x < 430.0 && y < y0 - 5.0 {
                rose = true;
            }
        }
    }
    assert!(rose, "a pea starting low on the slope rises as it flies right (started at {x0}, {y0})");
}

#[test]
fn butter_sits_on_the_zombies_head_anywhere_on_the_slope() {
    let mut t = test_board!("5-4");
    for x in [300.0, 520.0, 700.0] {
        let id = t.zombie(ZombieType::Normal, 0, x);
        t.board.zombies.get_mut(id).unwrap().set_buttered_counter(400);
    }
    t.ticks(5);
    t.with(|board, effects, res, _, _| {
        let butter = res.image("IMAGE_REANIM_CORNPULT_BUTTER_SPLAT").unwrap();
        let (butter_w, butter_h) = (butter.cel_width() as f32, butter.cel_height() as f32);
        let mut offsets = Vec::new();
        for (_, z) in board.zombies.iter() {
            let (bx, by, _) = z.butter_position(effects, res).expect("buttered");
            // Where the head image is drawn, in the zombie's own coordinates.
            let body = effects.get(z.body_reanim().unwrap()).unwrap();
            let track = body.definition.find_track("anim_head1").unwrap();
            let head_id = body.current_transform(track).image.unwrap();
            let head = res.image(&head_id).unwrap();
            let m = body.track_matrix(track);
            let (head_cx, head_cy) = (m.m02 + head.cel_width() as f32 / 2.0, m.m12 + head.cel_height() as f32 / 2.0);
            let (butter_cx, butter_cy) = (bx + butter_w / 2.0, by + butter_h / 2.0);
            // DrawButter puts it on the crown, a little towards the face.
            assert!(
                (butter_cx - head_cx).abs() < 12.0 && (butter_cy - head_cy).abs() < 20.0,
                "butter centred at ({butter_cx}, {butter_cy}), head at ({head_cx}, {head_cy}) for a zombie at x {}",
                z.pos_x
            );
            offsets.push((butter_cx - head_cx, butter_cy - head_cy));
        }
        assert!(offsets.windows(2).all(|w| (w[0].0 - w[1].0).abs() < 2.0 && (w[0].1 - w[1].1).abs() < 2.0), "the same offset all along the slope (give or take the head bobbing): {offsets:?}");
    });
}

/// A bungee zombie dropping on a square in `col` (`AddZombie` then
/// `PickBungeeZombieTarget(col)`).
fn bungee_on(t: &mut super::TestBoard, col: i32) -> sexy::data_array::Id<crate::board::zombie::Zombie> {
    t.with(|board, effects, res, images, sounds| {
        let mut fx = super::super::zombie::Fx { effects, res, images, sounds };
        let id = board.add_zombie(ZombieType::Bungee, 0, &mut fx);
        board.pick_bungee_target(id, Some(col), &mut fx);
        id
    })
}

#[test]
fn a_bungee_zombie_aims_for_a_plant() {
    let mut t = test_board!("5-1");
    t.plant(SeedType::Flowerpot, 4, 1);
    t.plant(SeedType::Wallnut, 4, 1);
    let bungee = bungee_on(&mut t, 4);
    let z = t.zombie_ref(bungee).unwrap();
    assert_eq!((z.target_col, z.row), (4, 1), "the plant outweighs the empty squares");
    assert_eq!(z.phase, ZombiePhase::BungeeDiving);
}

#[test]
fn a_bungee_zombie_cannot_be_hit_until_it_lands() {
    use super::super::zombie::damage_range;
    let mut t = test_board!("5-1");
    t.plant(SeedType::Flowerpot, 4, 1);
    t.plant(SeedType::Wallnut, 4, 1);
    let bungee = bungee_on(&mut t, 4);
    t.ticks(50);
    assert!(!t.zombie_ref(bungee).unwrap().effected_by_damage(damage_range::GROUND), "still in the sky");
    assert!(t.run_until(1000, |b| b.zombies.get(bungee).is_some_and(|z| z.phase == ZombiePhase::BungeeAtBottom)), "it lands");
    assert!(t.zombie_ref(bungee).unwrap().effected_by_damage(damage_range::GROUND), "and can be hit on the ground");
}

#[test]
fn a_bungee_zombie_carries_a_plant_off_and_leaves_its_pot() {
    let mut t = test_board!("5-1");
    let pot = t.plant(SeedType::Flowerpot, 4, 1);
    t.plant(SeedType::Wallnut, 4, 1);
    let nut = plant_of(&t, SeedType::Wallnut, 4, 1);
    let bungee = bungee_on(&mut t, 4);
    assert!(t.run_until(1000, |b| b.plants.get(nut).is_some_and(|p| p.on_bungee == OnBungee::Grabbed)), "it grabs the wall-nut");
    assert!(t.run_until(500, |b| b.plants.get(nut).is_some_and(|p| p.on_bungee == OnBungee::Rising)), "and lifts it");
    assert!(t.run_until(500, |b| b.zombies.get(bungee).is_none_or(|z| z.dead)), "then flies off with it");
    assert!(t.plant_ref(nut).is_none(), "the wall-nut's gone");
    assert!(t.plant_ref(pot).is_some(), "the pot stays");
}

#[test]
fn killing_a_bungee_zombie_at_the_bottom_saves_the_plant() {
    let mut t = test_board!("5-1");
    t.plant(SeedType::Flowerpot, 4, 1);
    t.plant(SeedType::Wallnut, 4, 1);
    let nut = plant_of(&t, SeedType::Wallnut, 4, 1);
    let bungee = bungee_on(&mut t, 4);
    assert!(t.run_until(1000, |b| b.zombies.get(bungee).is_some_and(|z| z.phase == ZombiePhase::BungeeAtBottom)));
    t.plant(SeedType::Flowerpot, 0, 1);
    t.plant(SeedType::Jalapeno, 0, 1);
    assert!(t.run_until(400, |b| b.zombies.get(bungee).is_none_or(|z| z.is_dead_or_dying())), "the jalapeno gets it");
    t.ticks(5);
    let nut = t.plant_ref(nut).expect("the wall-nut's still there");
    assert_eq!(nut.on_bungee, OnBungee::No);
}

#[test]
fn a_bungee_zombie_killed_mid_grab_lets_go_of_the_plant() {
    let mut t = test_board!("5-1");
    t.plant(SeedType::Flowerpot, 4, 1);
    t.plant(SeedType::Wallnut, 4, 1);
    let nut = plant_of(&t, SeedType::Wallnut, 4, 1);
    let bungee = bungee_on(&mut t, 4);
    assert!(t.run_until(1000, |b| b.plants.get(nut).is_some_and(|p| p.on_bungee == OnBungee::Grabbed)));
    t.with(|board, effects, res, images, sounds| {
        let env = board.zombie_env();
        let mut fx = super::super::zombie::Fx { effects, res, images, sounds };
        board.zombies.get_mut(bungee).unwrap().take_damage(1000, 0, &env, &mut fx);
    });
    t.tick();
    assert!(t.zombie_ref(bungee).is_none_or(|z| z.is_dead_or_dying()));
    let nut = t.plant_ref(nut).expect("the wall-nut stays");
    assert_eq!(nut.on_bungee, OnBungee::No, "and is let go");
}

#[test]
fn an_umbrella_leaf_bounces_a_bungee_zombie_back() {
    use super::super::plant::PlantState;
    let mut t = test_board!("5-1");
    t.plant(SeedType::Flowerpot, 3, 1);
    t.plant(SeedType::Umbrella, 3, 1);
    let umbrella = plant_of(&t, SeedType::Umbrella, 3, 1);
    t.plant(SeedType::Flowerpot, 4, 1);
    t.plant(SeedType::Wallnut, 4, 1);
    let nut = plant_of(&t, SeedType::Wallnut, 4, 1);
    let bungee = bungee_on(&mut t, 4);
    assert_eq!(t.zombie_ref(bungee).unwrap().row, 1);
    let mut blocked = false;
    let mut landed = false;
    assert!(
        t.run_until(1500, |b| {
            blocked |= b.plants.get(umbrella).is_some_and(|p| p.state() == PlantState::UmbrellaTriggered);
            landed |= b.zombies.get(bungee).is_some_and(|z| z.phase == ZombiePhase::BungeeAtBottom);
            b.zombies.get(bungee).is_none_or(|z| z.dead)
        }),
        "the bungee zombie goes back up"
    );
    assert!(blocked, "the umbrella leaf opens");
    assert!(!landed, "the bungee zombie never lands");
    let nut = t.plant_ref(nut).expect("the wall-nut stays");
    assert_eq!(nut.on_bungee, OnBungee::No);
    assert!(t.run_until(300, |b| b.plants.get(umbrella).is_some_and(|p| p.state() == PlantState::NotReady)), "and the leaf folds back");
}

#[test]
fn an_umbrella_leaf_only_shelters_the_squares_next_to_it() {
    let mut t = test_board!("5-1");
    t.plant(SeedType::Flowerpot, 2, 1);
    t.plant(SeedType::Umbrella, 2, 1);
    t.plant(SeedType::Flowerpot, 4, 1);
    t.plant(SeedType::Wallnut, 4, 1);
    let bungee = bungee_on(&mut t, 4);
    assert_eq!(t.zombie_ref(bungee).unwrap().row, 1);
    assert!(t.run_until(1000, |b| b.zombies.get(bungee).is_some_and(|z| z.phase == ZombiePhase::BungeeAtBottom)), "two squares away, it lands");
}

/// Crazy Dave talks on a first visit to 4-1 (the fog) and 5-1 (the roof) only.
#[test]
fn crazy_dave_talks_at_the_start_of_the_fog_and_the_roof() {
    use super::super::cutscene;
    let mut t = test_board!("5-1");
    assert_eq!(cutscene::crazy_dave_dialog_start(&t.board), 1201, "5-1: they found a way onto the roof");
    t.board.level = 31;
    assert_eq!(cutscene::crazy_dave_dialog_start(&t.board), 801, "4-1: Fog Man");
    t.board.first_time_adventure = false;
    assert_eq!(cutscene::crazy_dave_dialog_start(&t.board), -1, "4-1 replayed: no talk");
    t.board.level = 41;
    assert_eq!(cutscene::crazy_dave_dialog_start(&t.board), -1, "5-1 replayed: no talk");
}

/// A zombie that gets into the house on the roof drops down the chimney.
/// Behaviour per ruslan831 decomp, Zombie::WalkIntoHouse and UpdateZombieChimney.
#[test]
fn a_zombie_that_wins_on_the_roof_drops_down_the_chimney() {
    use super::super::GameScene;
    let mut t = test_board!("5-1");
    let id = t.zombie(ZombieType::Normal, 2, -90.0);
    assert!(t.run_until(500, |b| b.scene == GameScene::ZombiesWon), "it gets in");
    let zombie = t.zombie_ref(id).unwrap();
    assert_eq!((zombie.pos_x, zombie.pos_y), (-180.0, 250.0), "at the chimney");
    t.tick();
    assert!(t.zombie_ref(id).unwrap().altitude() > 100.0, "high above the chimney to begin with");
    assert!(t.run_until(1200, |b| b.cut_scene.time() > 5200), "the lost scene runs its course");
    assert_eq!(t.zombie_ref(id).unwrap().altitude(), 0.0, "and it's all the way down");
}
