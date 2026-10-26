//! Saving and loading a level in progress (`board::save`): the snapshot
//! round-trips through TOML, rebuilds the level, and the file helpers
//! write, find and erase it.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Board.cpp` (`TryToSaveGame`,
//! `NeedSaveGame`), `Lawn/System/SaveGame.cpp` (what a save holds) and
//! `LawnApp.cpp` (`TryLoadGame`, `SaveFileExists`).

use crate::plant::SeedType;
use crate::player::ProfileMgr;
use crate::zombie::ZombieType;

use super::super::save::{SaveData, board_from_save, erase_save, has_save, load_save_data, try_to_save_game};
use super::super::{GameMode, need_save_game};
use super::{TestBoard, test_board};

fn temp_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("pvz-save-{}-{}", name, std::process::id()))
}

fn saver_profiles(dir: &std::path::Path) -> ProfileMgr {
    let mut profiles = ProfileMgr::new(Some(dir.to_path_buf()));
    assert!(profiles.add_profile("Saver"));
    profiles.set_current(Some("Saver"));
    profiles
}

#[test]
fn quitting_mid_level_saves_and_continue_rebuilds_it() {
    let mut t = test_board!("2-2");
    t.give_sun(500);
    t.set_bank(&[SeedType::Peashooter, SeedType::Sunflower, SeedType::Wallnut]);
    t.plant_from_bank(0, 2, 1);
    t.plant_from_bank(1, 1, 0);
    t.plant_from_bank(2, 3, 1);
    t.zombie(ZombieType::Normal, 1, 500.0);
    t.zombie(ZombieType::TrafficCone, 0, 600.0);
    // An imitater waiting to morph, and a bitten cone, must survive the trip.
    let imitater = t.plant(SeedType::Imitater, 4, 1);
    t.board.plants.get_mut(imitater).expect("imitater planted").set_imitating(SeedType::Repeater);
    t.ticks(100);
    assert!(need_save_game(&t.board), "an adventure level in progress needs saving");
    let (plants, zombies) = (t.board.plants.len(), t.board.zombies.len());
    assert!(plants >= 3 && zombies >= 2);
    let cone_helm = t.board.zombies.iter().find(|(_, z)| z.zombie_type == ZombieType::TrafficCone).map(|(_, z)| z.helm_health()).expect("a cone zombie");
    assert!(cone_helm > 0, "the cone still has its helmet");

    // The snapshot survives a trip through TOML.
    let data = SaveData::from_board(&t.board);
    let text = toml::to_string(&data).expect("the save serializes");
    let back: SaveData = toml::from_str(&text).expect("the save deserializes");

    // And rebuilds the level: same level, sun, plants and zombies, playing.
    let profiles = saver_profiles(temp_dir("shape").as_path());
    let loaded = t.with(|_, effects, res, images, sounds| {
        board_from_save(&back, &profiles, res, images, effects, sounds).expect("the save loads")
    });
    assert_eq!(loaded.level, t.board.level);
    assert_eq!(loaded.sun_money, t.board.sun_money);
    assert_eq!(loaded.scene, crate::board::GameScene::Playing);
    assert_eq!(loaded.plants.len(), plants, "every plant comes back");
    assert_eq!(loaded.zombies.len(), zombies, "every zombie comes back");
    for (_, plant) in loaded.plants.iter() {
        assert!([SeedType::Peashooter, SeedType::Sunflower, SeedType::Wallnut, SeedType::Imitater].contains(&plant.seed));
    }
    let back_imitater = loaded.plants.iter().find(|(_, p)| p.seed == SeedType::Imitater).map(|(_, p)| p).expect("the imitater comes back");
    assert_eq!(back_imitater.imitater_type, Some(SeedType::Repeater), "it still knows its copy");
    let back_cone = loaded.zombies.iter().find(|(_, z)| z.zombie_type == ZombieType::TrafficCone).map(|(_, z)| z).expect("the cone comes back");
    assert_eq!(back_cone.helm_health(), cone_helm, "its helmet damage comes back");
    // The rebuilt level runs.
    t.board = loaded;
    t.ticks(60);
    assert!(!t.board.plants.is_empty(), "plants survive the load");
}

#[test]
fn save_files_are_written_found_and_erased() {
    let mut t = test_board!("1-2");
    t.give_sun(300);
    t.plant(SeedType::Peashooter, 2, 1);
    t.ticks(30);
    let dir = temp_dir("files");
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = saver_profiles(&dir);
    let id = profiles.player().map(|p| p.id).expect("a current player");

    assert!(!has_save(&dir, id), "no save before quitting");
    assert!(try_to_save_game(&t.board, &profiles, &dir), "quitting mid-level saves");
    assert!(has_save(&dir, id), "the menu offers Continue");
    let data = load_save_data(&dir, id, GameMode::Adventure).expect("the save reads back");
    assert_eq!(data.level, t.board.level, "it holds this level");

    erase_save(&dir, id, GameMode::Adventure);
    assert!(!has_save(&dir, id), "starting over erases it");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_saves_before_play_starts() {
    let t = TestBoard::at_intro("1-1");
    let Some(t) = t else { return };
    assert!(!need_save_game(&t.board), "the intro is not a level in progress");
    let dir = temp_dir("intro");
    let profiles = saver_profiles(&dir);
    assert!(!try_to_save_game(&t.board, &profiles, &dir), "leaving the intro saves nothing");
    let _ = std::fs::remove_dir_all(&dir);
}
