mod devshot;
mod graphics;
mod input;
mod install;
mod pak_source;
mod resources;
mod setup;
mod title;

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
}

/// Command-line flag that wipes all saved app data before starting.
const CLEAN_FLAG: &str = "--clean";

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
        .insert_resource(Time::<Fixed>::from_hz(TICKS_PER_SECOND))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((
            setup::SetupPlugin,
            graphics::GraphicsPlugin,
            input::InputPlugin,
            resources::ResourcesPlugin,
            title::TitlePlugin,
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
