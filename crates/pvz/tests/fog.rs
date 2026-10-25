//! World 4's fog: where it is, what clears it, and the blover.

use crate::plant::SeedType;

use super::test_board;

#[test]
fn only_the_fog_levels_have_fog() {
    let t = test_board!("3-1");
    assert!(!t.board.stage_has_fog());
    drop(t);
    let t = test_board!("4-1");
    assert!(t.board.stage_has_fog());
    assert_eq!(t.board.left_fog_column(), 6, "4-1's fog starts further right");
    drop(t);
    let t = test_board!("4-2");
    assert_eq!(t.board.left_fog_column(), 5);
    drop(t);
    let t = test_board!("4-8");
    assert_eq!(t.board.left_fog_column(), 4, "and it creeps closer on the later levels");
    drop(t);
    let t = test_board!("4-5");
    assert!(!t.board.stage_has_fog(), "the stormy night level has none");
}

#[test]
fn the_fog_rolls_in_and_thickens() {
    let mut t = test_board!("4-2");
    assert!(t.board.fog.offset > 0.0, "it starts off to the right");
    t.ticks(400);
    assert!(t.board.fog.offset < 0.01, "and settles over the lawn");
    assert_eq!(t.board.fog.cel[8][3], 255, "thick on the right");
    assert_eq!(t.board.fog.cel[5][3], 200, "thinner at its edge");
    assert_eq!(t.board.fog.cel[4][3], 0, "none beyond the left column");
}

#[test]
fn a_plantern_lights_up_the_fog_around_it() {
    let mut t = test_board!("4-2");
    t.plant(SeedType::Plantern, 6, 2);
    t.ticks(800);
    assert_eq!(t.board.fog.cel[6][2], 0, "clear at the plantern");
    assert_eq!(t.board.fog.cel[8][2], 0, "and two squares along");
    assert_eq!(t.board.fog.cel[8][4], 0, "and two rows down");
    assert_eq!(t.board.fog.cel[8][0], 0, "its light is a rounded block two rows up too");
    assert_eq!(t.board.fog.cel[8][5], 255, "far below it, the fog stays thick");
}

#[test]
fn a_torchwood_clears_only_a_little() {
    let mut t = test_board!("4-2");
    t.plant(SeedType::Torchwood, 7, 2);
    t.ticks(800);
    assert_eq!(t.board.fog.cel[7][2], 0);
    assert_eq!(t.board.fog.cel[8][2], 0);
    assert_eq!(t.board.fog.cel[7][4], 255, "two rows away is still foggy");
}

#[test]
fn a_blover_blows_the_fog_away_for_a_while() {
    let mut t = test_board!("4-2");
    t.ticks(400);
    let blover = t.plant(SeedType::Blover, 3, 2);
    assert!(t.plant_ref(blover).is_some());
    t.ticks(60);
    assert!(t.board.fog.blown_count_down > 3000, "it blows");
    let before = t.board.fog.offset;
    t.ticks(60);
    let swept = t.board.fog.offset;
    assert!(swept > before, "and the fog sweeps off to the right");
    assert!(t.run_until(400, |b| b.plants.get(blover).is_none_or(|p| p.dead)), "the blover is spent");
    t.ticks(2500);
    assert!(t.board.fog.offset < swept, "it starts to come back");
    t.ticks(2500);
    assert!(t.board.fog.offset < 0.01, "all the way");
}

#[test]
fn the_fog_levels_run_their_waves_without_incident() {
    for stage in ["4-1", "4-3", "4-6", "4-8", "4-9"] {
        let mut t = test_board!(stage).with_waves();
        t.give_sun(9000);
        t.plant(SeedType::Plantern, 6, 1);
        for row in [0, 1, 4, 5] {
            t.plant(SeedType::Peashooter, 0, row);
        }
        for _ in 0..25_000 {
            t.tick();
        }
        assert!(t.board.fog.cel.iter().flatten().all(|&c| (0..=255).contains(&c)), "{stage}: fog stays in range");
    }
}

#[test]
fn the_fog_picture_is_loaded() {
    let mut t = test_board!("4-1");
    let cols = t.with(|_, _, res, _, _| res.image("IMAGE_FOG").map(|i| i.cols));
    assert!(cols.is_some_and(|c| c > 1), "IMAGE_FOG should be a strip of cels, got {cols:?}");
}

#[test]
fn the_pool_pictures_are_loaded_on_the_pool_levels() {
    let mut t = test_board!("4-1");
    let night = t.with(|_, _, res, _, _| res.image("IMAGE_POOL_NIGHT").is_some());
    assert!(night, "the fog levels' water is the night pool");
    drop(t);
    let mut t = test_board!("3-1");
    let day = t.with(|_, _, res, _, _| res.image("IMAGE_POOL").is_some());
    assert!(day);
}
