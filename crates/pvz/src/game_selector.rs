//! The main menu: the tombstone with its game-mode buttons, clouds, flowers
//! and the wooden name sign.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Widget/GameSelector.cpp`. Options
//! opens the options dialog (`options`), Help the zombies' help note (the
//! award screen). Not yet ported: tooltips, the trophy, zen garden needs,
//! and the screens the other buttons lead to.

use bevy::math::Affine2;
use bevy::prelude::*;
use sexy::matrix::Matrix3;
use sexy::reanim::{LoopType, RENDER_GROUP_HIDDEN, RENDER_GROUP_NORMAL};

use crate::audio::{FoleyType, Music, MusicTune, Sounds};
use crate::dialog::{ButtonMode, DialogId, Dialogs, OptionsFrom};
use crate::graphics::{Graphics, SexyColor};
use crate::input::GameCursor;
use crate::almanac::{AlmanacStart, do_almanac_dialog};
use crate::award::AwardScreen;
use crate::particle::{ParticleEffect, draw_particle_system};
use crate::plant::SeedType;
use crate::zombie::ZombieType;
use crate::lawn_app::{PlayerChanged, confirm_quit, do_continue_dialog, do_create_user_dialog, do_user_dialog, lawn_message_box};
use crate::options::{Settings, do_new_options};
use crate::player::{PlayerInfo, ProfileMgr};
use crate::reanim::{EffectSystem, ReanimationId, ReanimationType, draw_reanimation, draw_render_group};
use crate::resources::GameResources;
use crate::widgets::{ButtonEvent, GameButton, update_buttons};
use crate::{AppState, UiSet};

pub struct GameSelectorPlugin;

impl Plugin for GameSelectorPlugin {
    fn build(&self, app: &mut App) {
        let open = resource_exists::<GameSelector>;
        app.add_systems(OnEnter(AppState::GameSelector), start_selector)
            .add_systems(OnExit(AppState::GameSelector), kill_selector)
            .add_systems(FixedUpdate, update_selector.run_if(open))
            .add_systems(Update, (selector_input, player_changed).chain().in_set(UiSet::ScreenInput).run_if(open))
            .add_systems(Update, draw_selector.in_set(UiSet::ScreenDraw).run_if(open));
    }
}

/// Where the three clickable flowers on the right are.
const FLOWER_CENTERS: [(f32, f32); 3] = [(765.0, 483.0), (663.0, 455.0), (701.0, 439.0)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonId {
    Adventure,
    Minigame,
    Puzzle,
    Survival,
    ZenGarden,
    Options,
    Help,
    Quit,
    ChangeUser,
    Store,
    Almanac,
}

/// `SelectorAnimState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectorState {
    /// The tombstone rising.
    Open,
    /// Waiting for the first player to enter a name.
    NewUser,
    /// The name sign dropping in.
    ShowSign,
    Idle,
}

struct Buttons {
    adventure: GameButton<ButtonId>,
    minigame: GameButton<ButtonId>,
    puzzle: GameButton<ButtonId>,
    survival: GameButton<ButtonId>,
    zen_garden: GameButton<ButtonId>,
    options: GameButton<ButtonId>,
    help: GameButton<ButtonId>,
    quit: GameButton<ButtonId>,
    change_user: GameButton<ButtonId>,
    store: GameButton<ButtonId>,
    almanac: GameButton<ButtonId>,
}

impl Buttons {
    /// Front to back, as `OrderInManagerChanged` stacks them over the screen.
    fn front_to_back(&mut self) -> [&mut GameButton<ButtonId>; 11] {
        [
            &mut self.almanac,
            &mut self.store,
            &mut self.help,
            &mut self.quit,
            &mut self.options,
            &mut self.adventure,
            &mut self.minigame,
            &mut self.puzzle,
            &mut self.zen_garden,
            &mut self.survival,
            &mut self.change_user,
        ]
    }

    fn all_mut(&mut self) -> [&mut GameButton<ButtonId>; 11] {
        self.front_to_back()
    }
}

#[derive(Resource)]
pub struct GameSelector {
    level: i32,
    show_start_button: bool,
    minigames_locked: bool,
    puzzle_locked: bool,
    survival_locked: bool,
    state: SelectorState,
    starting_game: bool,
    starting_game_counter: i32,
    selector_reanim: ReanimationId,
    cloud_reanims: [ReanimationId; 6],
    cloud_counters: [i32; 6],
    flower_reanims: [ReanimationId; 3],
    leaf_reanim: ReanimationId,
    leaf_counter: i32,
    hand_reanim: Option<ReanimationId>,
    /// `mHasTrophy`: adventure has been finished, so the sunflower stands here.
    has_trophy: bool,
    trophy_sparkle: Option<crate::reanim::ParticleSystemId>,
    buttons: Buttons,
}

const SELECTOR_IMAGES: [&str; 29] = [
    "IMAGE_REANIM_SELECTORSCREEN_ADVENTURE_BUTTON",
    "IMAGE_REANIM_SELECTORSCREEN_ADVENTURE_HIGHLIGHT",
    "IMAGE_REANIM_SELECTORSCREEN_STARTADVENTURE_BUTTON",
    "IMAGE_REANIM_SELECTORSCREEN_STARTADVENTURE_HIGHLIGHT",
    "IMAGE_REANIM_SELECTORSCREEN_SURVIVAL_BUTTON",
    "IMAGE_REANIM_SELECTORSCREEN_SURVIVAL_HIGHLIGHT",
    "IMAGE_REANIM_SELECTORSCREEN_CHALLENGES_BUTTON",
    "IMAGE_REANIM_SELECTORSCREEN_CHALLENGES_HIGHLIGHT",
    "IMAGE_REANIM_SELECTORSCREEN_VASEBREAKER_BUTTON",
    "IMAGE_REANIM_SELECTORSCREEN_VASEBREAKER_HIGHLIGHT",
    "IMAGE_REANIM_SELECTORSCREEN_ALMANAC_SHADOW",
    "IMAGE_REANIM_SELECTORSCREEN_KEY_SHADOW",
    "IMAGE_REANIM_SELECTORSCREEN_WOODSIGN2_PRESS",
    "IMAGE_SELECTORSCREEN_ZENGARDEN",
    "IMAGE_SELECTORSCREEN_ZENGARDENHIGHLIGHT",
    "IMAGE_SELECTORSCREEN_OPTIONS1",
    "IMAGE_SELECTORSCREEN_OPTIONS2",
    "IMAGE_SELECTORSCREEN_HELP1",
    "IMAGE_SELECTORSCREEN_HELP2",
    "IMAGE_SUNFLOWER_TROPHY",
    "IMAGE_SELECTORSCREEN_QUIT1",
    "IMAGE_SELECTORSCREEN_QUIT2",
    "IMAGE_SELECTORSCREEN_STORE",
    "IMAGE_SELECTORSCREEN_STOREHIGHLIGHT",
    "IMAGE_SELECTORSCREEN_ALMANAC",
    "IMAGE_SELECTORSCREEN_ALMANACHIGHLIGHT",
    "IMAGE_SELECTORSCREEN_LEVELNUMBERS",
    "IMAGE_BLANK",
    "IMAGE_PLANTSPEECHBUBBLE",
];

fn polygon(points: [(f32, f32); 4]) -> Option<[Vec2; 4]> {
    Some(points.map(|(x, y)| Vec2::new(x, y)))
}

/// `LawnApp::ShowGameSelector` + `GameSelector::GameSelector`.
fn start_selector(
    mut commands: Commands,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut effects: ResMut<EffectSystem>,
    mut music: ResMut<Music>,
    mut sounds: Sounds,
    profiles: Res<ProfileMgr>,
) {
    for id in SELECTOR_IMAGES {
        if let Err(e) = resources.load_image(id, &mut images) {
            error!("{e}");
        }
    }
    music.make_sure_music_is_playing(MusicTune::TitleCrazyDaveMainTheme);
    match build_selector(&mut resources, &mut images, &mut effects, sounds.rng(), profiles.player()) {
        Ok(selector) => {
            commands.insert_resource(selector);
            sounds.play_sample("SOUND_ROLL_IN");
        }
        Err(e) => error!("couldn't open the main menu: {e}"),
    }
}

fn build_selector(
    resources: &mut GameResources,
    images: &mut Assets<Image>,
    effects: &mut EffectSystem,
    rng: &mut sexy::rand::MtRand,
    player: Option<&PlayerInfo>,
) -> Result<GameSelector, String> {
    let res = &*resources;
    let height_of = |id: &str| res.image(id).map_or(0, |i| i.height as i32);
    let width_of = |id: &str| res.image(id).map_or(0, |i| i.width as i32);
    let new_button = |id, normal: &str, over: &str| GameButton::new(id, res, normal, over, over);

    let mut adventure = new_button(
        ButtonId::Adventure,
        "IMAGE_REANIM_SELECTORSCREEN_ADVENTURE_BUTTON",
        "IMAGE_REANIM_SELECTORSCREEN_ADVENTURE_HIGHLIGHT",
    );
    adventure.resize(0, 0, width_of("IMAGE_REANIM_SELECTORSCREEN_ADVENTURE_BUTTON"), 125);
    adventure.polygon = polygon([(7.0, 1.0), (328.0, 30.0), (314.0, 125.0), (1.0, 78.0)]);

    let mut minigame = new_button(
        ButtonId::Minigame,
        "IMAGE_REANIM_SELECTORSCREEN_SURVIVAL_BUTTON",
        "IMAGE_REANIM_SELECTORSCREEN_SURVIVAL_HIGHLIGHT",
    );
    minigame.resize(0, 0, width_of("IMAGE_REANIM_SELECTORSCREEN_SURVIVAL_BUTTON"), 130);
    minigame.polygon = polygon([(4.0, 2.0), (312.0, 51.0), (296.0, 130.0), (7.0, 77.0)]);

    let mut puzzle = new_button(
        ButtonId::Puzzle,
        "IMAGE_REANIM_SELECTORSCREEN_CHALLENGES_BUTTON",
        "IMAGE_REANIM_SELECTORSCREEN_CHALLENGES_HIGHLIGHT",
    );
    puzzle.resize(0, 0, width_of("IMAGE_REANIM_SELECTORSCREEN_CHALLENGES_BUTTON"), 121);
    puzzle.polygon = polygon([(2.0, 0.0), (281.0, 55.0), (268.0, 121.0), (3.0, 60.0)]);

    let mut survival = new_button(
        ButtonId::Survival,
        "IMAGE_REANIM_SELECTORSCREEN_VASEBREAKER_BUTTON",
        "IMAGE_REANIM_SELECTORSCREEN_VASEBREAKER_HIGHLIGHT",
    );
    survival.resize(0, 0, width_of("IMAGE_REANIM_SELECTORSCREEN_VASEBREAKER_BUTTON"), 124);
    survival.polygon = polygon([(7.0, 1.0), (267.0, 62.0), (257.0, 124.0), (7.0, 57.0)]);

    for b in [&mut adventure, &mut minigame, &mut puzzle, &mut survival] {
        b.btn_no_draw = true;
        b.mouse_visible = false;
    }

    let mut zen_garden =
        new_button(ButtonId::ZenGarden, "IMAGE_SELECTORSCREEN_ZENGARDEN", "IMAGE_SELECTORSCREEN_ZENGARDENHIGHLIGHT");
    zen_garden.resize(0, 0, 130, 130);
    zen_garden.mouse_visible = false;

    let mut options = new_button(ButtonId::Options, "IMAGE_SELECTORSCREEN_OPTIONS1", "IMAGE_SELECTORSCREEN_OPTIONS2");
    options.resize(
        0,
        0,
        width_of("IMAGE_SELECTORSCREEN_OPTIONS1"),
        height_of("IMAGE_SELECTORSCREEN_OPTIONS1") + 23,
    );
    options.button_offset = (0, 15);

    let mut help = new_button(ButtonId::Help, "IMAGE_SELECTORSCREEN_HELP1", "IMAGE_SELECTORSCREEN_HELP2");
    help.resize(0, 0, width_of("IMAGE_SELECTORSCREEN_HELP1"), height_of("IMAGE_SELECTORSCREEN_HELP1") + 33);
    help.button_offset = (0, 30);

    let mut quit = new_button(ButtonId::Quit, "IMAGE_SELECTORSCREEN_QUIT1", "IMAGE_SELECTORSCREEN_QUIT2");
    quit.resize(0, 0, width_of("IMAGE_SELECTORSCREEN_QUIT1") + 10, height_of("IMAGE_SELECTORSCREEN_QUIT1") + 10);
    quit.button_offset = (5, 5);

    let mut change_user = new_button(ButtonId::ChangeUser, "IMAGE_BLANK", "IMAGE_BLANK");
    change_user.resize(0, 0, 250, 30);

    for b in [&mut options, &mut help, &mut quit, &mut change_user] {
        b.btn_no_draw = true;
        b.mouse_visible = false;
    }

    let mut store = new_button(ButtonId::Store, "IMAGE_SELECTORSCREEN_STORE", "IMAGE_SELECTORSCREEN_STOREHIGHLIGHT");
    store.resize(405, 484, width_of("IMAGE_SELECTORSCREEN_STORE"), height_of("IMAGE_SELECTORSCREEN_STORE"));
    store.mouse_visible = false;

    let mut almanac =
        new_button(ButtonId::Almanac, "IMAGE_SELECTORSCREEN_ALMANAC", "IMAGE_SELECTORSCREEN_ALMANACHIGHLIGHT");
    almanac.resize(327, 428, width_of("IMAGE_SELECTORSCREEN_ALMANAC"), height_of("IMAGE_SELECTORSCREEN_ALMANAC"));
    almanac.mouse_visible = false;

    // MakeSureMusicIsPlaying(MUSIC_TUNE_TITLE_CRAZY_DAVE_MAIN_THEME) happens in start_selector.

    let (selector_reanim, selector) = effects.add_reanimation(0.5, 0.5, ReanimationType::SelectorScreen, resources, images)?;
    selector.play("anim_open", LoopType::PlayOnceAndHold, 0, 30.0);
    selector.assign_render_group_to_prefix("flower", RENDER_GROUP_HIDDEN);
    selector.assign_render_group_to_prefix("leaf", RENDER_GROUP_HIDDEN);
    selector.assign_render_group_to_track("SelectorScreen_BG", 1);
    let (sign_start, sign_count) = selector.definition.frames_for_layer("anim_sign");
    selector.frame_base_pose = (sign_start + sign_count - 1) as i32;

    let mut cloud_reanims = Vec::new();
    let mut cloud_counters = [0; 6];
    for (i, counter) in cloud_counters.iter_mut().enumerate() {
        let (id, cloud) = effects.add_reanimation(0.5, 0.5, ReanimationType::SelectorScreen, resources, images)?;
        let name = format!("anim_cloud{}", if i > 1 { i + 2 } else { i + 1 });
        cloud.play(&name, LoopType::PlayOnceAndHold, 0, 0.0);
        *counter = rng.range_int(-6000, 2000);
        if *counter < 0 {
            cloud.anim_time = -*counter as f32 / 6000.0;
            cloud.anim_rate = 0.5;
            *counter = 0;
        } else {
            cloud.anim_rate = 0.0;
        }
        cloud_reanims.push(id);
    }

    let mut flower_reanims = Vec::new();
    for i in 0..3 {
        let (id, flower) = effects.add_reanimation(0.5, 0.5, ReanimationType::SelectorScreen, resources, images)?;
        flower.play(&format!("anim_flower{}", i + 1), LoopType::PlayOnceAndHold, 0, 0.0);
        flower.anim_rate = 0.0;
        flower_reanims.push(id);
    }

    let (leaf_reanim, leaf) = effects.add_reanimation(0.5, 0.5, ReanimationType::SelectorScreen, resources, images)?;
    leaf.play("anim_grass", LoopType::Loop, 0, 6.0);
    leaf.anim_rate = 0.0;

    let mut gs = GameSelector {
        level: 1,
        show_start_button: false,
        minigames_locked: false,
        puzzle_locked: false,
        survival_locked: false,
        state: SelectorState::Open,
        starting_game: false,
        starting_game_counter: 0,
        selector_reanim,
        cloud_reanims: cloud_reanims.try_into().unwrap(),
        cloud_counters,
        flower_reanims: flower_reanims.try_into().unwrap(),
        leaf_reanim,
        leaf_counter: 50,
        hand_reanim: None,
        has_trophy: false,
        trophy_sparkle: None,
        buttons: Buttons {
            adventure,
            minigame,
            puzzle,
            survival,
            zen_garden,
            options,
            help,
            quit,
            change_user,
            store,
            almanac,
        },
    };
    gs.sync_profile(player, effects);
    Ok(gs)
}

impl GameSelector {
    /// `GameSelector::SyncProfile`. With no player, everything is locked.
    fn sync_profile(&mut self, player: Option<&PlayerInfo>, effects: &mut EffectSystem) {
        // `--lvl-2-1` shows (and starts) that level without touching the player.
        self.level = crate::level_flag().or(player.map(|p| p.level)).unwrap_or(1);
        self.show_start_button = true;
        self.minigames_locked = true;
        self.puzzle_locked = true;
        self.survival_locked = true;
        if let Some(player) = player {
            if self.level >= 2 {
                self.show_start_button = false;
            }
            if player.has_finished_adventure() {
                self.minigames_locked = false;
                self.survival_locked = false;
                self.puzzle_locked = false;
                self.show_start_button = false;
            }
            self.minigames_locked &= !player.has_unlocked_minigames;
            self.puzzle_locked &= !player.has_unlocked_puzzle_mode;
            self.survival_locked &= !player.has_unlocked_survival_mode;
        }
        self.has_trophy = player.is_some_and(PlayerInfo::has_finished_adventure);
        if let Some(sparkle) = self.trophy_sparkle.take() {
            effects.remove_particle(sparkle);
        }
        self.sync_buttons(player, effects);
    }

    /// `GameSelector::SyncButtons`.
    fn sync_buttons(&mut self, player: Option<&PlayerInfo>, effects: &mut EffectSystem) {
        let almanac_open = player.is_some_and(PlayerInfo::can_show_almanac);
        let store_open = player.is_some_and(PlayerInfo::can_show_store);
        let zen_open = player.is_some_and(PlayerInfo::can_show_zen_garden);
        let b = &mut self.buttons;
        (b.almanac.disabled, b.almanac.visible) = (!almanac_open, almanac_open);
        (b.store.disabled, b.store.visible) = (!store_open, store_open);
        (b.zen_garden.disabled, b.zen_garden.visible) = (!zen_open, zen_open);

        let Some(selector) = effects.get_mut(self.selector_reanim) else { return };
        if almanac_open {
            selector.assign_render_group_to_prefix("almanac_key_shadow", RENDER_GROUP_NORMAL);
            let shadow = (!store_open).then_some("IMAGE_REANIM_SELECTORSCREEN_ALMANAC_SHADOW");
            selector.set_image_override("almanac_key_shadow", shadow);
        } else if store_open {
            selector.assign_render_group_to_prefix("almanac_key_shadow", RENDER_GROUP_NORMAL);
            selector.set_image_override("almanac_key_shadow", Some("IMAGE_REANIM_SELECTORSCREEN_KEY_SHADOW"));
        } else {
            selector.assign_render_group_to_prefix("almanac_key_shadow", RENDER_GROUP_HIDDEN);
        }

        // Locked modes show no highlight and are greyed out.
        let lock = |button: &mut GameButton<ButtonId>, locked: bool, normal: &str, highlight: &str| {
            let over = if locked { normal } else { highlight };
            button.over_image = over.to_owned();
            button.down_image = over.to_owned();
            button.color = if locked { SexyColor::rgba(128, 128, 128, 255) } else { SexyColor::WHITE };
        };
        lock(
            &mut b.minigame,
            self.minigames_locked,
            "IMAGE_REANIM_SELECTORSCREEN_SURVIVAL_BUTTON",
            "IMAGE_REANIM_SELECTORSCREEN_SURVIVAL_HIGHLIGHT",
        );
        lock(
            &mut b.puzzle,
            self.puzzle_locked,
            "IMAGE_REANIM_SELECTORSCREEN_CHALLENGES_BUTTON",
            "IMAGE_REANIM_SELECTORSCREEN_CHALLENGES_HIGHLIGHT",
        );
        lock(
            &mut b.survival,
            self.survival_locked,
            "IMAGE_REANIM_SELECTORSCREEN_VASEBREAKER_BUTTON",
            "IMAGE_REANIM_SELECTORSCREEN_VASEBREAKER_HIGHLIGHT",
        );
        let tint = |c: SexyColor| [c.r, c.g, c.b, c.a];
        let i = selector.find_track("SelectorScreen_Survival_button");
        selector.tracks[i].track_color = tint(b.minigame.color);
        let i = selector.find_track("SelectorScreen_Challenges_button");
        selector.tracks[i].track_color = tint(b.puzzle.color);
        let i = selector.find_track("SelectorScreen_ZenGarden_button");
        selector.tracks[i].track_color = tint(b.survival.color);

        // The original only ever hides here, so switching between a new and
        // a returning player left both buttons' shadows hidden. Un-hide first.
        for prefix in ["SelectorScreen_Adventure_", "SelectorScreen_StartAdventure_"] {
            selector.assign_render_group_to_prefix(prefix, RENDER_GROUP_NORMAL);
        }
        if self.state != SelectorState::Open {
            for track in ["SelectorScreen_Adventure_button", "SelectorScreen_StartAdventure_button"] {
                selector.assign_render_group_to_track(track, RENDER_GROUP_HIDDEN);
            }
        }
        let (hidden, prefix) = if self.show_start_button {
            ("SelectorScreen_Adventure_", "STARTADVENTURE")
        } else {
            ("SelectorScreen_StartAdventure_", "ADVENTURE")
        };
        selector.assign_render_group_to_prefix(hidden, RENDER_GROUP_HIDDEN);
        b.adventure.button_image = format!("IMAGE_REANIM_SELECTORSCREEN_{prefix}_BUTTON");
        b.adventure.over_image = format!("IMAGE_REANIM_SELECTORSCREEN_{prefix}_HIGHLIGHT");
        b.adventure.down_image = b.adventure.over_image.clone();
    }

    /// `GameSelector::TrackButton`: keeps a button on a reanim track.
    fn track_positions(&mut self, effects: &EffectSystem) {
        let Some(selector) = effects.get(self.selector_reanim) else { return };
        let pos = |track: &str, dx: f32, dy: f32| {
            let t = selector.current_transform(selector.find_track(track));
            ((t.x + dx) as i32, (t.y + dy) as i32)
        };
        let adventure_track =
            if self.show_start_button { "SelectorScreen_StartAdventure_button" } else { "SelectorScreen_Adventure_button" };
        let b = &mut self.buttons;
        for (button, track, dx, dy) in [
            (&mut b.adventure, adventure_track, 0.0, 0.0),
            (&mut b.minigame, "SelectorScreen_Survival_button", 0.0, 0.0),
            (&mut b.puzzle, "SelectorScreen_Challenges_button", 0.0, 0.0),
            (&mut b.survival, "SelectorScreen_ZenGarden_button", 0.0, 0.0),
            (&mut b.zen_garden, "SelectorScreen_BG_Right", 100.0, 360.0),
            (&mut b.options, "SelectorScreen_BG_Right", 494.0, 434.0),
            (&mut b.quit, "SelectorScreen_BG_Right", 644.0, 469.0),
            (&mut b.help, "SelectorScreen_BG_Right", 576.0, 458.0),
            (&mut b.almanac, "SelectorScreen_BG_Right", 256.0, 387.0),
            (&mut b.store, "SelectorScreen_BG_Right", 334.0, 441.0),
            (&mut b.change_user, "woodsign2", 24.0, 10.0),
        ] {
            (button.x, button.y) = pos(track, dx, dy);
        }
    }

    /// `GameSelector::ClickedAdventure`.
    fn clicked_adventure(
        &mut self,
        effects: &mut EffectSystem,
        resources: &mut GameResources,
        images: &mut Assets<Image>,
        music: &mut Music,
        sounds: &mut Sounds,
    ) {
        music.stop_all_music();
        sounds.play_sample("SOUND_LOSEMUSIC");
        self.starting_game = true;
        for b in self.buttons.all_mut() {
            b.disabled = true;
        }
        match effects.add_reanimation(-70.0, 10.0, ReanimationType::ZombieHand, resources, images) {
            Ok((id, hand)) => {
                hand.loop_type = LoopType::PlayOnceAndHold;
                for (inst, track) in hand.tracks.iter_mut().zip(&hand.definition.tracks) {
                    if track.name.get(..4).is_some_and(|n| n.eq_ignore_ascii_case("rock")) {
                        inst.ignore_clip_rect = true;
                    }
                }
                self.hand_reanim = Some(id);
                sounds.play_foley(FoleyType::DirtRise);
            }
            Err(e) => error!("{e}"),
        }
    }

    /// `GameSelector::ButtonDepress`.
    #[allow(clippy::too_many_arguments)]
    fn button_depress(
        &mut self,
        id: ButtonId,
        effects: &mut EffectSystem,
        resources: &mut GameResources,
        images: &mut Assets<Image>,
        dialogs: &mut Dialogs,
        profiles: &ProfileMgr,
        settings: &Settings,
        music: &mut Music,
        sounds: &mut Sounds,
        commands: &mut Commands,
        next_state: &mut NextState<AppState>,
    ) {
        let locked = match id {
            ButtonId::Minigame if self.minigames_locked => Some("[MINIGAME_LOCKED_MESSAGE]"),
            ButtonId::Puzzle if self.puzzle_locked => Some("[PUZZLE_LOCKED_MESSAGE]"),
            ButtonId::Survival if self.survival_locked => Some("[SURVIVAL_LOCKED_MESSAGE]"),
            _ => None,
        };
        if let Some(message) = locked {
            let (header, ok) = ("[MODE_LOCKED]", "[DIALOG_BUTTON_OK]");
            lawn_message_box(dialogs, resources, DialogId::Message, header, message, ok, "", ButtonMode::Footer);
            return;
        }
        match id {
            // PreNewGame(GAMEMODE_ADVENTURE, true): a save resumes through
            // the Continue dialog; otherwise the level starts fresh.
            ButtonId::Adventure => {
                commands.insert_resource(crate::board::SelectedMode(None));
                let save = crate::board::save_dir()
                    .is_some_and(|dir| profiles.player().is_some_and(|p| crate::board::has_save(&dir, p.id)));
                if save {
                    commands.insert_resource(crate::board::ContinueMode(crate::board::GameMode::Adventure));
                    do_continue_dialog(dialogs, resources, true);
                } else {
                    self.clicked_adventure(effects, resources, images, music, sounds);
                }
            }
            ButtonId::Quit => confirm_quit(dialogs, resources),
            ButtonId::ChangeUser => do_user_dialog(dialogs, resources, profiles),
            // The title theme comes back when it closes (`almanac`).
            ButtonId::Almanac => do_almanac_dialog(dialogs, resources, AlmanacStart::Index),
            // KillGameSelector + ShowStoreScreen: the store opens over the
            // menu's place and the main menu comes back when it closes.
            ButtonId::Store => next_state.set(AppState::Store),
            // KillGameSelector + Zen Garden.
            ButtonId::ZenGarden => next_state.set(AppState::ZenGarden),
            // KillGameSelector + ShowChallengeScreen(CHALLENGE_PAGE_SURVIVAL).
            ButtonId::Survival => {
                commands.insert_resource(crate::challenge::ChallengePageRequest(crate::challenge::ChallengePage::Survival));
                next_state.set(AppState::Challenges);
            }
            // KillGameSelector + ShowChallengeScreen(CHALLENGE_PAGE_CHALLENGE).
            ButtonId::Minigame => {
                commands.insert_resource(crate::challenge::ChallengePageRequest(crate::challenge::ChallengePage::Challenge));
                next_state.set(AppState::Challenges);
            }
            // KillGameSelector + ShowChallengeScreen(CHALLENGE_PAGE_PUZZLE).
            ButtonId::Puzzle => {
                commands.insert_resource(crate::challenge::ChallengePageRequest(crate::challenge::ChallengePage::Puzzle));
                next_state.set(AppState::Challenges);
            }
            ButtonId::Options => {
                let finished_adventure = profiles.player().is_some_and(PlayerInfo::has_finished_adventure);
                do_new_options(dialogs, resources, settings, OptionsFrom::GameSelector { finished_adventure });
            }
            // KillGameSelector + ShowAwardScreen(AWARD_HELP_ZOMBIENOTE).
            ButtonId::Help => {
                commands.insert_resource(AwardScreen::help(profiles.player().map_or(1, |p| p.level)));
                next_state.set(AppState::Award);
            }
        }
    }
}

/// `LawnApp::KillGameSelector`.
fn kill_selector(mut commands: Commands, mut effects: ResMut<EffectSystem>) {
    commands.remove_resource::<GameSelector>();
    effects.clear();
}

/// `GameSelector::Update`, at 100 ticks per second.
#[allow(clippy::too_many_arguments)]
fn update_selector(
    mut selector: ResMut<GameSelector>,
    mut next_state: ResMut<NextState<AppState>>,
    mut effects: ResMut<EffectSystem>,
    mut dialogs: ResMut<Dialogs>,
    mut resources: ResMut<GameResources>,
    mut profiles: ResMut<ProfileMgr>,
    mut play_intro: ResMut<crate::board::PlayIntro>,
    mut images: ResMut<Assets<Image>>,
    mut sounds: Sounds,
) {
    let gs = &mut *selector;

    // The trophy's sparkle keeps going.
    if let Some(system) = gs.trophy_sparkle.and_then(|id| effects.particle_mut(id)) {
        system.update(sounds.rng());
    }
    // `AddTrophySparkle`: the sunflower's sparkle, once the selector is up.
    if gs.has_trophy && gs.trophy_sparkle.is_none() && !matches!(gs.state, SelectorState::Open) {
        let rng = sounds.rng();
        gs.trophy_sparkle =
            effects.add_particle(85.0, 380.0, crate::board::render_layer::TOP, ParticleEffect::TrophySparkle, &mut resources, &mut images, rng);
    }

    if gs.starting_game {
        gs.starting_game_counter += 1;
        if gs.starting_game_counter > 450 {
            // KillGameSelector + PreNewGame(GAMEMODE_ADVENTURE): the
            // first-ever adventure opens with the intro cinematic. TODO:
            // Dave's taco level and the zen garden tutorial before 5-5.
            let first_time = profiles.player().is_some_and(|p| !p.has_finished_adventure() && p.level == 1);
            play_intro.0 = first_time && crate::level_flag().is_none();
            next_state.set(AppState::Board);
            return;
        }
        gs.buttons.adventure.color =
            if gs.starting_game_counter % 20 < 10 { SexyColor::rgba(80, 80, 80, 255) } else { SexyColor::WHITE };
        if gs.starting_game_counter == 125 {
            sounds.play_sample("SOUND_EVILLAUGH");
        }
    }

    let Some(selector) = effects.get_mut(gs.selector_reanim) else { return };
    match gs.state {
        SelectorState::Open if selector.loop_count > 0 => {
            for track in [
                "SelectorScreen_Adventure_button",
                "SelectorScreen_StartAdventure_button",
                "SelectorScreen_Survival_button",
                "SelectorScreen_Challenges_button",
                "SelectorScreen_ZenGarden_button",
            ] {
                selector.assign_render_group_to_track(track, RENDER_GROUP_HIDDEN);
            }
            for b in gs.buttons.all_mut() {
                b.mouse_visible = true;
                if b.id != ButtonId::ChangeUser && b.id != ButtonId::Store && b.id != ButtonId::Almanac {
                    b.btn_no_draw = false;
                }
            }
            if profiles.player().is_none() {
                do_create_user_dialog(&mut dialogs, &resources);
                gs.state = SelectorState::NewUser;
            } else {
                selector.play("anim_sign", LoopType::PlayOnceAndHold, 0, 30.0);
                gs.state = SelectorState::Idle;
            }
        }
        SelectorState::NewUser if dialogs.get(DialogId::CreateUser).is_none() => {
            selector.play("anim_sign", LoopType::PlayOnceAndHold, 0, 30.0);
            gs.state = SelectorState::ShowSign;
        }
        SelectorState::ShowSign if selector.loop_count > 0 => {
            gs.state = SelectorState::Idle;
            // `mNeedsMessageOnGameSelector`: adventure was finished.
            let needs = profiles.player().is_some_and(|p| p.needs_message_on_game_selector != 0);
            if needs {
                if let Some(player) = profiles.player_mut() {
                    player.needs_message_on_game_selector = 0;
                }
                profiles.save_current_details();
                dialogs.do_dialog(
                    &resources,
                    DialogId::Message,
                    &resources.translate("[ADVENTURE_COMPLETE_HEADER]"),
                    &resources.translate("[ADVENTURE_COMPLETE_BODY]"),
                    &resources.translate("[DIALOG_BUTTON_OK]"),
                    ButtonMode::Footer,
                );
            }
        }
        _ => {}
    }

    for (i, counter) in gs.cloud_counters.iter_mut().enumerate() {
        let Some(cloud) = effects.get_mut(gs.cloud_reanims[i]) else { continue };
        if *counter > 0 {
            *counter -= 1;
            if *counter == 0 {
                cloud.loop_count = 0;
                cloud.anim_time = 0.0;
                cloud.anim_rate = 0.5;
            }
        } else if cloud.loop_count > 0 {
            *counter = sounds.rng().range_int(2000, 4000);
        }
    }

    let Some(selector) = effects.get_mut(gs.selector_reanim) else { return };
    selector.update();
    let bg_right = selector.find_track("SelectorScreen_BG_Right");
    let bg = selector.current_transform(bg_right);
    // The flowers are attached to the right background (AttachToAnotherReanimation).
    let flower_overlay = selector.attachment_overlay_matrix(bg_right);

    if let Some(leaf) = effects.get_mut(gs.leaf_reanim) {
        leaf.update();
        leaf.set_position(bg.x - 71.0, bg.y - 41.0);
        gs.leaf_counter -= 1;
        if gs.leaf_counter == 0 {
            let rate = sounds.rng().range_f32(3.0, 12.0);
            leaf.play("anim_grass", LoopType::Loop, 20, rate);
            gs.leaf_counter = sounds.rng().range_int(200, 400);
        }
    }
    for id in gs.flower_reanims {
        if let Some(flower) = effects.get_mut(id) {
            flower.overlay = flower_overlay;
            flower.update();
        }
    }
    for id in gs.cloud_reanims.into_iter().chain(gs.hand_reanim) {
        if let Some(r) = effects.get_mut(id) {
            r.update();
        }
    }

    gs.track_positions(&effects);
    let change_user_active = gs.buttons.change_user.is_over || gs.buttons.change_user.is_down;
    if let Some(selector) = effects.get_mut(gs.selector_reanim) {
        selector.set_image_override("woodsign2", change_user_active.then_some("IMAGE_REANIM_SELECTORSCREEN_WOODSIGN2_PRESS"));
    }
}

/// Mouse handling: buttons first, then the screen itself (`MouseDown`).
#[allow(clippy::too_many_arguments)]
fn selector_input(
    mut commands: Commands,
    mut selector: ResMut<GameSelector>,
    mut effects: ResMut<EffectSystem>,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    cursor: Res<GameCursor>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut dialogs: ResMut<Dialogs>,
    profiles: Res<ProfileMgr>,
    settings: Res<Settings>,
    mut music: ResMut<Music>,
    mut sounds: Sounds,
    mut next_state: ResMut<NextState<AppState>>,
    mut auto_pressed: Local<bool>,
) {
    let gs = &mut *selector;
    // A dialog takes the mouse: to the screen it's as if it left the window.
    let cursor = if dialogs.is_modal() { GameCursor(None) } else { *cursor };
    let mut result = update_buttons(&mut gs.buttons.front_to_back(), &cursor, &mouse);
    // Development aid: `PVZ_AUTO_START=options`, `help`, `store` or `almanac` presses
    // that button once the menu is up; `almanac-plants` and `almanac-zombies`
    // open the almanac on that page, and `credits` rolls the credits.
    if gs.state == SelectorState::Idle && !*auto_pressed {
        *auto_pressed = true;
        match std::env::var("PVZ_AUTO_START").as_deref() {
            Ok("options") => result.events.push(ButtonEvent::Depress(ButtonId::Options)),
            Ok("help") => result.events.push(ButtonEvent::Depress(ButtonId::Help)),
            Ok("almanac") => result.events.push(ButtonEvent::Depress(ButtonId::Almanac)),
            Ok("store") => result.events.push(ButtonEvent::Depress(ButtonId::Store)),
            Ok("credits") => next_state.set(AppState::Credits),
            // `survival` opens the challenge screen's survival page; `survival-day`
            // starts a normal day survival run straight away (eye checks).
            Ok("survival") => {
                commands.insert_resource(crate::challenge::ChallengePageRequest(crate::challenge::ChallengePage::Survival));
                next_state.set(AppState::Challenges);
            }
            Ok("minigames") => {
                commands.insert_resource(crate::challenge::ChallengePageRequest(crate::challenge::ChallengePage::Challenge));
                next_state.set(AppState::Challenges);
            }
            Ok("minigame-war") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::MiniGame(crate::board::MiniGame::WarAndPeas))));
                next_state.set(AppState::Board);
            }
            Ok("minigame-bowling") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::MiniGame(
                    crate::board::MiniGame::WallnutBowling,
                ))));
                next_state.set(AppState::Board);
            }
            Ok("minigame-pogo") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::MiniGame(crate::board::MiniGame::PogoParty))));
                next_state.set(AppState::Board);
            }
            Ok("minigame-column") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::MiniGame(crate::board::MiniGame::Column))));
                next_state.set(AppState::Board);
            }
            Ok("minigame-invisighoul") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::MiniGame(
                    crate::board::MiniGame::Invisighoul,
                ))));
                next_state.set(AppState::Board);
            }
            Ok("minigame-speed") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::MiniGame(crate::board::MiniGame::Speed))));
                next_state.set(AppState::Board);
            }
            Ok("zen") => next_state.set(AppState::ZenGarden),
            Ok("tree") => next_state.set(AppState::TreeOfWisdom),
            Ok("puzzles") => {
                commands.insert_resource(crate::challenge::ChallengePageRequest(crate::challenge::ChallengePage::Puzzle));
                next_state.set(AppState::Challenges);
            }
            Ok("puzzle-iz-1") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::Puzzle(
                    crate::board::PuzzleLevel::IZombie(1),
                ))));
                next_state.set(AppState::Board);
            }
            Ok("puzzle-1") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::Puzzle(
                    crate::board::PuzzleLevel::Vasebreaker(1),
                ))));
                next_state.set(AppState::Board);
            }
            Ok("minigame-aquarium") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::MiniGame(
                    crate::board::MiniGame::Zombiquarium,
                ))));
                next_state.set(AppState::Board);
            }
            Ok("minigame-beghouled") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::MiniGame(
                    crate::board::MiniGame::Beghouled,
                ))));
                next_state.set(AppState::Board);
            }
            Ok("minigame-slot") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::MiniGame(
                    crate::board::MiniGame::SlotMachine,
                ))));
                next_state.set(AppState::Board);
            }
            Ok("minigame-laststand") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::MiniGame(
                    crate::board::MiniGame::LastStand,
                ))));
                next_state.set(AppState::Board);
            }
            Ok("minigame-portal") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::MiniGame(
                    crate::board::MiniGame::PortalCombat,
                ))));
                next_state.set(AppState::Board);
            }
            Ok("minigame-rain") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::MiniGame(
                    crate::board::MiniGame::RainingSeeds,
                ))));
                next_state.set(AppState::Board);
            }
            Ok("minigame-stars") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::MiniGame(
                    crate::board::MiniGame::SeeingStars,
                ))));
                next_state.set(AppState::Board);
            }
            Ok("minigame-boss") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::MiniGame(crate::board::MiniGame::FinalBoss))));
                next_state.set(AppState::Board);
            }
            Ok("minigame-bobsled") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::MiniGame(
                    crate::board::MiniGame::BobsledBonanza,
                ))));
                next_state.set(AppState::Board);
            }
            Ok("survival-day") => {
                commands.insert_resource(crate::board::SelectedMode(Some(crate::board::GameMode::Survival {
                    lawn: crate::board::SurvivalLawn::Day,
                    difficulty: crate::board::SurvivalDifficulty::Normal,
                })));
                next_state.set(AppState::Board);
            }
            Ok("almanac-plants") => do_almanac_dialog(&mut dialogs, &resources, AlmanacStart::Plant(SeedType::Peashooter)),
            Ok("almanac-zombies") => do_almanac_dialog(&mut dialogs, &resources, AlmanacStart::Zombie(ZombieType::Normal)),
            _ => {}
        }
    }
    for event in result.events {
        match event {
            // ButtonMouseEnter: locked modes stay quiet.
            ButtonEvent::MouseEnter(id) => {
                let locked = (id == ButtonId::Minigame && gs.minigames_locked)
                    || (id == ButtonId::Puzzle && gs.puzzle_locked)
                    || (id == ButtonId::Survival && gs.survival_locked);
                if !locked {
                    sounds.play_foley(FoleyType::Bleep);
                }
            }
            // ButtonPress: the tombstone buttons sound like stone.
            ButtonEvent::Press(id) => match id {
                ButtonId::Adventure | ButtonId::Minigame | ButtonId::Puzzle | ButtonId::Survival => {
                    sounds.play_sample("SOUND_GRAVEBUTTON")
                }
                _ => sounds.play_sample("SOUND_TAP"),
            },
            ButtonEvent::Depress(id) => gs.button_depress(
                id,
                &mut effects,
                &mut resources,
                &mut images,
                &mut dialogs,
                &profiles,
                &settings,
                &mut music,
                &mut sounds,
                &mut commands,
                &mut next_state,
            ),
        }
    }
    if let Some(p) = result.background_press {
        for (i, (fx, fy)) in FLOWER_CENTERS.into_iter().enumerate() {
            if let Some(flower) = effects.get_mut(gs.flower_reanims[i])
                && flower.anim_rate <= 0.0
                && Vec2::new(fx, fy).distance(p) < 20.0
            {
                flower.anim_rate = 24.0;
                sounds.play_foley(FoleyType::LimbsPop);
            }
        }
    }
}

/// `LawnApp` calls `SyncProfile` whenever the player changes.
fn player_changed(
    mut changed: MessageReader<PlayerChanged>,
    mut selector: ResMut<GameSelector>,
    mut effects: ResMut<EffectSystem>,
    profiles: Res<ProfileMgr>,
) {
    if changed.read().count() > 0 {
        selector.sync_profile(profiles.player(), &mut effects);
    }
}

fn to_affine(m: Matrix3) -> Affine2 {
    Affine2::from_cols_array(&[m.m00, m.m10, m.m01, m.m11, m.m02, m.m12])
}

/// `GameSelector::Draw`, the buttons, then `DrawOverlay`.
fn draw_selector(
    selector: Res<GameSelector>,
    effects: Res<EffectSystem>,
    resources: Res<GameResources>,
    profiles: Res<ProfileMgr>,
    cursor: Res<crate::input::GameCursor>,
    mut g: ResMut<Graphics>,
) {
    let gs = &*selector;
    let Some(reanim) = effects.get(gs.selector_reanim) else { return };

    draw_render_group(&mut g, reanim, 1, &resources);
    for id in gs.cloud_reanims {
        if let Some(cloud) = effects.get(id) {
            draw_reanimation(&mut g, cloud, &resources);
        }
    }
    draw_render_group(&mut g, reanim, RENDER_GROUP_NORMAL, &resources);

    if gs.state == SelectorState::Open {
        let bg = reanim.current_transform(reanim.find_track("SelectorScreen_BG_Right"));
        let (fx, fy) = (bg.x % 1.0, bg.y % 1.0);
        for b in [&gs.buttons.options, &gs.buttons.quit, &gs.buttons.help] {
            if let Some(image) = resources.image(&b.button_image) {
                let x = (b.x + b.button_offset.0) as f32 + fx;
                let y = (b.y + b.button_offset.1) as f32 + fy;
                g.draw_image(image, x, y);
            }
        }
    }

    if let Some(player) = profiles.player()
        && !player.name.is_empty()
        && !matches!(gs.state, SelectorState::Open | SelectorState::NewUser)
        && let Some(font) = resources.font("FONT_BRIANNETOD16")
    {
        let welcome = format!("{}!", player.name);
        let sign = reanim.attachment_overlay_matrix(reanim.find_track("woodsign1"));
        let width = font.font.string_width(&welcome) as f32;
        let offset = Matrix3::translation(170.5 - (width * 0.5).trunc(), 102.5);
        g.set_color(SexyColor::rgba(255, 245, 200, 255));
        g.draw_string_matrix(font, to_affine(sign * offset), &welcome);
    }
    // The sunflower trophy once adventure has been finished. (Gold takes
    // achievements, which aren't ported: the silver cel is used.)
    if gs.has_trophy
        && gs.state != SelectorState::Open
        && let Some(trophy) = resources.image("IMAGE_SUNFLOWER_TROPHY")
    {
        let left = reanim.current_transform(reanim.find_track("SelectorScreen_BG_Left"));
        g.draw_image_cel(trophy, left.x + 10.0, left.y + 390.0, 0, 0);
    }
    if let Some(sparkle) = gs.trophy_sparkle.and_then(|id| effects.particle(id)) {
        draw_particle_system(&mut g, sparkle, &resources);
    }
    // `GameSelector::UpdateTooltip`: the silver trophy's label.
    if gs.has_trophy
        && cursor.0.is_some_and(|p| p.x >= 50.0 && p.x < 135.0 && p.y >= 325.0 && p.y <= 550.0)
        && let Some(mut tip) = crate::board::tooltip::ToolTip::new(&resources, "", "[SILVER_SUNFLOWER_TOOLTIP]", "")
    {
        tip.x = 20;
        tip.y = 495;
        tip.draw(&mut g, &resources);
    }

    // Widgets stacked over the screen, back to front.
    let b = &gs.buttons;
    for button in [
        &b.change_user,
        &b.survival,
        &b.zen_garden,
        &b.puzzle,
        &b.minigame,
        &b.adventure,
        &b.options,
        &b.quit,
        &b.help,
        &b.store,
        &b.almanac,
    ] {
        button.draw(&mut g, &resources);
    }
    if let Some(player) = profiles.player()
        && b.zen_garden.visible
        && crate::zen_garden::plants_need_water(player)
        && let Some(image) = resources.image("IMAGE_WATERDROP")
    {
        g.draw_image(image, b.zen_garden.x as f32 + 123.0, b.zen_garden.y as f32 + 45.0);
    }

    draw_overlay(gs, reanim, &effects, &resources, &mut g);
}

/// `GameSelector::DrawOverlay`: level number on the adventure button,
/// the zombie hand, leaves and flowers.
fn draw_overlay(
    gs: &GameSelector,
    reanim: &crate::reanim::Reanimation,
    effects: &EffectSystem,
    resources: &GameResources,
    g: &mut Graphics,
) {
    if !gs.show_start_button && let Some(numbers) = resources.image("IMAGE_SELECTORSCREEN_LEVELNUMBERS") {
        let pressed = gs.buttons.adventure.is_down && gs.buttons.adventure.is_over;
        let offset = if pressed { 1.0 } else { 0.0 };
        let bg = reanim.current_transform(reanim.find_track("SelectorScreen_BG_Right"));
        let mut area_x = bg.x + offset;
        let mut sub_x = area_x;
        // The "LEVEL -" label is part of the button image, so sit on the button
        // (the original's BG_Right + 125 lands lower than the label in this pak).
        let button_y = gs.buttons.adventure.y as f32 + offset;
        let (mut label_area_y, label_sub_y) = (button_y + 23.0, button_y + 26.0);
        let stage = ((gs.level - 1) / 10 + 1).clamp(1, 6);
        let sub = gs.level - (stage - 1) * 10;
        if stage == 1 {
            label_area_y += 1.0;
        } else if stage == 4 {
            area_x -= 1.0;
        }
        if sub == 3 {
            sub_x -= 1.0;
        }
        g.push_state();
        g.set_colorize_images(true);
        g.set_color(gs.buttons.adventure.color);
        g.draw_image_cel(numbers, area_x + 486.0, label_area_y, stage as u32, 0);
        if sub < 10 {
            g.draw_image_cel(numbers, sub_x + 513.0, label_sub_y, sub as u32, 0);
        } else if sub == 10 {
            g.draw_image_cel(numbers, sub_x + 513.0, label_sub_y, 1, 0);
            g.draw_image_cel(numbers, sub_x + 522.0, label_sub_y + 1.0, 0, 0);
        }
        g.pop_state();
    }


    if let Some(hand) = gs.hand_reanim.and_then(|id| effects.get(id)) {
        // The hand goes down behind the lawn's edge; its rocks ignore the clip.
        g.push_state();
        g.clip_rect(
            0.0,
            0.0,
            crate::board::BOARD_WIDTH as f32,
            (crate::board::BOARD_HEIGHT - 40) as f32,
        );
        draw_reanimation(g, hand, resources);
        g.pop_state();
    }
    if let Some(leaf) = effects.get(gs.leaf_reanim) {
        draw_reanimation(g, leaf, resources);
    }
    for id in gs.flower_reanims {
        if let Some(flower) = effects.get(id) {
            draw_reanimation(g, flower, resources);
        }
    }
}
