//! The title screen: PopCap logo, then the loading bar while resources load.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Widget/TitleScreen.cpp`. Not yet
//! ported: the "Loading..."/"Click to Start" label (needs image fonts), the
//! sprouts popping out of the bar (needs reanimation) and sounds.

use std::f32::consts::PI;

use bevy::prelude::*;
use sexy::tod::{TodCurve, animate_curve, animate_curve_float_time};

use crate::graphics::{Graphics, SexyColor, scale_rotate_transform};
use crate::pak_source::GamePak;
use crate::resources::{GameResources, INIT_GROUP, LOADER_BAR_GROUP, LoadingThread};
use crate::{AppState, GAME_HEIGHT, GAME_WIDTH};

pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Title), start_title)
            .add_systems(FixedUpdate, update_title.run_if(resource_exists::<TitleScreen>))
            .add_systems(Update, (title_input, draw_title).chain().run_if(resource_exists::<TitleScreen>));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TitleState {
    WaitingForFirstDraw,
    PopcapLogo,
    Screen,
}

/// The "Loading..." / "Click to Start" link under the bar. Only its
/// position matters until fonts are ported.
#[derive(Debug, Clone, Copy)]
struct StartButton {
    x: f32,
    y: f32,
}

#[derive(Resource)]
pub struct TitleScreen {
    cur_bar_width: f32,
    total_bar_width: f32,
    bar_vel: f32,
    bar_start_progress: f32,
    prev_loading_percent: f32,
    title_age: i32,
    loading_thread_complete: bool,
    drawn_yet: bool,
    need_to_init: bool,
    state: TitleState,
    state_duration: i32,
    state_counter: i32,
    start_button: StartButton,
}

impl Default for TitleScreen {
    fn default() -> Self {
        Self {
            cur_bar_width: 0.0,
            total_bar_width: 314.0,
            bar_vel: 0.2,
            bar_start_progress: 0.0,
            prev_loading_percent: 0.0,
            title_age: 0,
            loading_thread_complete: false,
            drawn_yet: false,
            need_to_init: true,
            state: TitleState::WaitingForFirstDraw,
            state_duration: 0,
            state_counter: 0,
            start_button: StartButton { x: 0.0, y: 0.0 },
        }
    }
}

const WIDTH: i32 = GAME_WIDTH as i32;
const HEIGHT: i32 = GAME_HEIGHT as i32;

fn start_title(
    mut commands: Commands,
    game_pak: Res<GamePak>,
    mut images: ResMut<Assets<Image>>,
    mut exit: MessageWriter<AppExit>,
) {
    let files = game_pak.get().expect("an install is chosen before the title screen");
    let resources = GameResources::new(files).and_then(|mut res| {
        res.load_group_now(INIT_GROUP, &mut images)?;
        Ok(res)
    });
    match resources {
        Ok(resources) => {
            commands.insert_resource(resources);
            commands.insert_resource(TitleScreen::default());
        }
        Err(e) => {
            // TODO: show this on screen like the original's resource error dialog.
            error!("couldn't load game resources: {e}");
            exit.write(AppExit::error());
        }
    }
}

/// `TitleScreen::Update`, run at the original's 100 ticks per second.
fn update_title(
    mut commands: Commands,
    mut title: ResMut<TitleScreen>,
    resources: Res<GameResources>,
    thread: Option<Res<LoadingThread>>,
) {
    let t = &mut *title;
    if !t.drawn_yet {
        return;
    }

    if t.state == TitleState::WaitingForFirstDraw {
        // TODO: MusicTitleScreenInit once music is ported.
        commands.insert_resource(resources.start_loading_thread());
        t.state = TitleState::PopcapLogo;
        t.state_duration = 200;
        t.state_counter = t.state_duration;
    }

    t.title_age += 1;
    if t.state_counter > 0 {
        t.state_counter -= 1;
    }

    if t.state == TitleState::PopcapLogo {
        if t.state_counter == 0 {
            t.state = TitleState::Screen;
            t.state_duration = 100;
            t.state_counter = 100;
        }
        return;
    }

    let (Some(thread), Some(dirt)) = (thread, resources.image("IMAGE_LOADBAR_DIRT")) else { return };
    if !resources.is_group_loaded(LOADER_BAR_GROUP) {
        return;
    }

    let current_progress = thread.progress();
    if t.need_to_init {
        t.need_to_init = false;
        t.start_button = StartButton { x: (WIDTH / 2 - dirt.width as i32 / 2) as f32, y: 650.0 };

        let estimated_total_load_time =
            if current_progress > 0.000001 { t.title_age as f32 / current_progress } else { 3000.0 };
        let load_time = (estimated_total_load_time * (1.0 - current_progress)).clamp(100.0, 3000.0);
        t.bar_vel = t.total_bar_width / load_time;
        t.bar_start_progress = current_progress.min(0.9);
    }

    let loading_percent = (current_progress - t.bar_start_progress) / (1.0 - t.bar_start_progress);

    t.start_button.y = if t.state_counter > 10 {
        animate_curve(60, 10, t.state_counter, 650, 534, TodCurve::EaseIn)
    } else {
        animate_curve(10, 0, t.state_counter, 534, 529, TodCurve::Bounce)
    } as f32;

    if t.state_counter > 0 {
        return;
    }

    t.cur_bar_width += t.bar_vel;
    if !t.loading_thread_complete {
        t.cur_bar_width = t.cur_bar_width.min(t.total_bar_width * 0.99);
    } else if t.cur_bar_width > t.total_bar_width {
        t.cur_bar_width = t.total_bar_width;
    }

    if loading_percent > t.prev_loading_percent + 0.01 || t.loading_thread_complete {
        let bar_width =
            animate_curve_float_time(0.0, 1.0, loading_percent, 0.0, t.total_bar_width, TodCurve::EaseIn);
        let diff = bar_width - t.cur_bar_width;
        let acceleration = if t.loading_thread_complete {
            0.0001
        } else {
            animate_curve_float_time(0.0, 1.0, loading_percent, 0.0001, 0.00001, TodCurve::Linear)
        };
        t.bar_vel += diff * diff.abs() * acceleration;

        let min_velocity = animate_curve_float_time(0.0, 1.0, loading_percent, 0.2, 0.01, TodCurve::Linear);
        t.bar_vel = t.bar_vel.clamp(min_velocity, 2.0);
        t.prev_loading_percent = loading_percent;
    }

    if !t.loading_thread_complete && thread.is_complete() {
        t.loading_thread_complete = true;
        if let Some(e) = thread.error() {
            error!("loading thread failed: {e}");
        }
    }
}

/// `TitleScreen::MouseDown`/`KeyDown`: any click or key once loading is done.
fn title_input(
    title: Res<TitleScreen>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let pressed = mouse.get_just_pressed().next().is_some() || keys.get_just_pressed().next().is_some();
    if pressed && title.loading_thread_complete {
        // TODO: LawnApp::LoadingCompleted → game selector, once it exists.
        info!("loading completed; the main menu isn't ported yet");
    }
}

/// `TitleScreen::Draw`.
fn draw_title(mut title: ResMut<TitleScreen>, resources: Res<GameResources>, mut g: ResMut<Graphics>) {
    let (w, h) = (GAME_WIDTH, GAME_HEIGHT);
    let t = &mut *title;

    if t.state == TitleState::WaitingForFirstDraw {
        g.set_color(SexyColor::BLACK);
        g.fill_rect(0.0, 0.0, w, h);
        t.drawn_yet = true;
        return;
    }

    if t.state == TitleState::PopcapLogo {
        g.set_color(SexyColor::BLACK);
        g.fill_rect(0.0, 0.0, w, h);

        let alpha = if t.state_counter < t.state_duration - 50 {
            animate_curve(50, 0, t.state_counter, 255, 0, TodCurve::Linear)
        } else {
            animate_curve(t.state_duration, t.state_duration - 50, t.state_counter, 0, 255, TodCurve::Linear)
        };
        if let Some(logo) = resources.image("IMAGE_POPCAP_LOGO") {
            g.set_colorize_images(true);
            g.set_color(SexyColor::WHITE.with_alpha(alpha));
            let x = (WIDTH - logo.width as i32) / 2;
            let y = (HEIGHT - logo.height as i32) / 2;
            g.draw_image(logo, x as f32, y as f32);
            g.set_colorize_images(false);
        }
        return;
    }

    let images = ["IMAGE_TITLESCREEN", "IMAGE_PVZ_LOGO", "IMAGE_LOADBAR_DIRT", "IMAGE_LOADBAR_GRASS"]
        .map(|id| resources.image(id));
    let [Some(screen), Some(logo), Some(dirt), Some(grass)] = images else {
        g.set_color(SexyColor::BLACK);
        g.fill_rect(0.0, 0.0, w, h);
        return;
    };

    g.draw_image(screen, 0.0, 0.0);
    if t.need_to_init {
        return;
    }

    let logo_y = if t.state_counter > 60 {
        animate_curve(100, 60, t.state_counter, -150, 10, TodCurve::EaseIn)
    } else {
        animate_curve(60, 50, t.state_counter, 10, 15, TodCurve::Bounce)
    };
    g.draw_image(logo, (WIDTH / 2 - logo.width as i32 / 2) as f32, logo_y as f32);

    let grass_x = t.start_button.x;
    let grass_y = t.start_button.y - 17.0;
    g.draw_image(dirt, grass_x, grass_y + 18.0);

    if t.cur_bar_width >= t.total_bar_width {
        g.draw_image(grass, grass_x, grass_y);
    } else {
        g.push_state();
        g.clip_rect(240.0, grass_y, t.cur_bar_width, grass.height as f32);
        g.draw_image(grass, grass_x, grass_y);
        g.pop_state();

        if let Some(cap) = resources.image("IMAGE_REANIM_SODROLLCAP") {
            let roll_len = t.cur_bar_width * 0.94;
            let rotation = -roll_len / 180.0 * PI * 2.0;
            let scale =
                animate_curve_float_time(0.0, t.total_bar_width, t.cur_bar_width, 1.0, 0.5, TodCurve::Linear);
            let transform = scale_rotate_transform(
                grass_x + 11.0 + roll_len,
                grass_y - 3.0 - 35.0 * scale + 35.0,
                rotation,
                scale,
                scale,
            );
            g.draw_image_matrix(cap, transform);
        }
    }
}
