//! Development aid: `PVZ_SCREENSHOT=out.png` saves the window after a short
//! delay and quits, so screens can be checked without a person at the keyboard.
//! `PVZ_SCREENSHOT_FRAME=<n>` picks the frame (default 120); a comma-separated
//! list (`PVZ_SCREENSHOT_FRAME=900,1500`) takes several, the file name's `{}`
//! standing for the frame number (`out_{}.png`), and quits after the last.

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
            let frames: Vec<u32> = std::env::var(ENV_FRAME)
                .ok()
                .map(|f| f.split(',').filter_map(|n| n.trim().parse().ok()).collect())
                .filter(|f: &Vec<u32>| !f.is_empty())
                .unwrap_or_else(|| vec![DEFAULT_FRAME]);
            app.insert_resource(ShotPath(path.into(), frames)).add_systems(Update, capture);
        }
    }
}

#[derive(Resource)]
struct ShotPath(std::path::PathBuf, Vec<u32>);

fn capture(
    mut commands: Commands,
    mut frame: Local<u32>,
    path: Res<ShotPath>,
    mut exit: MessageWriter<AppExit>,
) {
    *frame += 1;
    if path.1.contains(&*frame) {
        let name = path.0.to_string_lossy().replace("{}", &frame.to_string());
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(std::path::PathBuf::from(name)));
    }
    if path.1.iter().max().is_some_and(|last| *frame == last + EXIT_DELAY) {
        exit.write(AppExit::Success);
    }
}
