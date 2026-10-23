//! Development aid: `PVZ_SCREENSHOT=out.png` saves the window after a short
//! delay and quits, so screens can be checked without a person at the keyboard.
//! `PVZ_SCREENSHOT_FRAME=<n>` picks the frame (default 120).

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

const ENV: &str = "PVZ_SCREENSHOT";
const ENV_FRAME: &str = "PVZ_SCREENSHOT_FRAME";
const DEFAULT_FRAME: u32 = 120;
/// Frames to wait after capturing so the file is written before exiting.
const EXIT_DELAY: u32 = 30;

pub struct DevShotPlugin;

impl Plugin for DevShotPlugin {
    fn build(&self, app: &mut App) {
        if let Some(path) = std::env::var_os(ENV) {
            let frame = std::env::var(ENV_FRAME).ok().and_then(|f| f.parse().ok()).unwrap_or(DEFAULT_FRAME);
            app.insert_resource(ShotPath(path.into(), frame))
                .add_systems(Update, capture);
        }
    }
}

#[derive(Resource)]
struct ShotPath(std::path::PathBuf, u32);

fn capture(
    mut commands: Commands,
    mut frame: Local<u32>,
    path: Res<ShotPath>,
    mut exit: MessageWriter<AppExit>,
) {
    *frame += 1;
    if *frame == path.1 {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path.0.clone()));
    } else if *frame == path.1 + EXIT_DELAY {
        exit.write(AppExit::Success);
    }
}
