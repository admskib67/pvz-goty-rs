//! Development aid: `PVZ_SCREENSHOT=out.png` saves the window after a short
//! delay and quits, so screens can be checked without a person at the keyboard.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

const ENV: &str = "PVZ_SCREENSHOT";
const CAPTURE_FRAME: u32 = 120;
const EXIT_FRAME: u32 = CAPTURE_FRAME + 30;

pub struct DevShotPlugin;

impl Plugin for DevShotPlugin {
    fn build(&self, app: &mut App) {
        if let Some(path) = std::env::var_os(ENV) {
            app.insert_resource(ShotPath(path.into()))
                .add_systems(Update, capture);
        }
    }
}

#[derive(Resource)]
struct ShotPath(std::path::PathBuf);

fn capture(
    mut commands: Commands,
    mut frame: Local<u32>,
    path: Res<ShotPath>,
    mut exit: MessageWriter<AppExit>,
) {
    *frame += 1;
    if *frame == CAPTURE_FRAME {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path.0.clone()));
    } else if *frame == EXIT_FRAME {
        exit.write(AppExit::Success);
    }
}
