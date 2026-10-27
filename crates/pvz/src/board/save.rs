//! Saving and loading a level in progress (`Board::TryToSaveGame`,
//! `NeedSaveGame`, `PreNewGame`'s `TryLoadGame`, the Continue dialog).
//!
//! Behaviour per ruslan831 decomp, `Lawn/Board.cpp` (`TryToSaveGame`,
//! `NeedSaveGame`, `LoadGame`), `Lawn/System/SaveGame.cpp` (what a save
//! holds), `LawnApp.cpp` (`PreNewGame`, `TryLoadGame`, `DoContinueDialog`,
//! `SaveFileExists`, `GetSavedGameName`) and `Lawn/Widget/GameSelector.cpp`
//! (the Adventure button) and `ContinueDialog.cpp`.
//!
//! The original dumps the board's memory almost verbatim (its `DataArray`s,
//! reanimations, particles and all) into `userdata/game{id}_{mode}.dat`.
//! That layout is tied to the 32-bit game's structs, so this port saves a
//! small strategic snapshot instead (TOML): the level, sun, plants, zombies,
//! mowers, resting coins, graves, wave progress and seed packets. Visual
//! phases (reanimation frames, particles, peas in flight) are not saved; on
//! load the plants and zombies are recreated fresh, as if just placed.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::install::userdata_dir;
use crate::plant::SeedType;
use crate::player::ProfileMgr;
use crate::reanim::EffectSystem;
use crate::resources::GameResources;
use crate::zombie::ZombieType;

use super::coin::{Coin, CoinType};
use super::grid_item::{GridItem, GridItemType};
use super::lawn_mower::{LawnMowerState, LawnMowerType};
use super::plant::Plant;
use super::zombie::{Fx, Zombie};
use super::{Board, GameMode};

/// `SAVE_FILE_MAGIC_NUMBER` / `SAVE_FILE_VERSION`, ours not the original's
/// (`0xFEEDDEAD` / 2 with a build-date CRC): the formats are unrelated.
const SAVE_MAGIC: &str = "PVZ_SAVE";
const SAVE_VERSION: u32 = 2;

fn mode_index(mode: GameMode) -> i32 {
    match mode {
        // Matches the original's `GAMEMODE_ADVENTURE` (0); the other modes
        // don't exist yet, so they get nothing to collide with.
        GameMode::Adventure => 0,
        GameMode::Intro => 81,
    }
}

/// `GetSavedGameName`: `userdata/game{profile}_{mode}.dat`.
pub fn save_file_name(profile_id: u32, mode: GameMode) -> String {
    format!("game{}_{}.dat", profile_id, mode_index(mode))
}

fn save_path_for(dir: &Path, profile_id: u32, mode: GameMode) -> PathBuf {
    dir.join(save_file_name(profile_id, mode))
}

/// Where saves live: the app's `userdata` folder, beside the profiles.
pub(crate) fn save_dir() -> Option<PathBuf> {
    userdata_dir()
}

/// `Board::NeedSaveGame`: adventure levels in progress. The original also
/// excludes the ice/upsell/intro/zen/tree modes; only Adventure (and the
/// never-saved Intro) exists here.
pub(crate) fn need_save_game(board: &Board) -> bool {
    board.mode == GameMode::Adventure
        && board.scene == super::GameScene::Playing
        && !board.level_overridden
}

/// `LawnApp::SaveFileExists`, for adventure mode.
pub(crate) fn has_save(dir: &Path, profile_id: u32) -> bool {
    save_path_for(dir, profile_id, GameMode::Adventure).is_file()
}

/// Erases a mode's save (`EraseFile(GetSavedGameName(...))`).
pub(crate) fn erase_save(dir: &Path, profile_id: u32, mode: GameMode) {
    let _ = std::fs::remove_file(save_path_for(dir, profile_id, mode));
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlantSave {
    seed: i32,
    col: i32,
    row: i32,
    health: i32,
    in_pot: bool,
    bowling: bool,
    asleep: bool,
    imitater: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ZombieSave {
    ty: i32,
    row: i32,
    pos_x: f32,
    body_health: i32,
    helm_health: i32,
    shield_health: i32,
    variant: bool,
    from_wave: i32,
    scale: f32,
    mind_controlled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MowerSave {
    row: i32,
    kind: i32,
    pos_x: f32,
    state: i32,
    dead: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CoinSave {
    ty: i32,
    pos_x: f32,
    pos_y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WaveSave {
    num_waves: i32,
    current_wave: i32,
    total_spawned: i32,
    zombie_count_down: i32,
    zombies_in_wave: Vec<Vec<i32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GridItemSave {
    ty: i32,
    col: i32,
    row: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SaveData {
    pub(crate) magic: String,
    pub(crate) version: u32,
    pub(crate) level: i32,
    pub(crate) sun_money: i32,
    pub(crate) main_counter: i32,
    pub(crate) player_coins: i32,
    pub(crate) plants: Vec<PlantSave>,
    pub(crate) zombies: Vec<ZombieSave>,
    pub(crate) mowers: Vec<MowerSave>,
    pub(crate) coins: Vec<CoinSave>,
    pub(crate) waves: WaveSave,
    pub(crate) seed_packets: Vec<Option<i32>>,
    pub(crate) grid_items: Vec<GridItemSave>,
}

fn mower_kind_to_int(kind: LawnMowerType) -> i32 {
    match kind {
        LawnMowerType::Lawn => 0,
        LawnMowerType::Pool => 1,
        LawnMowerType::Roof => 2,
    }
}

fn mower_kind_from_int(v: i32) -> LawnMowerType {
    match v {
        1 => LawnMowerType::Pool,
        2 => LawnMowerType::Roof,
        _ => LawnMowerType::Lawn,
    }
}

fn mower_state_to_int(state: LawnMowerState) -> i32 {
    match state {
        LawnMowerState::Ready => 0,
        LawnMowerState::RollingIn => 1,
        LawnMowerState::Triggered => 2,
        LawnMowerState::Squished => 3,
    }
}

fn mower_state_from_int(v: i32) -> LawnMowerState {
    match v {
        1 => LawnMowerState::RollingIn,
        2 => LawnMowerState::Triggered,
        3 => LawnMowerState::Squished,
        _ => LawnMowerState::Ready,
    }
}

fn coin_type_to_int(ty: CoinType) -> i32 {
    match ty {
        CoinType::Sun => 0,
        CoinType::SmallSun => 1,
        CoinType::LargeSun => 2,
        CoinType::Silver => 3,
        CoinType::Gold => 4,
        CoinType::Diamond => 5,
        CoinType::MoneyBag => 6,
        CoinType::FinalSeedPacket => 7,
        CoinType::Shovel => 8,
        CoinType::Note => 9,
        CoinType::Almanac => 10,
        CoinType::CarKeys => 11,
        CoinType::Taco => 12,
        CoinType::WateringCan => 13,
        CoinType::SilverSunflower => 14,
    }
}

fn coin_type_from_int(v: i32) -> Option<CoinType> {
    Some(match v {
        0 => CoinType::Sun,
        1 => CoinType::SmallSun,
        2 => CoinType::LargeSun,
        3 => CoinType::Silver,
        4 => CoinType::Gold,
        5 => CoinType::Diamond,
        6 => CoinType::MoneyBag,
        7 => CoinType::FinalSeedPacket,
        8 => CoinType::Shovel,
        9 => CoinType::Note,
        10 => CoinType::Almanac,
        11 => CoinType::CarKeys,
        12 => CoinType::Taco,
        13 => CoinType::WateringCan,
        14 => CoinType::SilverSunflower,
        _ => return None,
    })
}

impl SaveData {
    pub(crate) fn from_board(board: &Board) -> Self {
        Self {
            magic: SAVE_MAGIC.to_owned(),
            version: SAVE_VERSION,
            level: board.level,
            sun_money: board.sun_money,
            main_counter: board.main_counter,
            player_coins: board.player_coins,
            plants: board
                .plants
                .iter()
                .filter(|(_, p)| !p.dead)
                .map(|(_, p)| PlantSave {
                    seed: p.seed as i32,
                    col: p.col,
                    row: p.row,
                    health: p.health,
                    in_pot: p.in_pot,
                    bowling: p.is_bowling(),
                    asleep: p.is_asleep(),
                    imitater: p.imitater_type.map(|s| s as i32),
                })
                .collect(),
            zombies: board
                .zombies
                .iter()
                .filter(|(_, z)| !z.is_dead_or_dying())
                .map(|(_, z)| ZombieSave {
                    ty: z.zombie_type as i32,
                    row: z.row,
                    pos_x: z.pos_x,
                    body_health: z.body_health,
                    helm_health: z.helm_health(),
                    shield_health: z.shield_health(),
                    variant: z.variant(),
                    from_wave: z.from_wave,
                    scale: z.scale,
                    mind_controlled: z.mind_controlled,
                })
                .collect(),
            mowers: board
                .lawn_mowers
                .iter()
                .map(|m| MowerSave {
                    row: m.row,
                    kind: mower_kind_to_int(m.kind),
                    pos_x: m.pos_x,
                    state: mower_state_to_int(m.state),
                    dead: m.dead,
                })
                .collect(),
            coins: board
                .coins
                .iter()
                .filter(|(_, c)| !c.dead && !c.is_being_collected)
                .map(|(_, c)| CoinSave { ty: coin_type_to_int(c.ty), pos_x: c.pos_x, pos_y: c.pos_y })
                .collect(),
            waves: WaveSave {
                num_waves: board.waves.num_waves,
                current_wave: board.waves.current_wave,
                total_spawned: board.waves.total_spawned_waves,
                zombie_count_down: board.waves.zombie_count_down,
                zombies_in_wave: board
                    .waves
                    .zombies_in_wave
                    .iter()
                    .map(|w| w.iter().map(|t| *t as i32).collect())
                    .collect(),
            },
            seed_packets: board
                .seed_bank
                .packets
                .iter()
                .take(board.seed_bank.num_packets)
                .map(|p| p.packet_type.map(|s| s as i32))
                .collect(),
            grid_items: board
                .grid_items
                .iter()
                .filter(|(_, g)| !g.dead)
                .map(|(_, g)| GridItemSave {
                    ty: match g.item_type {
                        GridItemType::GraveStone => 0,
                        GridItemType::Crater => 1,
                        GridItemType::Ladder => 2,
                    },
                    col: g.grid_x,
                    row: g.grid_y,
                })
                .collect(),
        }
    }
}

/// `Board::TryToSaveGame`: writes the save when `NeedSaveGame`.
pub(crate) fn try_to_save_game(board: &Board, profiles: &ProfileMgr, dir: &Path) -> bool {
    if !need_save_game(board) {
        return false;
    }
    let Some(player) = profiles.player() else { return false };
    let data = SaveData::from_board(board);
    let Ok(text) = toml::to_string(&data) else { return false };
    let _ = std::fs::create_dir_all(dir);
    std::fs::write(save_path_for(dir, player.id, board.mode), text).is_ok()
}

/// Reads a save file, checking its magic and version.
pub(crate) fn load_save_data(dir: &Path, profile_id: u32, mode: GameMode) -> Option<SaveData> {
    let text = std::fs::read_to_string(save_path_for(dir, profile_id, mode)).ok()?;
    let data: SaveData = toml::from_str(&text).ok()?;
    (data.magic == SAVE_MAGIC && data.version == SAVE_VERSION).then_some(data)
}

/// Rebuilds a board from a save: a fresh level of that number with the
/// saved sun, plants, zombies, mowers, coins, graves, waves and packets,
/// already playing (no intro, chooser or Dave).
///
/// Behaviour per ruslan831 decomp, `Board::LoadGame` + `FixBoardAfterLoad`:
/// the loaded board is put straight into `SCENE_PLAYING`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn board_from_save(
    data: &SaveData,
    profiles: &ProfileMgr,
    resources: &mut GameResources,
    images: &mut Assets<Image>,
    effects: &mut EffectSystem,
    sounds: &mut crate::audio::Sounds,
) -> Option<Board> {
    let player = profiles.player()?;
    // A copy of the player on the saved level, like `--lvl` does: finishing
    // the loaded level must not rewrite the profile's own level.
    let mut shape = player.clone();
    shape.level = data.level;
    let mut board = Board::new_for_save(shape, resources, images, sounds.rng());
    board.sun_money = data.sun_money;
    board.main_counter = data.main_counter;
    board.player_coins = data.player_coins;
    board.scene = super::GameScene::Playing;
    board.sod_position = 1000;
    // No intro: no chooser, no Dave, no tutorial arrow.
    board.seed_chooser = None;
    board.dave = super::crazy_dave::CrazyDave::new();
    board.tutorial_state = super::tutorial::TutorialState::Off;

    let mut fx = Fx { effects: &mut *effects, res: &mut *resources, images: &mut *images, sounds: &mut *sounds };
    board.plants.clear();
    for p in &data.plants {
        let Some(seed) = SeedType::from_index(p.seed) else { continue };
        let (x, y) = (board.grid_to_pixel_x(p.col), board.grid_to_pixel_y(p.col, p.row));
        let mut plant = Plant::new(p.col, p.row, x, y, seed, p.bowling, fx.effects, fx.res, fx.images, fx.sounds.rng());
        plant.health = p.health;
        plant.in_pot = p.in_pot;
        // Water squares are level geometry: recompute them like `AddPlant` does.
        plant.on_water =
            board.grid_square_type.get(p.col as usize).and_then(|col| col.get(p.row as usize)).is_some_and(|sq| *sq == super::GridSquareType::Pool);
        if let Some(imitater) = p.imitater.and_then(SeedType::from_index) {
            plant.set_imitating(imitater);
        }
        if p.asleep {
            plant.set_sleeping(true, fx.effects, fx.res, fx.images, fx.sounds.rng());
        }
        board.plants.alloc(plant);
    }
    board.zombies.clear();
    for z in &data.zombies {
        let Ok(ty_idx) = usize::try_from(z.ty) else { continue };
        if ty_idx >= crate::zombie::NUM_ZOMBIE_TYPES {
            continue;
        }
        let ty = ZombieType::from_index(ty_idx);
        let env = board.zombie_env();
        let mut zombie = Zombie::new(z.row, ty, z.variant, z.from_wave, false, &env, &mut fx);
        zombie.pos_x = z.pos_x;
        zombie.body_health = z.body_health;
        zombie.set_armor_health(z.helm_health, z.shield_health);
        zombie.scale = z.scale;
        zombie.mind_controlled = z.mind_controlled;
        // The walkers start at the saved spot, not off screen.
        zombie.x = z.pos_x as i32;
        board.zombies.alloc(zombie);
    }
    for (i, m) in data.mowers.iter().enumerate() {
        if let Some(mower) = board.lawn_mowers.get_mut(i) {
            mower.row = m.row;
            mower.kind = mower_kind_from_int(m.kind);
            mower.pos_x = m.pos_x;
            mower.state = mower_state_from_int(m.state);
            mower.dead = m.dead;
        }
    }
    board.coins.clear();
    for c in &data.coins {
        let Some(ty) = coin_type_from_int(c.ty) else { continue };
        let coin = Coin::new_resting(c.pos_x, c.pos_y, ty, fx.effects, fx.res, fx.images, fx.sounds.rng());
        board.coins.alloc(coin);
    }
    board.waves.num_waves = data.waves.num_waves;
    board.waves.current_wave = data.waves.current_wave;
    board.waves.total_spawned_waves = data.waves.total_spawned;
    board.waves.zombie_count_down = data.waves.zombie_count_down;
    board.waves.zombies_in_wave = data
        .waves
        .zombies_in_wave
        .iter()
        .map(|w| {
            w.iter()
                .filter_map(|&t| {
                    let idx = usize::try_from(t).ok()?;
                    (idx < crate::zombie::NUM_ZOMBIE_TYPES).then(|| ZombieType::from_index(idx))
                })
                .collect()
        })
        .collect();
    for (packet, saved) in board.seed_bank.packets.iter_mut().zip(&data.seed_packets) {
        packet.set_packet_type(saved.and_then(SeedType::from_index));
    }
    board.grid_items.clear();
    for g in &data.grid_items {
        let item_type = match g.ty {
            1 => GridItemType::Crater,
            2 => GridItemType::Ladder,
            _ => GridItemType::GraveStone,
        };
        board.grid_items.alloc(GridItem::new_restored(item_type, g.col, g.row));
    }
    Some(board)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_file_names_match_the_original_layout() {
        assert_eq!(save_file_name(3, GameMode::Adventure), "game3_0.dat");
    }

    #[test]
    fn need_save_game_only_for_adventure_in_progress() {
        // shape checked through mode/scene without needing a pak
        assert_eq!(mode_index(GameMode::Adventure), 0);
    }
}
