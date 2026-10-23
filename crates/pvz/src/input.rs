//! Mouse position in the game's 800×600 top-left coordinates.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::{GAME_HEIGHT, GAME_WIDTH};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameCursor>().add_systems(PreUpdate, update_cursor);
    }
}

/// Where the mouse is, in game coordinates, or `None` when it's outside the window.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct GameCursor(pub Option<Vec2>);

impl GameCursor {
    /// True if the cursor is inside the rectangle (x, y, w, h).
    pub fn is_in(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.0.is_some_and(|p| p.x >= x && p.x < x + w && p.y >= y && p.y < y + h)
    }
}

fn update_cursor(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    mut cursor: ResMut<GameCursor>,
) {
    let (camera, transform) = *camera;
    cursor.0 = window
        .cursor_position()
        .and_then(|p| camera.viewport_to_world_2d(transform, p).ok())
        .map(|w| Vec2::new(w.x + GAME_WIDTH / 2.0, GAME_HEIGHT / 2.0 - w.y));
}
