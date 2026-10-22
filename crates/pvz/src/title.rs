//! Placeholder title screen: proves assets load from the pak.

use bevy::prelude::*;

use crate::pak_source::{GamePak, SOURCE};
use crate::AppState;

pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Title), spawn_title);
    }
}

fn spawn_title(mut commands: Commands, assets: Res<AssetServer>, game_pak: Res<GamePak>) {
    let pak = game_pak.get().expect("pak is loaded before the title screen");
    let Some(entry) = pak.find_with_extensions("images/titlescreen", &["png", "jpg", "gif"]) else {
        error!("images/titlescreen not found in pak");
        return;
    };
    commands.spawn((
        DespawnOnExit(AppState::Title),
        Sprite::from_image(assets.load(format!("{SOURCE}://{}", entry.name))),
    ));
}
