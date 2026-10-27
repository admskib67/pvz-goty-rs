//! The credits: a movie of three reanimations (`Credits_Main`, `Main2`,
//! `Main3`) sung along to "Zombies on Your Lawn", ending on the names of
//! everyone who made the game.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Widget/CreditScreen.cpp`:
//! `CreditScreen::PlayReanim`, `GetTiming`, `UpdateMovie`, `UpdateBlink`,
//! `TurnOffTongues`, `Draw`, `DrawDisco`, `DrawFogEffect`, `DrawOverlay`,
//! `DrawFinalCredits`, `PauseCredits` and the buttons. The movie runs at
//! the game's 100 Hz, so it stays in step with the song without the
//! original's catch-up logic. Not yet: the pool's water behind the
//! swimming zombies and the bottom of the house's door (their drawing needs
//! the game-over background pieces) and the debug keys that jump about.

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::math::{Affine2, Vec2};
use bevy::prelude::*;
use sexy::reanim::{LoopType, RENDER_GROUP_HIDDEN};
use sexy::tod::{TodCurve, animate_curve, animate_curve_float_time};

use crate::audio::{FoleyType, Music, Sounds};
use crate::credits_timing::{Brain, TIMING, Timing, Word};
use crate::dialog::{ButtonMode, DialogId, Dialogs};
use crate::graphics::{Graphics, Justify, SexyColor, SexyRect};
use crate::input::GameCursor;
use crate::lawn_app::lawn_message_box;
use crate::options::Settings;
use crate::particle::{ParticleEffect, draw_particle_system};
use crate::reanim::{EffectSystem, ParticleSystemId, ReanimationId, ReanimationType};
use crate::resources::GameResources;
use crate::widgets::{ImageLook, StoneButton};
use crate::{AppState, GAME_HEIGHT, GAME_WIDTH, UiSet};

/// Where the credits song lives in the pak.
const SONG: &str = "sounds/ZombiesOnYourLawn.ogg";
/// `SECONDS_PER_UPDATE`.
const SECONDS_PER_UPDATE: f32 = 0.01;
/// The render order everything on top is drawn at (`RENDER_LAYER_TOP`, whatever its exact value).
const TOP_ORDER: i32 = crate::board::render_layer::TOP;

pub struct CreditsPlugin;

impl Plugin for CreditsPlugin {
    fn build(&self, app: &mut App) {
        let open = resource_exists::<CreditScreen>;
        app.add_systems(OnEnter(AppState::Credits), start_credits)
            .add_systems(OnExit(AppState::Credits), end_credits)
            .add_systems(FixedUpdate, update_credits.run_if(open))
            .add_systems(Update, credits_input.in_set(UiSet::ScreenInput).run_if(open))
            .add_systems(Update, draw_credits.in_set(UiSet::ScreenDraw).run_if(open));
    }
}

/// `CreditsPhase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditsPhase {
    Main1,
    Main2,
    Main3,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreditsButton {
    MainMenu,
    Replay,
}

/// What's playing and where it's up to (`CreditScreen`).
#[derive(Resource)]
pub struct CreditScreen {
    pub phase: CreditsPhase,
    phase_counter: i32,
    reanim: Option<ReanimationId>,
    blink_countdown: i32,
    /// The brain that bounces along the words, if it's on (`mDrawBrain`).
    brain: Option<(f32, f32)>,
    fog_particle: Option<ParticleSystemId>,
    /// The melon's splash while it lasts.
    melon_particle: Option<ParticleSystemId>,
    pub paused: bool,
    main_menu: StoneButton<CreditsButton>,
    replay: StoneButton<CreditsButton>,
    song: Option<Entity>,
    ticks: i32,
    /// The pool's animated water (`mPoolEffect`).
    water: Option<crate::board::pool::PoolWater>,
}

impl CreditScreen {
    pub(crate) fn new(res: &GameResources) -> Self {
        let mut main_menu = StoneButton::new(CreditsButton::MainMenu, res.translate("[CREDITS_MAIN_MENU_BUTTON]"));
        main_menu.resize(298, 554, 209, 46);
        main_menu.visible = false;
        let look = ImageLook {
            normal: "IMAGE_CREDITS_PLAYBUTTON",
            over: "IMAGE_CREDITS_PLAYBUTTON",
            down: "IMAGE_CREDITS_PLAYBUTTON",
            font: "FONT_HOUSEOFTERROR16",
            hilite_font: "FONT_HOUSEOFTERROR16",
            translate: (1, 1),
            text_offset: (33, -5),
            text_down_offset: (1, 1),
        };
        let mut replay = StoneButton::new_image(CreditsButton::Replay, res.translate("[CREDITS_REPLAY_BUTTON]"), look, res);
        replay.resize(10, 530, 125, 65);
        replay.visible = false;
        Self {
            phase: CreditsPhase::Main1,
            phase_counter: 0,
            reanim: None,
            blink_countdown: 700,
            brain: None,
            fog_particle: None,
            melon_particle: None,
            paused: false,
            main_menu,
            replay,
            song: None,
            ticks: 0,
            water: None,
        }
    }

    /// `CreditScreen::PlayReanim`: starts one of the three parts of the movie.
    pub(crate) fn play_reanim(&mut self, index: i32, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>) {
        if let Some(old) = self.reanim.take() {
            effects.kill(old);
        }
        let (ty, groups): (ReanimationType, &[(&str, i32)]) = match index {
            1 => (
                ReanimationType::CreditsMain,
                &[("Background", 1), ("attacher__Zombie", 2), ("Words", 3), ("SpotFront", 3)],
            ),
            2 => (
                ReanimationType::CreditsMain2,
                &[("Background", 1), ("attacher__Zombie", 2), ("Words", 3), ("SpotFront", 3), ("attacher__undead", 2)],
            ),
            _ => (
                ReanimationType::CreditsMain3,
                &[
                    ("Background", 1),
                    ("attacher__Zombie", 2),
                    ("attacher__DiscoLights", 2),
                    ("Words", 3),
                    ("attacher__cattail", 3),
                    ("SpotFront", 3),
                    ("attacher__undead", 2),
                ],
            ),
        };
        match effects.add_reanimation(0.0, 0.0, ty, res, images) {
            Ok((id, reanim)) => {
                for (prefix, group) in groups {
                    reanim.assign_render_group_to_prefix(prefix, *group);
                }
                reanim.is_attachment = true;
                reanim.loop_type = LoopType::PlayOnceAndHold;
                self.reanim = Some(id);
            }
            Err(e) => error!("{e}"),
        }
    }

    /// The movie's reanimation.
    #[cfg(test)]
    pub fn reanim_id(&self) -> Option<ReanimationId> {
        self.reanim
    }

    /// Whether the Main Menu and Replay buttons are up.
    #[cfg(test)]
    pub fn buttons_shown(&self) -> bool {
        self.main_menu.visible && self.replay.visible
    }

    /// The singing brain's place, while it's out.
    #[cfg(test)]
    pub fn brain(&self) -> Option<(f32, f32)> {
        self.brain
    }

    /// `CreditScreen::GetTiming`: the two entries the movie is between, and how far.
    fn timing(&self, effects: &EffectSystem) -> (Option<&'static Timing>, Option<&'static Timing>, f32) {
        let Some(reanim) = self.reanim.and_then(|r| effects.get(r)) else { return (None, None, 0.0) };
        let mut frames = reanim.definition.frame_count() as f32 * reanim.anim_time - 1.0;
        match self.phase {
            CreditsPhase::Main1 => frames += 2.0,
            CreditsPhase::Main2 => frames += 400.0,
            CreditsPhase::Main3 => frames += 785.0,
            CreditsPhase::End => return (None, None, 0.0),
        }
        if frames < TIMING[0].frame {
            return (None, Some(&TIMING[0]), frames / TIMING[0].frame);
        }
        for pair in TIMING.windows(2) {
            if pair[1].frame > frames {
                return (Some(&pair[0]), Some(&pair[1]), (frames - pair[0].frame) / (pair[1].frame - pair[0].frame));
            }
        }
        (TIMING.last(), None, 0.0)
    }
}

/// `PreLoadCredits`: the credits' pictures, and the words sung in the dark
/// drawn in black.
pub(crate) fn prepare_credits(resources: &mut GameResources, images: &mut Assets<Image>) {
    for group in ["DelayLoad_Credits", "DelayLoad_Background1", "DelayLoad_Background2", "DelayLoad_Background3", "DelayLoad_Background4", "DelayLoad_Background5", "DelayLoad_Background6"] {
        if let Err(e) = resources.load_group_now(group, images) {
            error!("{e}");
        }
    }
    for ty in [ReanimationType::CreditsMain2, ReanimationType::CreditsMain3] {
        let edit = |def: &mut sexy::reanim::ReanimDefinition| {
            for track in def.tracks.iter_mut().filter(|t| t.name == "Words" || t.name == "Words2") {
                for transform in track.transforms.iter_mut().take(124) {
                    transform.font = Some("FONT_BRIANNETOD32BLACK".to_owned());
                }
            }
        };
        if let Err(e) = resources.modify_reanim_definition(ty.file(), images, edit) {
            error!("{e}");
        }
    }
    let edit = |def: &mut sexy::reanim::ReanimDefinition| {
        for transform in def.tracks.iter_mut().flat_map(|t| t.transforms.iter_mut()).filter(|t| t.font.is_some()) {
            transform.font = Some("FONT_BRIANNETOD32BLACK".to_owned());
        }
    };
    if let Err(e) = resources.modify_reanim_definition("reanim/Credits_WeAreTheUndead.reanim", images, edit) {
        error!("{e}");
    }
}

fn start_credits(
    mut commands: Commands,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut audio_assets: ResMut<Assets<AudioSource>>,
    mut effects: ResMut<EffectSystem>,
    mut music: ResMut<Music>,
    settings: Res<Settings>,
) {
    effects.clear();
    music.stop_all_music();
    prepare_credits(&mut resources, &mut images);

    let mut screen = CreditScreen::new(&resources);
    screen.water = crate::board::pool::PoolWater::new(&resources, &mut images);
    screen.play_reanim(1, &mut effects, &mut resources, &mut images);
    // Dev aid: `PVZ_CREDITS_JUMP=2:110` starts part 2 at its frame 110 (the song isn't moved).
    if let Some((part, frame)) = std::env::var("PVZ_CREDITS_JUMP").ok().as_deref().and_then(|v| v.split_once(':'))
        && let (Ok(part), Ok(frame)) = (part.parse::<i32>(), frame.parse::<f32>())
    {
        screen.phase = match part {
            2 => CreditsPhase::Main2,
            3 => CreditsPhase::Main3,
            _ => CreditsPhase::Main1,
        };
        screen.play_reanim(part, &mut effects, &mut resources, &mut images);
        if let Some(reanim) = screen.reanim.and_then(|id| effects.get_mut(id)) {
            reanim.anim_time = frame / (reanim.definition.frame_count() as f32 - 1.0);
        }
    }
    // The song, from the start; the music volume is at least 0.1 here.
    if let Some(bytes) = resources.read_file(SONG) {
        let volume = settings.music_volume.max(0.1);
        let handle = audio_assets.add(AudioSource { bytes: bytes.into() });
        screen.song = Some(commands.spawn((AudioPlayer(handle), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)))).id());
    } else {
        warn!("{SONG} isn't in the pak: the credits will be silent");
    }
    commands.insert_resource(screen);
}

fn end_credits(mut commands: Commands, screen: Option<Res<CreditScreen>>, mut effects: ResMut<EffectSystem>, mut sounds: Sounds) {
    if let Some(song) = screen.as_ref().and_then(|s| s.song) {
        commands.entity(song).try_despawn();
    }
    sounds.stop_foley(FoleyType::Scream);
    effects.clear();
    commands.remove_resource::<CreditScreen>();
}

/// `CreditScreen::Update` and `UpdateMovie`: one tick of the movie.
#[allow(clippy::too_many_arguments)]
fn update_credits(
    mut screen: ResMut<CreditScreen>,
    mut effects: ResMut<EffectSystem>,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut sounds: Sounds,
) {
    if screen.paused {
        return;
    }
    tick_movie(&mut screen, &mut effects, &mut resources, &mut images, &mut sounds);
}

/// One tick of the movie (`UpdateMovie`).
pub(crate) fn tick_movie(
    screen: &mut CreditScreen,
    effects: &mut EffectSystem,
    resources: &mut GameResources,
    images: &mut Assets<Image>,
    sounds: &mut Sounds,
) {
    screen.ticks += 1;
    if let Some(water) = screen.water.as_mut() {
        water.tick(images);
    }
    let Some(id) = screen.reanim else { return };
    update_blink(screen, effects, resources, images);

    effects.update_reanimation(id);
    effects.update_attachers(id, resources, images);
    effects.place_attachments(id);
    effects.update(sounds.rng());
    turn_off_tongues(effects, id, 0);

    let Some(reanim) = effects.get(id) else { return };
    let frame_factor = 1.0 / (reanim.definition.frame_count() as f32 - 1.0);
    let looped = reanim.loop_count > 0;
    let mut id = id;
    match screen.phase {
        CreditsPhase::Main1 if looped => {
            screen.play_reanim(2, effects, resources, images);
            screen.phase = CreditsPhase::Main2;
        }
        CreditsPhase::Main2 if looped => {
            screen.play_reanim(3, effects, resources, images);
            screen.phase = CreditsPhase::Main3;
        }
        CreditsPhase::Main3 if looped => screen.phase = CreditsPhase::End,
        CreditsPhase::End => {
            screen.phase_counter += 1;
            if screen.phase_counter == 50 {
                screen.main_menu.visible = true;
                screen.replay.visible = true;
            }
        }
        _ => {}
    }
    if let Some(new) = screen.reanim {
        id = new;
    }
    let at = |effects: &EffectSystem, frame: f32| effects.get(id).is_some_and(|r| r.should_trigger_timed_event(frame_factor * frame));
    let middle = (GAME_WIDTH / 2.0, GAME_HEIGHT / 2.0);
    let particle = |effects: &mut EffectSystem, resources: &mut GameResources, images: &mut Assets<Image>, sounds: &mut Sounds, x: f32, y: f32, effect: ParticleEffect| {
        effects.add_particle(x, y, TOP_ORDER, effect, resources, images, sounds.rng())
    };

    match screen.phase {
        CreditsPhase::Main1 => {
            if [128.0, 130.0, 132.0, 134.0, 136.0, 138.0, 140.0, 142.0].iter().any(|&f| at(effects, f)) {
                particle(effects, resources, images, sounds, middle.0, middle.1, ParticleEffect::CreditStrobe);
            }
            if at(effects, 136.5) {
                particle(effects, resources, images, sounds, middle.0, middle.1, ParticleEffect::CreditsRayswipe);
            }
            if at(effects, 330.0) {
                sounds.play_foley(FoleyType::Scream);
            }
            if at(effects, 336.0) {
                sounds.stop_foley(FoleyType::Scream);
            }
        }
        CreditsPhase::Main2 | CreditsPhase::Main3 => {
            // The words are white once the lights come up.
            let early = effects.get(id).is_some_and(|r| r.anim_time < frame_factor * 124.0);
            let color = if early { [0, 0, 0, 255] } else { [255, 255, 255, 255] };
            if let Some(reanim) = effects.get_mut(id) {
                for name in ["Words", "Words2"] {
                    if let Some(track) = reanim.definition.find_track(name) {
                        reanim.tracks[track].track_color = color;
                    }
                }
            }
            if let Some(undead) = effects.find_attached(id, "Credits_WeAreTheUndead") {
                let late = effects.get(id).is_some_and(|r| r.anim_time > frame_factor * 112.0);
                if let Some(reanim) = effects.get_mut(undead) {
                    for name in ["bubbletext1", "bubbletext2", "bubbletext3"] {
                        if let Some(track) = reanim.definition.find_track(name) {
                            reanim.tracks[track].track_color = [0, 0, 0, 255];
                        }
                    }
                    // Its shaky text jitters once he's loud (SetShakeOverride).
                    reanim.set_shake_override("ShakyText", if late { 2.0 } else { 0.0 });
                }
            }
            if at(effects, 120.0) {
                particle(effects, resources, images, sounds, middle.0, middle.1, ParticleEffect::CreditsZombieheadwipe);
            }
        }
        CreditsPhase::End => {}
    }
    if screen.phase == CreditsPhase::Main2 {
        let list = [111.5f32, 115.5, 119.5, 121.5, 123.5, 125.5, 127.5]
            .into_iter()
            .chain((131..=243).step_by(4).map(|f| f as f32 + 0.5));
        if list.into_iter().any(|f| at(effects, f)) || at(effects, 342.0) {
            particle(effects, resources, images, sounds, middle.0, middle.1, ParticleEffect::CreditStrobe);
        }
        if at(effects, 332.75) {
            screen.melon_particle = particle(effects, resources, images, sounds, 678.0, 352.0, ParticleEffect::Melonsplash);
        }
        if at(effects, 336.0)
            && let Some(melon) = screen.melon_particle.take()
        {
            effects.remove_particle(melon);
        }
        // The fog machine's cloud follows the stage.
        let background2 = effects.get(id).map(|r| r.current_transform(r.find_track("Background2")));
        let fog_x = background2.map_or(0.0, |t| t.x) + 856.0;
        if at(effects, 188.0) {
            screen.fog_particle = particle(effects, resources, images, sounds, fog_x, 230.0, ParticleEffect::CreditsFog);
        }
        if let Some(fog) = screen.fog_particle {
            if at(effects, 248.0) {
                effects.remove_particle(fog);
                screen.fog_particle = None;
            } else if let Some(system) = effects.particle_mut(fog) {
                system.system_move(fog_x, 230.0);
            }
        }
    }
    if screen.phase == CreditsPhase::Main3 {
        if at(effects, 65.0) {
            sounds.play_foley(FoleyType::DolphinAppears);
        }
        let strobes = [111.0, 115.0, 119.0, 121.0, 123.0, 219.0, 223.0, 227.0, 231.0, 235.0, 239.0, 243.0, 247.0];
        if strobes.into_iter().any(|f| at(effects, f)) {
            particle(effects, resources, images, sounds, middle.0, middle.1, ParticleEffect::CreditStrobe);
        }
    }

    // The singing brain and the sunflower's mouth.
    let (before, after, fraction) = screen.timing(effects);
    let mut sunflower_frame = 0;
    screen.brain = None;
    if let (Some(before), Some(after)) = (before, after) {
        let word_x = |t: &Timing| t.x as f32;
        let curve = |from: f32, to: f32, curve: TodCurve| animate_curve_float_time(0.0, 1.0, fraction, from, to, curve);
        screen.brain = match (after.brain, before.brain) {
            (Brain::FlyOn, _) => Some((curve(-50.0, word_x(after) - 15.0, TodCurve::EaseInOut), curve(505.0, 485.0, TodCurve::BounceFastMiddle))),
            (Brain::FastOn, _) => Some((curve(word_x(after) - 50.0, word_x(after) - 15.0, TodCurve::EaseInOut), curve(485.0, 505.0, TodCurve::EaseInOut))),
            (Brain::FlyOff, _) => Some((curve(word_x(before) - 15.0, 850.0, TodCurve::EaseInOut), curve(505.0, 485.0, TodCurve::BounceFastMiddle))),
            (Brain::FastOff, _) => Some((curve(word_x(before) - 15.0, word_x(before) + 25.0, TodCurve::EaseInOut), curve(505.0, 485.0, TodCurve::EaseInOut))),
            (_, Brain::FlyOn | Brain::FastOn | Brain::NextWord) => {
                Some((curve(word_x(before) - 15.0, word_x(after) - 15.0, TodCurve::EaseInOut), curve(505.0, 485.0, TodCurve::BounceFastMiddle)))
            }
            _ => None,
        };
        let frames_for_word = after.frame - before.frame;
        let frames_till_end = (1.0 - fraction) * frames_for_word;
        sunflower_frame = if before.word != Word::Off && (frames_for_word * fraction < 0.2 || frames_till_end < 0.4) {
            1
        } else {
            match before.word {
                Word::Aa => 2,
                Word::Ee => 3,
                Word::Aw => 4,
                Word::Oh => 5,
                Word::Off => 0,
            }
        };
    }
    if let Some(sunflower) = effects.find_attached(id, "Sunflower") {
        let late_in_main3 = screen.phase == CreditsPhase::Main3 && effects.get(id).is_some_and(|r| r.anim_time > frame_factor * 255.0);
        if late_in_main3 {
            // She winks and holds the pose: neither she nor the stage moves on.
            if let Some(reanim) = effects.get_mut(sunflower) {
                reanim.set_image_override("anim_idle", Some("IMAGE_REANIM_SUNFLOWER_HEAD_WINK"));
                reanim.anim_time -= reanim.anim_rate * SECONDS_PER_UPDATE / reanim.frame_count.max(1) as f32;
            }
            if let Some(stage) = effects.find_attached(id, "Credits_stage")
                && let Some(reanim) = effects.get_mut(stage)
            {
                reanim.anim_time -= reanim.anim_rate * SECONDS_PER_UPDATE / reanim.frame_count.max(1) as f32;
            }
        } else if let Some(reanim) = effects.get_mut(sunflower) {
            let image = match sunflower_frame {
                1..=5 => Some(format!("IMAGE_REANIM_SUNFLOWER_HEAD_SING{sunflower_frame}")),
                _ => None,
            };
            reanim.set_image_override("anim_idle", image.as_deref());
        }
    }
}

/// `CreditScreen::UpdateBlink`: the singing sunflower blinks now and then.
fn update_blink(screen: &mut CreditScreen, effects: &mut EffectSystem, resources: &mut GameResources, images: &mut Assets<Image>) {
    screen.blink_countdown -= 1;
    if screen.blink_countdown > 0 {
        return;
    }
    screen.blink_countdown = 700;
    let Some(root) = screen.reanim else { return };
    let Some(sunflower) = effects.find_attached(root, "Sunflower") else { return };
    if screen.phase == CreditsPhase::Main3 {
        let late = effects.get(root).is_some_and(|r| r.anim_time > (1.0 / (r.definition.frame_count() as f32 - 1.0)) * 200.0);
        if late {
            return;
        }
    }
    if let Ok((blink, reanim)) = effects.add_reanimation(0.0, 0.0, ReanimationType::Sunflower, resources, images) {
        reanim.set_frames_for_layer("anim_blink");
        reanim.anim_rate = 15.0;
        reanim.loop_type = LoopType::PlayOnceFullLastFrame;
        effects.attach_to_another(blink, sunflower, "anim_idle");
    }
}

/// `CreditScreen::TurnOffTongues`: only some of the dancing zombies stick their tongues out.
fn turn_off_tongues(effects: &mut EffectSystem, id: ReanimationId, parent_track: usize) {
    if effects.is_attached_as(id, "zombie_credits_dance") && parent_track % 4 != 1
        && let Some(reanim) = effects.get_mut(id)
    {
        for (i, track) in reanim.definition.tracks.iter().enumerate() {
            if track.name.eq_ignore_ascii_case("anim_tongue") {
                reanim.tracks[i].render_group = RENDER_GROUP_HIDDEN;
            }
        }
    }
    for (track, child) in effects.attachments_of(id) {
        turn_off_tongues(effects, child, track);
    }
}

/// The buttons, and Escape for the pause dialog.
#[allow(clippy::too_many_arguments)]
fn credits_input(
    mut screen: ResMut<CreditScreen>,
    cursor: Res<GameCursor>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut dialogs: ResMut<Dialogs>,
    resources: Res<GameResources>,
    mut next_state: ResMut<NextState<AppState>>,
    mut sounds: Sounds,
) {
    if dialogs.is_modal() || dialogs.took_input() {
        return;
    }
    let screen = &mut *screen;
    let p = cursor.0.map(|p| IVec2::new(p.x.floor() as i32, p.y.floor() as i32));
    screen.main_menu.is_over = p.is_some_and(|p| screen.main_menu.contains(p));
    screen.replay.is_over = p.is_some_and(|p| screen.replay.contains(p));
    if mouse.just_pressed(MouseButton::Left) {
        if screen.main_menu.is_over {
            sounds.play_sample("SOUND_GRAVEBUTTON");
            screen.main_menu.is_down = true;
        } else if screen.replay.is_over {
            sounds.play_sample("SOUND_TAP");
            screen.replay.is_down = true;
        }
    }
    if mouse.just_released(MouseButton::Left) {
        if screen.main_menu.is_down && screen.main_menu.is_over {
            next_state.set(AppState::GameSelector);
        } else if screen.replay.is_down && screen.replay.is_over {
            // KillCreditScreen + ShowCreditScreen: leaving and entering the state starts the movie over.
            next_state.set(AppState::Credits);
        }
        screen.main_menu.is_down = false;
        screen.replay.is_down = false;
    }
    // PauseCredits.
    if keys.any_just_pressed([KeyCode::Space, KeyCode::Enter, KeyCode::Escape]) && !screen.paused {
        sounds.stop_foley(FoleyType::Scream);
        sounds.play_sample("SOUND_PAUSE");
        screen.paused = true;
        lawn_message_box(
            &mut dialogs,
            &resources,
            DialogId::CreditsPause,
            "[CREDITS_PAUSE_HEADER]",
            "[CREDITS_PAUSE_BODY]",
            "[CREDITS_RESUME_BUTTON]",
            "[MAIN_MENU_BUTTON]",
            ButtonMode::YesNo,
        );
    }
}

fn draw_credits(screen: Res<CreditScreen>, resources: Res<GameResources>, effects: Res<EffectSystem>, mut g: ResMut<Graphics>) {
    let (res, g) = (&*resources, &mut *g);
    g.push_state();
    g.set_color(SexyColor::BLACK);
    g.fill_rect(0.0, 0.0, GAME_WIDTH, GAME_HEIGHT);
    g.pop_state();
    let Some(id) = screen.reanim else { return };
    let Some(reanim) = effects.get(id) else { return };
    let frame_factor = 1.0 / (reanim.definition.frame_count() as f32 - 1.0);
    let anim_time = reanim.anim_time;
    let transform_of = |name: &str| reanim.current_transform(reanim.find_track(name));
    let bg1 = transform_of("Background");
    let bg2 = transform_of("Background2");

    // The backgrounds, cut out where the movie's rectangles are.
    let clipped_backgrounds_1 = screen.phase == CreditsPhase::Main2 && frame_factor * 125.0 > anim_time;
    let clipped_backgrounds_2 = screen.phase == CreditsPhase::Main3 && frame_factor * 125.0 > anim_time;
    if screen.phase == CreditsPhase::End {
        draw_final_credits(g, res);
    }
    let draw_clipped = |g: &mut Graphics, t: &sexy::reanim::ReanimTransform, background: &str| {
        if t.f == -1.0 {
            return;
        }
        let Some(size) = t.image.as_deref().and_then(|i| res.image(i)).map(|i| (i.width as f32 - 1.0, i.height as f32 - 1.0)) else { return };
        let Some(image) = res.image(background) else { return };
        g.push_state();
        g.clip_rect(t.x, t.y, size.0, size.1);
        g.draw_image(image, t.x - GAME_WIDTH / 2.0, t.y - GAME_HEIGHT / 2.0);
        g.pop_state();
    };
    if clipped_backgrounds_1 {
        let (bg3, bg4) = (transform_of("Background3"), transform_of("Background4"));
        draw_clipped(g, &bg2, "IMAGE_BACKGROUND1");
        draw_clipped(g, &bg3, "IMAGE_BACKGROUND1");
        draw_clipped(g, &bg4, "IMAGE_BACKGROUND2");
    }
    if clipped_backgrounds_2 {
        let (bg3, bg4) = (transform_of("Background3"), transform_of("Background4"));
        draw_clipped(g, &bg1, "IMAGE_BACKGROUND1");
        if bg3.f != -1.0
            && let Some(size) = bg3.image.as_deref().and_then(|i| res.image(i)).map(|i| (i.width as f32 - 1.0, i.height as f32 - 1.0))
        {
            g.push_state();
            g.translate(bg3.x - 20.0, bg3.y - 260.0);
            g.clip_rect(20.0, 260.0, size.0, size.1);
            if let Some(image) = res.image("IMAGE_BACKGROUND3") {
                g.draw_image(image, -220.0, 0.0);
            }
            if let Some(pool) = res.image("IMAGE_POOL") {
                g.draw_image(pool, 34.0, 278.0);
            }
            g.pop_state();
        }
        draw_clipped(g, &bg4, "IMAGE_BACKGROUND2");
    }
    effects.draw_group(g, id, 1, res);

    // What the movie shows in front of its stage.
    let (mut disco, mut fog, mut pool, mut night_pool, mut door_bottom, mut chimney) = (false, false, false, false, false, false);
    let between = |from: f32, to: f32| anim_time > frame_factor * from && anim_time < frame_factor * to;
    match screen.phase {
        CreditsPhase::Main1 => {
            pool = between(203.0, 268.0);
            if between(305.0, 339.0) {
                pool = true;
                door_bottom = true;
            }
        }
        CreditsPhase::Main2 => {
            if between(187.0, 249.0) {
                pool = true;
                night_pool = true;
            }
            if (between(187.0, 249.0) && (bg1.f != -1.0 || bg2.f != -1.0)) || (between(123.0, 189.0) && bg1.f != -1.0) {
                disco = true;
            }
            if between(189.0, 249.0) && bg2.f != -1.0 {
                disco = true;
                fog = true;
            }
        }
        CreditsPhase::Main3 => {
            chimney = between(123.0, 218.0);
            disco = anim_time > frame_factor * 217.0 && bg1.f != -1.0;
        }
        _ => {}
    }
    if bg2.f == -1.0 {
        door_bottom = false;
        chimney = false;
    }
    // The pool, the door's inside and the chimney move with the second background.
    let with_background2 = |g: &mut Graphics, draw: &mut dyn FnMut(&mut Graphics)| {
        g.push_state();
        g.translate(bg2.x + 220.0, 0.0);
        draw(g);
        g.pop_state();
    };
    if bg2.f != -1.0 && pool && let Some(water) = &screen.water {
        with_background2(g, &mut |g| {
            water.draw(g, res, night_pool);
        });
    }
    if door_bottom && let Some(image) = res.image("IMAGE_BACKGROUND3_GAMEOVER_INTERIOR_OVERLAY") {
        with_background2(g, &mut |g| g.draw_image(image, -171.0, 241.0));
    }
    effects.draw(g, id, res);
    g.push_state();
    if door_bottom {
        g.clip_rect(48.0, 0.0, GAME_WIDTH, GAME_HEIGHT);
    }
    if disco {
        let time = reanim.definition.frame_count() as f32 * anim_time / reanim.anim_rate;
        draw_disco(g, res, 600.0, 450.0, time);
        draw_disco(g, res, 200.0, 450.0, time);
    }
    if fog && let Some(machine) = res.image("IMAGE_REANIM_CREDITS_FOGMACHINE") {
        g.push_state();
        g.translate(bg2.x + 220.0, 0.0);
        g.draw_image(machine, 600.0, 200.0);
        g.pop_state();
    }
    effects.draw_group(g, id, 2, res);
    g.pop_state();
    if door_bottom && let Some(image) = res.image("IMAGE_BACKGROUND3_GAMEOVER_MASK") {
        with_background2(g, &mut |g| g.draw_image(image, -172.0, 234.0));
    }
    if chimney && let Some(image) = res.image("IMAGE_BACKGROUND5_GAMEOVER_MASK") {
        with_background2(g, &mut |g| g.draw_image(image, -220.0, 81.0));
    }
    effects.draw_group(g, id, 3, res);

    for (_, system) in effects.particle_ids() {
        if !system.is_attachment && !system.dead {
            draw_particle_system(g, system, res);
        }
    }
    if fog {
        let percent = animate_curve_float_time(frame_factor * 189.0, frame_factor * 249.0, anim_time, 0.0, 1.0, TodCurve::Linear);
        g.push_state();
        g.translate(bg2.x + 220.0, 0.0);
        draw_fog_effect(g, res, reanim, percent);
        g.pop_state();
    }
    effects.draw_group(g, id, 3, res);
    if let Some((x, y)) = screen.brain
        && let Some(brain) = res.image("IMAGE_BRAIN")
    {
        g.draw_image(brain, x, y);
    }

    // The buttons, and (in the end) the fade in from black.
    screen.main_menu.draw(g, res);
    screen.replay.draw(g, res);
    if screen.phase == CreditsPhase::End {
        let alpha = animate_curve(50, 100, screen.phase_counter, 255, 0, TodCurve::Linear);
        if alpha > 0 {
            g.push_state();
            g.set_color(SexyColor::rgba(0, 0, 0, alpha.clamp(0, 255) as u8));
            g.fill_rect(0.0, 0.0, GAME_WIDTH, GAME_HEIGHT);
            g.pop_state();
        }
    }
}

/// `CreditScreen::DrawFinalCredits`.
fn draw_final_credits(g: &mut Graphics, res: &GameResources) {
    let tr = |key: &str| res.translate(key);
    let Some(big) = res.font("FONT_HOUSEOFTERROR28") else { return };
    let Some(small) = res.font("FONT_HOUSEOFTERROR16") else { return };
    g.tod_draw_string(big, 400, 60, &tr("[CREDITS_GAMENAME]"), SexyColor::WHITE, Justify::Center);
    let white = SexyColor::WHITE;
    g.tod_draw_string_wrapped(small, SexyRect::new(405.0, 90.0, 200.0, 200.0), &tr("[CREDITS_NAMES1]"), white, Justify::Left);
    g.tod_draw_string_wrapped(small, SexyRect::new(190.0, 90.0, 200.0, 200.0), &tr("[CREDITS_ROLES1]"), white, Justify::Right);
    g.tod_draw_string_wrapped(small, SexyRect::new(340.0, 280.0, 450.0, 250.0), &tr("[CREDITS_NAMES2]"), white, Justify::Left);
    g.tod_draw_string_wrapped(small, SexyRect::new(30.0, 280.0, 300.0, 250.0), &tr("[CREDITS_ROLES2]"), white, Justify::Right);
    g.tod_draw_string(small, 400, 530, &tr("[CREDITS_THANKS]"), white, Justify::Center);
}

/// `DrawDisco`: a rotating screen of light, as two triangles of the light image.
fn draw_disco(g: &mut Graphics, res: &GameResources, center_x: f32, center_y: f32, time: f32) {
    let Some(image) = res.image("IMAGE_REANIM_CREDITS_DISCOLIGHTS") else { return };
    let corner = |angle: f32| Vec2::new(angle.cos() * 600.0 + center_x, angle.sin() * 200.0 + center_y);
    let (p1, p2, p4) = (corner(time), corner(time + std::f32::consts::FRAC_PI_2), corner(time + std::f32::consts::PI + std::f32::consts::FRAC_PI_2));
    // The picture's top-left corner is at p1, its right side runs to p2 and its bottom to p4.
    let (u, v) = (p2 - p1, p4 - p1);
    let (w, h) = (image.width as f32, image.height as f32);
    let transform = Affine2::from_cols(u / w, v / h, p1 + (u + v) / 2.0);
    g.push_state();
    g.set_colorize_images(true);
    g.set_color(SexyColor::rgba(255, 255, 255, 128));
    g.draw_image_matrix(image, transform);
    g.pop_state();
}

/// `CreditScreen::DrawFogEffect`: the fog rolling over the stage, `fade` of the way in.
fn draw_fog_effect(g: &mut Graphics, res: &GameResources, reanim: &crate::reanim::Reanimation, fade: f32) {
    let Some(fog) = res.image("IMAGE_FOG") else { return };
    let alpha = (fade * 255.0) as i32;
    let anim_seconds = reanim.definition.frame_count() as f32 * reanim.anim_time / (reanim.anim_rate * SECONDS_PER_UPDATE);
    let time = anim_seconds * std::f32::consts::PI * 2.0;
    g.push_state();
    g.set_colorize_images(true);
    for x in 0..14 {
        for y in 0..7 {
            let look = x + (x + 17) * y;
            let col = (look % 8) as u32;
            let (pos_x, pos_y) = (x as f32 * 80.0 - 15.0, y as f32 * 85.0 + 200.0);
            let phase_x = 6.0 * std::f32::consts::PI * x as f32 / 14.0;
            let phase_y = 6.0 * std::f32::consts::PI * y as f32 / 7.0;
            let motion = 13.0 + 4.0 * (time / 900.0 + phase_y).sin() + 8.0 * (time / 500.0 + phase_x).sin();
            let color = (255.0 - (look % 20) as f32 * 1.5 - motion * 1.5) as i32;
            let lightness = (255.0 - (look % 20) as f32 - motion) as i32;
            g.set_color(SexyColor::rgba(color.clamp(0, 255) as u8, color.clamp(0, 255) as u8, lightness.clamp(0, 255) as u8, alpha.clamp(0, 255) as u8));
            g.draw_image_cel(fog, pos_x, pos_y, col, 0);
        }
    }
    g.pop_state();
}
