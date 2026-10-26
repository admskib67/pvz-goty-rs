//! The zombies the roof levels bring: the ladder zombie, the catapult, the
//! gargantuar and its imp.

use crate::plant::SeedType;
use crate::zombie::ZombieType;

use super::super::grid_item::GridItemType;
use super::test_board;

fn has_ladder(b: &super::super::Board, col: i32, row: i32) -> bool {
    b.grid_items.iter().any(|(_, g)| g.item_type == GridItemType::Ladder && g.grid_x == col && g.grid_y == row && !g.dead)
}

#[test]
fn a_ladder_zombie_props_its_ladder_on_a_wall_nut_and_climbs_over() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Wallnut, 4, 2);
    let zombie = t.zombie(ZombieType::Ladder, 2, 700.0);
    assert!(t.run_until(2000, |b| has_ladder(b, 4, 2)), "the ladder goes up against the wall-nut");
    assert!(t.run_until(300, |b| b.zombies.get(zombie).is_some_and(|z| z.altitude() > 40.0)), "it climbs");
    let nut_x = t.plant_ref(nut).unwrap().x as f32;
    assert!(t.run_until(1000, |b| b.zombies.get(zombie).is_some_and(|z| z.pos_x < nut_x - 60.0 && z.altitude() == 0.0)), "and comes down the far side");
    assert_eq!(t.plant_ref(nut).unwrap().health, 4000, "without a bite out of the wall-nut");
}

#[test]
fn a_ladder_zombie_eats_plants_that_are_not_nuts() {
    let mut t = test_board!("1-6");
    let flower = t.plant(SeedType::Sunflower, 4, 2);
    t.zombie(ZombieType::Ladder, 2, 700.0);
    assert!(t.run_until(2000, |b| b.plants.get(flower).is_none_or(|p| p.dead || p.health < 300)), "it bites the sunflower");
    assert!(!has_ladder(&t.board, 4, 2), "no ladder for a sunflower");
}

#[test]
fn other_zombies_climb_a_ladder_left_on_a_nut() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Wallnut, 4, 2);
    t.board.add_a_ladder(4, 2);
    let zombie = t.zombie(ZombieType::Pail, 2, 600.0);
    assert!(t.run_until(3000, |b| b.zombies.get(zombie).is_some_and(|z| z.altitude() > 40.0)), "the zombie climbs the ladder");
    let nut_x = t.plant_ref(nut).unwrap().x as f32;
    assert!(t.run_until(1500, |b| b.zombies.get(zombie).is_some_and(|z| z.pos_x < nut_x - 60.0 && z.altitude() == 0.0)));
    assert_eq!(t.plant_ref(nut).unwrap().health, 4000);
}

#[test]
fn a_ladder_goes_when_its_plant_does() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Wallnut, 4, 2);
    t.board.add_a_ladder(4, 2);
    t.dig(4, 2);
    t.tick();
    assert!(!has_ladder(&t.board, 4, 2));
}

#[test]
fn a_jalapeno_burns_the_ladders_in_its_row() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Wallnut, 4, 2);
    t.board.add_a_ladder(4, 2);
    t.plant(SeedType::Jalapeno, 0, 2);
    assert!(t.run_until(300, |b| !has_ladder(b, 4, 2)));
}

fn basketballs(b: &super::super::Board) -> usize {
    b.projectiles.iter().filter(|(_, p)| p.ty == super::super::projectile::ProjectileType::Basketball && !p.dead).count()
}

#[test]
fn a_catapult_stops_and_lobs_basketballs_at_the_plants() {
    let mut t = test_board!("1-6");
    let flower = t.plant(SeedType::Sunflower, 1, 2);
    let catapult = t.zombie(ZombieType::Catapult, 2, 700.0);
    assert!(t.run_until(3000, |b| basketballs(b) > 0), "a basketball is thrown");
    let x = t.zombie_ref(catapult).unwrap().pos_x;
    assert!(x <= 650.0, "from 650 px in");
    assert!(t.run_until(400, |b| b.plants.get(flower).is_some_and(|p| p.health < 300)), "it lands on the sunflower");
    assert_eq!(t.zombie_ref(catapult).unwrap().pos_x, x, "the catapult stands still while it throws");
}

#[test]
fn an_umbrella_leaf_bats_basketballs_away() {
    let mut t = test_board!("1-6");
    let flower = t.plant(SeedType::Sunflower, 1, 2);
    t.plant(SeedType::Umbrella, 2, 2);
    t.zombie(ZombieType::Catapult, 2, 700.0);
    let mut thrown = 0;
    let mut was_out = false;
    for _ in 0..3000 {
        t.tick();
        let out = basketballs(&t.board) > 0;
        if out && !was_out {
            thrown += 1;
        }
        was_out = out;
    }
    assert!(thrown >= 3, "{thrown} basketballs thrown");
    assert_eq!(t.plant_ref(flower).unwrap().health, 300, "none get through");
}

#[test]
fn spikeweed_pops_a_catapults_tire() {
    let mut t = test_board!("1-6");
    t.plant(SeedType::Spikeweed, 5, 2);
    let catapult = t.zombie(ZombieType::Catapult, 2, 700.0);
    assert!(t.run_until(4000, |b| b.zombies.get(catapult).is_none_or(|z| z.is_dead_or_dying())), "the spikes get it");
    assert!(t.run_until(400, |b| b.zombies.get(catapult).is_none_or(|z| z.dead)), "and it blows up");
}

#[test]
fn a_catapult_with_no_balls_left_drives_over_plants() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Wallnut, 3, 2);
    let catapult = t.zombie(ZombieType::Catapult, 2, 600.0);
    t.board.zombies.get_mut(catapult).unwrap().set_basketballs(0);
    assert!(t.run_until(6000, |b| b.plants.get(nut).is_none_or(|p| p.dead)), "the wall-nut's flattened");
}

#[test]
fn a_gargantuar_smashes_the_plant_in_front_of_it() {
    let mut t = test_board!("1-6");
    let nut = t.plant(SeedType::Wallnut, 4, 2);
    let gargantuar = t.zombie(ZombieType::Gargantuar, 2, 650.0);
    let mut smashed = false;
    assert!(
        t.run_until(3000, |b| {
            smashed |= b.zombies.get(gargantuar).is_some_and(|z| z.phase == super::super::zombie::ZombiePhase::GargantuarSmashing);
            b.plants.get(nut).is_none_or(|p| p.dead)
        }),
        "the wall-nut is flattened"
    );
    assert!(smashed, "by the gargantuar's pole");
}

/// Hurts a zombie the way plants do.
fn hurt(t: &mut super::TestBoard, id: sexy::data_array::Id<super::super::zombie::Zombie>, damage: i32) {
    t.with(|board, effects, res, images, sounds| {
        let env = board.zombie_env();
        let mut fx = super::super::zombie::Fx { effects, res, images, sounds };
        board.zombies.get_mut(id).unwrap().take_damage(damage, 0, &env, &mut fx);
    });
}

fn imps(b: &super::super::Board) -> Vec<(f32, f32, super::super::zombie::ZombiePhase)> {
    b.zombies.iter().filter(|(_, z)| z.zombie_type == ZombieType::Imp && !z.dead).map(|(_, z)| (z.pos_x, z.altitude(), z.phase)).collect()
}

#[test]
fn a_gargantuar_flings_its_imp_at_half_health() {
    use super::super::zombie::ZombiePhase;
    for stage in ["1-6", "5-4"] {
        let mut t = test_board!(stage);
        let gargantuar = t.zombie(ZombieType::Gargantuar, 2, 700.0);
        t.ticks(10);
        hurt(&mut t, gargantuar, 1400);
        assert!(imps(&t.board).is_empty(), "{stage}: not yet, at over half health");
        hurt(&mut t, gargantuar, 200);
        assert!(t.run_until(300, |b| !imps(b).is_empty()), "{stage}: an imp is thrown");
        assert!(t.run_until(60, |b| imps(b).iter().any(|i| i.1 > 50.0)), "{stage}: it flies");
        let garg_x = t.zombie_ref(gargantuar).unwrap().pos_x;
        assert!(t.run_until(400, |b| imps(b).iter().any(|i| i.2 == ZombiePhase::Normal && i.1 == 0.0)), "{stage}: it lands and walks");
        let (imp_x, _, _) = imps(&t.board)[0];
        assert!(imp_x < garg_x - 150.0, "{stage}: well ahead of the gargantuar ({imp_x} vs {garg_x})");
        // The gargantuar has no imp left to throw.
        hurt(&mut t, gargantuar, 100);
        t.ticks(300);
        assert_eq!(imps(&t.board).len(), 1, "{stage}: one imp only");
    }
}

#[test]
fn a_gargantuar_leaves_a_flat_plant_that_can_be_replanted() {
    let mut t = test_board!("1-6");
    let flower = t.plant(SeedType::Sunflower, 4, 2);
    t.zombie(ZombieType::Gargantuar, 2, 520.0);
    assert!(t.run_until(2000, |b| b.plants.get(flower).is_some_and(|p| p.squished)), "the sunflower is stamped flat");
    assert!(!t.plant_ref(flower).unwrap().dead, "but still there");
    assert_eq!(t.board.can_plant_at(4, 2, SeedType::Wallnut), super::super::cursor::PlantingReason::Ok, "the square is free again");
    assert!(t.run_until(600, |b| b.plants.get(flower).is_none()), "and the flat plant goes after five seconds");
}
