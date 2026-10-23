//! The title screen: PopCap logo, then the loading bar while resources load.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Widget/TitleScreen.cpp` and
//! `HyperlinkWidget::Draw`. Not yet ported: sounds.

use std::f32::consts::PI;

use bevy::prelude::*;
use sexy::reanim::LoopType;
use sexy::tod::{TodCurve, animate_curve, animate_curve_float_time};

use crate::graphics::{Graphics, SexyColor, scale_rotate_transform};
use crate::input::GameCursor;
use crate::pak_source::GamePak;
use crate::reanim::{EffectSystem, ReanimationType, draw_reanimation};
use crate::resources::{GameResources, INIT_GROUP, LOADER_BAR_GROUP, LoadingThread};
use crate::{AppState, GAME_HEIGHT, GAME_WIDTH};

pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Title), start_title)
            .add_systems(OnExit(AppState::Title), end_title)
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

/// The "LOADING..." / "CLICK TO START!" link under the bar (a `HyperlinkWidget`).
#[derive(Debug, Clone, Copy)]
struct StartButton {
    x: f32,
    y: f32,
    label: &'static str,
    disabled: bool,
    visible: bool,
}

impl StartButton {
    const HEIGHT: f32 = 50.0;
    const COLOR: SexyColor = SexyColor::rgba(218, 184, 33, 255);
    const OVER_COLOR: SexyColor = SexyColor::rgba(250, 90, 15, 255);
    const FONT: &str = "FONT_BRIANNETOD16";
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
            start_button: StartButton { x: 0.0, y: 0.0, label: "", disabled: true, visible: false },
        }
    }
}

/// Development aid: continue past the title screen as soon as loading is done.
const AUTO_START_ENV: &str = "PVZ_AUTO_START";

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
            commands.insert_resource(EffectSystem::default());
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
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut effects: ResMut<EffectSystem>,
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
        t.start_button.label = "[LOADING]";
        t.start_button.x = (WIDTH / 2 - dirt.width as i32 / 2) as f32;
        t.start_button.y = 650.0;
        t.start_button.visible = true;

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

    effects.update();

    let prev_width = t.cur_bar_width;
    t.cur_bar_width += t.bar_vel;
    if !t.loading_thread_complete {
        t.cur_bar_width = t.cur_bar_width.min(t.total_bar_width * 0.99);
    } else if t.cur_bar_width > t.total_bar_width {
        t.start_button.label = "[CLICK_TO_START]";
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
        t.start_button.disabled = false;
        t.start_button.visible = true;
        if let Some(e) = thread.error() {
            error!("loading thread failed: {e}");
        }
    }

    // Sprouts (and finally a zombie head) pop out as the bar passes these points.
    let trigger_points = [0.11, 0.32, 0.54, 0.72, 0.91].map(|f| t.total_bar_width * f);
    for (i, trigger) in trigger_points.into_iter().enumerate() {
        if !(prev_width < trigger && t.cur_bar_width >= trigger) {
            continue;
        }
        let ty = if i == 4 { ReanimationType::LoadbarZombieHead } else { ReanimationType::LoadbarSprout };
        let (x, y) = (trigger + 225.0, 511.0);
        let sprout = match effects.add_reanimation(x, y, ty, &mut resources, &mut images) {
            Ok((_, sprout)) => sprout,
            Err(e) => {
                error!("{e}");
                continue;
            }
        };
        sprout.anim_rate = 18.0;
        sprout.loop_type = LoopType::PlayOnceAndHold;
        match i {
            1 | 3 => sprout.override_scale(-1.0, 1.0),
            2 => {
                sprout.set_position(x, y - 5.0);
                sprout.override_scale(1.1, 1.3);
            }
            4 => sprout.set_position(x - 20.0, y),
            _ => {}
        }
        // TODO: SOUND_LOADINGBAR_FLOWER (and SOUND_LOADINGBAR_ZOMBIE for the head).
    }
}

/// `TitleScreen::MouseDown`/`KeyDown`: any click or key once loading is done
/// calls `LawnApp::LoadingCompleted`, which shows the main menu.
fn title_input(
    title: Res<TitleScreen>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<AppState>>,
) {
    let pressed = mouse.get_just_pressed().next().is_some()
        || keys.get_just_pressed().next().is_some()
        || std::env::var_os(AUTO_START_ENV).is_some();
    if pressed && title.loading_thread_complete {
        // TODO: SOUND_BUTTONCLICK.
        next.set(AppState::GameSelector);
    }
}

/// Removes the title screen. Its sprouts go too; the original leaves them in
/// the effect system, where nothing draws them again.
fn end_title(mut commands: Commands, mut effects: ResMut<EffectSystem>) {
    commands.remove_resource::<TitleScreen>();
    effects.clear();
}

/// Draws the title screen, then its start link on top (it's a child widget).
fn draw_title(
    mut title: ResMut<TitleScreen>,
    resources: Res<GameResources>,
    effects: Res<EffectSystem>,
    cursor: Res<GameCursor>,
    mut g: ResMut<Graphics>,
) {
    if draw_screen(&mut title, &resources, &mut g) {
        for reanim in effects.reanimations() {
            draw_reanimation(&mut g, reanim, &resources);
        }
    }
    draw_start_button(&title, &resources, &cursor, &mut g);
}

/// `HyperlinkWidget::Draw` for the start link.
fn draw_start_button(t: &TitleScreen, resources: &GameResources, cursor: &GameCursor, g: &mut Graphics) {
    let b = &t.start_button;
    let Some(font) = resources.font(StartButton::FONT).filter(|_| b.visible) else { return };
    let label = resources.translate(b.label);
    let over = !b.disabled && cursor.is_in(b.x, b.y, t.total_bar_width, StartButton::HEIGHT);

    let font_x = (t.total_bar_width as i32 - font.font.string_width(&label)) / 2;
    let font_y = (StartButton::HEIGHT as i32 + font.font.ascent) / 2 - 1;
    g.set_color(if over { StartButton::OVER_COLOR } else { StartButton::COLOR });
    g.draw_string(font, b.x as i32 + font_x, b.y as i32 + font_y, &label);
}

/// `TitleScreen::Draw` up to its effects. Returns false where the original
/// returns early (before effects are drawn).
fn draw_screen(t: &mut TitleScreen, resources: &GameResources, g: &mut Graphics) -> bool {
    let (w, h) = (GAME_WIDTH, GAME_HEIGHT);

    if t.state == TitleState::WaitingForFirstDraw {
        g.set_color(SexyColor::BLACK);
        g.fill_rect(0.0, 0.0, w, h);
        t.drawn_yet = true;
        return false;
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
        return false;
    }

    let images = ["IMAGE_TITLESCREEN", "IMAGE_PVZ_LOGO", "IMAGE_LOADBAR_DIRT", "IMAGE_LOADBAR_GRASS"]
        .map(|id| resources.image(id));
    let [Some(screen), Some(logo), Some(dirt), Some(grass)] = images else {
        g.set_color(SexyColor::BLACK);
        g.fill_rect(0.0, 0.0, w, h);
        return false;
    };

    g.draw_image(screen, 0.0, 0.0);
    if t.need_to_init {
        return false;
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
    true
}
