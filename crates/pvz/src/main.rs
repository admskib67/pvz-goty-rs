mod audio;
mod award;
mod board;
mod devshot;
mod dialog;
mod game_selector;
mod graphics;
mod input;
mod install;
mod lawn_app;
mod pak_source;
mod particle;
mod plant;
mod player;
mod reanim;
mod resources;
mod setup;
mod title;
mod widgets;
mod zombie;

use std::path::PathBuf;

use bevy::camera::ScalingMode;
use bevy::prelude::*;

use install::Config;
use pak_source::{GamePak, PakSourcePlugin};
use setup::SetupStatus;

/// The game's fixed logical resolution; the window scales it.
pub const GAME_WIDTH: f32 = 800.0;
pub const GAME_HEIGHT: f32 = 600.0;
/// The original updates game logic 100 times per second (`FixedUpdate` here).
pub const TICKS_PER_SECOND: f64 = 100.0;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    /// Opening the remembered install, if any.
    #[default]
    Boot,
    /// Asking the player where their copy of PvZ is.
    Setup,
    Title,
    /// The main menu.
    GameSelector,
    /// Playing a level.
    Board,
    /// The new plant (or other prize) after winning a level.
    Award,
}

/// Frame-by-frame order of input handling and drawing: dialogs get the
/// input first (they block the screen behind) and are drawn last (on top).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum UiSet {
    DialogInput,
    /// Acting on dialog buttons (`lawn_app`).
    DialogActions,
    ScreenInput,
    ScreenDraw,
    DialogDraw,
}

/// Command-line flag that wipes all saved app data before starting.
const CLEAN_FLAG: &str = "--clean";

/// `--lvl-8` or `--lvl 8`: start adventure on that level, without touching the saved player.
pub fn level_flag() -> Option<i32> {
    static FLAG: std::sync::OnceLock<Option<i32>> = std::sync::OnceLock::new();
    *FLAG.get_or_init(parse_level_flag)
}

fn parse_level_flag() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    args.iter().enumerate().find_map(|(i, a)| {
        a.strip_prefix("--lvl-")
            .or_else(|| a.strip_prefix("--lvl="))
            .map(str::to_owned)
            .or_else(|| (a == "--lvl").then(|| args.get(i + 1).cloned()).flatten())
            .and_then(|v| v.parse::<i32>().ok())
            .map(|l| l.clamp(1, 50))
    })
}

fn main() {
    if std::env::args().skip(1).any(|a| a == CLEAN_FLAG) {
        match install::clean_user_data() {
            Ok(removed) if removed.is_empty() => println!("{CLEAN_FLAG}: no saved data to remove"),
            Ok(removed) => {
                for dir in removed {
                    println!("{CLEAN_FLAG}: removed {}", dir.display());
                }
            }
            Err(e) => eprintln!("{CLEAN_FLAG}: couldn't remove saved data: {e}"),
        }
    }

    App::new()
        // Must come before DefaultPlugins builds the AssetPlugin.
        .add_plugins(PakSourcePlugin)
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Plants vs. Zombies".into(),
                resolution: (GAME_WIDTH as u32, GAME_HEIGHT as u32).into(),
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .init_resource::<SetupStatus>()
        .init_resource::<player::GameRng>()
        .insert_resource(board::LevelOverride(level_flag()))
        .insert_resource(Time::<Fixed>::from_hz(TICKS_PER_SECOND))
        .insert_resource(ClearColor(Color::BLACK))
        .configure_sets(
            Update,
            (UiSet::DialogInput, UiSet::DialogActions, UiSet::ScreenInput, UiSet::ScreenDraw, UiSet::DialogDraw).chain(),
        )
        .add_plugins((
            setup::SetupPlugin,
            graphics::GraphicsPlugin,
            audio::AudioPlugin,
            input::InputPlugin,
            resources::ResourcesPlugin,
            title::TitlePlugin,
            game_selector::GameSelectorPlugin,
            board::BoardPlugin,
            award::AwardPlugin,
            dialog::DialogPlugin,
            lawn_app::LawnAppPlugin,
            devshot::DevShotPlugin,
        ))
        .add_systems(Startup, spawn_camera)
        .add_systems(OnEnter(AppState::Boot), boot)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin { min_width: GAME_WIDTH, min_height: GAME_HEIGHT },
            ..OrthographicProjection::default_2d()
        }),
    ));
}

/// Uses `$PVZ_GAME_PATH` or the saved install; falls back to the setup screen.
fn boot(game_pak: Res<GamePak>, mut status: ResMut<SetupStatus>, mut next: ResMut<NextState<AppState>>) {
    let chosen = std::env::var_os(install::ENV_GAME_PATH)
        .map(PathBuf::from)
        .or_else(|| Config::load().game_path);
    let Some(chosen) = chosen else {
        next.set(AppState::Setup);
        return;
    };
    match install::open_install(&chosen) {
        Ok((pak_path, pak)) => {
            info!("using {} ({} files)", pak_path.display(), pak.entries().len());
            game_pak.set(&pak_path, pak);
            next.set(AppState::Title);
        }
        Err(e) => {
            status.message = format!("Your saved install couldn't be opened. {e}");
            next.set(AppState::Setup);
        }
    }
}
