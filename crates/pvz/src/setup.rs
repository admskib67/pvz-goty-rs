//! First-run screen: asks where the player's copy of PvZ is installed.

use std::path::PathBuf;

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::tasks::{futures::check_ready, AsyncComputeTaskPool, Task};
use bevy::ui::Pressed;
use bevy::ui_widgets::{observe, Activate, Button};
use pak::Pak;

use crate::install::{self, Config};
use crate::pak_source::GamePak;
use crate::AppState;

pub struct SetupPlugin;

impl Plugin for SetupPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Setup), spawn_screen).add_systems(
            Update,
            (poll_pick, button_visuals, sync_status_text).run_if(in_state(AppState::Setup)),
        );
    }
}

/// Why the setup screen is showing, e.g. a previously chosen install moved.
#[derive(Resource, Default)]
pub struct SetupStatus {
    pub message: String,
    busy: bool,
}

type PickResult = Option<Result<(PathBuf, PathBuf, Pak), String>>;

/// A pending folder pick + validation, run off the main thread.
#[derive(Component)]
struct PickTask(Task<PickResult>);

#[derive(Component)]
struct StatusText;

// Dark theme palette.
const BG: Color = Color::srgb(0.071, 0.071, 0.078);
const PANEL: Color = Color::srgb(0.118, 0.118, 0.129);
const PANEL_BORDER: Color = Color::srgb(0.204, 0.204, 0.220);
const TEXT: Color = Color::srgb(0.910, 0.910, 0.925);
const TEXT_MUTED: Color = Color::srgb(0.620, 0.620, 0.650);
const TEXT_ERROR: Color = Color::srgb(1.0, 0.541, 0.502);

const PRIMARY: ButtonColors = ButtonColors {
    normal: Color::srgb(0.239, 0.459, 0.271),
    hover: Color::srgb(0.290, 0.541, 0.325),
    pressed: Color::srgb(0.192, 0.380, 0.220),
    border: Color::srgb(0.353, 0.620, 0.392),
};
const SECONDARY: ButtonColors = ButtonColors {
    normal: Color::srgb(0.180, 0.180, 0.196),
    hover: Color::srgb(0.231, 0.231, 0.251),
    pressed: Color::srgb(0.145, 0.145, 0.157),
    border: Color::srgb(0.290, 0.290, 0.310),
};

/// Per-button palette, read by `button_visuals`.
#[derive(Component, Clone, Copy)]
struct ButtonColors {
    normal: Color,
    hover: Color,
    pressed: Color,
    border: Color,
}

fn spawn_screen(mut commands: Commands) {
    let buttons = commands
        .spawn(Node { column_gap: px(12), margin: UiRect::top(px(8)), ..default() })
        .with_child((
            button("Browse...", PRIMARY),
            observe(|_: On<Activate>, mut commands: Commands, mut status: ResMut<SetupStatus>| {
                let dialog = rfd::AsyncFileDialog::new()
                    .set_title("Select your Plants vs Zombies folder");
                start_pick(&mut commands, &mut status, async move {
                    dialog.pick_folder().await.map(|h| h.path().to_path_buf())
                });
            }),
        ))
        .with_child((
            button("Quit", SECONDARY),
            observe(|_: On<Activate>, mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::Success);
            }),
        ))
        .id();

    let panel = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(14),
                padding: UiRect::axes(px(40), px(32)),
                max_width: px(560),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(12)),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(PANEL_BORDER),
        ))
        .with_child(text("Plants vs. Zombies", 30.0, TEXT))
        .with_child(text(
            "This port needs the game data from your own copy of \
             Plants vs Zombies Game of the Year Edition \
             Click Browse and choose the folder it's installed in.",
            15.0,
            TEXT_MUTED,
        ))
        .add_child(buttons)
        .with_child((text("", 14.0, TEXT_ERROR), StatusText))
        .id();

    commands
        .spawn((
            DespawnOnExit(AppState::Setup),
            Node {
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(px(24)),
                ..default()
            },
            BackgroundColor(BG),
        ))
        .add_child(panel);
}

fn start_pick(
    commands: &mut Commands,
    status: &mut SetupStatus,
    choose: impl Future<Output = Option<PathBuf>> + Send + 'static,
) {
    if status.busy {
        return;
    }
    status.busy = true;
    status.message = "Checking...".to_owned();
    let task = AsyncComputeTaskPool::get().spawn(async move {
        let chosen = choose.await?;
        Some(install::open_install(&chosen).map(|(pak_path, pak)| (chosen, pak_path, pak)))
    });
    commands.spawn((PickTask(task), DespawnOnExit(AppState::Setup)));
}

fn poll_pick(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut PickTask)>,
    mut status: ResMut<SetupStatus>,
    game_pak: Res<GamePak>,
    mut next: ResMut<NextState<AppState>>,
) {
    for (entity, mut task) in &mut tasks {
        let Some(result) = check_ready(&mut task.0) else { continue };
        commands.entity(entity).despawn();
        status.busy = false;
        match result {
            None => status.message.clear(),
            Some(Err(e)) => status.message = e,
            Some(Ok((chosen, pak_path, pak))) => {
                info!("using {} ({} files)", pak_path.display(), pak.entries().len());
                if let Err(e) = (Config { game_path: Some(chosen) }).save() {
                    warn!("couldn't save config: {e}");
                }
                game_pak.set(&pak_path, pak);
                next.set(AppState::Title);
            }
        }
    }
}

fn sync_status_text(status: Res<SetupStatus>, mut text: Single<&mut Text, With<StatusText>>) {
    if status.is_changed() {
        text.0.clone_from(&status.message);
    }
}

fn text(s: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont { font_size: FontSize::Px(size), ..default() },
        TextColor(color),
        TextLayout::justify(Justify::Center),
    )
}

fn button(label: &str, colors: ButtonColors) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        Node {
            padding: UiRect::axes(px(20), px(10)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(8)),
            ..default()
        },
        colors,
        BorderColor::all(colors.border),
        BackgroundColor(colors.normal),
        children![text(label, 16.0, TEXT)],
    )
}

fn button_visuals(
    mut buttons: Query<(&Hovered, Has<Pressed>, &ButtonColors, &mut BackgroundColor), With<Button>>,
) {
    for (hovered, pressed, colors, mut bg) in &mut buttons {
        bg.0 = match (hovered.get(), pressed) {
            (_, true) => colors.pressed,
            (true, false) => colors.hover,
            _ => colors.normal,
        };
    }
}
