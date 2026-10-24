//! The lawn where levels are played (`Board`), and the scene around it:
//! the level intro (`CutScene`), the seed bank, the advice banner and the
//! lawn mowers.
//!
//! So far a level plays its intro, then sun falls and can be collected,
//! plants can be planted (level 1's tutorial included), and the plain
//! zombies come in waves and are shot by peashooters. Lawn mowers run over
//! zombies that get to them, a zombie in the house loses the level, and the
//! last zombie's seed packet wins it.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Board.cpp`: the constructor,
//! `InitLevel`, `PickBackground`, `LoadBackgroundImages`, `InitLawnMowers`,
//! `StartLevel`, `Update`/`UpdateGame`, `DrawGameObjects`, `DrawBackdrop`,
//! `DrawUIBottom`/`DrawUITop`, `DrawLevel`, `DisplayAdvice`/`ClearAdvice` and
//! the grid helpers; `LawnApp::NewGame`, `MakeNewBoard`, `StartPlaying`,
//! `DoConfirmBackToMain` and `DoBackToMain`.

mod coin;
mod cursor;
mod cutscene;
mod lawn_mower;
mod level_end;
mod message;
mod plant;
mod projectile;
mod seed_bank;
mod tutorial;
mod waves;
mod zombie;

use std::collections::HashSet;

use bevy::prelude::*;
use sexy::data_array::DataArray;

use crate::audio::{FoleyType, Music, MusicTune, Sounds};
use crate::dialog::{ButtonMode, DialogId, Dialogs};
use crate::graphics::{Graphics, Justify, SexyColor, SexyRect};
use crate::input::GameCursor;
use crate::player::{PlayerInfo, ProfileMgr};
use crate::plant::{SeedType, seeds_available};
use crate::particle::{ParticleEffect, draw_particle_system};
use crate::reanim::{EffectSystem, ParticleSystemId, ReanimationType};
use crate::resources::GameResources;
use crate::widgets::draw_stone_button;
use crate::{AppState, UiSet};

use coin::{Coin, CoinEvent, CoinMotion, CoinType};
use cursor::{CursorObject, CursorPreview};
use cutscene::CutScene;
use lawn_mower::LawnMower;
use message::{MessageStyle, MessageWidget};
use plant::{Plant, PlantAction};
use projectile::{Projectile, ProjectileType};
use seed_bank::SeedBank;
use tutorial::{AdviceType, TutorialState};
use waves::WaveState;
use zombie::{Fx, Zombie};

pub struct BoardPlugin;

impl Plugin for BoardPlugin {
    fn build(&self, app: &mut App) {
        let open = resource_exists::<Board>;
        app.add_systems(OnEnter(AppState::Board), new_game)
            .add_systems(OnExit(AppState::Board), kill_board)
            .add_systems(FixedUpdate, update_board.run_if(open))
            .add_systems(Update, board_input.in_set(UiSet::ScreenInput).run_if(open))
            .add_systems(Update, draw_board.in_set(UiSet::ScreenDraw).run_if(open));
    }
}

pub const BOARD_WIDTH: i32 = 800;
pub const BOARD_HEIGHT: i32 = 600;
/// How far left of the lawn the backgrounds start (they include the house).
pub const BOARD_OFFSET: i32 = 220;
/// Width of a background image less the house part to its left.
pub const BOARD_IMAGE_WIDTH_OFFSET: i32 = 1180;
pub const LAWN_YMIN: i32 = 80;
pub const MAX_GRID_SIZE_X: usize = 9;
pub const MAX_GRID_SIZE_Y: usize = 6;
pub const SEED_BANK_OFFSET_X: i32 = 0;
pub const SEED_BANK_OFFSET_X_END: i32 = 10;
const LEVELS_PER_AREA: i32 = 10;
const FINAL_LEVEL: i32 = 50;

/// `GameMode`. Only adventure mode so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Adventure,
}

/// `GameScenes`, for the parts of the game that are on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameScene {
    LevelIntro,
    Playing,
    /// A zombie got into the house.
    ZombiesWon,
}

/// `BackgroundType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundType {
    Day,
    Night,
    Pool,
    Fog,
    Roof,
    Boss,
}

impl BackgroundType {
    pub fn is_night(self) -> bool {
        matches!(self, Self::Night | Self::Fog | Self::Boss)
    }

    pub fn has_pool(self) -> bool {
        matches!(self, Self::Pool | Self::Fog)
    }

    pub fn has_roof(self) -> bool {
        matches!(self, Self::Roof | Self::Boss)
    }
}

/// `GetCircleRectOverlap`.
fn circle_rect_overlap(cx: i32, cy: i32, radius: i32, rect: zombie::IRect) -> bool {
    let dx = if cx < rect.x {
        rect.x - cx
    } else if cx > rect.x + rect.w {
        cx - rect.x - rect.w
    } else {
        0
    };
    let dy = if cy < rect.y {
        rect.y - cy
    } else if cy > rect.y + rect.h {
        cy - rect.y - rect.h
    } else {
        0
    };
    dx * dx + dy * dy <= radius * radius
}

/// `GridToPixelY` for the grid's left column.
pub fn grid_to_pixel_y(background: BackgroundType, grid_x: i32, grid_y: i32) -> i32 {
    if background.has_roof() {
        let slope = if grid_x < 5 { (5 - grid_x) * 20 } else { 0 };
        grid_y * 85 + slope + LAWN_YMIN - 10
    } else if background.has_pool() {
        grid_y * 85 + LAWN_YMIN
    } else {
        grid_y * 100 + LAWN_YMIN
    }
}

/// `GetPosYBasedOnRow`.
pub fn pos_y_based_on_row(background: BackgroundType, x: f32, row: i32) -> f32 {
    if background.has_roof() {
        let slope = if x < 440.0 { (440.0 - x) * 0.25 } else { 0.0 };
        return grid_to_pixel_y(background, 8, row) as f32 + slope;
    }
    grid_to_pixel_y(background, 0, row) as f32
}

/// `PlantRowType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlantRowType {
    Normal,
    Dirt,
    Pool,
}

/// `GridSquareType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridSquareType {
    Grass,
    Dirt,
    Pool,
}

/// `RenderLayer`: the board draws everything sorted by these, plus the row
/// times [`render_layer::ROW_OFFSET`], plus a small offset.
#[allow(dead_code, reason = "the full set; objects use more of it as they're ported")]
pub mod render_layer {
    pub const ROW_OFFSET: i32 = 10000;
    pub const UI_BOTTOM: i32 = 100000;
    pub const GROUND: i32 = 200000;
    pub const LAWN: i32 = 300000;
    pub const GRAVE_STONE: i32 = 301000;
    pub const PLANT: i32 = 302000;
    pub const ZOMBIE: i32 = 303000;
    pub const BOSS: i32 = 304000;
    pub const PROJECTILE: i32 = 305000;
    pub const LAWN_MOWER: i32 = 306000;
    pub const PARTICLE: i32 = 307000;
    pub const TOP: i32 = 400000;
    pub const FOG: i32 = 500000;
    pub const COIN_BANK: i32 = 600000;
    pub const UI_TOP: i32 = 700000;
    pub const ABOVE_UI: i32 = 800000;
    pub const SCREEN_FADE: i32 = 900000;
}

/// `Board::MakeRenderOrder`.
pub fn make_render_order(layer: i32, row: i32, offset: i32) -> i32 {
    row * render_layer::ROW_OFFSET + layer + offset
}

/// The board's "Menu" button (a `GameButton` drawn as a stone button).
#[derive(Debug, Clone)]
struct MenuButton {
    rect: IRect,
    label: String,
    no_draw: bool,
    disabled: bool,
    is_over: bool,
    is_down: bool,
}

impl MenuButton {
    /// `GameButton::IsMouseOver`.
    fn is_mouse_over(&self) -> bool {
        self.is_over && !self.disabled && !self.no_draw
    }
}

/// The level being played.
#[derive(Resource)]
pub struct Board {
    pub mode: GameMode,
    pub scene: GameScene,
    /// The board widget's position: the level intro pans it sideways.
    pub x: i32,
    pub y: i32,
    pub level: i32,
    /// `LawnApp::IsFirstTimeAdventureMode`: the player hasn't finished adventure yet.
    pub first_time_adventure: bool,
    pub background: BackgroundType,
    pub plant_row: [PlantRowType; MAX_GRID_SIZE_Y],
    pub grid_square_type: [[GridSquareType; MAX_GRID_SIZE_Y]; MAX_GRID_SIZE_X],
    pub sun_money: i32,
    /// `Challenge::mConveyorBeltCounter`: ticks until the next seed packet rides in.
    conveyor_belt_counter: i32,
    last_conveyor_seed: Option<SeedType>,
    /// `Challenge::mShowBowlingLine`.
    show_bowling_line: bool,
    /// The player's money, as shown by the coin bank (the profile holds the real total).
    pub player_coins: i32,
    /// Money collected since the last tick, for the profile.
    pub pending_money: i32,
    coin_bank_fade_count: i32,
    dropped_first_coin: bool,
    packet_upgrade_bought: bool,
    /// How far the intro's sod has rolled out, 0..=1000.
    pub sod_position: i32,
    pub main_counter: i32,
    pub sun_count_down: i32,
    pub draw_count: u32,
    pub paused: bool,
    pub show_shovel: bool,
    pub out_of_money_counter: i32,
    pub seed_bank: SeedBank,
    cut_scene: CutScene,
    advice: MessageWidget,
    /// The hint the banner is showing, if it's one shown once per level.
    help_index: Option<AdviceType>,
    help_displayed: HashSet<AdviceType>,
    lawn_mowers: Vec<LawnMower>,
    plants: DataArray<Plant>,
    zombies: DataArray<Zombie>,
    projectiles: DataArray<Projectile>,
    waves: WaveState,
    plant_actions: Vec<PlantAction>,
    shake_counter: i32,
    shake_amount: (i32, i32),
    /// A zombie reached the house this tick (`Board::ZombiesWon`).
    zombies_won_trigger: Option<sexy::data_array::Id<Zombie>>,
    /// The last zombie's award has dropped (`mLevelAwardSpawned`).
    level_award_spawned: bool,
    /// Counts down from the award being picked up to the level ending (-1 when not).
    board_fade_out_counter: i32,
    /// The award was picked up; the board will fade out the level next tick.
    pending_fade_out: bool,
    /// The level is over and the app should move on (`mLevelComplete`).
    level_complete: bool,
    /// The lost-level cutscene is over: show the game over dialog.
    pending_game_over: bool,
    coins: DataArray<Coin>,
    cursor: CursorObject,
    cursor_preview: CursorPreview,
    tutorial_state: TutorialState,
    tutorial_timer: i32,
    shown_more_sun_tutorial: bool,
    num_suns_fallen: i32,
    /// The mouse in board coordinates, if it's over the window.
    mouse: Option<Vec2>,
    menu_button: MenuButton,
    /// The level 1 tutorial's bouncing arrow (`mTutorialParticleID`).
    tutorial_arrow: Option<ParticleSystemId>,
    /// Particle effects started where the effect system isn't at hand,
    /// started at the end of the tick or frame.
    particle_requests: Vec<ParticleRequest>,
}

/// A queued change to the board's particle effects.
enum ParticleRequest {
    Add { x: f32, y: f32, render_order: i32, effect: ParticleEffect, tutorial_arrow: bool },
    RemoveTutorialArrow,
}

/// The next adventure level of `player` (`PlayerInfo::GetLevel`).
fn adventure_level(player: Option<&PlayerInfo>) -> i32 {
    player.map_or(1, |p| p.level)
}

impl Board {
    /// `Board::Board` + `InitLevel`.
    fn new(
        mode: GameMode,
        player: Option<&PlayerInfo>,
        resources: &mut GameResources,
        images: &mut Assets<Image>,
        rng: &mut sexy::rand::MtRand,
    ) -> Self {
        let level = match mode {
            GameMode::Adventure => adventure_level(player),
        };
        let first_time_adventure = mode == GameMode::Adventure && !player.is_some_and(PlayerInfo::has_finished_adventure);
        let mut board = Self {
            mode,
            scene: GameScene::LevelIntro,
            x: 0,
            y: 0,
            level,
            first_time_adventure,
            background: BackgroundType::Day,
            plant_row: [PlantRowType::Normal; MAX_GRID_SIZE_Y],
            grid_square_type: [[GridSquareType::Grass; MAX_GRID_SIZE_Y]; MAX_GRID_SIZE_X],
            sun_money: 50,
            conveyor_belt_counter: 0,
            last_conveyor_seed: None,
            show_bowling_line: false,
            player_coins: player.map_or(0, |p| p.coins),
            pending_money: 0,
            coin_bank_fade_count: 0,
            dropped_first_coin: false,
            packet_upgrade_bought: player.is_some_and(|p| p.purchases[21] >= 1),
            sod_position: 0,
            main_counter: 0,
            sun_count_down: 0,
            draw_count: 0,
            paused: false,
            show_shovel: false,
            out_of_money_counter: 0,
            seed_bank: SeedBank::new(resources),
            cut_scene: CutScene::default(),
            advice: MessageWidget::default(),
            help_index: None,
            help_displayed: HashSet::new(),
            lawn_mowers: Vec::new(),
            plants: DataArray::default(),
            zombies: DataArray::default(),
            projectiles: DataArray::default(),
            waves: WaveState::default(),
            plant_actions: Vec::new(),
            shake_counter: 0,
            shake_amount: (0, 0),
            zombies_won_trigger: None,
            level_award_spawned: false,
            board_fade_out_counter: -1,
            pending_fade_out: false,
            level_complete: false,
            pending_game_over: false,
            coins: DataArray::default(),
            cursor: CursorObject::default(),
            cursor_preview: CursorPreview::default(),
            tutorial_state: TutorialState::Off,
            tutorial_timer: -1,
            shown_more_sun_tutorial: false,
            num_suns_fallen: 0,
            mouse: None,
            tutorial_arrow: None,
            particle_requests: Vec::new(),
            menu_button: MenuButton {
                rect: IRect::new(681, -10, 681 + 117, -10 + 46),
                label: resources.translate("[MENU_BUTTON]"),
                no_draw: false,
                disabled: false,
                is_over: false,
                is_down: false,
            },
        };

        // InitLevel.
        board.pick_background(resources, images);
        board.sun_money = if board.first_time_adventure && level == 1 { 150 } else { 50 };
        if !board.stage_is_night() {
            board.sun_count_down = rng.range_int(425, 700);
        }
        let num_packets = board.num_seeds_in_bank(player);
        board.seed_bank.conveyor = board.has_conveyor_belt_seed_bank();
        board.seed_bank.num_packets = num_packets;
        board.seed_bank.update_width(resources);
        // TODO: the seed chooser; until it exists every level gets the
        // first plants in order, as levels without choosing do.
        if !board.has_conveyor_belt_seed_bank() {
            for i in 0..num_packets {
                let seed = SeedType::from_index(i as i32);
                board.seed_bank.packets[i].set_packet_type(seed);
            }
        }
        board.init_zombie_waves(rng);
        // Challenge::InitLevel for wall-nut bowling.
        if board.is_wallnut_bowling_level() {
            board.seed_bank.add_seed(SeedType::Wallnut);
            board.conveyor_belt_counter = 400;
            board.show_bowling_line = true;
        }
        board
    }

    /// `Board::PickBackground` (adventure mode) and `LoadBackgroundImages`.
    fn pick_background(&mut self, resources: &mut GameResources, images: &mut Assets<Image>) {
        let level = self.level;
        self.background = match level {
            l if l <= LEVELS_PER_AREA => BackgroundType::Day,
            l if l <= 2 * LEVELS_PER_AREA => BackgroundType::Night,
            l if l <= 3 * LEVELS_PER_AREA => BackgroundType::Pool,
            // TODO: 4-5 (a scary potter level) is at night.
            l if l <= 4 * LEVELS_PER_AREA => BackgroundType::Fog,
            l if l < FINAL_LEVEL => BackgroundType::Roof,
            FINAL_LEVEL => BackgroundType::Boss,
            _ => BackgroundType::Day,
        };

        let mut groups = vec![match self.background {
            BackgroundType::Day => "DelayLoad_Background1",
            BackgroundType::Night => "DelayLoad_Background2",
            BackgroundType::Pool => "DelayLoad_Background3",
            BackgroundType::Fog => "DelayLoad_Background4",
            BackgroundType::Roof => "DelayLoad_Background5",
            BackgroundType::Boss => "DelayLoad_Background6",
        }];
        if self.background == BackgroundType::Day && level <= 4 {
            groups.push("DelayLoad_BackgroundUnsodded");
        }
        for group in groups {
            if let Err(e) = resources.load_group_now(group, images) {
                error!("{e}");
            }
        }

        use PlantRowType::*;
        self.plant_row = match self.background {
            BackgroundType::Day | BackgroundType::Night | BackgroundType::Roof | BackgroundType::Boss => {
                [Normal, Normal, Normal, Normal, Normal, Dirt]
            }
            BackgroundType::Pool | BackgroundType::Fog => [Normal, Normal, Pool, Pool, Normal, Normal],
        };
        if self.background == BackgroundType::Day && self.first_time_adventure {
            match level {
                1 => {
                    for row in [0, 1, 3, 4] {
                        self.plant_row[row] = Dirt;
                    }
                }
                2 | 3 => {
                    self.plant_row[0] = Dirt;
                    self.plant_row[4] = Dirt;
                }
                _ => {}
            }
        }
        for x in 0..MAX_GRID_SIZE_X {
            for y in 0..MAX_GRID_SIZE_Y {
                self.grid_square_type[x][y] = match self.plant_row[y] {
                    Dirt => GridSquareType::Dirt,
                    Pool => GridSquareType::Pool,
                    Normal => GridSquareType::Grass,
                };
            }
        }
        // TODO: grave stones on night levels.
    }

    pub fn stage_is_night(&self) -> bool {
        self.background.is_night()
    }

    pub fn stage_has_pool(&self) -> bool {
        self.background.has_pool()
    }

    pub fn stage_has_roof(&self) -> bool {
        self.background.has_roof()
    }

    pub fn stage_has_fog(&self) -> bool {
        self.background == BackgroundType::Fog
    }

    /// `ChooseSeedsOnCurrentLevel`: early adventure levels hand out the plants.
    pub fn choose_seeds_on_current_level(&self) -> bool {
        !self.first_time_adventure || self.level > 7
    }

    /// `GetNumSeedsInBank`.
    fn num_seeds_in_bank(&self, player: Option<&PlayerInfo>) -> usize {
        if self.has_conveyor_belt_seed_bank() {
            return 10;
        }
        let Some(player) = player else { return 0 };
        // TODO: STORE_ITEM_PACKET_UPGRADE adds slots once purchases exist.
        let slots = 6;
        slots.min(seeds_available(player)).max(0) as usize
    }

    /// `LawnApp::IsWallnutBowlingLevel`, for adventure mode.
    pub fn is_wallnut_bowling_level(&self) -> bool {
        self.mode == GameMode::Adventure && self.level == 5
    }

    /// `Board::HasConveyorBeltSeedBank`.
    pub fn has_conveyor_belt_seed_bank(&self) -> bool {
        self.is_wallnut_bowling_level()
    }

    /// `GridToPixelY` for the grid's left column.
    pub fn grid_to_pixel_y(&self, grid_x: i32, grid_y: i32) -> i32 {
        grid_to_pixel_y(self.background, grid_x, grid_y)
    }

    /// `GetPosYBasedOnRow`.
    pub fn pos_y_based_on_row(&self, x: f32, row: i32) -> f32 {
        pos_y_based_on_row(self.background, x, row)
    }

    /// `Board::InitLawnMowers`: one per row that zombies can walk down.
    fn init_lawn_mowers(&mut self) {
        // TODO: the roof needs the roof cleaner upgrade, and pool rows the pool cleaner.
        if self.stage_has_roof() {
            return;
        }
        for row in 0..MAX_GRID_SIZE_Y as i32 {
            if self.plant_row[row as usize] != PlantRowType::Dirt {
                let mut mower = LawnMower::new(self, row);
                mower.visible = false;
                self.lawn_mowers.push(mower);
            }
        }
    }

    fn find_lawn_mower_in_row(&mut self, row: i32) -> Option<&mut LawnMower> {
        self.lawn_mowers.iter_mut().find(|m| m.row == row)
    }

    /// `LawnApp::StartPlaying` + `Board::StartLevel`.
    fn start_playing(&mut self, music: &mut Music) {
        self.scene = GameScene::Playing;
        music.make_sure_music_is_playing(self.game_music());
    }

    /// `Music::StartGameMusic`'s choice of tune.
    fn game_music(&self) -> MusicTune {
        if self.is_wallnut_bowling_level() {
            MusicTune::MinigameLoonboon
        } else if self.mode == GameMode::Adventure && matches!(self.level, 10 | 20 | 30) {
            MusicTune::Conveyer
        } else if self.level == FINAL_LEVEL {
            MusicTune::FinalBossBrainiacManiac
        } else if self.stage_has_fog() {
            MusicTune::FogRigormormist
        } else if self.stage_is_night() {
            MusicTune::NightMoongrains
        } else if self.stage_has_pool() {
            MusicTune::PoolWaterygraves
        } else if self.stage_has_roof() {
            MusicTune::RoofGrazetheroof
        } else {
            MusicTune::DayGrasswalk
        }
    }

    /// `LawnApp::GetStageString`: "1-1" and so on.
    fn stage_string(&self) -> String {
        let area = ((self.level - 1) / LEVELS_PER_AREA + 1).clamp(1, 6);
        format!("{}-{}", area, self.level - (area - 1) * LEVELS_PER_AREA)
    }
}

/// `IRect`-like rectangle for the menu button, in board coordinates.
#[derive(Debug, Clone, Copy)]
struct IRect {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl IRect {
    const fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    fn contains(&self, p: IVec2) -> bool {
        p.x >= self.x0 && p.y >= self.y0 && p.x < self.x1 && p.y < self.y1
    }
}

/// `LawnApp::PreNewGame` + `NewGame`: a fresh board and its level intro.
fn new_game(
    mut commands: Commands,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut effects: ResMut<EffectSystem>,
    mut music: ResMut<Music>,
    mut sounds: Sounds,
    profiles: Res<ProfileMgr>,
) {
    // TODO: the first adventure ever starts with the intro (GAMEMODE_INTRO);
    // saved games aren't loaded yet either.
    effects.clear();
    music.stop_all_music();
    // CutScene::PreloadResources, for what this stage draws.
    for ty in [ReanimationType::SodRoll, ReanimationType::LawnMower, ReanimationType::Zombie, ReanimationType::Flag, ReanimationType::Polevaulter] {
        if let Err(e) = resources.reanim_definition(ty.file(), &mut images) {
            error!("{e}");
        }
    }
    let player = profiles.player();
    let mut board = Board::new(GameMode::Adventure, player, &mut resources, &mut images, sounds.rng());
    for packet in &board.seed_bank.packets[..board.seed_bank.num_packets] {
        if let Some(seed) = packet.packet_type
            && let Err(e) = resources.reanim_definition(seed.definition().reanim.file(), &mut images)
        {
            error!("{e}");
        }
    }
    let player_name = player.map_or("", |p| p.name.as_str());
    cutscene::start_level_intro(&mut board, &resources, &mut music, player_name);
    {
        let mut fx = Fx { effects: &mut effects, res: &mut resources, images: &mut images, sounds: &mut sounds };
        board.place_street_zombies(&mut fx);
    }
    commands.insert_resource(board);
}

/// `LawnApp::KillBoard`.
fn kill_board(mut commands: Commands, mut effects: ResMut<EffectSystem>, mut music: ResMut<Music>) {
    commands.remove_resource::<Board>();
    effects.clear();
    music.stop_all_music();
}

/// `Board::Update`, at 100 ticks per second.
#[allow(clippy::too_many_arguments)]
fn update_board(
    mut board: ResMut<Board>,
    mut effects: ResMut<EffectSystem>,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut music: ResMut<Music>,
    mut sounds: Sounds,
    mut dialogs: ResMut<Dialogs>,
    mut profiles: ResMut<ProfileMgr>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let board = &mut *board;
    // LawnApp::NeedPauseGame: an open dialog pauses the game.
    board.paused = dialogs.is_modal();

    cutscene::update(board, &mut effects, &mut resources, &mut images, &mut music, &mut sounds);
    if board.paused {
        return;
    }
    if !board.menu_button.no_draw {
        board.menu_button.disabled = !board.can_interact_with_board_buttons();
    }

    effects.update(sounds.rng());
    board.update_shake(sounds.rng());
    board.coin_bank_fade_count = (board.coin_bank_fade_count - 1).max(0);
    if board.pending_money > 0
        && let Some(player) = profiles.player_mut()
    {
        player.coins += std::mem::take(&mut board.pending_money);
    }
    board.advice.update();
    board.update_tutorial(&resources);

    // UpdateGame.
    board.update_game_objects(&mut effects, &mut resources, &mut images, &mut sounds);
    if let Some(zombie) = board.zombies_won_trigger.take() {
        board.zombies_won(zombie, &mut music, &mut sounds, &resources);
    }
    if board.scene == GameScene::ZombiesWon
        && cutscene::update_zombies_won(board, &mut effects, &mut resources, &mut images, &mut sounds)
    {
        board.pending_game_over = true;
    }
    if board.scene == GameScene::Playing {
        board.main_counter += 1;
        board.update_sun_spawning(&mut effects, &mut resources, &mut images, &mut sounds);
        if board.has_conveyor_belt_seed_bank() {
            board.update_conveyor_belt(sounds.rng());
        }
        board.update_zombie_spawning(&mut effects, &mut resources, &mut images, &mut sounds);
        if board.main_counter == 1 && board.first_time_adventure && board.level == 1 {
            board.set_tutorial_state(&resources, TutorialState::Level1PickUpPeashooter);
        } else if board.main_counter == 1 && board.first_time_adventure && board.level == 2 {
            board.set_tutorial_state(&resources, TutorialState::Level2PickUpSunflower);
            board.display_advice(&resources, "[ADVICE_PLANT_SUNFLOWER1]", MessageStyle::TutorialLevel2, None);
            board.tutorial_timer = 500;
        }
        board.update_progress_meter();
    }
    if board.pending_fade_out {
        board.fade_out_level(&mut music, &mut sounds, &resources);
    }
    board.update_level_end_sequence(&mut sounds);
    board.process_delete_queue();
    if board.pending_game_over {
        board.pending_game_over = false;
        do_game_over_dialog(&mut dialogs, &resources);
    }
    if board.level_complete {
        // LawnApp::CheckForGameEnd. TODO: the award screen shows the new plant first.
        board.finish_level(&mut profiles);
        next_state.set(AppState::Board);
    }
    board.flush_particle_requests(&mut effects, &mut resources, &mut images, sounds.rng());
}

impl Board {
    /// Queues a particle effect (`LawnApp::AddTodParticle`).
    fn add_particle(&mut self, x: f32, y: f32, render_order: i32, effect: ParticleEffect) {
        self.particle_requests.push(ParticleRequest::Add { x, y, render_order, effect, tutorial_arrow: false });
    }

    /// Starts and removes queued particle effects.
    fn flush_particle_requests(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        rng: &mut sexy::rand::MtRand,
    ) {
        for request in std::mem::take(&mut self.particle_requests) {
            match request {
                ParticleRequest::Add { x, y, render_order, effect, tutorial_arrow } => {
                    let id = effects.add_particle(x, y, render_order, effect, res, images, rng);
                    if tutorial_arrow {
                        self.tutorial_arrow = id;
                    }
                }
                ParticleRequest::RemoveTutorialArrow => {
                    if let Some(id) = self.tutorial_arrow.take() {
                        effects.remove_particle(id);
                    }
                }
            }
        }
    }
}

impl Board {
    /// `Board::UpdateGameObjects`.
    fn update_game_objects(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        let playing = self.scene == GameScene::Playing;
        let zombies_won = self.scene == GameScene::ZombiesWon;
        if playing {
            let row_y: [i32; MAX_GRID_SIZE_Y] = std::array::from_fn(|row| self.grid_to_pixel_y(0, row as i32));
            for (_, plant) in self.plants.iter_mut() {
                plant.update(&self.zombies, &mut self.plant_actions, effects, res, images, &row_y, sounds.rng());
            }
        }
        let env = self.zombie_env();
        let show_walking = self.cut_scene.show_zombie_walking();
        {
            let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
            for (id, zombie) in self.zombies.iter_mut() {
                if playing {
                    zombie.update(&mut self.plants, &env, &mut fx);
                    if zombie.reached_house() {
                        self.zombies_won_trigger.get_or_insert(id);
                    }
                } else if zombies_won && zombie.is_winner() {
                    zombie.update_winner(show_walking, &mut self.plants, &mut fx);
                }
                if !zombie.is_on_board() {
                    zombie.update_street(fx.effects);
                }
            }
            if playing {
                for (_, projectile) in self.projectiles.iter_mut() {
                    projectile.update(&mut self.zombies, &env, &mut fx);
                }
            }
            for mower in &mut self.lawn_mowers {
                let background = self.background;
                mower.update(
                    playing,
                    |x, row| pos_y_based_on_row(background, x, row),
                    &mut self.zombies,
                    self.waves.current_wave,
                    &mut self.waves.wave_row_got_lawn_mowered,
                    &mut fx,
                );
            }
        }
        if playing {
            for action in std::mem::take(&mut self.plant_actions) {
                self.do_plant_action(action, effects, res, images, sounds);
            }
            self.collect_zombie_loot(effects, res, images, sounds);
            let mut scored = 0;
            for (_, coin) in self.coins.iter_mut() {
                match coin.update(effects, res, images, sounds.rng()) {
                    Some(CoinEvent::Scored(sun)) => scored += sun,
                    Some(CoinEvent::Money(money)) => {
                        self.player_coins += money;
                        self.pending_money += money;
                    }
                    Some(CoinEvent::Landed(CoinType::Gold)) => sounds.play_foley(FoleyType::MoneyFalls),
                    Some(CoinEvent::Landed(_)) | None => {}
                }
            }
            self.add_sun_money(scored);
        }
        self.update_cursor_preview();
        for i in 0..self.seed_bank.num_packets {
            if self.seed_bank.packets[i].update(self.scene, self.main_counter) {
                self.flash_if_ready(res, i);
            }
        }
    }

    /// `Board::ShakeBoard`.
    fn shake_board(&mut self, amount_x: i32, amount_y: i32) {
        self.shake_counter = 12;
        self.shake_amount = (amount_x, amount_y);
    }

    /// The shake part of `Board::Update`: the board jolts for a moment.
    fn update_shake(&mut self, rng: &mut sexy::rand::MtRand) {
        if self.shake_counter == 0 {
            return;
        }
        self.shake_counter -= 1;
        if self.shake_counter == 0 {
            (self.x, self.y) = (0, 0);
            return;
        }
        if rng.next_range(3) == 0 {
            self.shake_amount.0 = -self.shake_amount.0;
        }
        let bounce = |amount| sexy::tod::animate_curve(12, 0, self.shake_counter, 0, amount, sexy::tod::TodCurve::Bounce);
        self.x = bounce(self.shake_amount.0);
        self.y = bounce(self.shake_amount.1);
    }

    /// `Board::KillAllZombiesInRadius`: with `burn`, the zombies are
    /// charred, otherwise blown to bits.
    #[allow(clippy::too_many_arguments)]
    fn kill_all_zombies_in_radius(
        &mut self,
        row: i32,
        x: i32,
        y: i32,
        radius: i32,
        row_range: i32,
        burn: bool,
        env: &zombie::ZombieEnv,
        fx: &mut Fx,
    ) {
        for (_, zombie) in self.zombies.iter_mut() {
            if !zombie.effected_by_damage(zombie::damage_range::GROUND) {
                continue;
            }
            let row_dist = zombie.row - row;
            if row_dist.abs() <= row_range && circle_rect_overlap(x, y, radius, zombie.zombie_rect()) {
                if burn {
                    zombie.apply_burn(env, fx);
                } else {
                    zombie.take_damage(1800, 18, env, fx);
                }
            }
        }
    }

    /// Carries out what a plant did this tick.
    fn do_plant_action(
        &mut self,
        action: PlantAction,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        match action {
            PlantAction::Shoot { x, y, row, render_order, snow } => {
                sounds.play_foley(FoleyType::Throw);
                let grid_x = self.pixel_to_grid_x(x).max(0);
                let shadow_y = self.grid_to_pixel_y(grid_x, row) as f32 + 67.0;
                // TODO: shadows sit 12 pixels higher on the roof's left side.
                let ty = if snow { ProjectileType::Snowpea } else { ProjectileType::Pea };
                self.projectiles.alloc(Projectile::new(x, y, render_order, row, shadow_y, ty));
            }
            PlantAction::Particle { x, y, render_order, effect } => {
                self.add_particle(x as f32, y as f32, render_order, effect);
            }
            PlantAction::Foley(foley) => sounds.play_foley(foley),
            PlantAction::EatZombie(id) => {
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                if let Some(zombie) = self.zombies.get_mut(id) {
                    zombie.die_with_loot(&mut fx);
                }
            }
            PlantAction::CherryBomb { x, y, row } => {
                let env = self.zombie_env();
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                self.kill_all_zombies_in_radius(row, x, y, 115, 1, true, &env, &mut fx);
                self.add_particle(x as f32, y as f32, render_layer::TOP, ParticleEffect::Powie);
                self.shake_board(3, -4);
            }
            PlantAction::PotatoMine { x, y, row } => {
                sounds.play_sample("SOUND_POTATO_MINE");
                let env = self.zombie_env();
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                self.kill_all_zombies_in_radius(row, x, y, 60, 0, false, &env, &mut fx);
                let order = make_render_order(render_layer::PARTICLE, row, 0);
                self.add_particle((x + 20) as f32, y as f32, order, ParticleEffect::PotatoMine);
                self.shake_board(3, -4);
            }
            PlantAction::BowlingHit(id) => {
                sounds.play_foley(FoleyType::BowlingImpact);
                self.shake_board(1, -2);
                let env = self.zombie_env();
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                if let Some(zombie) = self.zombies.get_mut(id) {
                    zombie.bowling_damage(&env, &mut fx);
                }
            }
            PlantAction::ExplodeONut { x, y, row } => {
                sounds.play_foley(FoleyType::Cherrybomb);
                sounds.play_sample("SOUND_BOWLINGIMPACT2");
                let env = self.zombie_env();
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                self.kill_all_zombies_in_radius(row, x, y, 90, 1, true, &env, &mut fx);
                self.add_particle(x as f32, y as f32, render_layer::TOP, ParticleEffect::Powie);
                self.shake_board(3, -4);
            }
            PlantAction::MakeSun { x, y } => {
                sounds.play_foley(FoleyType::SpawnSun);
                self.add_coin(x as f32, y as f32, CoinType::Sun, CoinMotion::FromPlant, effects, res, images, sounds);
            }
        }
    }

    /// `Board::ProcessDeleteQueue`: frees dead plants, zombies, projectiles and coins.
    fn process_delete_queue(&mut self) {
        self.plants.retain(|p| !p.dead);
        self.zombies.retain(|z| !z.dead);
        self.lawn_mowers.retain(|m| !m.dead);
        self.projectiles.retain(|p| !p.dead);
        self.coins.retain(|c| !c.dead);
    }

    /// `Board::UpdateSunSpawning`: sun falls from the sky every few seconds
    /// on day levels (not before the level 1 tutorial's first plant).
    fn update_sun_spawning(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        if self.stage_is_night() || self.has_level_award_dropped() || self.has_conveyor_belt_seed_bank() {
            return;
        }
        let waiting = matches!(self.tutorial_state, TutorialState::Level1PickUpPeashooter | TutorialState::Level1PlantPeashooter);
        if waiting && self.plants.is_empty() {
            return;
        }
        self.sun_count_down -= 1;
        if self.sun_count_down != 0 {
            return;
        }
        self.num_suns_fallen += 1;
        let rng = sounds.rng();
        self.sun_count_down = SUN_COUNTDOWN_MAX.min(SUN_COUNTDOWN + self.num_suns_fallen * 10)
            + rng.next_range(SUN_COUNTDOWN_RANGE as u32) as i32;
        let x = rng.range_int(100, 649);
        self.add_coin(x as f32, 60.0, CoinType::Sun, CoinMotion::FromSky, effects, res, images, sounds);
    }

    /// `Board::AddCoin`.
    #[allow(clippy::too_many_arguments)]
    fn add_coin(
        &mut self,
        x: f32,
        y: f32,
        ty: CoinType,
        motion: CoinMotion,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        let coin = Coin::new(x, y, ty, motion, effects, res, images, sounds.rng());
        self.coins.alloc(coin);
        if self.first_time_adventure && self.level == 1 {
            let style = MessageStyle::TutorialLevel1Stay;
            self.display_advice(res, "[ADVICE_CLICK_ON_SUN]", style, Some(AdviceType::ClickOnSun));
        }
    }
}

/// Sun falls every `SUN_COUNTDOWN` ticks plus 10 per sun so far (up to
/// `SUN_COUNTDOWN_MAX`), plus up to `SUN_COUNTDOWN_RANGE` at random.
const SUN_COUNTDOWN: i32 = 425;
const SUN_COUNTDOWN_RANGE: i32 = 275;
const SUN_COUNTDOWN_MAX: i32 = 950;

impl Board {
    /// `Board::CanInteractWithBoardButtons`: not while carrying something.
    fn can_interact_with_board_buttons(&self) -> bool {
        !self.paused && self.cursor.cursor_type == cursor::CursorType::Normal
    }
}

/// Mouse handling: `Board::MouseDown`/`MouseUp` and the menu button
/// (`GameButton::Update`). TODO: the hand cursor over things to click.
#[allow(clippy::too_many_arguments)]
fn board_input(
    mut board: ResMut<Board>,
    cursor: Res<GameCursor>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut dialogs: ResMut<Dialogs>,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut effects: ResMut<EffectSystem>,
    mut sounds: Sounds,
) {
    let board = &mut *board;
    board.mouse = cursor.0.map(|p| p - Vec2::new(board.x as f32, board.y as f32));
    // A dialog takes the mouse.
    let modal = dialogs.is_modal();
    let screen = cursor.0.filter(|_| !modal);
    let button = &mut board.menu_button;
    button.is_over = screen.is_some_and(|p| button.rect.contains(p.floor().as_ivec2()));
    button.is_down = mouse.pressed(MouseButton::Left) && button.is_over;
    let Some(p) = board.mouse.filter(|_| !modal) else { return };

    if mouse.just_pressed(MouseButton::Left) {
        if board.menu_button.is_mouse_over() && board.can_interact_with_board_buttons() {
            sounds.play_sample("SOUND_GRAVEBUTTON");
        } else {
            board.mouse_down(p, false, &mut effects, &mut resources, &mut images, &mut sounds);
        }
    }
    if mouse.just_pressed(MouseButton::Right) {
        board.mouse_down(p, true, &mut effects, &mut resources, &mut images, &mut sounds);
    }
    if mouse.just_released(MouseButton::Left)
        && board.menu_button.is_mouse_over()
        && board.can_interact_with_board_buttons()
    {
        let button = &mut board.menu_button;
        button.is_over = false;
        button.is_down = false;
        sounds.play_sample("SOUND_PAUSE");
        // TODO: the options dialog (NewOptionsDialog), whose "Main Menu"
        // button leads here.
        do_confirm_back_to_main(&mut dialogs, &resources);
    }
    board.flush_particle_requests(&mut effects, &mut resources, &mut images, sounds.rng());
}

/// `GameOverDialog`: try again or go back to the main menu. TODO: the
/// original puts the Main Menu button at the top right instead.
fn do_game_over_dialog(dialogs: &mut Dialogs, res: &GameResources) {
    let dialog = dialogs.do_dialog(res, DialogId::GameOver, &res.translate("[GAME_OVER]"), "", "", ButtonMode::YesNo);
    if let Some(yes) = &mut dialog.yes_button {
        yes.label = res.translate("[TRY_AGAIN]");
    }
    if let Some(no) = &mut dialog.no_button {
        no.label = res.translate("[MAIN_MENU_BUTTON]");
    }
}

/// `LawnApp::DoConfirmBackToMain`.
fn do_confirm_back_to_main(dialogs: &mut Dialogs, res: &GameResources) {
    let lines = "Do you want to return\nto the main menu?\n\nYour game will be saved.";
    let dialog = dialogs.do_dialog(res, DialogId::ConfirmBackToMain, "Leave Game?", lines, "", ButtonMode::YesNo);
    if let Some(yes) = &mut dialog.yes_button {
        yes.label = res.translate("[LEAVE_BUTTON]");
    }
    if let Some(no) = &mut dialog.no_button {
        no.label = res.translate("[DIALOG_BUTTON_CANCEL]");
    }
}

/// What the board draws, sorted by render order (`RenderObjectType`).
enum RenderItem {
    Backdrop,
    BottomUi,
    TopUi,
    LawnMower(usize),
    Reanimation(crate::reanim::ReanimationId),
    Plant(sexy::data_array::Id<Plant>),
    Zombie(sexy::data_array::Id<Zombie>),
    ZombieShadow(sexy::data_array::Id<Zombie>),
    Projectile(sexy::data_array::Id<Projectile>),
    ProjectileShadow(sexy::data_array::Id<Projectile>),
    Particle(ParticleSystemId),
    FadeOut,
    Coin(sexy::data_array::Id<Coin>),
    CursorPreview,
}

/// `Board::Draw` / `DrawGameObjects`.
fn draw_board(
    mut board: ResMut<Board>,
    effects: Res<EffectSystem>,
    resources: Res<GameResources>,
    mut g: ResMut<Graphics>,
) {
    board.draw_count += 1;
    let board = &*board;

    let mut items: Vec<(i32, RenderItem)> = Vec::new();
    for (id, plant) in board.plants.iter() {
        items.push((plant.render_order, RenderItem::Plant(id)));
    }
    for (id, coin) in board.coins.iter() {
        items.push((coin.render_order, RenderItem::Coin(id)));
    }
    for (id, zombie) in board.zombies.iter() {
        items.push((zombie.render_order, RenderItem::Zombie(id)));
        items.push((make_render_order(render_layer::GROUND, zombie.row, 3), RenderItem::ZombieShadow(id)));
    }
    for (id, projectile) in board.projectiles.iter() {
        items.push((projectile.render_order, RenderItem::Projectile(id)));
        items.push((make_render_order(render_layer::GROUND, projectile.row, 3), RenderItem::ProjectileShadow(id)));
    }
    for (i, mower) in board.lawn_mowers.iter().enumerate() {
        items.push((mower.render_order, RenderItem::LawnMower(i)));
    }
    for (id, reanim) in effects.reanimation_ids() {
        if !reanim.is_attachment {
            items.push((reanim.render_order, RenderItem::Reanimation(id)));
        }
    }
    for (id, system) in effects.particle_ids() {
        if !system.is_attachment {
            items.push((system.render_order, RenderItem::Particle(id)));
        }
    }
    items.push((CursorPreview::RENDER_ORDER, RenderItem::CursorPreview));
    items.push((make_render_order(render_layer::SCREEN_FADE, 0, 0), RenderItem::FadeOut));
    let bottom_ui = if board.scene == GameScene::Playing || board.cut_scene.is_after_seed_chooser() {
        make_render_order(render_layer::UI_BOTTOM, 0, 1)
    } else {
        make_render_order(render_layer::ABOVE_UI, 0, 0)
    };
    items.push((make_render_order(render_layer::UI_BOTTOM, 0, 0), RenderItem::Backdrop));
    items.push((bottom_ui, RenderItem::BottomUi));
    items.push((make_render_order(render_layer::UI_TOP, 0, 0), RenderItem::TopUi));
    // TODO: the coin bank, screen fade and cursor preview.
    items.sort_by_key(|(z, _)| *z);

    g.push_state();
    g.translate(board.x as f32, board.y as f32);
    for (_, item) in items {
        match item {
            RenderItem::Backdrop => draw_backdrop(board, &resources, &mut g),
            RenderItem::BottomUi => draw_ui_bottom(board, &resources, &mut g),
            RenderItem::TopUi => draw_ui_top(board, &resources, &mut g),
            RenderItem::LawnMower(i) => board.lawn_mowers[i].draw(board, &effects, &resources, &mut g),
            RenderItem::Reanimation(id) => effects.draw(&mut g, id, &resources),
            RenderItem::Plant(id) => {
                if let Some(plant) = board.plants.get(id) {
                    plant.draw(&mut g, board.stage_is_night(), &effects, &resources);
                }
            }
            RenderItem::FadeOut => board.draw_fade_out(&mut g),
            RenderItem::Zombie(id) => {
                if let Some(zombie) = board.zombies.get(id).filter(|z| !z.is_winner() || board.cut_scene.show_zombie_walking()) {
                    zombie.draw(&mut g, &effects, &resources);
                }
            }
            RenderItem::ZombieShadow(id) => {
                if let Some(zombie) = board.zombies.get(id).filter(|z| !z.is_winner() || board.cut_scene.show_zombie_walking()) {
                    zombie.draw_shadow(&mut g, board.stage_is_night(), &resources);
                }
            }
            RenderItem::Projectile(id) => {
                if let Some(projectile) = board.projectiles.get(id) {
                    projectile.draw(&mut g, &resources);
                }
            }
            RenderItem::ProjectileShadow(id) => {
                if let Some(projectile) = board.projectiles.get(id) {
                    projectile.draw_shadow(&mut g, board.stage_is_night(), &resources);
                }
            }
            RenderItem::Coin(id) => {
                if let Some(coin) = board.coins.get(id) {
                    coin.draw(&mut g, &effects, &resources);
                }
            }
            RenderItem::CursorPreview => board.draw_cursor_preview(&resources, &mut g),
            RenderItem::Particle(id) => {
                if let Some(system) = effects.particle(id) {
                    draw_particle_system(&mut g, system, &resources);
                }
            }
        }
    }
    g.pop_state();
}

/// `Board::DrawBackdrop`: the background, with the intro's unsodded lawn
/// and rolling sod on the first levels.
fn draw_backdrop(board: &Board, res: &GameResources, g: &mut Graphics) {
    let image = |id| res.image(id);
    let offset = -BOARD_OFFSET as f32;
    let sod_width = |w: u32| sexy::tod::animate_curve(0, 1000, board.sod_position, 0, w as i32, sexy::tod::TodCurve::Linear);
    let first_time = board.first_time_adventure;
    match board.level {
        1 if first_time => {
            if let (Some(unsodded), Some(sod)) = (image("IMAGE_BACKGROUND1UNSODDED"), image("IMAGE_SOD1ROW")) {
                g.draw_image(unsodded, offset, 0.0);
                let w = sod_width(sod.width);
                g.draw_image_src(sod, (239 - BOARD_OFFSET) as f32, 265.0, SexyRect::new(0.0, 0.0, w as f32, sod.height as f32));
            }
        }
        2 | 3 if first_time => {
            if let (Some(unsodded), Some(sod1), Some(sod3)) =
                (image("IMAGE_BACKGROUND1UNSODDED"), image("IMAGE_SOD1ROW"), image("IMAGE_SOD3ROW"))
            {
                g.draw_image(unsodded, offset, 0.0);
                g.draw_image(sod1, (239 - BOARD_OFFSET) as f32, 265.0);
                let w = sod_width(sod3.width);
                g.draw_image_src(sod3, (235 - BOARD_OFFSET) as f32, 149.0, SexyRect::new(0.0, 0.0, w as f32, sod3.height as f32));
            }
        }
        4 if first_time => {
            if let (Some(unsodded), Some(sod3), Some(day)) =
                (image("IMAGE_BACKGROUND1UNSODDED"), image("IMAGE_SOD3ROW"), image("IMAGE_BACKGROUND1"))
            {
                g.draw_image(unsodded, offset, 0.0);
                g.draw_image(sod3, (235 - BOARD_OFFSET) as f32, 149.0);
                let w = sexy::tod::animate_curve(0, 1000, board.sod_position, 0, 773, sexy::tod::TodCurve::Linear);
                g.draw_image_src(day, (232 - BOARD_OFFSET) as f32, 0.0, SexyRect::new(232.0, 0.0, w as f32, day.height as f32));
            }
        }
        _ => {
            let id = match board.background {
                BackgroundType::Day => "IMAGE_BACKGROUND1",
                BackgroundType::Night => "IMAGE_BACKGROUND2",
                BackgroundType::Pool => "IMAGE_BACKGROUND3",
                BackgroundType::Fog => "IMAGE_BACKGROUND4",
                BackgroundType::Roof => "IMAGE_BACKGROUND5",
                BackgroundType::Boss => "IMAGE_BACKGROUND6BOSS",
            };
            if let Some(bg) = image(id) {
                g.draw_image(bg, offset, 0.0);
            }
        }
    }
    // Challenge::DrawBackground: the line wall-nuts start behind.
    if board.is_wallnut_bowling_level()
        && board.show_bowling_line
        && let Some(stripe) = image("IMAGE_WALLNUT_BOWLINGSTRIPE")
    {
        g.draw_image(stripe, 268.0, 77.0);
    }
    // TODO: the pool's water effect, the level 1 tutorial's flashing sod row,
    // and the night intro's graveyard.
}

/// `Board::DrawUIBottom`: the seed bank and the menu button.
fn draw_ui_bottom(board: &Board, res: &GameResources, g: &mut Graphics) {
    board.seed_bank.draw(board, res, g);
    // `Board::DrawShovel`.
    if board.show_shovel
        && let Some(bank) = res.image("IMAGE_SHOVELBANK")
    {
        let (x, y, _, _) = board.shovel_button_rect(res);
        g.draw_image(bank, x as f32, y as f32);
        if board.cursor.cursor_type != cursor::CursorType::Shovel
            && let Some(shovel) = res.image("IMAGE_SHOVEL")
        {
            g.draw_image(shovel, (x - 7) as f32, (y - 3) as f32);
        }
    }
    if !board.stage_has_fog() {
        draw_top_right_ui(board, res, g);
    }
}

/// `Board::DrawTopRightUI`.
fn draw_top_right_ui(board: &Board, res: &GameResources, g: &mut Graphics) {
    let b = &board.menu_button;
    if b.no_draw {
        return;
    }
    let is_down = b.is_down && b.is_over && !b.disabled;
    let r = b.rect;
    draw_stone_button(g, res, r.x0, r.y0, r.x1 - r.x0, r.y1 - r.y0, is_down, b.is_mouse_over(), &b.label);
}

/// `Board::DrawUITop`: the level name and the advice banner.
fn draw_ui_top(board: &Board, res: &GameResources, g: &mut Graphics) {
    if board.stage_has_fog() {
        draw_top_right_ui(board, res, g);
    }
    if board.scene == GameScene::Playing {
        board.draw_progress_meter(res, g);
        draw_level(board, res, g);
    }
    board.draw_coin_bank(res, g);
    board.advice.draw(board, res, g);
    board.draw_cursor_object(res, g);
}

/// `Board::DrawLevel`: "Level 1-1" in the bottom-right corner.
fn draw_level(board: &Board, res: &GameResources, g: &mut Graphics) {
    let Some(font) = res.font("FONT_HOUSEOFTERROR16") else { return };
    let text = format!("{} {}", res.translate("[LEVEL]"), board.stage_string());
    let x = if board.has_progress_meter() { 593 } else { 780 };
    g.tod_draw_string(font, x, 595, &text, SexyColor::rgba(224, 187, 98, 255), Justify::Right);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_order_sorts_rows_within_layers() {
        assert!(make_render_order(render_layer::LAWN_MOWER, 0, 0) < make_render_order(render_layer::PLANT, 1, 0));
        assert_eq!(make_render_order(render_layer::TOP, 0, 1), 400001);
    }
}
