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

mod bungee;
mod coin;
mod crazy_dave;
mod boss;
mod conveyor;
mod cursor;
mod cutscene;
mod dev_scene;
mod fog;
mod grid_item;
mod lawn_mower;
mod level_end;
mod message;
mod minigame;
mod plant;
mod save;
pub(crate) use save::{
    board_from_save, erase_save, has_save, load_save_data, need_save_game, save_dir, try_to_save_game,
};
pub(crate) mod pool;
pub use minigame::MiniGame;
mod projectile;
mod seed_bank;
mod seed_chooser;
mod survival;
mod tooltip;
mod tutorial;
mod waves;
mod whack_a_zombie;
mod zombie;

/// What the almanac needs to show plants and zombies off the board, as the
/// original's `AlmanacDialog` makes its own `Plant` and `Zombie`.
pub(crate) mod figures {
    pub(crate) use super::coin::money_string;
    pub(crate) use super::crazy_dave::{CrazyDave, DaveState, DaveVars};
    pub(crate) use super::plant::{Plant, PlantAction};
    pub(crate) use super::zombie::{Fx, ZOMBIE_WAVE_UI, Zombie, ZombieEnv, draw_cached_zombie};
}
#[cfg(test)]
#[path = "../../tests/mod.rs"]
mod tests;

use std::collections::HashSet;

use bevy::prelude::*;
use sexy::data_array::DataArray;

use crate::audio::{FoleyType, Music, MusicTune, Sounds};
use crate::dialog::{ButtonMode, DialogId, Dialogs};
use crate::graphics::{Graphics, Justify, SexyColor, SexyRect};
use crate::input::GameCursor;
use crate::player::{PlayerInfo, ProfileMgr};
use crate::plant::{SeedType, seeds_available};
use seed_chooser::ChooserWarning;
use crate::particle::{ParticleEffect, draw_particle_system};
use crate::reanim::{EffectSystem, ParticleSystemId, ReanimationType};
use crate::resources::GameResources;
use crate::store::StoreItem;
use crate::widgets::draw_stone_button;
use crate::{AppState, UiSet};

use coin::{Coin, CoinEvent, CoinMotion, CoinType};
use cursor::{CursorObject, CursorPreview};
use cutscene::CutScene;
use grid_item::{GridCels, GridItem};
use lawn_mower::{LawnMower, LawnMowerState};
use message::{MessageStyle, MessageWidget};
use plant::{Plant, PlantAction};
use projectile::Projectile;
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
            .add_systems(Update, update_mouse_icon.run_if(open))
            .add_systems(Update, draw_board.in_set(UiSet::ScreenDraw).run_if(open));
    }
}

pub const BOARD_WIDTH: i32 = 800;
/// `BOARD_ICE_START`: where an empty ice trail begins.
const BOARD_ICE_START: i32 = 800;
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
/// `STORE_ITEM_PACKET_UPGRADE`, the seed slots' entry in `PlayerInfo::purchases`.
pub const STORE_ITEM_PACKET_UPGRADE: usize = 21;

/// `StoreScreen::GetItemCost` for a seed slot after `bought` of them, in coins.
pub fn packet_upgrade_cost(bought: i32) -> i32 {
    match bought {
        0 => 75,
        1 => 500,
        2 => 2000,
        _ => 8000,
    }
}
pub const FINAL_LEVEL: i32 = 50;

/// `GameMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Adventure,
    /// The first-ever adventure's opening cinematic (`GAMEMODE_INTRO`).
    Intro,
    /// Survival (`GAMEMODE_SURVIVAL_*`): the chosen lawn and difficulty.
    Survival { lawn: SurvivalLawn, difficulty: SurvivalDifficulty },
    /// A mini-game from the challenge screen (`GAMEMODE_CHALLENGE_*`).
    MiniGame(MiniGame),
}

/// The five survival lawns (`GAMEMODE_SURVIVAL_*_STAGE_1` to `_STAGE_5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurvivalLawn {
    Day,
    Night,
    Pool,
    Fog,
    Roof,
}

impl SurvivalLawn {
    pub const ALL: [Self; 5] = [Self::Day, Self::Night, Self::Pool, Self::Fog, Self::Roof];

    /// The lawn's index in `SurvivalLawn::ALL`, matching the original's
    /// stage numbers (stage 1 is the day lawn).
    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|&l| l == self).unwrap_or(0) as i32
    }

    pub fn from_index(index: i32) -> Option<Self> {
        Self::ALL.get(usize::try_from(index).ok()?).copied()
    }

    pub fn background(self) -> BackgroundType {
        match self {
            Self::Day => BackgroundType::Day,
            Self::Night => BackgroundType::Night,
            Self::Pool => BackgroundType::Pool,
            Self::Fog => BackgroundType::Fog,
            Self::Roof => BackgroundType::Roof,
        }
    }

    /// The tile's label string.
    pub fn name(self, difficulty: SurvivalDifficulty) -> &'static str {
        match (self, difficulty) {
            (Self::Day, SurvivalDifficulty::Normal) => "[SURVIVAL_DAY_NORMAL]",
            (Self::Night, SurvivalDifficulty::Normal) => "[SURVIVAL_NIGHT_NORMAL]",
            (Self::Pool, SurvivalDifficulty::Normal) => "[SURVIVAL_POOL_NORMAL]",
            (Self::Fog, SurvivalDifficulty::Normal) => "[SURVIVAL_FOG_NORMAL]",
            (Self::Roof, SurvivalDifficulty::Normal) => "[SURVIVAL_ROOF_NORMAL]",
            (Self::Day, SurvivalDifficulty::Hard) => "[SURVIVAL_DAY_HARD]",
            (Self::Night, SurvivalDifficulty::Hard) => "[SURVIVAL_NIGHT_HARD]",
            (Self::Pool, SurvivalDifficulty::Hard) => "[SURVIVAL_POOL_HARD]",
            (Self::Fog, SurvivalDifficulty::Hard) => "[SURVIVAL_FOG_HARD]",
            (Self::Roof, SurvivalDifficulty::Hard) => "[SURVIVAL_ROOF_HARD]",
            (Self::Day, SurvivalDifficulty::Endless) => "[SURVIVAL_DAY_ENDLESS]",
            (Self::Night, SurvivalDifficulty::Endless) => "[SURVIVAL_NIGHT_ENDLESS]",
            (Self::Pool, SurvivalDifficulty::Endless) => "[SURVIVAL_POOL_ENDLESS]",
            (Self::Fog, SurvivalDifficulty::Endless) => "[SURVIVAL_FOG_ENDLESS]",
            (Self::Roof, SurvivalDifficulty::Endless) => "[SURVIVAL_ROOF_ENDLESS]",
        }
    }
}

/// Survival's three difficulties (`GAMEMODE_SURVIVAL_*_STAGE_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurvivalDifficulty {
    Normal,
    Hard,
    Endless,
}

impl SurvivalDifficulty {
    pub const ALL: [Self; 3] = [Self::Normal, Self::Hard, Self::Endless];

    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|&d| d == self).unwrap_or(0) as i32
    }

    pub fn from_index(index: i32) -> Option<Self> {
        Self::ALL.get(usize::try_from(index).ok()?).copied()
    }

    /// `Board::GetNumWavesPerSurvivalStage`.
    pub fn waves_per_stage(self) -> i32 {
        match self {
            Self::Normal => 10,
            Self::Hard | Self::Endless => 20,
        }
    }

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

/// `PixelToGridYKeepOnBoard`: the row at a point, even one off the lawn.
pub fn row_at_pixel(background: BackgroundType, x: i32, y: i32) -> i32 {
    let grid_x = ((x.max(80) - cursor::LAWN_XMIN) / 80).clamp(0, MAX_GRID_SIZE_X as i32 - 1);
    if y < LAWN_YMIN {
        return 0;
    }
    if background.has_roof() {
        let y = if grid_x < 5 { y - (4 - grid_x) * 20 } else { y };
        ((y - LAWN_YMIN) / 85).clamp(0, MAX_GRID_SIZE_Y as i32 - 2)
    } else if background.has_pool() {
        ((y - LAWN_YMIN) / 85).clamp(0, MAX_GRID_SIZE_Y as i32 - 1)
    } else {
        ((y - LAWN_YMIN) / 100).clamp(0, MAX_GRID_SIZE_Y as i32 - 2)
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

/// `Board::CountZombiesOnScreen`: the ones the music's drum burst answers to
/// (hypnotised zombies aren't a threat).
pub fn count_zombies_on_screen(board: &Board) -> i32 {
    board
        .zombies
        .iter()
        .filter(|(_, z)| z.has_head && !z.is_dead_or_dying() && !z.mind_controlled && z.is_on_board())
        .count() as i32
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
    /// `Challenge::mSurvivalStage`: how many survival stages (each worth
    /// its flags) the run has cleared.
    pub survival_stage: i32,
    /// `mNextSurvivalStageCounter`: ticks of the "more zombies" pause
    /// between two survival stages.
    pub next_survival_stage_counter: i32,
    /// It's Raining Seeds: ticks until the next packet falls.
    raining_seeds_counter: i32,
    /// It's Raining Seeds: ticks between rain splashes.
    rain_splash_counter: i32,
    /// `LawnApp::IsFirstTimeAdventureMode`: the player hasn't finished adventure yet.
    pub first_time_adventure: bool,
    pub background: BackgroundType,
    pub plant_row: [PlantRowType; MAX_GRID_SIZE_Y],
    pub grid_square_type: [[GridSquareType; MAX_GRID_SIZE_Y]; MAX_GRID_SIZE_X],
    pub sun_money: i32,
    /// `Challenge::mConveyorBeltCounter`: ticks until the next seed packet rides in.
    conveyor_belt_counter: i32,
    last_conveyor_seed: Option<SeedType>,
    /// The seed chooser, while the level intro has it on screen.
    seed_chooser: Option<seed_chooser::SeedChooser>,
    /// Crazy Dave, when he's in the intro.
    dave: crazy_dave::CrazyDave,
    /// The player's name, for Dave's messages.
    player_name: String,
    /// Started with `--lvl-2-1`: finishing it leaves the saved player alone.
    level_overridden: bool,
    /// `GetSeedsAvailable`.
    /// `SeedTypeAvailable` for every plant: what the seed chooser offers.
    owned_seeds: Vec<bool>,
    finished_adventure: bool,
    /// The player bought First Aid: a hurt pumpkin or nut can be replaced.
    first_aid: bool,
    /// `Challenge::mShowBowlingLine`.
    show_bowling_line: bool,
    /// The player's money, as shown by the coin bank (the profile holds the real total).
    pub player_coins: i32,
    /// Money collected since the last tick, for the profile.
    pub pending_money: i32,
    coin_bank_fade_count: i32,
    dropped_first_coin: bool,
    /// Seed slots bought from Crazy Dave (`mPurchases[STORE_ITEM_PACKET_UPGRADE]`).
    packet_upgrades: i32,
    /// Times the player turned his offer down (`mDidntPurchasePacketUpgrade`).
    didnt_purchase_packet_upgrade: i32,
    /// Dave has just asked whether to buy a seed slot: the app shows the dialog.
    packet_offer: bool,
    /// The player's answer to that, for Dave to reply to.
    pub packet_offer_answer: Option<bool>,
    /// How far the intro's sod has rolled out, 0..=1000.
    pub sod_position: i32,
    pub main_counter: i32,
    /// The pool's animated water, on levels with a pool.
    pub pool_water: Option<pool::PoolWater>,
    pub sun_count_down: i32,
    pub draw_count: u32,
    pub paused: bool,
    /// Zombie types the player defeated in this level (`AlmanacPlayerDefeatedZombie`).
    pub defeated_zombies: [bool; crate::zombie::NUM_ZOMBIE_TYPES],
    pub show_shovel: bool,
    pub out_of_money_counter: i32,
    /// `mScoreNextMowerCounter`: ticks until the next lawn mower turns into money.
    score_next_mower_counter: i32,
    /// Coins a collected money bag or trophy is about to scatter: type, how many, where.
    pending_fan_out: Option<(CoinType, i32, f32, f32)>,
    /// `mCobCannonCursorDelayCounter` and `mCobCannonMouseX/Y`: the click that picked
    /// up a cob cannon's aim doesn't also fire it.
    cob_cannon_cursor_delay: i32,
    cob_cannon_mouse: (i32, i32),
    pub seed_bank: SeedBank,
    cut_scene: CutScene,
    advice: MessageWidget,
    /// The hint the banner is showing, if it's one shown once per level.
    help_index: Option<AdviceType>,
    help_displayed: HashSet<AdviceType>,
    lawn_mowers: Vec<LawnMower>,
    /// `mBonusLawnMowersRemaining`: bought mowers that roll in to replace a
    /// spent one.
    pub bonus_lawn_mowers_remaining: i32,
    /// `mIceTrapCounter`: ticks the pool's water stays frozen after an
    /// ice-shroom (`IceZombies`).
    ice_trap_counter: i32,
    /// How many rakes the player has bought and not yet used (`STORE_ITEM_RAKE`).
    rake_count: i32,
    /// `mPrevBoardResult`: the last board was lost or restarted.
    pub prev_result_restart: bool,
    /// The player already has the mode-unlocking presents.
    unlocked_minigames: bool,
    unlocked_puzzle: bool,
    /// A present reached the advice box this tick.
    pending_unlock: Option<coin::PresentUnlock>,
    /// Whether the player has bought the pool cleaner (`StoreItem::PoolCleaner`),
    /// which the pool rows' lawn mowers become.
    pool_cleaner: bool,
    /// Whether the player has bought the roof cleaner (`StoreItem::RoofCleaner`).
    roof_cleaner: bool,
    plants: DataArray<Plant>,
    zombies: DataArray<Zombie>,
    projectiles: DataArray<Projectile>,
    /// `mGridItems`: the night levels' graves.
    grid_items: DataArray<GridItem>,
    /// `mFwooshID`: the jalapeno's flames, a dozen along each row, and
    /// `mFwooshCountDown` until they die down.
    fwoosh: [[Option<crate::reanim::ReanimationId>; 12]; MAX_GRID_SIZE_Y],
    fwoosh_countdown: i32,
    grid_cels: GridCels,
    /// `mBoardRandSeed`, part of the level's random seed (`GetLevelRandSeed`).
    board_rand_seed: u32,
    /// `mEnableGraveStones`: the graves rise once the intro gets to them.
    enable_grave_stones: bool,
    /// `mGravesCleared`: graves eaten by grave busters.
    graves_cleared: i32,
    /// `mRiseFromGraveCounter`: counts down from the last wave to zombies
    /// climbing out of the graves.
    rise_from_grave_counter: i32,
    /// Where each row's ice trail starts (`mIceMinX`) and how long it lasts
    /// (`mIceTimer`): the Zamboni lays it, the bobsled team rides it.
    ice_min_x: [i32; MAX_GRID_SIZE_Y],
    ice_timer: [i32; MAX_GRID_SIZE_Y],
    /// World 4's fog.
    pub fog: fog::Fog,
    waves: WaveState,
    /// `mPlantsEaten`.
    plants_eaten: i32,
    /// Whack-a-zombie's mallet and timer.
    whack: whack_a_zombie::WhackState,
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
    /// The boss fell: the music fades out.
    boss_music_fade: bool,
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
/// Set by the main menu when the first-ever adventure should open with the
/// intro cinematic (`PreNewGame(GAMEMODE_INTRO)`).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct PlayIntro(pub bool);
/// How the last board ended (`mApp->mBoardResult`): the intro skips Crazy
/// Dave on a boss level retried after a loss, like the original.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardResult {
    #[default]
    None,
    Lost,
    Restart,
}


/// Set when the Continue dialog resumes a save: the next board loads it
/// instead of starting fresh (`LawnApp::TryLoadGame`).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct LoadSavedGame(pub bool);

/// The level given on the command line (`--lvl-2-1`), if any.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct LevelOverride(pub Option<i32>);

/// `LawnApp::mGameMode` for the next board: the challenge screen picks
/// survival here; adventure leaves it unset. It stays set so that a restart
/// or a "Try Again" begins the same mode again.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct SelectedMode(pub Option<GameMode>);

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
            GameMode::Intro => 1,
            // `Challenge::InitLevel` runs survival on level 50 so that
            // every zombie definition is on the table; the survival lawn
            // then replaces the background and the zombie table.
            GameMode::Survival { .. } => FINAL_LEVEL,
            // Mini-games run on challenge level 0, as in `InitLevel`; their
            // own tables replace everything the level would decide.
            GameMode::MiniGame(_) => 0,
        };
        let first_time_adventure =
            mode == GameMode::Adventure && !player.is_some_and(PlayerInfo::has_finished_adventure);
        let mut board = Self {
            mode,
            scene: GameScene::LevelIntro,
            x: 0,
            y: 0,
            level,
            survival_stage: 0,
            next_survival_stage_counter: 0,
            raining_seeds_counter: if mode == GameMode::MiniGame(MiniGame::RainingSeeds) { 100 } else { 0 },
            rain_splash_counter: 0,
            first_time_adventure,
            background: BackgroundType::Day,
            plant_row: [PlantRowType::Normal; MAX_GRID_SIZE_Y],
            grid_square_type: [[GridSquareType::Grass; MAX_GRID_SIZE_Y]; MAX_GRID_SIZE_X],
            sun_money: 50,
            conveyor_belt_counter: 0,
            last_conveyor_seed: None,
            seed_chooser: None,
            dave: crazy_dave::CrazyDave::new(),
            player_name: String::new(),
            level_overridden: false,
            owned_seeds: (0..=SeedType::Imitater as i32).map(|i| player.is_some_and(|p| SeedType::from_index(i).is_some_and(|s| crate::plant::has_seed_type(p, s)))).collect(),
            finished_adventure: player.is_some_and(|p| p.has_finished_adventure()),
            first_aid: player.is_some_and(|p| p.purchases[crate::store::StoreItem::FirstAid.slot()] > 0),
            show_bowling_line: false,
            player_coins: player.map_or(0, |p| p.coins),
            pending_money: 0,
            coin_bank_fade_count: 0,
            dropped_first_coin: false,
            packet_upgrades: player.map_or(0, |p| p.purchases[STORE_ITEM_PACKET_UPGRADE]),
            didnt_purchase_packet_upgrade: player.map_or(0, |p| p.didnt_purchase_packet_upgrade),
            packet_offer: false,
            packet_offer_answer: None,
            sod_position: 0,
            main_counter: 0,
            pool_water: None,
            sun_count_down: 0,
            draw_count: 0,
            paused: false,
            defeated_zombies: [false; crate::zombie::NUM_ZOMBIE_TYPES],
            show_shovel: false,
            out_of_money_counter: 0,
            score_next_mower_counter: 0,
            pending_fan_out: None,
            cob_cannon_cursor_delay: 0,
            cob_cannon_mouse: (0, 0),
            seed_bank: SeedBank::new(resources),
            cut_scene: CutScene::default(),
            advice: MessageWidget::default(),
            help_index: None,
            help_displayed: HashSet::new(),
            lawn_mowers: Vec::new(),
            bonus_lawn_mowers_remaining: 0,
            ice_trap_counter: 0,
            rake_count: player.map_or(0, |p| p.purchases[StoreItem::Rake.slot()]),
            prev_result_restart: false,
            unlocked_minigames: player.is_some_and(|p| p.has_unlocked_minigames),
            unlocked_puzzle: player.is_some_and(|p| p.has_unlocked_puzzle_mode),
            pending_unlock: None,
            pool_cleaner: player.is_some_and(|p| p.purchases[StoreItem::PoolCleaner.slot()] != 0),
            roof_cleaner: player.is_some_and(|p| p.purchases[StoreItem::RoofCleaner.slot()] != 0),
            plants: DataArray::default(),
            zombies: DataArray::default(),
            projectiles: DataArray::default(),
            grid_items: DataArray::default(),
            fwoosh: [[None; 12]; MAX_GRID_SIZE_Y],
            fwoosh_countdown: 0,
            grid_cels: GridCels::new(rng),
            board_rand_seed: rng.next_u31(),
            enable_grave_stones: false,
            graves_cleared: 0,
            rise_from_grave_counter: 0,
            ice_min_x: [BOARD_ICE_START; MAX_GRID_SIZE_Y],
            ice_timer: [0; MAX_GRID_SIZE_Y],
            fog: fog::Fog::default(),
            waves: WaveState::default(),
            plants_eaten: 0,
            whack: Default::default(),
            plant_actions: Vec::new(),
            shake_counter: 0,
            shake_amount: (0, 0),
            zombies_won_trigger: None,
            level_award_spawned: false,
            board_fade_out_counter: -1,
            pending_fade_out: false,
            level_complete: false,
            boss_music_fade: false,
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
        board.pool_water = if board.stage_has_pool() { pool::PoolWater::new(resources, images) } else { None };
        board.init_fog();
        let level_seed = board.level_rand_seed(player.map_or(0, |p| p.id), player.map_or(0, |p| p.finished_adventure));
        board.add_level_grave_stones(&mut sexy::rand::MtRand::new(level_seed), rng);
        board.sun_money = if board.first_time_adventure && level == 1 { 150 } else { 50 };
        if !board.stage_is_night() {
            board.sun_count_down = rng.range_int(425, 700);
        }
        let num_packets = board.num_seeds_in_bank(player);
        board.seed_bank.conveyor = board.has_conveyor_belt_seed_bank();
        board.seed_bank.num_packets = num_packets;
        board.seed_bank.update_width(resources);
        // Levels without a chooser get the first plants in order, as the
        // original does.
        if board.is_whack_a_zombie_level() {
            for (packet, seed) in board.seed_bank.packets.iter_mut().zip(Self::WHACK_SEEDS) {
                packet.set_packet_type(Some(seed));
            }
        } else if !board.has_conveyor_belt_seed_bank() && !board.choose_seeds_on_current_level() {
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
            // The line appears when Crazy Dave mentions bowling (or play starts).
            board.show_bowling_line = !board.first_time_adventure;
        } else if board.is_mini_boss_level() || board.is_little_trouble_level() {
            // Challenge::StartLevel: the belt starts running at once.
            board.conveyor_belt_counter = 200;
        } else if board.is_final_boss_level() {
            // Challenge::InitLevel: four packets are on the belt from the start.
            for seed in [SeedType::Cabbagepult, SeedType::Jalapeno, SeedType::Cabbagepult, SeedType::Iceshroom] {
                board.seed_bank.add_seed(seed);
            }
            board.conveyor_belt_counter = 1000;
        } else if board.minigame() == Some(MiniGame::Column) {
            // Challenge::InitLevel: Column starts with these six on the belt.
            use SeedType::{InstantCoffee, Magnetshroom, Melonpult, Potatomine, Tallnut};
            for seed in [Potatomine, Tallnut, Melonpult, Magnetshroom, InstantCoffee, Melonpult] {
                board.seed_bank.add_seed(seed);
            }
            board.conveyor_belt_counter = 1000;
        } else if board.minigame() == Some(MiniGame::Invisighoul) {
            // Challenge::InitLevel + StartLevel: two packets, and the belt
            // runs at the challenge's speed.
            for seed in [SeedType::Peashooter, SeedType::Iceshroom] {
                board.seed_bank.add_seed(seed);
            }
            board.conveyor_belt_counter = 200;
        }
        board
    }

    /// A fresh board for a save file's level, before the save is applied.
    /// Like `--lvl`, the shape is a copy of the player on that level.
    pub(super) fn new_for_save(
        shape: PlayerInfo,
        resources: &mut GameResources,
        images: &mut Assets<Image>,
        rng: &mut sexy::rand::MtRand,
    ) -> Self {
        Self::new(GameMode::Adventure, Some(&shape), resources, images, rng)
    }

    /// `Board::PickBackground` (adventure mode) and `LoadBackgroundImages`.
    fn pick_background(&mut self, resources: &mut GameResources, images: &mut Assets<Image>) {
        let level = self.level;
        self.background = if self.mode == GameMode::Intro {
            // The cinematic's lawn is the pool's.
            BackgroundType::Pool
        } else if let GameMode::Survival { lawn, .. } = self.mode {
            // `Challenge::InitLevel`: the survival lawn picks the background.
            lawn.background()
        } else if let Some(game) = self.minigame() {
            // `PickBackground`: each mini-game has its own lawn.
            game.background()
        } else {
            match level {
            l if l <= LEVELS_PER_AREA => BackgroundType::Day,
            l if l <= 2 * LEVELS_PER_AREA => BackgroundType::Night,
            l if l <= 3 * LEVELS_PER_AREA => BackgroundType::Pool,
            // `PickBackground`: the scary-potter level is at night.
            fog::SCARY_POTTER_LEVEL => BackgroundType::Night,
            l if l <= 4 * LEVELS_PER_AREA => BackgroundType::Fog,
            l if l < FINAL_LEVEL => BackgroundType::Roof,
            FINAL_LEVEL => BackgroundType::Boss,
            _ => BackgroundType::Day,
            }
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
    }

    pub fn stage_is_night(&self) -> bool {
        self.background.is_night()
    }

    pub fn stage_has_pool(&self) -> bool {
        self.background.has_pool()
    }

    /// `Board::StageHas6Rows`: the pool and fog lawns have six.
    pub fn stage_has_6_rows(&self) -> bool {
        matches!(self.background, BackgroundType::Pool | BackgroundType::Fog)
    }

    pub fn stage_has_roof(&self) -> bool {
        self.background.has_roof()
    }

    /// `ChooseSeedsOnCurrentLevel`: early adventure levels hand out the plants.
    pub fn choose_seeds_on_current_level(&self) -> bool {
        !self.has_conveyor_belt_seed_bank()
            && !self.is_whack_a_zombie_level()
            && !self.is_challenge_without_seed_bank()
            && (!self.first_time_adventure || self.level > 7)
    }

    /// `GetNumSeedsInBank`.
    fn num_seeds_in_bank(&self, player: Option<&PlayerInfo>) -> usize {
        if self.is_challenge_without_seed_bank() {
            return 0;
        }
        if self.has_conveyor_belt_seed_bank() {
            return 10;
        }
        if self.is_whack_a_zombie_level() {
            return Self::WHACK_SEEDS.len();
        }
        let Some(player) = player else { return 0 };
        let slots = 6 + player.purchases[STORE_ITEM_PACKET_UPGRADE];
        slots.min(seeds_available(player)).max(0) as usize
    }

    /// `StoreScreen::GetItemCost` for the next seed slot, in coins.
    pub fn packet_upgrade_cost(&self) -> i32 {
        packet_upgrade_cost(self.packet_upgrades)
    }

    /// `CutScene::CanGetPacketUpgrade(n)`: Dave offers the slot after `n`
    /// bought ones if the player can afford it and hasn't refused twice.
    pub fn can_get_packet_upgrade(&self, bought: i32) -> bool {
        self.packet_upgrades == bought && self.player_coins >= self.packet_upgrade_cost() && self.didnt_purchase_packet_upgrade < 2
    }

    /// What Dave's messages fill in.
    fn dave_vars(&self) -> crazy_dave::DaveVars {
        crazy_dave::DaveVars { name: self.player_name.clone(), money: self.player_coins, upgrade_cost: self.packet_upgrade_cost() }
    }

    /// Dave's reply once the player has answered his seed slot offer.
    fn answer_packet_offer(&mut self, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>, sounds: &mut Sounds) {
        let Some(yes) = self.packet_offer_answer.take() else { return };
        let asked = self.dave.message_index;
        if yes {
            self.player_coins -= self.packet_upgrade_cost();
            self.packet_upgrades += 1;
            // The bank (and the seed chooser) gets the new slot.
            self.seed_bank.num_packets += 1;
            self.seed_bank.update_width(res);
        } else {
            self.didnt_purchase_packet_upgrade += 1;
        }
        let reply = match (asked, yes) {
            (1503, true) => 1510,
            (1503, false) => 1520,
            (_, true) => 1560,
            (_, false) => 1570,
        };
        let vars = self.dave_vars();
        self.dave.talk_index(reply, &vars, effects, res, images, sounds);
    }

    /// `LawnApp::UpdateCrazyDave`.
    fn update_crazy_dave(&mut self, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>, sounds: &mut Sounds) {
        self.dave.update(effects, res, images, sounds);
    }

    /// `LawnApp::IsWallnutBowlingLevel`: adventure 1-5 and the two
    /// Wall-nut Bowling mini-games.
    pub fn is_wallnut_bowling_level(&self) -> bool {
        (self.mode == GameMode::Adventure && self.level == 5)
            || matches!(self.minigame(), Some(MiniGame::WallnutBowling | MiniGame::WallnutBowling2))
    }

    /// `LawnApp::IsLittleTroubleLevel`: adventure 3-5 and the
    /// Big Trouble Little Zombie mini-game.
    pub fn is_little_trouble_level(&self) -> bool {
        (self.mode == GameMode::Adventure && self.level == 25) || self.minigame() == Some(MiniGame::LittleTrouble)
    }

    /// `Board::HasConveyorBeltSeedBank`.
    pub fn has_conveyor_belt_seed_bank(&self) -> bool {
        self.is_wallnut_bowling_level()
            || self.is_mini_boss_level()
            || self.is_little_trouble_level()
            || self.is_final_boss_level()
            || matches!(self.minigame(), Some(MiniGame::Column | MiniGame::Invisighoul))
    }

    /// `LawnApp::IsMiniBossLevel`: the conveyor belt levels at the end of each area.
    pub fn is_mini_boss_level(&self) -> bool {
        self.mode == GameMode::Adventure && matches!(self.level, 10 | 20 | 30)
    }

    /// `GridToPixelY` for the grid's left column.
    pub fn grid_to_pixel_y(&self, grid_x: i32, grid_y: i32) -> i32 {
        grid_to_pixel_y(self.background, grid_x, grid_y)
    }

    /// `GetPosYBasedOnRow`.
    pub fn pos_y_based_on_row(&self, x: f32, row: i32) -> f32 {
        pos_y_based_on_row(self.background, x, row)
    }

    /// `Board::InitLawnMowers`: one per row that zombies can walk down. Pool
    /// rows get the pool cleaner when it's been bought (see `LawnMower::new`).
    fn init_lawn_mowers(&mut self) {
        // The roof has mowers only once the player has bought the roof cleaner.
        if self.stage_has_roof() && !self.roof_cleaner {
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
        self.seed_chooser = None;
        if self.is_wallnut_bowling_level() {
            self.show_bowling_line = true;
        }
        if self.is_whack_a_zombie_level() {
            self.start_whack_a_zombie();
        }
        // The final boss's music starts during the intro (`CutScene`).
        if !self.is_final_boss_level() {
            music.make_sure_music_is_playing(self.game_music());
        }
    }

    /// `Music::StartGameMusic`'s choice of tune.
    fn game_music(&self) -> MusicTune {
        if self.is_wallnut_bowling_level() || self.is_whack_a_zombie_level() || self.is_little_trouble_level() {
            MusicTune::MinigameLoonboon
        } else if self.mode == GameMode::Adventure && matches!(self.level, 10 | 20 | 30) {
            MusicTune::Conveyer
        } else if self.is_final_boss_level() {
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
#[allow(clippy::too_many_arguments)]
fn new_game(
    mut commands: Commands,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut effects: ResMut<EffectSystem>,
    mut music: ResMut<Music>,
    mut sounds: Sounds,
    profiles: Res<ProfileMgr>,
    level_override: Res<LevelOverride>,
    load: Option<Res<LoadSavedGame>>,
    mut play_intro: ResMut<PlayIntro>,
    selected_mode: Option<Res<SelectedMode>>,
    last_result: Option<Res<BoardResult>>,
) {
    // `Board::Board` copies the app's last result before it's cleared.
    let was_lost = last_result.as_deref().is_some_and(|r| matches!(r, BoardResult::Lost | BoardResult::Restart));
    commands.insert_resource(BoardResult::None);
    // `PlayIntro` plays the very first adventure's intro cinematic; a saved
    // game resumes instead of starting the level afresh.
    effects.clear();
    music.stop_all_music();
    // CutScene::PreloadResources, for what this stage draws.
    for ty in [ReanimationType::SodRoll, ReanimationType::LawnMower, ReanimationType::Zombie, ReanimationType::Flag, ReanimationType::Polevaulter] {
        if let Err(e) = resources.reanim_definition(ty.file(), &mut images) {
            error!("{e}");
        }
    }
    // `--lvl-2-1` plays a level as a copy of the player who has reached it.
    let player_copy = level_override.0.and_then(|level| profiles.player().map(|p| PlayerInfo { level, ..p.clone() }));
    let player = player_copy.as_ref().or(profiles.player());
    // `LawnApp::mGameMode`: the challenge screen picks survival; `--lvl` and
    // the intro cinematic stay adventure's.
    let mode = if std::mem::take(&mut play_intro.0) {
        GameMode::Intro
    } else if level_override.0.is_some() {
        GameMode::Adventure
    } else {
        selected_mode.as_deref().and_then(|m| m.0).unwrap_or(GameMode::Adventure)
    };
    // PreNewGame's TryLoadGame: the Continue dialog resumes the save.
    let from_save = load.as_deref().is_some_and(|l| l.0);
    commands.remove_resource::<LoadSavedGame>();
    if from_save
        && mode == GameMode::Adventure
        && let Some(dir) = save_dir()
        && let Some(id) = profiles.player().map(|p| p.id)
        && let Some(data) = load_save_data(&dir, id, GameMode::Adventure)
        && let Some(board) =
            board_from_save(&data, &profiles, &mut resources, &mut images, &mut effects, &mut sounds)
    {
        // Like Board::LoadGame: straight into the level, no intro.
        music.stop_all_music();
        commands.insert_resource(board);
        return;
    }
    let mut board = Board::new(mode, player, &mut resources, &mut images, sounds.rng());
    board.prev_result_restart = was_lost;
    board.level_overridden = level_override.0.is_some();
    // PreNewGame erases the mode's save before a fresh game (never for a
    // `--lvl` copy, which must leave the player's save alone).
    if !board.level_overridden
        && board.mode == GameMode::Adventure
        && let Some(dir) = save_dir()
        && let Some(id) = profiles.player().map(|p| p.id)
    {
        erase_save(&dir, id, board.mode);
    }
    // CutScene::PreloadResources: the pool's splash and cleaner.
    if board.stage_has_pool() {
        for ty in [ReanimationType::Splash, ReanimationType::PoolCleaner] {
            if let Err(e) = resources.reanim_definition(ty.file(), &mut images) {
                error!("{e}");
            }
        }
    }
    for packet in &board.seed_bank.packets[..board.seed_bank.num_packets] {
        if let Some(seed) = packet.packet_type
            && let Err(e) = resources.reanim_definition(seed.definition().reanim.file(), &mut images)
        {
            error!("{e}");
        }
    }
    // Conveyor belt levels hand out plants that must be drawable when they ride in.
    if board.is_mini_boss_level() || board.is_little_trouble_level() || board.is_final_boss_level() {
        for seed in conveyor::belt_seeds(&board).iter().map(|&(seed, _)| seed) {
            if let Err(e) = resources.reanim_definition(seed.definition().reanim.file(), &mut images) {
                error!("{e}");
            }
        }
    }
    // The seed chooser shows every plant the player has.
    for index in (0..board.owned_seeds.len()).filter(|&i| board.owned_seeds[i]) {
        if let Some(seed) = SeedType::from_index(index as i32)
            && let Err(e) = resources.reanim_definition(seed.definition().reanim.file(), &mut images)
        {
            error!("{e}");
        }
    }
    if mode == GameMode::Intro {
        cutscene::start_intro(&mut board, &mut music);
        commands.insert_resource(board);
        return;
    }
    let player_name = player.map_or("", |p| p.name.as_str());
    cutscene::start_level_intro(&mut board, &mut effects, &mut resources, &mut images, &mut music, &mut sounds, player_name);
    // Level 1-5 starts with peashooters for Crazy Dave to have the player dig up.
    if board.first_time_adventure && board.level == 5 {
        for (col, row) in [(5, 1), (7, 2), (6, 3)] {
            let (x, y) = (board.grid_to_pixel_x(col), board.grid_to_pixel_y(col, row));
            let plant = plant::Plant::new(col, row, x, y, SeedType::Peashooter, true, &mut effects, &mut resources, &mut images, sounds.rng());
            board.plants.alloc(plant);
        }
    }
    if board.is_whack_a_zombie_level() {
        board.add_hammer(&mut effects, &mut resources, &mut images);
    } else {
        // StageHasZombieWalkInFromRight: whack-a-zombie's come out of the graves instead.
        let mut fx = Fx { effects: &mut effects, res: &mut resources, images: &mut images, sounds: &mut sounds };
        board.place_street_zombies(&mut fx);
    }
    let mut fx = Fx { effects: &mut effects, res: &mut resources, images: &mut images, sounds: &mut sounds };
    dev_scene::set_up(&mut board, &mut music, &mut fx);
    commands.insert_resource(board);
}

/// `LawnApp::KillBoard`.
fn kill_board(mut commands: Commands, mut effects: ResMut<EffectSystem>, mut music: ResMut<Music>) {
    commands.remove_resource::<Board>();
    effects.clear();
    music.stop_all_music();
}

/// What a tick of the board leaves for the app to do.
#[derive(Debug, Default)]
pub(super) struct TickOutcome {
    /// The intro cinematic is over: start level 1.
    intro_done: bool,
    /// Money picked up this tick, for the player's profile.
    money: i32,
    /// The lost-level cutscene is over: show the game over dialog.
    game_over: bool,
    /// The level is won and over (`mLevelComplete`).
    level_complete: bool,
    /// Dave is offering a seed slot: ask the player (slots after buying, price in coins).
    packet_offer: Option<(i32, i32)>,
    /// The seed chooser has another question before play starts.
    chooser_warning: Option<ChooserWarning>,
    /// A present was picked up: unlock the mode for the player.
    unlocked: Option<coin::PresentUnlock>,
}

/// `Board::Update`, at 100 ticks per second: the game in [`Board::tick`],
/// then what it means for the app (dialogs, the profile, the next screen).
#[allow(clippy::too_many_arguments)]
fn update_board(
    mut commands: Commands,
    mut board: ResMut<Board>,
    mut effects: ResMut<EffectSystem>,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut music: ResMut<Music>,
    mut sounds: Sounds,
    mut dialogs: ResMut<Dialogs>,
    mut profiles: ResMut<ProfileMgr>,
    mut next_state: ResMut<NextState<AppState>>,
    mut defeated: ResMut<crate::almanac::ZombiesDefeated>,
) {
    let board = &mut *board;
    // LawnApp::NeedPauseGame: an open dialog pauses the game.
    board.paused = dialogs.is_modal();
    let outcome = board.tick(&mut effects, &mut resources, &mut images, &mut music, &mut sounds);
    for (seen, &now) in defeated.0.iter_mut().zip(&board.defeated_zombies) {
        *seen |= now;
    }
    if outcome.intro_done {
        next_state.set(AppState::Board);
    }
    if outcome.money > 0
        && let Some(player) = profiles.player_mut()
    {
        player.coins += outcome.money;
    }
    // `PlaceRake` spends one of the player's bought rakes, once per level.
    let rake_slot = crate::store::StoreItem::Rake.slot();
    let rake_used = profiles.player().is_some_and(|p| p.purchases[rake_slot] != board.rake_count);
    if rake_used {
        if let Some(player) = profiles.player_mut() {
            player.purchases[rake_slot] = board.rake_count;
        }
        profiles.save_current_details();
    }
    if let Some(unlock) = outcome.unlocked {
        if let Some(player) = profiles.player_mut() {
            match unlock {
                coin::PresentUnlock::Minigames => player.has_unlocked_minigames = true,
                coin::PresentUnlock::Puzzle => player.has_unlocked_puzzle_mode = true,
            }
        }
        profiles.save_current_details();
    }
    if outcome.game_over {
        commands.insert_resource(BoardResult::Lost);
        // The run is over: no Continue from a lost level.
        if !board.level_overridden
            && board.mode == GameMode::Adventure
            && let Some(dir) = save_dir()
            && let Some(id) = profiles.player().map(|p| p.id)
        {
            erase_save(&dir, id, board.mode);
        }
        // An endless run's best flags are kept even when it ends.
        if board.is_challenge() {
            crate::challenge::record_challenge_end(&profiles, board);
            commands.insert_resource(crate::challenge::request_page_for(board));
        }
        do_game_over_dialog(&mut dialogs, &resources, board);
    }
    if let Some(warning) = outcome.chooser_warning {
        do_chooser_warning_dialog(&mut dialogs, &resources, warning);
    }
    if let Some((slots, cost)) = outcome.packet_offer {
        do_purchase_packet_slot_dialog(&mut dialogs, &resources, slots, cost);
        board.show_coin_bank();
    }
    if outcome.level_complete {
        // LawnApp::CheckForGameEnd: the award screen shows what was won.
        // The run is over: a finished level leaves no save behind.
        if !board.level_overridden
            && board.mode == GameMode::Adventure
            && let Some(dir) = save_dir()
            && let Some(id) = profiles.player().map(|p| p.id)
        {
            erase_save(&dir, id, board.mode);
        }
        // A won challenge run earns its trophy and goes back to the
        // challenge screen (LawnApp::ShowChallengeScreen).
        if board.is_challenge() {
            crate::challenge::record_challenge_win(&profiles, board);
            commands.insert_resource(crate::challenge::request_page_for(board));
            board.finish_challenge();
            next_state.set(AppState::Challenges);
            return;
        }
        let (first_time, level) = (board.first_time_adventure, board.level);
        board.finish_level(&mut profiles);
        let next_level = if board.level_overridden { level + 1 } else { profiles.player().map_or(level + 1, |p| p.level) };
        let award = crate::award::AwardScreen::after_level(first_time, level, board.level_overridden, next_level);
        match award {
            Some(screen) => {
                commands.insert_resource(screen);
                next_state.set(AppState::Award);
            }
            None => next_state.set(AppState::Board),
        }
    }
}

impl Board {
    /// One 100 Hz tick of the level (`Board::Update`): everything but the
    /// app's side of things, so tests can run it too.
    pub(super) fn tick(
        &mut self,
        effects: &mut EffectSystem,
        resources: &mut GameResources,
        images: &mut Assets<Image>,
        music: &mut Music,
        sounds: &mut Sounds,
    ) -> TickOutcome {
        let mut outcome = TickOutcome::default();
        if self.mode == GameMode::Intro {
            outcome.intro_done = cutscene::update_intro(self, effects, resources, images, music, sounds);
            if !self.paused {
                effects.update(sounds.rng());
                self.update_pool_water(images);
            }
            self.flush_particle_requests(effects, resources, images, sounds.rng());
            return outcome;
        }
        self.answer_packet_offer(effects, resources, images, sounds);
        outcome.chooser_warning = self.seed_chooser_warning_answered();
        if std::mem::take(&mut self.packet_offer) {
            outcome.packet_offer = Some((self.packet_upgrades + 7, self.packet_upgrade_cost()));
        }
        cutscene::update(self, effects, resources, images, music, sounds);
        self.update_seed_chooser();
        if self.paused {
            return outcome;
        }
        self.update_pool_water(images);
        if !self.menu_button.no_draw {
            self.menu_button.disabled = !self.can_interact_with_board_buttons();
        }

        effects.update(sounds.rng());
        self.update_shake(sounds.rng());
        self.update_fwoosh(effects);
        self.coin_bank_fade_count = (self.coin_bank_fade_count - 1).max(0);
        outcome.money = std::mem::take(&mut self.pending_money);
        self.advice.update(effects, resources, images);
        self.update_tutorial(resources);

        self.update_grid_items();
        if self.scene == GameScene::Playing {
            let mut fx = Fx { effects, res: resources, images, sounds: &mut *sounds };
            self.update_rakes(&mut fx);
        }
        if self.is_whack_a_zombie_level() {
            self.update_whack_a_zombie(effects);
        }
        // UpdateGame, twice on Zombies on Speed (`Challenge::Update` calls
        // `Board::UpdateGame` a second time there).
        self.update_game(effects, resources, images, music, sounds);
        if self.minigame() == Some(MiniGame::Speed) && self.scene == GameScene::Playing {
            self.update_game(effects, resources, images, music, sounds);
        }
        // An art challenge's picture can be finished by anything that plants
        // (an imitater counts once it turns into the real thing).
        self.check_art_challenge_complete(effects, resources, images, sounds);
        if let Some(zombie) = self.zombies_won_trigger.take() {
            self.zombies_won(zombie, effects, resources, images, music, sounds);
        }
        if self.scene == GameScene::ZombiesWon && cutscene::update_zombies_won(self, effects, resources, images, sounds) {
            self.pending_game_over = true;
        }
        if self.pending_fade_out {
            self.fade_out_level(music, sounds, resources);
        }
        if std::mem::take(&mut self.boss_music_fade) {
            music.fade_out(200);
        }
        self.update_level_end_sequence(effects, resources, images, sounds);
        self.process_delete_queue();
        outcome.game_over = std::mem::take(&mut self.pending_game_over);
        outcome.unlocked = self.pending_unlock.take();
        outcome.level_complete = self.level_complete;
        self.flush_particle_requests(effects, resources, images, sounds.rng());
        outcome
    }
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
    /// `Board::UpdateGame`: the objects, then the level's timers and waves.
    #[allow(clippy::too_many_arguments)]
    fn update_game(
        &mut self,
        effects: &mut EffectSystem,
        resources: &mut GameResources,
        images: &mut Assets<Image>,
        music: &mut Music,
        sounds: &mut Sounds,
    ) {
        self.update_game_objects(effects, resources, images, sounds);
        self.update_fog_offset();
        if self.scene != GameScene::Playing {
            return;
        }
        self.main_counter += 1;
        self.cob_cannon_cursor_delay = (self.cob_cannon_cursor_delay - 1).max(0);
        self.out_of_money_counter = (self.out_of_money_counter - 1).max(0);
        self.tick_fog_countdown();
        self.ice_trap_counter = (self.ice_trap_counter - 1).max(0);
        self.update_sun_spawning(effects, resources, images, sounds);
        if self.has_conveyor_belt_seed_bank() {
            self.update_conveyor_belt(sounds.rng());
        }
        self.update_zombie_spawning(effects, resources, images, music, sounds);
        if self.main_counter == 1 && self.first_time_adventure && self.level == 1 {
            self.set_tutorial_state(resources, TutorialState::Level1PickUpPeashooter);
        } else if self.main_counter == 1 && self.first_time_adventure && self.level == 2 {
            self.set_tutorial_state(resources, TutorialState::Level2PickUpSunflower);
            self.display_advice(resources, "[ADVICE_PLANT_SUNFLOWER1]", MessageStyle::TutorialLevel2, None);
            self.tutorial_timer = 500;
        }
        self.update_progress_meter();
        self.update_raining_seeds(effects, resources, images, sounds);
    }

    /// `Board::UpdateGameObjects`.
    fn update_game_objects(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        let playing = self.scene == GameScene::Playing;
        let intro = self.scene == GameScene::LevelIntro;
        let zombies_won = self.scene == GameScene::ZombiesWon;
        // Plants wait for the level to start, except the lawn's on the bowling level.
        if playing || (self.scene == GameScene::LevelIntro && self.is_wallnut_bowling_level()) {
            let row_y: [i32; MAX_GRID_SIZE_Y] = std::array::from_fn(|row| self.grid_to_pixel_y(0, row as i32));
            let open_rows: [bool; MAX_GRID_SIZE_Y] = std::array::from_fn(|row| self.row_can_have_zombies(row as i32));
            let last_wave = self.waves.current_wave == self.waves.num_waves;
            let bowling_pays = !self.first_time_adventure || self.level > 10;
            let coin_available = self.coins.iter().any(|(_, c)| c.is_gold_magnet_target());
            // `Plant::IsAGoldMagnetAboutToSuck`: the magnets take turns.
            let ladders: Vec<(i32, i32)> = self
                .grid_items
                .iter()
                .filter(|(_, g)| g.item_type == grid_item::GridItemType::Ladder && !g.dead)
                .map(|(_, g)| (g.grid_x, g.grid_y))
                .collect();
            let magnet_about = self.plants.iter().any(|(_, p)| {
                p.seed == SeedType::GoldMagnet
                    && !p.not_on_ground()
                    && p.state() == plant::PlantState::MagnetSucking
                    && p.body_anim_time(effects).is_some_and(|t| t < 0.5)
            });
            // `UpdateReanimColor`: a base plant flashes while an upgrade for it
            // is held over it (the cob cannon's kernel-pult too).
            let held = self.cursor.seed;
            let flashes: Vec<(sexy::data_array::Id<Plant>, bool)> = self
                .plants
                .iter()
                .map(|(id, p)| {
                    let flash = held.is_some_and(|s| {
                        !p.dead
                            && ((s.upgrade_base() == Some(p.seed) && s.upgrade_is_ported() && self.can_plant_at(p.col, p.row, s) == cursor::PlantingReason::Ok)
                                || (s == SeedType::Cobcannon
                                    && p.seed == SeedType::Kernelpult
                                    && self.can_plant_at(p.col - 1, p.row, s) == cursor::PlantingReason::Ok))
                    });
                    (id, flash)
                })
                .collect();
            for (id, plant) in self.plants.iter_mut() {
                plant.last_wave = last_wave;
                plant.bowling_pays = bowling_pays;
                plant.coin_available = coin_available;
                plant.another_magnet_about_to_suck = magnet_about;
                plant.upgrade_flash = flashes.iter().find(|(plant_id, _)| *plant_id == id).is_some_and(|(_, flash)| *flash);
                plant.update(
                    &self.zombies,
                    &mut self.plant_actions,
                    effects,
                    res,
                    images,
                    &row_y,
                    &open_rows,
                    &ladders,
                    self.main_counter,
                    sounds.rng(),
                );
            }
        }
        if playing {
            self.update_ice();
            self.update_dancer_troupes();
            self.find_zombie_targets();
        }
        let env = self.zombie_env();
        let show_walking = self.cut_scene.show_zombie_walking();
        let cutscene_time = self.cut_scene.time();
        {
            let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
            let mut placed_ladders = Vec::new();
            let mut basketballs = Vec::new();
            let mut fired_peas = Vec::new();
            let mut jack_blasts = Vec::new();
            let mut jalapeno_rows = Vec::new();
            let mut imps = Vec::new();
            let mut shakes = Vec::new();
            let mut boss_requests = Vec::new();
            for (id, zombie) in self.zombies.iter_mut() {
                if playing || (intro && zombie.is_boss()) {
                    zombie.update(&mut self.plants, &env, &mut fx);
                    if zombie.is_boss() {
                        boss_requests.extend(zombie.take_boss_requests().into_iter().map(|r| (id, r)));
                    }
                    if zombie.ice_keep {
                        zombie.ice_keep = false;
                        let row = zombie.row as usize;
                        self.ice_timer[row] = self.ice_timer[row].max(500);
                    }
                    if let Some((col, row)) = zombie.placed_ladder.take() {
                        placed_ladders.push((col, row));
                    }
                    if let Some(shot) = zombie.thrown_basketball.take() {
                        basketballs.push((shot, zombie.row, zombie.render_order));
                    }
                    if let Some(imp) = zombie.thrown_imp.take() {
                        imps.push((imp, zombie.from_wave));
                    }
                    if let Some(pea) = zombie.fired_pea.take() {
                        fired_peas.push((pea, zombie.render_order));
                    }
                    if let Some(blast) = zombie.jack_blast.take() {
                        jack_blasts.push(blast);
                    }
                    if let Some(blast) = zombie.jalapeno_blast.take() {
                        jalapeno_rows.push(blast);
                    }
                    if let Some(shake) = zombie.shake.take() {
                        shakes.push(shake);
                    }
                    if let Some(x) = zombie.ice_trail.take() {
                        // Behaviour per ruslan831 decomp, Zombie::UpdateZamboni.
                        let row = zombie.row as usize;
                        self.ice_min_x[row] = self.ice_min_x[row].min(x);
                        self.ice_timer[row] = 3000;
                    }
                    if zombie.reached_house() {
                        self.zombies_won_trigger.get_or_insert(id);
                    }
                } else if zombies_won && zombie.is_winner() {
                    zombie.update_winner(show_walking, cutscene_time, &mut self.plants, &mut fx);
                }
                if !zombie.is_on_board() {
                    zombie.update_street(fx.effects);
                }
            }
            for (col, row) in placed_ladders {
                self.add_a_ladder(col, row);
            }
            self.handle_boss_requests(boss_requests, &mut fx);
            if self.is_final_boss_level() {
                self.update_boss_cord_clips(fx.effects);
            }
            for (imp, wave) in imps {
                let id = self.add_zombie_in_row(crate::zombie::ZombieType::Imp, imp.row, wave, &mut fx);
                let env = self.zombie_env();
                if let Some(zombie) = self.zombies.get_mut(id) {
                    zombie.throw_as_imp(&imp, &env, &mut fx);
                }
            }
            for (x, y) in shakes {
                self.shake_board(x, y);
            }
            for (shot, row, render_order) in basketballs {
                let shadow_y = self.projectile_shadow_y(shot.x as i32, row);
                let mut ball = Projectile::new(shot.x as i32, shot.y as i32, render_order, row, shadow_y, projectile::ProjectileType::Basketball);
                ball.launch_basketball(&shot, fx.sounds.rng());
                self.projectiles.alloc(ball);
            }
            // A zombotany head's pea flies backwards at the plants (or at
            // the zombies, when it's hypnotized).
            for (pea, render_order) in fired_peas {
                let shadow_y = self.projectile_shadow_y(pea.x as i32, pea.row);
                let mut shot = Projectile::new(pea.x as i32, pea.y as i32, render_order, pea.row, shadow_y, projectile::ProjectileType::ZombiePea);
                let motion = if pea.mind_controlled { projectile::ProjectileMotion::Straight } else { projectile::ProjectileMotion::Backwards };
                shot.set_motion(motion, fx.sounds.rng());
                self.projectiles.alloc(shot);
            }
            // A jack-in-the-box blows up everything around it.
            for blast in jack_blasts {
                use zombie::damage_range;
                let range = damage_range::GROUND | damage_range::FLYING | damage_range::SUBMERGED | damage_range::OFF_GROUND | damage_range::DYING | damage_range::UNDERGROUND;
                let env = self.zombie_env();
                self.kill_all_zombies_in_radius(blast.row, blast.x, blast.y, 115, 1, true, range, &env, &mut fx);
                self.kill_all_plants_in_radius(blast.x, blast.y, 90, fx.effects);
                fx.sounds.play_foley(FoleyType::Cherrybomb);
                self.add_particle(blast.x as f32, blast.y as f32, render_layer::TOP, ParticleEffect::Jackexplode);
                self.shake_board(4, -6);
            }
            // A jalapeno head burns its row and the plants on it (or, when
            // hypnotized, the zombies instead).
            for blast in jalapeno_rows {
                let row = blast.row;
                if blast.mind_controlled {
                    let env = self.zombie_env();
                    for (_, zombie) in self.zombies.iter_mut() {
                        if zombie.row == row && zombie.effected_by_damage(zombie::damage_range::EXPLOSION) {
                            zombie.remove_cold_effects(fx.effects);
                            zombie.apply_burn(&env, &mut fx);
                        }
                    }
                } else {
                    for (_, plant) in self.plants.iter_mut() {
                        if plant.row == row && !plant.dead && !plant.not_on_ground() {
                            plant.die(fx.effects);
                        }
                    }
                }
                for (_, item) in self.grid_items.iter_mut() {
                    if item.item_type == grid_item::GridItemType::Ladder && item.grid_y == row {
                        item.dead = true;
                    }
                }
                for (_, zombie) in self.zombies.iter_mut() {
                    if zombie.is_boss() {
                        zombie.boss_destroy_iceball_in_row(row, &mut fx);
                    }
                }
                self.do_fwoosh(row, fx.effects, fx.res, fx.images, fx.sounds.rng());
                self.shake_board(3, -4);
            }
            if playing {
                self.update_torchwoods(&mut fx);
                let mut cob_landed = false;
                for (_, projectile) in self.projectiles.iter_mut() {
                    projectile.update(&mut self.zombies, &mut self.plants, &env, &mut fx);
                    cob_landed |= std::mem::take(&mut projectile.landed);
                }
                if cob_landed {
                    self.shake_board(3, -4);
                }
            }
            let kelp_targets: Vec<_> = self.plants.iter().filter_map(|(_, plant)| plant.tangle_kelp_target()).collect();
            for mower in &mut self.lawn_mowers {
                let background = self.background;
                mower.update(
                    playing,
                    |x, row| pos_y_based_on_row(background, x, row),
                    &mut self.zombies,
                    &kelp_targets,
                    self.waves.current_wave,
                    &mut self.waves.wave_row_got_lawn_mowered,
                    &mut fx,
                );
            }
            // `LawnMower::Die`: a bought bonus mower rolls in to replace it.
            let spent: Vec<i32> = self.lawn_mowers.iter().filter(|m| m.dead).map(|m| m.row).collect();
            for row in spent {
                if self.bonus_lawn_mowers_remaining > 0 && !self.has_level_award_dropped() {
                    self.bonus_lawn_mowers_remaining -= 1;
                    let mut mower = LawnMower::new(self, row);
                    mower.state = LawnMowerState::RollingIn;
                    self.lawn_mowers.push(mower);
                }
            }
        }
        if playing {
            let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
            self.summon_backup_dancers(&mut fx);
            self.update_bobsleds(&mut fx);
            self.update_hypnotized_zombies(&mut fx);
            self.after_plants_eaten(res);
            for action in std::mem::take(&mut self.plant_actions) {
                self.do_plant_action(action, effects, res, images, sounds);
            }
            self.update_bungee_plants(effects);
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
                    Some(CoinEvent::Unlocked(unlock)) => self.pending_unlock = Some(unlock),
                    Some(CoinEvent::Landed(_)) | None => {}
                }
            }
            self.add_sun_money(scored);
        }
        self.update_fog();
        self.update_cursor_preview();
        for i in 0..self.seed_bank.num_packets {
            if self.seed_bank.packets[i].update(self.scene, self.main_counter) {
                self.flash_if_ready(res, i);
            }
        }
    }

    /// `Board::UpdateIce`: the trail melts away when its timer runs out.
    fn update_ice(&mut self) {
        for row in 0..MAX_GRID_SIZE_Y {
            if self.ice_timer[row] > 0 {
                self.ice_timer[row] -= 1;
                if self.ice_timer[row] == 0 {
                    self.ice_min_x[row] = BOARD_ICE_START;
                }
            }
        }
    }

    /// `Board::DrawIce`: a strip of ice from the trail's start to the right edge.
    fn draw_ice(&self, row: usize, res: &GameResources, g: &mut Graphics) {
        let (Some(ice), Some(cap)) = (res.image("IMAGE_ICE"), res.image("IMAGE_ICE_CAP")) else { return };
        let y = self.grid_to_pixel_y(8, row as i32) as f32 + 20.0;
        let (w, h) = (ice.width as i32, ice.height as i32);
        let alpha = (self.ice_timer[row].saturating_mul(255) / 10).clamp(0, 255) as u8;
        g.push_state();
        if alpha < 255 {
            g.set_colorize_images(true);
            g.set_color(SexyColor::rgba(255, 255, 255, alpha));
        }
        let start = self.ice_min_x[row] + 13;
        let mut x = start;
        while x < BOARD_WIDTH {
            let dx = if x == start {
                let rest = (BOARD_WIDTH - start) % w;
                if rest == 0 { w } else { rest }
            } else {
                w
            };
            g.draw_image_src(ice, x as f32, y, SexyRect::new((w - dx) as f32, 0.0, dx as f32, h as f32));
            x += dx;
        }
        g.draw_image(cap, self.ice_min_x[row] as f32, y);
        g.pop_state();
    }

    /// `Board::ShakeBoard`.
    fn shake_board(&mut self, amount_x: i32, amount_y: i32) {
        self.shake_counter = 12;
        self.shake_amount = (amount_x, amount_y);
    }

    /// The shake part of `Board::Update`: the board jolts for a moment.
    /// `Plant::UpdateTorchwood`: peas that pass a torchwood catch fire and
    /// snow peas thaw.
    fn update_torchwoods(&mut self, fx: &mut Fx) {
        use projectile::ProjectileType;
        for (_, plant) in self.plants.iter() {
            if plant.seed != SeedType::Torchwood || plant.dead {
                continue;
            }
            let reach = plant.attack_rect();
            for (_, shot) in self.projectiles.iter_mut() {
                if shot.row == plant.row && zombie::rect_overlap(reach, shot.rect()) >= 10 {
                    match shot.ty {
                        ProjectileType::Pea => shot.convert_to_fireball(plant.col, fx),
                        ProjectileType::Snowpea => shot.convert_to_pea(plant.col, fx),
                        _ => {}
                    }
                }
            }
        }
    }

    /// `Board::DoFwoosh`: flames sweep along `row`.
    fn do_fwoosh(&mut self, row: i32, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>, rng: &mut sexy::rand::MtRand) {
        use sexy::reanim::LoopType;
        let render_order = make_render_order(render_layer::PARTICLE, row, 1);
        for i in 0..12 {
            if let Some(old) = self.fwoosh[row as usize][i].take() {
                effects.kill(old);
            }
            let x = 750.0 * i as f32 / 11.0 + 10.0;
            let y = self.pos_y_based_on_row(x + 10.0, row) - 10.0;
            if let Ok((id, flame)) = effects.add_reanimation(x, y, ReanimationType::JalapenoFire, res, images) {
                flame.set_frames_for_layer("anim_flame");
                flame.loop_type = LoopType::LoopFullLastFrame;
                flame.anim_rate *= rng.range_f32(0.7, 1.3);
                flame.render_order = render_order;
                let flip = if rng.next_range(2) != 0 { 1.0 } else { -1.0 };
                flame.override_scale(rng.range_f32(0.9, 1.1) * flip, 1.0);
                self.fwoosh[row as usize][i] = Some(id);
            }
        }
        self.fwoosh_countdown = 100;
    }

    /// `Board::UpdateFwoosh`: the flames die down from the left.
    fn update_fwoosh(&mut self, effects: &mut EffectSystem) {
        use sexy::reanim::LoopType;
        use sexy::tod::{TodCurve, animate_curve};
        if self.fwoosh_countdown == 0 {
            return;
        }
        self.fwoosh_countdown -= 1;
        let remaining = animate_curve(50, 0, self.fwoosh_countdown, 12, 0, TodCurve::Linear);
        for row in &mut self.fwoosh {
            for flame in row.iter_mut().take((12 - remaining).max(0) as usize) {
                if let Some(id) = flame.take()
                    && let Some(flame) = effects.get_mut(id)
                {
                    flame.set_frames_for_layer("anim_done");
                    flame.anim_rate = 15.0;
                    flame.loop_type = LoopType::PlayOnceFullLastFrame;
                }
            }
        }
    }

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
    /// charred, otherwise blown to bits. `damage_range` is what the plant
    /// can reach (`GetDamageRangeFlags`).
    #[allow(clippy::too_many_arguments)]
    fn kill_all_zombies_in_radius(
        &mut self,
        row: i32,
        x: i32,
        y: i32,
        radius: i32,
        row_range: i32,
        burn: bool,
        damage_range: u32,
        env: &zombie::ZombieEnv,
        fx: &mut Fx,
    ) {
        for (_, zombie) in self.zombies.iter_mut() {
            if !zombie.effected_by_damage(damage_range) {
                continue;
            }
            let row_dist = if zombie.is_boss() { 0 } else { zombie.row - row };
            if row_dist.abs() <= row_range && circle_rect_overlap(x, y, radius, zombie.zombie_rect()) {
                if burn {
                    zombie.apply_burn(env, fx);
                } else {
                    zombie.take_damage(1800, 18, env, fx);
                }
            }
        }
    }

    /// `Board::KillAllPlantsInRadius`: a jack-in-the-box takes the plants
    /// around it with it.
    fn kill_all_plants_in_radius(&mut self, x: i32, y: i32, radius: i32, effects: &mut EffectSystem) {
        for (_, plant) in self.plants.iter_mut() {
            if plant.dead {
                continue;
            }
            let rect = plant.rect();
            let (cx, cy) = (rect.x + rect.w / 2, rect.y + rect.h / 2);
            if (cx - x).abs() <= radius && (cy - y).abs() <= radius {
                plant.die(effects);
            }
        }
    }

    /// Where a shot fired at `x` in `row` casts its shadow.
    fn projectile_shadow_y(&self, x: i32, row: i32) -> f32 {
        let grid_x = self.pixel_to_grid_x(x).max(0);
        let shadow_y = self.grid_to_pixel_y(grid_x, row) as f32 + 67.0;
        // Shadows sit 12 pixels higher on the roof's left side.
        if self.stage_has_roof() && x < 480 { shadow_y - 12.0 } else { shadow_y }
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
            PlantAction::Shoot { x, y, row, render_order, projectile, motion, range_flags, short_range, drift, lob } => {
                sounds.play_foley(FoleyType::Throw);
                let shadow_y = self.projectile_shadow_y(x, row);
                let mut shot = Projectile::new(x, y, render_order, row, shadow_y, projectile);
                if let Some(lob) = lob {
                    shot.launch_lob(lob.range_x, lob.range_y, lob.range_flags, sounds.rng());
                } else {
                    shot.set_damage_range(range_flags);
                }
                shot.short_range = short_range;
                shot.set_motion(motion, sounds.rng());
                if drift != 0.0 {
                    shot.set_lane_drift(drift);
                }
                if let Some((effect, dx, dy)) = Projectile::trail_effect(projectile)
                    && let Some(trail) = effects.add_particle(x as f32 + dx, y as f32 + dy, 400000, effect, res, images, sounds.rng())
                {
                    shot.attach_trail(trail, dx, dy);
                }
                self.projectiles.alloc(shot);
            }
            PlantAction::Cob { x, y, render_order, target_x, target_y } => {
                // Behaviour per ruslan831 decomp, Plant::Fire: the cob comes down
                // 40 px left of where the cannon was aimed.
                let row = self.pixel_to_grid_y(target_x.max(80), target_y).max(0);
                let col = self.pixel_to_grid_x(target_x - 40).max(0);
                let land_y = self.grid_to_pixel_y(col, row) as f32;
                let shadow_y = self.projectile_shadow_y(x, row);
                let size = res.image("IMAGE_REANIM_COBCANNON_COB").map_or((40, 40), |i| (i.cel_width() as i32, i.cel_height() as i32));
                let mut cob = Projectile::new(x, y, render_order, row, shadow_y, projectile::ProjectileType::Cob);
                cob.launch_cob((target_x - 40) as f32, row, land_y, size);
                self.projectiles.alloc(cob);
            }
            PlantAction::Particle { x, y, render_order, effect } => {
                self.add_particle(x as f32, y as f32, render_order, effect);
            }
            PlantAction::Foley(foley) => sounds.play_foley(foley),
            PlantAction::RowAreaDamage { col, row, spread, rect, damage, flags } => {
                let env = self.zombie_env();
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                let mut popped_vehicle = false;
                for (_, zombie) in self.zombies.iter_mut() {
                    if (row - spread..=row + spread).any(|r| zombie.in_row(r))
                        && zombie.effected_by_damage(zombie::damage_range::GROUND)
                        && zombie::rect_overlap(rect, zombie.zombie_rect()) > 0
                    {
                        popped_vehicle |= zombie.is_vehicle() && flags & zombie::damage::SPIKE != 0;
                        zombie.take_damage(damage, flags, &env, &mut fx);
                        fx.sounds.play_foley(FoleyType::Splat);
                    }
                }
                // Behaviour per ruslan831 decomp, Plant::DoRowAreaDamage: a
                // vehicle that runs onto spikes wrecks them, or a spikerock's spike.
                if popped_vehicle {
                    let spikes = self.plants.iter().find(|(_, p)| p.col == col && p.row == row && !p.dead && p.seed.is_spiky()).map(|(id, _)| id);
                    let mut more = Vec::new();
                    if let Some(plant) = spikes.and_then(|id| self.plants.get_mut(id)) {
                        if plant.seed == SeedType::Spikerock {
                            plant.spike_rock_take_damage(&mut more, effects);
                        } else {
                            plant.die(effects);
                        }
                    }
                    for action in more {
                        self.do_plant_action(action, effects, res, images, sounds);
                    }
                }
            }
            PlantAction::Jalapeno { row } => {
                let env = self.zombie_env();
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                for (_, zombie) in self.zombies.iter_mut() {
                    if zombie.in_row(row) && zombie.effected_by_damage(zombie::damage_range::EXPLOSION) {
                        zombie.remove_cold_effects(fx.effects);
                        zombie.apply_burn(&env, &mut fx);
                    }
                }
                // Plant::BurnRow: the boss's iceball in this row melts.
                for (_, zombie) in self.zombies.iter_mut() {
                    if zombie.is_boss() {
                        zombie.boss_destroy_iceball_in_row(row, &mut fx);
                    }
                }
                // Plant::BurnRow: the row's ladders burn too.
                for (_, item) in self.grid_items.iter_mut() {
                    if item.item_type == grid_item::GridItemType::Ladder && item.grid_y == row {
                        item.dead = true;
                    }
                }
                self.do_fwoosh(row, effects, res, images, sounds.rng());
                self.shake_board(3, -4);
            }
            PlantAction::Squash { row, rect } => {
                let env = self.zombie_env();
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                for (_, zombie) in self.zombies.iter_mut() {
                    if zombie.in_row(row) && zombie.effected_by_damage(13) {
                        let reach = if zombie.zombie_type == crate::zombie::ZombieType::Football { -20 } else { 0 };
                        if zombie::rect_overlap(rect, zombie.zombie_rect()) > reach {
                            zombie.take_damage(1800, 18, &env, &mut fx);
                        }
                    }
                }
            }
            PlantAction::TangleKelp(id) => {
                let env = self.zombie_env();
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                if let Some(zombie) = self.zombies.get_mut(id) {
                    zombie.take_damage(1800, 0, &env, &mut fx);
                }
            }
            PlantAction::BlowFog => {
                for (_, zombie) in self.zombies.iter_mut() {
                    zombie.blow_away_if_flying();
                }
                sounds.play_sample("SOUND_BLOVER");
                self.fog.blown_count_down = fog::BLOVER_BLOW_TIME;
            }
            PlantAction::ShakeBoard { x, y } => self.shake_board(x, y),
            PlantAction::DoomShroom { x, y, col, row } => {
                sounds.play_sample("SOUND_DOOMSHROOM");
                let env = self.zombie_env();
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                self.kill_all_zombies_in_radius(row, x, y, 250, 3, true, zombie::damage_range::EXPLOSION, &env, &mut fx);
                // KillAllPlantsNearDoom: whatever else is on its square goes too.
                for (_, plant) in self.plants.iter_mut() {
                    if plant.col == col && plant.row == row && !plant.dead {
                        plant.die(effects);
                    }
                }
                self.add_particle(x as f32, y as f32, render_layer::TOP, ParticleEffect::Doom);
                self.add_a_crater(col, row, 18000);
                self.shake_board(3, -4);
            }
            PlantAction::IceShroom { x, y } => {
                let env = self.zombie_env();
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                for (_, zombie) in self.zombies.iter_mut() {
                    zombie.hit_ice_trap(&env, &mut fx);
                    // The boss's fireball goes out.
                    if zombie.is_boss() {
                        zombie.boss_destroy_fireball(&mut fx);
                    }
                }
                self.add_particle(x as f32, y as f32, render_layer::TOP, ParticleEffect::IceTrap);
                // `IceZombies`: the pool's water freezes with them.
                self.ice_trap_counter = 300;
            }
            PlantAction::GraveBusterDone { x, y, col, row, render_order } => {
                if let Some(grave) = self.grave_stone_at(col, row) {
                    self.remove_grave_stone(grave);
                    self.graves_cleared += 1;
                }
                self.add_particle((x + 40) as f32, (y + 40) as f32, render_order, ParticleEffect::GraveBusterDie);
                self.drop_loot_piece(x + 40, y, 12, effects, res, images, sounds);
            }
            PlantAction::Bite { zombie, damage } => {
                let env = self.zombie_env();
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                if let Some(z) = self.zombies.get_mut(zombie) {
                    z.take_damage(damage, 0, &env, &mut fx);
                }
            }
            PlantAction::EatZombie(id) => {
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                if let Some(zombie) = self.zombies.get_mut(id) {
                    zombie.die_with_loot(&mut fx);
                }
            }
            PlantAction::CherryBomb { x, y, row } => {
                let env = self.zombie_env();
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                self.kill_all_zombies_in_radius(row, x, y, 115, 1, true, zombie::damage_range::EXPLOSION, &env, &mut fx);
                self.add_particle(x as f32, y as f32, render_layer::TOP, ParticleEffect::Powie);
                self.shake_board(3, -4);
            }
            PlantAction::PotatoMine { x, y, row } => {
                sounds.play_sample("SOUND_POTATO_MINE");
                let env = self.zombie_env();
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                self.kill_all_zombies_in_radius(row, x, y, 60, 0, false, zombie::damage_range::POTATO_MINE, &env, &mut fx);
                let order = make_render_order(render_layer::PARTICLE, row, 0);
                self.add_particle((x + 20) as f32, y as f32, order, ParticleEffect::PotatoMine);
                self.shake_board(3, -4);
            }
            PlantAction::BowlingHit { zombie: id, giant, bouncing } => {
                sounds.play_foley(FoleyType::BowlingImpact);
                self.shake_board(1, -2);
                let env = self.zombie_env();
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                if let Some(zombie) = self.zombies.get_mut(id) {
                    zombie.bowling_damage(giant, bouncing, &env, &mut fx);
                }
            }
            PlantAction::ExplodeONut { x, y, row } => {
                sounds.play_foley(FoleyType::Cherrybomb);
                sounds.play_sample("SOUND_BOWLINGIMPACT2");
                let env = self.zombie_env();
                let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                self.kill_all_zombies_in_radius(row, x, y, 90, 1, true, zombie::damage_range::EXPLOSION, &env, &mut fx);
                self.add_particle(x as f32, y as f32, render_layer::TOP, ParticleEffect::Powie);
                self.shake_board(3, -4);
            }
            PlantAction::MagnetTake { col, row, zombie } => {
                let env = self.zombie_env();
                let loot = {
                    let mut fx = Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
                    self.zombies.get_mut(zombie).and_then(|z| z.magnet_take(&env, &mut fx))
                };
                if let Some(loot) = loot {
                    let (w, h) = res.image(&loot.image).map_or((0.0, 0.0), |i| (i.width as f32, i.height as f32));
                    let rng = sounds.rng();
                    let (dest_x, dest_y) = (rng.range_f32(-10.0, 10.0) + loot.dest.0, rng.range_f32(-10.0, 10.0) + loot.dest.1);
                    let (x, y) = if loot.centre { (loot.x - w / 2.0, loot.y - h / 2.0) } else { (loot.x, loot.y) };
                    let magnet = self.plants.iter_mut().find(|(_, p)| p.col == col && p.row == row && p.seed == SeedType::Magnetshroom && !p.dead);
                    if let Some((_, plant)) = magnet {
                        plant.hold_magnet_item(plant::MagnetItem { image: loot.image, x, y, dest_x, dest_y, value: 0 });
                    }
                }
            }
            PlantAction::MagnetPullLadder { plant_col, plant_row, col, row } => {
                let ladder = self
                    .grid_items
                    .iter()
                    .find(|(_, g)| g.item_type == grid_item::GridItemType::Ladder && g.grid_x == col && g.grid_y == row)
                    .map(|(id, _)| id);
                if let Some(ladder) = ladder
                    && let Some(item) = self.grid_items.get_mut(ladder)
                {
                    item.dead = true;
                }
                let (x, y) = (self.grid_to_pixel_x(col) as f32 + 40.0, self.grid_to_pixel_y(col, row) as f32);
                let rng = sounds.rng();
                let (dest_x, dest_y) = (rng.range_f32(-10.0, 10.0) + 10.0, rng.range_f32(-10.0, 10.0));
                let magnet = self
                    .plants
                    .iter_mut()
                    .find(|(_, p)| p.col == plant_col && p.row == plant_row && p.seed == SeedType::Magnetshroom && !p.dead);
                if let Some((_, plant)) = magnet {
                    plant.hold_magnet_item(plant::MagnetItem {
                        image: "IMAGE_REANIM_ZOMBIE_LADDER_5".to_owned(),
                        x,
                        y,
                        dest_x,
                        dest_y,
                        value: 0,
                    });
                }
            }
            PlantAction::ShowCoinBank => self.show_coin_bank(),
            PlantAction::AddMoney { amount } => {
                self.pending_money += amount;
                sounds.play_foley(FoleyType::Coin);
            }
            PlantAction::GoldMagnetCollect { col, row } => {
                let magnet = self.plants.iter().find(|(_, p)| p.col == col && p.row == row && p.seed == SeedType::GoldMagnet && !p.dead);
                let Some((magnet_id, centre)) = magnet.map(|(id, p)| (id, (p.x as f32 + 40.0, p.y as f32 + 40.0))) else { return };
                // Behaviour per ruslan831 decomp, Plant::GoldMagnetFindTargets: the
                // nearest money first, until its five places are full.
                while self.plants.get(magnet_id).is_some_and(Plant::has_room_for_coin) {
                    let nearest = self
                        .coins
                        .iter()
                        .filter(|(_, c)| c.is_gold_magnet_target())
                        .map(|(id, c)| (id, (centre.0 - c.middle().0).hypot(centre.1 - c.middle().1)))
                        .min_by(|a, b| a.1.total_cmp(&b.1));
                    let Some((coin_id, _)) = nearest else { break };
                    let Some(coin) = self.coins.get_mut(coin_id) else { break };
                    let rng = sounds.rng();
                    let item = plant::MagnetItem {
                        image: coin.magnet_image().to_string(),
                        x: coin.pos_x + 15.0,
                        y: coin.pos_y + 15.0,
                        dest_x: rng.range_f32(20.0, 40.0),
                        dest_y: rng.range_f32(-20.0, 0.0) + 20.0,
                        value: coin.coin_value(),
                    };
                    coin.die(effects);
                    if let Some(plant) = self.plants.get_mut(magnet_id) {
                        plant.add_magnet_coin(item);
                    }
                }
            }
            PlantAction::ImitaterMorph { col, row, seed } => {
                // Behaviour per ruslan831 decomp, Plant::ImitaterMorph: the copy
                // takes its place, washed out.
                self.add_plant(col, row, seed, effects, res, images, sounds);
                let copy = self.plants.iter().filter(|(_, p)| p.col == col && p.row == row && p.seed == seed && !p.dead).map(|(id, _)| id).last();
                if let Some(plant) = copy.and_then(|id| self.plants.get_mut(id)) {
                    plant.wash_out(effects);
                }
            }
            PlantAction::WakeUp { col, row } => {
                let sleeper = self.plants.iter().find(|(_, p)| p.col == col && p.row == row && !p.dead && p.is_asleep()).map(|(id, _)| id);
                if let Some(plant) = sleeper.and_then(|id| self.plants.get_mut(id)) {
                    plant.start_waking();
                }
            }
            PlantAction::MakeCoin { x, y, gold } => {
                sounds.play_foley(FoleyType::SpawnSun);
                let ty = if gold { CoinType::Gold } else { CoinType::Silver };
                self.add_coin(x as f32, y as f32, ty, CoinMotion::Coin, effects, res, images, sounds);
            }
            PlantAction::MakeSun { x, y, small } => {
                sounds.play_foley(FoleyType::SpawnSun);
                let ty = if small { CoinType::SmallSun } else { CoinType::Sun };
                self.add_coin(x as f32, y as f32, ty, CoinMotion::FromPlant, effects, res, images, sounds);
            }
        }
    }

    /// `Board::ProcessDeleteQueue`: frees dead plants, zombies, projectiles and coins.
    fn process_delete_queue(&mut self) {
        // Behaviour per ruslan831 decomp, Plant::Die: a ladder goes with the
        // plant it leant on.
        let gone: Vec<_> = self.plants.iter().filter(|(_, p)| (p.dead || p.squished) && p.seed != SeedType::InstantCoffee).map(|(_, p)| (p.col, p.row)).collect();
        for (col, row) in gone {
            if let Some(ladder) = self.ladder_at(col, row).and_then(|id| self.grid_items.get_mut(id)) {
                ladder.dead = true;
            }
        }
        self.plants.retain(|p| !p.dead);
        self.zombies.retain(|z| !z.dead);
        self.lawn_mowers.retain(|m| !m.dead);
        self.projectiles.retain(|p| !p.dead);
        self.coins.retain(|c| !c.dead);
        self.grid_items.retain(|g| !g.dead);
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
    ) -> sexy::data_array::Id<Coin> {
        let coin = Coin::new(x, y, ty, motion, effects, res, images, sounds.rng());
        let id = self.coins.alloc(coin);
        if self.first_time_adventure && self.level == 1 {
            let style = MessageStyle::TutorialLevel1Stay;
            self.display_advice(res, "[ADVICE_CLICK_ON_SUN]", style, Some(AdviceType::ClickOnSun));
        }
        id
    }
}

/// Sun falls every `SUN_COUNTDOWN` ticks plus 10 per sun so far (up to
/// `SUN_COUNTDOWN_MAX`), plus up to `SUN_COUNTDOWN_RANGE` at random.
const SUN_COUNTDOWN: i32 = 425;
const SUN_COUNTDOWN_RANGE: i32 = 275;
const SUN_COUNTDOWN_MAX: i32 = 950;

/// The menu button and Escape: `LawnApp::DoNewOptions(false)`, the options as
/// the pause menu (a dialog being open pauses the board).
fn open_options(board: &Board, dialogs: &mut Dialogs, res: &GameResources, settings: &crate::options::Settings) {
    // LawnApp::CanShowAlmanac, for the level being played.
    let can_show_almanac = !board.first_time_adventure || board.level >= 15;
    let from = crate::dialog::OptionsFrom::Board { level_intro: board.scene == GameScene::LevelIntro, can_show_almanac };
    crate::options::do_new_options(dialogs, res, settings, from);
}

impl Board {
    /// `Board::CanInteractWithBoardButtons`: not while carrying something.
    fn can_interact_with_board_buttons(&self) -> bool {
        !self.paused && self.cursor.cursor_type == cursor::CursorType::Normal
    }

    /// `Board::SetCursor`: whether the pointer is a hand (or hidden) at `p`
    /// and whether it's shown at all.
    fn pointer_icon(&self, p: Vec2, res: &GameResources, modal: bool) -> (bool, bool) {
        if modal || self.paused || self.seed_chooser.is_some() || self.scene == GameScene::ZombiesWon {
            return (false, true);
        }
        // The hammer has its own cursor, drawn in its place.
        if self.cursor.cursor_type == cursor::CursorType::Hammer {
            return (false, false);
        }
        let over_packet = self
            .seed_packet_at(p.as_ivec2())
            .is_some_and(|i| self.seed_bank.packets[i].active && self.packet_can_pick_up(i));
        let (sx, sy, sw, sh) = self.shovel_button_rect(res);
        let over_shovel = self.show_shovel
            && p.x >= sx as f32
            && p.x < (sx + sw) as f32
            && p.y >= sy as f32
            && p.y < (sy + sh) as f32;
        let hand = over_packet || over_shovel || self.coin_at(p).is_some() || self.menu_button.is_mouse_over();
        (hand, true)
    }
}

/// `Board::SetCursor`: the hand over anything clickable, hidden for the hammer.
#[allow(clippy::too_many_arguments, reason = "a Bevy system's parameters")]
fn update_mouse_icon(
    board: Res<Board>,
    cursor: Res<crate::input::GameCursor>,
    dialogs: Res<Dialogs>,
    res: Res<GameResources>,
    window: Single<Entity, With<bevy::window::PrimaryWindow>>,
    icons: Query<&bevy::window::CursorIcon>,
    mut options: Single<&mut bevy::window::CursorOptions, With<bevy::window::PrimaryWindow>>,
    mut commands: Commands,
) {
    use bevy::window::{CursorIcon, SystemCursorIcon};
    let Some(p) = cursor.0 else { return };
    let (hand, visible) = board.pointer_icon(p, &res, dialogs.is_modal());
    options.visible = visible;
    let icon = if hand { CursorIcon::System(SystemCursorIcon::Pointer) } else { CursorIcon::default() };
    if icons.get(*window).ok() != Some(&icon) {
        commands.entity(*window).insert(icon);
    }
}

/// Mouse handling: `Board::MouseDown`/`MouseUp` and the menu button
/// (`GameButton::Update`).
#[allow(clippy::too_many_arguments)]
fn board_input(
    mut board: ResMut<Board>,
    cursor: Res<GameCursor>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut music: ResMut<Music>,
    mut dialogs: ResMut<Dialogs>,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut effects: ResMut<EffectSystem>,
    mut sounds: Sounds,
    settings: Res<crate::options::Settings>,
) {
    let board = &mut *board;
    if board.mode == GameMode::Intro {
        return;
    }
    // Board::KeyDown: Escape opens the options too (not when the key just
    // closed a dialog).
    if keys.just_pressed(KeyCode::Escape) && !dialogs.took_input() && board.can_interact_with_board_buttons() {
        open_options(board, &mut dialogs, &resources, &settings);
    }
    // Space and Enter move Crazy Dave's talk along, like a click.
    if !dialogs.is_modal()
        && board.scene == GameScene::LevelIntro
        && board.cut_scene.is_showing_crazy_dave()
        && keys.any_just_pressed([KeyCode::Space, KeyCode::Enter])
        && cutscene::advance_crazy_dave_dialog(board, &mut effects, &mut resources, &mut images, &mut sounds)
    {
        music.fade_out(50);
    }
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
        } else if board.scene == GameScene::LevelIntro && board.cut_scene.is_showing_crazy_dave() {
            if cutscene::advance_crazy_dave_dialog(board, &mut effects, &mut resources, &mut images, &mut sounds) {
                music.fade_out(50);
            }
        } else if board.cut_scene.seed_choosing {
            let screen = p + Vec2::new(board.x as f32, board.y as f32);
            match board.seed_chooser_mouse_down(screen.floor().as_ivec2(), &mut sounds) {
                Some(seed_chooser::ChooserClick::Warning(warning)) => {
                    do_chooser_warning_dialog(&mut dialogs, &resources, warning);
                }
                Some(seed_chooser::ChooserClick::Almanac) => {
                    crate::almanac::do_almanac_dialog(&mut dialogs, &resources, crate::almanac::AlmanacStart::Index);
                }
                None => {}
            }
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
        open_options(board, &mut dialogs, &resources, &settings);
    }
    board.flush_particle_requests(&mut effects, &mut resources, &mut images, sounds.rng());
}

/// `GameOverDialog`: Try Again in the dialog, Main Menu at the screen's top
/// right, and the board's own buttons hidden.
fn do_game_over_dialog(dialogs: &mut Dialogs, res: &GameResources, board: &mut Board) {
    use crate::dialog::{DialogButton, DialogKind};
    use crate::widgets::StoneButton;
    let dialog = dialogs.do_dialog(
        res,
        DialogId::GameOver,
        &res.translate("[GAME_OVER]"),
        "",
        &res.translate("[TRY_AGAIN]"),
        ButtonMode::Footer,
    );
    let (x, y) = (dialog.x, dialog.y);
    let mut menu = StoneButton::new(DialogButton::MainMenu, res.translate("[MAIN_MENU_BUTTON]"));
    menu.resize(635 - x, -10 - y, 163, 46);
    dialog.kind = DialogKind::GameOver { menu };
    board.show_shovel = false;
    board.menu_button.no_draw = true;
}

/// `CutScene::AdvanceCrazyDaveDialog`'s question: buy a seed slot for `cost` coins?
fn do_purchase_packet_slot_dialog(dialogs: &mut Dialogs, res: &GameResources, slots: i32, cost: i32) {
    let body = res.translate("[UPGRADE_DIALOG_BODY]").replace("{SLOTS}", &slots.to_string());
    let header = coin::money_string(cost);
    let dialog = dialogs.do_dialog(res, DialogId::PurchasePacketSlot, &header, &body, "", ButtonMode::YesNo);
    dialog.x += 120;
    dialog.y += 130;
}

/// `SeedChooserScreen::DisplayRepickWarningDialog`.
fn do_chooser_warning_dialog(dialogs: &mut Dialogs, res: &GameResources, warning: ChooserWarning) {
    let header = res.translate("[DIALOG_WARNING]");
    let dialog = dialogs.do_dialog(res, DialogId::ChooserWarning, &header, &warning.text(res), "", ButtonMode::YesNo);
    if let Some(yes) = &mut dialog.yes_button {
        yes.label = res.translate("[DIALOG_BUTTON_YES]");
    }
    if let Some(no) = &mut dialog.no_button {
        no.label = res.translate("[REPICK_BUTTON]");
    }
}

/// What the board draws, sorted by render order (`RenderObjectType`).
enum RenderItem {
    PumpkinBack(sexy::data_array::Id<Plant>),
    Backdrop,
    BottomUi,
    IntroOverlay,
    SeedChooser,
    CrazyDave,
    TopUi,
    LawnMower(usize),
    GridItem(sexy::data_array::Id<GridItem>),
    Reanimation(crate::reanim::ReanimationId),
    Plant(sexy::data_array::Id<Plant>),
    Zombie(sexy::data_array::Id<Zombie>),
    BossPart(sexy::data_array::Id<Zombie>, zombie::BossPart),
    ZombieShadow(sexy::data_array::Id<Zombie>),
    BungeeTarget(sexy::data_array::Id<Zombie>),
    Projectile(sexy::data_array::Id<Projectile>),
    ProjectileShadow(sexy::data_array::Id<Projectile>),
    Particle(ParticleSystemId),
    FadeOut,
    Coin(sexy::data_array::Id<Coin>),
    CursorPreview,
    Ice(usize),
    Fog,
    /// The door frame over the lawn when the zombies have won.
    DoorMask,
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
        // A bungee zombie draws the plant it has hold of.
        if plant.on_bungee == plant::OnBungee::No {
            items.push((plant.render_order, RenderItem::Plant(id)));
            if plant.seed == SeedType::Pumpkinshell {
                items.push((plant.pumpkin_back_order(), RenderItem::PumpkinBack(id)));
            }
        }
    }
    for (id, coin) in board.coins.iter() {
        items.push((coin.render_order, RenderItem::Coin(id)));
    }
    for (id, zombie) in board.zombies.iter() {
        // Invisighoul: their bites still land, but nothing of them shows.
        if board.zombie_hidden(zombie) {
            continue;
        }
        if zombie.is_boss() {
            // The boss is drawn in parts, each among the lawn's objects.
            items.extend(zombie.boss_part_orders(&effects).into_iter().map(|(order, part)| (order, RenderItem::BossPart(id, part))));
            continue;
        }
        items.push((zombie.render_order, RenderItem::Zombie(id)));
        items.push((make_render_order(render_layer::GROUND, zombie.row, 3), RenderItem::ZombieShadow(id)));
        if zombie.zombie_type == crate::zombie::ZombieType::Bungee && !board.is_final_boss_level() {
            items.push((make_render_order(render_layer::PROJECTILE, zombie.row, 1), RenderItem::BungeeTarget(id)));
        }
    }
    for (id, projectile) in board.projectiles.iter() {
        items.push((projectile.render_order, RenderItem::Projectile(id)));
        items.push((make_render_order(render_layer::GROUND, projectile.row, 3), RenderItem::ProjectileShadow(id)));
    }
    for (id, item) in board.grid_items.iter() {
        items.push((item.render_order, RenderItem::GridItem(id)));
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
    for row in 0..MAX_GRID_SIZE_Y {
        if board.ice_timer[row] > 0 {
            items.push((make_render_order(render_layer::GROUND, row as i32, 2), RenderItem::Ice(row)));
        }
    }
    if board.stage_has_fog() {
        items.push((make_render_order(render_layer::FOG, 0, 0), RenderItem::Fog));
    }
    items.push((CursorPreview::RENDER_ORDER, RenderItem::CursorPreview));
    items.push((make_render_order(render_layer::SCREEN_FADE, 0, 0), RenderItem::FadeOut));
    let bottom_ui = if board.scene == GameScene::Playing || board.cut_scene.is_after_seed_chooser() {
        make_render_order(render_layer::UI_BOTTOM, 0, 1)
    } else {
        make_render_order(render_layer::ABOVE_UI, 0, 0)
    };
    items.push((make_render_order(render_layer::UI_BOTTOM, 0, 0), RenderItem::Backdrop));
    if board.mode == GameMode::Intro {
        items.push((make_render_order(render_layer::SCREEN_FADE, 0, 5), RenderItem::IntroOverlay));
    } else {
        items.push((bottom_ui, RenderItem::BottomUi));
    }
    // The chooser sits in front of the seed bank its seeds fly into.
    items.push((make_render_order(render_layer::ABOVE_UI, 0, 2), RenderItem::SeedChooser));
    if board.scene == GameScene::LevelIntro {
        items.push((make_render_order(render_layer::ABOVE_UI, 0, 3), RenderItem::CrazyDave));
    }
    if board.mode != GameMode::Intro {
        items.push((make_render_order(render_layer::UI_TOP, 0, 0), RenderItem::TopUi));
    }
    if board.scene == GameScene::ZombiesWon {
        let z = if board.stage_has_roof() {
            make_render_order(render_layer::GRAVE_STONE, 0, 4)
        } else {
            make_render_order(render_layer::GRAVE_STONE, 3, 2)
        };
        items.push((z, RenderItem::DoorMask));
    }
    // TODO: the coin bank, screen fade and cursor preview.
    items.sort_by_key(|(z, _)| *z);

    g.push_state();
    g.translate(board.x as f32, board.y as f32);
    for (_, item) in items {
        match item {
            RenderItem::Backdrop => draw_backdrop(board, &resources, &mut g),
            RenderItem::BottomUi => draw_ui_bottom(board, &resources, &mut g),
            RenderItem::TopUi => draw_ui_top(board, &effects, &resources, &mut g),
            RenderItem::IntroOverlay => cutscene::draw_intro(board, &resources, &mut g),
            RenderItem::SeedChooser => board.draw_seed_chooser(&resources, &mut g),
            RenderItem::CrazyDave => {
                g.push_state();
                g.translate(-board.x as f32, -board.y as f32);
                board.dave.draw(&effects, &resources, &mut g);
                g.pop_state();
            }
            RenderItem::LawnMower(i) => board.lawn_mowers[i].draw(board, &effects, &resources, &mut g),
            RenderItem::GridItem(id) => {
                if let Some(item) = board.grid_items.get(id) {
                    board.draw_grid_item(item, &effects, &resources, &mut g);
                }
            }
            RenderItem::Reanimation(id) => effects.draw(&mut g, id, &resources),
            RenderItem::Plant(id) => {
                if let Some(plant) = board.plants.get(id) {
                    plant.draw(&mut g, board.stage_is_night(), &effects, &resources);
                }
            }
            RenderItem::PumpkinBack(id) => {
                if let Some(plant) = board.plants.get(id) {
                    plant.draw_pumpkin_back(&mut g, &effects, &resources);
                }
            }
            RenderItem::FadeOut => board.draw_fade_out(&mut g),
            RenderItem::Ice(row) => board.draw_ice(row, &resources, &mut g),
            RenderItem::Fog => board.draw_fog(&resources, &mut g),
            RenderItem::DoorMask => draw_house_door_mask(board, &resources, &mut g),
            RenderItem::Zombie(id) => {
                if let Some(zombie) = board.zombies.get(id).filter(|z| !z.is_winner() || board.cut_scene.show_zombie_walking()) {
                    let carried = zombie.target_plant.and_then(|p| board.plants.get(p)).filter(|p| !p.dead && p.on_bungee != plant::OnBungee::No);
                    zombie.draw(&mut g, &effects, &resources, carried, board.stage_is_night(), board.stage_has_roof());
                }
            }
            RenderItem::BossPart(id, part) => {
                if let Some(zombie) = board.zombies.get(id) {
                    zombie.draw_boss_part(&mut g, &effects, &resources, part);
                }
            }
            RenderItem::BungeeTarget(id) => {
                if let Some(zombie) = board.zombies.get(id) {
                    zombie.draw_bungee_target(&mut g, &resources);
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

/// `DrawHouseDoorBottom`: the inside of the house, seen once the zombies are in.
fn draw_house_door_bottom(board: &Board, res: &GameResources, g: &mut Graphics) {
    let (id, x, y) = match board.background {
        BackgroundType::Day => ("IMAGE_BACKGROUND1_GAMEOVER_INTERIOR_OVERLAY", -126.0, 225.0),
        BackgroundType::Night => ("IMAGE_BACKGROUND2_GAMEOVER_INTERIOR_OVERLAY", -125.0, 196.0),
        BackgroundType::Pool => ("IMAGE_BACKGROUND3_GAMEOVER_INTERIOR_OVERLAY", -171.0, 241.0),
        BackgroundType::Fog => ("IMAGE_BACKGROUND4_GAMEOVER_INTERIOR_OVERLAY", -172.0, 246.0),
        _ => return,
    };
    if let Some(image) = res.image(id) {
        g.draw_image(image, x, y);
    }
}

/// `DrawHouseDoorTop`: the door frame over the lawn when the zombies won.
fn draw_house_door_mask(board: &Board, res: &GameResources, g: &mut Graphics) {
    let (id, x, y) = match board.background {
        BackgroundType::Day => ("IMAGE_BACKGROUND1_GAMEOVER_MASK", -130.0, 202.0),
        BackgroundType::Night => ("IMAGE_BACKGROUND2_GAMEOVER_MASK", -128.0, 207.0),
        BackgroundType::Pool => ("IMAGE_BACKGROUND3_GAMEOVER_MASK", -172.0, 234.0),
        BackgroundType::Fog => ("IMAGE_BACKGROUND4_GAMEOVER_MASK", -173.0, 133.0),
        BackgroundType::Roof => ("IMAGE_BACKGROUND5_GAMEOVER_MASK", -220.0, 81.0),
        BackgroundType::Boss => ("IMAGE_BACKGROUND6_GAMEOVER_MASK", -220.0, 81.0),
    };
    if let Some(image) = res.image(id) {
        g.draw_image(image, x, y);
    }
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
    // `DrawHouseDoorBottom`: the inside of the house once the zombies are in.
    if board.scene == GameScene::ZombiesWon {
        draw_house_door_bottom(board, res, g);
    }
    // PoolEffectDraw: the animated water, or the static picture if that can't be drawn.
    if board.stage_has_pool() {
        let animated = board.pool_water.as_ref().is_some_and(|water| water.draw(g, res, board.stage_is_night()));
        if !animated && let Some(pool) = image(if board.stage_is_night() { "IMAGE_POOL_NIGHT" } else { "IMAGE_POOL" }) {
            g.draw_image(pool, 34.0, 278.0);
        }
    }
    // Level 1's tutorial flashes the sod row it wants the peashooter on.
    if board.tutorial_state == TutorialState::Level1PlantPeashooter
        && let Some(sod) = image("IMAGE_SOD1ROW")
    {
        let gray = seed_bank::flashing_gray(board.main_counter, 75) as u8;
        g.push_state();
        g.set_colorize_images(true);
        g.set_color(SexyColor::rgba(gray, gray, gray, 255));
        g.draw_image(sod, (239 - BOARD_OFFSET) as f32, 265.0);
        g.pop_state();
    }
    // Challenge::DrawBackground: the line wall-nuts start behind.
    if board.is_wallnut_bowling_level()
        && board.show_bowling_line
        && let Some(stripe) = image("IMAGE_WALLNUT_BOWLINGSTRIPE")
    {
        g.draw_image(stripe, 268.0, 77.0);
    }
    // The graveyard beyond the night lawn, seen as the intro pans right.
    if board.scene == GameScene::LevelIntro
        && board.stage_has_grave_stones()
        && let Some(graveyard) = image("IMAGE_NIGHT_GRAVE_GRAPHIC")
    {
        g.draw_image(graveyard, 1092.0, 40.0);
    }
    // `Challenge::DrawArtChallenge`: Seeing Stars' ghost picture.
    if board.minigame() == Some(MiniGame::SeeingStars) {
        for row in 0..6 {
            for col in 0..9 {
                if let Some(seed) = board.art_challenge_seed(col, row)
                    && board.normal_plant_at(col, row).is_none()
                {
                    seed_bank::draw_seed_packet(
                        g,
                        res,
                        board.grid_to_pixel_x(col) as f32,
                        board.grid_to_pixel_y(col, row) as f32,
                        seed,
                        0.0,
                        100,
                        false,
                    );
                }
            }
        }
    }
    // TODO: the level 1 tutorial's flashing sod row.
}

/// `Board::DrawUIBottom`: the seed bank and the menu button.
fn draw_ui_bottom(board: &Board, res: &GameResources, g: &mut Graphics) {
    // `LawnApp::IsChallengeWithoutSeedBank`: It's Raining Seeds has no bank.
    if !board.is_challenge_without_seed_bank() {
        board.seed_bank.draw(board, res, g);
    }
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
    // While the seed chooser is up it draws the button, in screen coordinates.
    if board.scene != GameScene::Playing && board.seed_chooser.is_some() {
        return;
    }
    draw_menu_button(board, res, g);
}

/// A seed packet drawn at a spot (the award screen).
pub fn draw_seed_packet_at(g: &mut Graphics, res: &GameResources, x: f32, y: f32, seed: SeedType) {
    seed_bank::draw_seed_packet(g, res, x, y, seed, 0.0, 255, true);
}

/// A seed packet with no cost under it (the store's items).
pub fn draw_seed_packet_free(g: &mut Graphics, res: &GameResources, x: f32, y: f32, seed: SeedType) {
    seed_bank::draw_seed_packet(g, res, x, y, seed, 0.0, 255, false);
}

/// The stone Menu button at the top right.
fn draw_menu_button(board: &Board, res: &GameResources, g: &mut Graphics) {
    let b = &board.menu_button;
    if b.no_draw {
        return;
    }
    let is_down = b.is_down && b.is_over && !b.disabled;
    let r = b.rect;
    draw_stone_button(g, res, r.x0, r.y0, r.x1 - r.x0, r.y1 - r.y0, is_down, b.is_mouse_over(), &b.label);
}

/// `Board::DrawUITop`: the level name and the advice banner.
fn draw_ui_top(board: &Board, effects: &EffectSystem, res: &GameResources, g: &mut Graphics) {
    if board.stage_has_fog() {
        draw_top_right_ui(board, res, g);
    }
    if board.scene == GameScene::Playing {
        board.draw_progress_meter(res, g);
        draw_level(board, res, g);
    }
    board.draw_coin_bank(res, g);
    board.advice.draw(board, res, g, effects);
    board.draw_cursor_object(res, g);
    board.draw_hammer(effects, res, g);
}

/// `Board::DrawLevel`: "Level 1-1" in the bottom-right corner.
fn draw_level(board: &Board, res: &GameResources, g: &mut Graphics) {
    let Some(font) = res.font("FONT_HOUSEOFTERROR16") else { return };
    let text = format!("{} {}", res.translate("[LEVEL]"), board.stage_string());
    let x = if board.has_progress_meter() { 593 } else { 780 };
    g.tod_draw_string(font, x, 595, &text, SexyColor::rgba(224, 187, 98, 255), Justify::Right);
}
