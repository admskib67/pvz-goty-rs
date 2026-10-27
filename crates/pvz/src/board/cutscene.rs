//! The level intro: the view starts on the house, pans right to the street
//! and back, then the seed bank slides in, the sod rolls out on the first
//! levels and the lawn mowers roll in before play starts.
//!
//! Times are in hundredths of a second (`mCutsceneTime` advances by 10 per
//! tick). On night levels the graves rise out of the lawn after the pan, and
//! the fog rolls back in on foggy ones.
//!
//! Behaviour per ruslan831 decomp, `Lawn/CutScene.cpp`: `StartLevelIntro`,
//! `Update`, `AnimateBoard`, `PlaceLawnItems`, `ShowShovel`,
//! `IsAfterSeedChooser` and `CalcPosition`.

use bevy::prelude::*;
use sexy::tod::{TodCurve, animate_curve};

use crate::audio::{FoleyType, Music, MusicTune, Sounds};
use crate::particle::ParticleEffect;
use crate::plant::SeedType;
use crate::reanim::{EffectSystem, ReanimationType};
use crate::resources::GameResources;

use super::message::MessageStyle;
use super::tutorial::TutorialState;
use super::{
    BOARD_IMAGE_WIDTH_OFFSET, BOARD_OFFSET, BOARD_WIDTH, BackgroundType, Board, GameScene, SEED_BANK_OFFSET_X,
    SEED_BANK_OFFSET_X_END, GameMode, make_render_order, render_layer,
};

const TIME_PAN_RIGHT_START: i32 = 1500;
const TIME_PAN_RIGHT_END: i32 = 3500;
const TIME_EARLY_DAVE_ENTER_START: i32 = 2000;
const TIME_EARLY_DAVE_ENTER_END: i32 = 2750;
const TIME_EARLY_DAVE_LEAVE_END: i32 = 4000;
const TIME_SEED_CHOOSER_SLIDE_ON_START: i32 = 4000;
const TIME_SEED_CHOOSER_SLIDE_ON_END: i32 = 4250;
const TIME_SEED_CHOOSER_SLIDE_OFF_END: i32 = 4750;
const TIME_SEED_CHOOSER_SLIDE_OFF_START: i32 = 4500;
const TIME_SEED_BANK_ON_START: i32 = 4000;
const TIME_SEED_BANK_ON_END: i32 = 4250;
const TIME_PAN_LEFT_START: i32 = 4500;
const TIME_PAN_LEFT_END: i32 = 6000;
const TIME_SEED_BANK_RIGHT_START: i32 = 4750;
const TIME_SEED_BANK_RIGHT_END: i32 = 6000;
const TIME_ROLL_SOD_START: i32 = 6000;
const TIME_ROLL_SOD_END: i32 = 8000;
const TIME_READY_SET_PLANT_START: i32 = 6000;
/// `TimeFogRollIn`: when the fog starts coming back in during the intro.
const TIME_FOG_ROLL_IN: i32 = 5950;
const TIME_READY_SET_PLANT_END: i32 = 7830;
const TIME_INTRO_END: i32 = 6000;
const TIME_GRAVE_STONE_START: i32 = 6000;
const TIME_GRAVE_STONE_END: i32 = 7000;
const TIME_LAWN_MOWER_DURATION: i32 = 250;
const LOST_TIME_BRAIN_GRAPHIC_START: i32 = 6000;
const LOST_TIME_BRAIN_GRAPHIC_SHAKE: i32 = 7000;
const LOST_TIME_BRAIN_GRAPHIC_CANCEL_SHAKE: i32 = 8000;
const LOST_TIME_BRAIN_GRAPHIC_END: i32 = 11000;
const LOST_TIME_END: i32 = 11000;
const TIME_LAWN_MOWER_START: [i32; 6] = [6300, 6250, 6200, 6150, 6100, 6050];

#[derive(Debug, Clone, Default)]
pub struct CutScene {
    cutscene_time: i32,
    sod_time: i32,
    grave_stone_time: i32,
    ready_set_plant_time: i32,
    fog_time: i32,
    boss_time: i32,
    crazy_dave_time: i32,
    /// `mCrazyDaveDialogStart`: the first message of his talk, or -1.
    pub(super) crazy_dave_dialog_start: i32,
    /// `IsScrolledLeftAtStart`: the view starts on the house; levels that don't scroll don't.
    scrolled_left: bool,
    lawn_mower_time: i32,
    placed_lawn_items: bool,
    placed_flower_pots: bool,
    /// `mSeedChoosing`: the intro's clock waits while seeds are picked.
    pub(super) seed_choosing: bool,
    /// The brains graphic of the lost-level cutscene.
    zombies_won_reanim: Option<crate::reanim::ReanimationId>,
}

impl CutScene {
    /// `CalcPosition`.
    fn calc_position(&self, start: i32, end: i32, from: i32, to: i32) -> i32 {
        animate_curve(start, end, self.cutscene_time, from, to, TodCurve::EaseInOut)
    }

    /// `ShowZombieWalking`: the zombie that won walks in once the view has
    /// panned to the house.
    pub fn time(&self) -> i32 {
        self.cutscene_time
    }

    pub fn show_zombie_walking(&self) -> bool {
        self.cutscene_time > TIME_PAN_RIGHT_START
    }

    /// `CutScene::StartZombiesWon`.
    pub fn start_zombies_won(&mut self) {
        self.cutscene_time = 0;
    }

    /// `CutScene::EndSeedChooser`.
    pub(super) fn end_seed_choosing(&mut self) {
        self.seed_choosing = false;
        self.cutscene_time = self.crazy_dave_time + TIME_SEED_CHOOSER_SLIDE_ON_END + 10;
    }

    pub(super) fn set_time(&mut self, time: i32) {
        self.cutscene_time = time;
    }

    /// `IsAfterSeedChooser`.
    pub fn is_after_seed_chooser(&self) -> bool {
        self.cutscene_time > TIME_SEED_CHOOSER_SLIDE_OFF_START + self.crazy_dave_time
    }

    fn is_scrolled_left_at_start(&self) -> bool {
        self.scrolled_left
    }

    /// `IsShowingCrazyDave`.
    pub(super) fn is_showing_crazy_dave(&self) -> bool {
        self.crazy_dave_time > 0 && self.cutscene_time < TIME_PAN_RIGHT_END + self.crazy_dave_time
    }

    fn time_to_start(&self) -> i32 {
        TIME_INTRO_END
            + self.lawn_mower_time
            + self.sod_time
            + self.grave_stone_time
            + self.crazy_dave_time
            + self.fog_time
            + self.boss_time
            + self.ready_set_plant_time
    }
}

/// `StartLevelIntro`'s choice of what Crazy Dave says first, or -1 when
/// he doesn't come.
pub(super) fn crazy_dave_dialog_start(board: &Board) -> i32 {
    let (first_time, level) = (board.first_time_adventure, board.level);
    let upgrade_level = |from: i32| first_time && (from..=24).contains(&level) && !matches!(level, 15 | 20 | 21);
    if first_time && level == 11 {
        201
    } else if first_time && level == 12 {
        1401
    } else if upgrade_level(13) && board.can_get_packet_upgrade(0) {
        1501
    } else if upgrade_level(16) && board.can_get_packet_upgrade(1) {
        1551
    } else if board.is_wallnut_bowling_level() && board.mode == GameMode::Adventure {
        if first_time { 2400 } else { 2411 }
    } else if first_time && level == 21 {
        // Behaviour per ruslan831 decomp, CutScene::StartLevelIntro: 3-1's talk
        // about the zombies attacking the backyard. Levels 22-24 and 26-30 have
        // no such talk (their seed-slot offers come from 1501/1551 above).
        501
    } else if board.is_whack_a_zombie_level() && board.mode == GameMode::Adventure {
        401
    } else if board.is_little_trouble_level() && board.mode == GameMode::Adventure {
        // Behaviour per ruslan831 decomp, CutScene::StartLevelIntro: 3-5's talk
        // about the zombies' small numbers, on every visit.
        701
    } else if first_time && level == 31 {
        // Behaviour per ruslan831 decomp, CutScene::StartLevelIntro: 4-1's
        // fog story, and 5-1's roof warning.
        801
    } else if level == super::fog::STORMY_NIGHT_LEVEL {
        // 4-5's stormy night: he warns about the lightning.
        1101
    } else if first_time && level == 41 {
        1201
    } else if !first_time && level == 1 {
        // Back at the start: he reminds the player how the game works.
        1601
    } else if board.is_final_boss_level() && board.mode == GameMode::Adventure {
        // Behaviour per ruslan831 decomp, CutScene::StartLevelIntro: on the
        // last level he warns about the boss and is carried off; a replay
        // after losing it skips him. The challenge version has no Dave.
        if board.prev_result_restart { -1 } else { 2300 }
    } else {
        -1
    }
}

/// `CutScene::StartLevelIntro`.
pub fn start_level_intro(
    board: &mut Board,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
    music: &mut Music,
    sounds: &mut Sounds,
    player_name: &str,
) {
    let seed_bank_height = res.image("IMAGE_SEEDBANK").map_or(0, |i| i.height as i32);
    board.seed_bank.x = SEED_BANK_OFFSET_X;
    board.seed_bank.y = -seed_bank_height;
    board.menu_button.no_draw = true;
    board.show_shovel = false;
    board.seed_bank.cut_scene_darken = 255;

    let level = board.level;
    let first_time = board.first_time_adventure;
    let bowling = board.is_wallnut_bowling_level();
    let whack = board.is_whack_a_zombie_level();
    // Levels that don't scroll start on the lawn (IsNonScrollingCutscene).
    let non_scrolling = bowling || whack;
    // Whack-a-zombie's graves come up when Crazy Dave says so.
    let graves = board.stage_has_grave_stones() && !whack;
    let dave_start = crazy_dave_dialog_start(board);
    let stage_has_fog = board.stage_has_fog();
    board.enable_grave_stones = false;
    let cs = &mut board.cut_scene;
    *cs = CutScene::default();
    cs.crazy_dave_dialog_start = -1;
    cs.scrolled_left = !non_scrolling;
    if first_time && matches!(level, 1 | 2 | 4) {
        cs.sod_time = TIME_ROLL_SOD_END - TIME_ROLL_SOD_START;
        board.sod_position = 0;
    } else {
        cs.sod_time = 0;
        board.sod_position = 1000;
    }
    cs.grave_stone_time = if graves { TIME_GRAVE_STONE_END - TIME_GRAVE_STONE_START } else { 0 };
    cs.ready_set_plant_time = if (first_time && level <= 2) || non_scrolling {
        0
    } else {
        TIME_READY_SET_PLANT_END - TIME_READY_SET_PLANT_START
    };
    cs.lawn_mower_time = 550;
    // TODO: fog and the boss.
    cs.crazy_dave_dialog_start = dave_start;
    cs.crazy_dave_time = if cs.crazy_dave_dialog_start != -1 { TIME_EARLY_DAVE_LEAVE_END - TIME_PAN_RIGHT_START } else { 0 };
    if cs.crazy_dave_time > 0 && level == super::FINAL_LEVEL {
        // He's taken a good while: the bungee carries him off.
        cs.crazy_dave_time += 4000;
    }
    cs.fog_time = if stage_has_fog {
        TIME_FOG_ROLL_IN - cs.sod_time - cs.lawn_mower_time - TIME_READY_SET_PLANT_START + 2000
    } else {
        0
    };
    cs.boss_time = if level == super::FINAL_LEVEL && board.mode == GameMode::Adventure { 4000 } else { 0 };

    board.x = if cs.is_scrolled_left_at_start() { BOARD_OFFSET } else { 0 };
    board.player_name = player_name.to_owned();
    if bowling {
        board.show_shovel = true;
    }
    if board.choose_seeds_on_current_level() {
        let y = super::seed_chooser::offset_y(res);
        board.seed_chooser = Some(super::seed_chooser::SeedChooser::new(&board.owned_seeds, board.has_seven_chooser_rows(), y));
    }
    if board.cut_scene.crazy_dave_time > 0 {
        // Dave talks first; the house's name isn't shown.
        music.make_sure_music_is_playing(MusicTune::TitleCrazyDaveMainTheme);
        return;
    }
    if non_scrolling {
        // IsNonScrollingCutscene with no Crazy Dave: CancelIntro.
        start_non_scrolling_intro(board, effects, res, images, sounds);
        return;
    }

    let house = match board.background {
        BackgroundType::Day | BackgroundType::Night => "[PLAYERS_HOUSE]",
        BackgroundType::Pool | BackgroundType::Fog => "[PLAYERS_BACKYARD]",
        BackgroundType::Roof | BackgroundType::Boss => "[PLAYERS_ROOF]",
    };
    let house = res.translate(house).replace("{PLAYER}", player_name);
    board.display_advice(res, &house, MessageStyle::HouseName, None);

    music.make_sure_music_is_playing(MusicTune::ChooseYourSeeds);
}

/// `CutScene::CancelIntro` for levels that don't scroll: the lawn is set up
/// and play starts in a moment.
fn start_non_scrolling_intro(
    board: &mut Board,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
    sounds: &mut Sounds,
) {
    place_lawn_items(board, effects, res, images, sounds);
    board.enable_grave_stones = true;
    board.x = 0;
    board.show_bowling_line = board.is_wallnut_bowling_level();
    board.cut_scene.cutscene_time = board.cut_scene.time_to_start() - 20;
    board.seed_bank.x = SEED_BANK_OFFSET_X_END;
    board.seed_bank.y = 0;
    board.seed_bank.cut_scene_darken = 255;
    board.menu_button.no_draw = false;
}

/// `CutScene::Update`, every tick during the level intro.
pub fn update(
    board: &mut Board,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
    music: &mut Music,
    sounds: &mut Sounds,
) {
    if board.paused || board.scene != GameScene::LevelIntro || board.draw_count == 0 {
        return;
    }
    board.update_crazy_dave(effects, res, images, sounds);
    // The clock waits while seeds are picked, Dave is talking or the shovel is being learned.
    let frozen = board.cut_scene.seed_choosing || board.dave.message_index != -1 || board.is_in_shovel_tutorial();
    if !frozen {
        board.cut_scene.cutscene_time += 10;
        let start_choosing = board.cut_scene.cutscene_time
            == TIME_SEED_CHOOSER_SLIDE_ON_END + board.cut_scene.crazy_dave_time
            && board.choose_seeds_on_current_level();
        if start_choosing {
            board.start_seed_choosing();
        }
    }

    if board.cut_scene.cutscene_time >= board.cut_scene.time_to_start() {
        // RemoveCutsceneZombies.
        for (_, zombie) in board.zombies.iter_mut() {
            if !zombie.is_on_board() {
                zombie.die_no_loot(effects);
            }
        }
        board.menu_button.no_draw = false;
        board.enable_grave_stones = true;
        show_shovel(board);
        board.start_playing(music);
        return;
    }
    animate_board(board, effects, res, images, music, sounds);
}

/// `CutScene::UpdateZombiesWon`, every tick after a zombie got into the
/// house. Returns true once it's over and the game over dialog should show.
pub fn update_zombies_won(
    board: &mut Board,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
    sounds: &mut Sounds,
) -> bool {
    let cs = &mut board.cut_scene;
    cs.cutscene_time += 10;
    let time = cs.cutscene_time;
    if time > TIME_PAN_RIGHT_START && time <= TIME_PAN_RIGHT_END {
        board.x = cs.calc_position(TIME_PAN_RIGHT_START, TIME_PAN_RIGHT_END, 0, BOARD_OFFSET);
    }
    // The zombies chew, then scream, as the screen fills.
    if time == LOST_TIME_BRAIN_GRAPHIC_START - 400 || time == LOST_TIME_BRAIN_GRAPHIC_START - 900 {
        sounds.play_foley(FoleyType::Chomp);
    }
    if time == LOST_TIME_BRAIN_GRAPHIC_START {
        let order = make_render_order(render_layer::SCREEN_FADE, 0, 0);
        match effects.add_reanimation(-(BOARD_OFFSET as f32), 0.0, ReanimationType::ZombiesWon, res, images) {
            Ok((id, reanim)) => {
                reanim.anim_rate = 12.0;
                reanim.loop_type = sexy::reanim::LoopType::PlayOnceAndHold;
                reanim.render_order = order;
                let fullscreen = reanim.find_track("fullscreen");
                reanim.tracks[fullscreen].track_color = [0, 0, 0, 255];
                reanim.set_frames_for_layer("ZombiesWon");
                cs.zombies_won_reanim = Some(id);
            }
            Err(e) => error!("{e}"),
        }
        sounds.play_foley(FoleyType::Scream);
    }
    // The brains graphic jitters while it's being eaten.
    if time == LOST_TIME_BRAIN_GRAPHIC_SHAKE
        && let Some(reanim) = cs.zombies_won_reanim.and_then(|id| effects.get_mut(id))
    {
        reanim.set_shake_override("ZombiesWon", 1.0);
    }
    if time == LOST_TIME_BRAIN_GRAPHIC_CANCEL_SHAKE
        && let Some(reanim) = cs.zombies_won_reanim.and_then(|id| effects.get_mut(id))
    {
        reanim.set_shake_override("ZombiesWon", 0.0);
    }
    if time == LOST_TIME_BRAIN_GRAPHIC_END
        && let Some(reanim) = cs.zombies_won_reanim.and_then(|id| effects.get_mut(id))
    {
        reanim.set_frames_for_layer("anim_screen");
    }
    time == LOST_TIME_END
}

/// `CutScene::ShowShovel`: not until the shovel is introduced in 1-5.
fn show_shovel(board: &mut Board) {
    if board.is_whack_a_zombie_level() || board.is_beghouled() {
        return;
    }
    if !board.first_time_adventure || board.level > 4 {
        board.show_shovel = true;
    }
}

/// `CutScene::PlaceLawnItems`.
fn place_lawn_items(
    board: &mut Board,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
    sounds: &mut Sounds,
) {
    if board.cut_scene.placed_lawn_items {
        return;
    }
    board.cut_scene.placed_lawn_items = true;
    board.init_lawn_mowers();
    let mut fx = super::zombie::Fx { effects, res, images, sounds };
    board.place_rake(&mut fx);
}

/// `CutScene::AddFlowerPots`: the roof starts with pots in its first columns.
fn add_flower_pots(
    board: &mut Board,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
    sounds: &mut Sounds,
) {
    if board.cut_scene.placed_flower_pots {
        return;
    }
    board.cut_scene.placed_flower_pots = true;
    let columns = match board.level {
        41 => 5,
        42 => 4,
        43..=50 => 3,
        _ if board.stage_has_roof() => 3,
        _ => 0,
    };
    for x in 0..columns {
        for y in 0..super::MAX_GRID_SIZE_Y as i32 {
            if board.can_plant_at(x, y, SeedType::Flowerpot) == super::cursor::PlantingReason::Ok {
                board.new_plant(x, y, SeedType::Flowerpot, effects, res, images, sounds.rng());
            }
        }
    }
}

/// The start of `CutScene::AnimateBoard`: Dave walks in, starts talking and
/// (on levels that don't scroll) the intro skips ahead once he's gone.
fn update_dave_timeline(
    board: &mut Board,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
    sounds: &mut Sounds,
) {
    if board.cut_scene.crazy_dave_time <= 0 {
        return;
    }
    let time = board.cut_scene.cutscene_time;
    if time == TIME_EARLY_DAVE_ENTER_START {
        board.dave.enter(effects, res, images, sounds.rng());
    }
    if time == TIME_EARLY_DAVE_ENTER_END && board.cut_scene.crazy_dave_dialog_start != -1 {
        let start = board.cut_scene.crazy_dave_dialog_start;
        let vars = board.dave_vars();
        board.dave.talk_index(start, &vars, effects, res, images, sounds);
        board.cut_scene.crazy_dave_dialog_start = -1;
    }
    if time == TIME_EARLY_DAVE_LEAVE_END && !board.cut_scene.is_scrolled_left_at_start() {
        board.cut_scene.cutscene_time = TIME_PAN_LEFT_END + board.cut_scene.crazy_dave_time;
    }
}

/// `CutScene::AdvanceCrazyDaveDialog`: a click moves Dave's talk along.
/// Returns true when his talk is over and the intro music should fade.
pub(super) fn advance_crazy_dave_dialog(
    board: &mut Board,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
    sounds: &mut Sounds,
) -> bool {
    if board.dave.message_index == -1 {
        return false;
    }
    // "LET THE DIGGING COMMENCE!": the shovel lesson starts.
    if board.dave.message_index == 2406 {
        board.set_tutorial_state(res, TutorialState::ShovelPickup);
        board.dave.leave(effects, sounds);
        return false;
    }
    // He's waiting for an answer, not a click.
    if board.dave.message_text().contains("{NO_CLICK}") {
        return false;
    }
    let vars = board.dave_vars();
    if !board.dave.advance_text(&vars, effects, res, images, sounds) {
        board.dave.leave(effects, sounds);
        if board.is_final_boss_level() {
            // Behaviour per ruslan831 decomp, CutScene::AdvanceCrazyDaveDialog: a
            // zombie reaches in and grabs him.
            board.dave.grab(effects);
            sounds.play_sample("SOUND_BUNGEE_SCREAM");
            return true;
        }
        return !board.cut_scene.is_scrolled_left_at_start();
    }
    match board.dave.message_index {
        405 | 2411 if board.is_wallnut_bowling_level() => board.show_bowling_line = true,
        // "How's that sound?": the seed slot offer.
        1503 | 1553 => board.packet_offer = true,
        // "And instead of ME, it's YOU!": whack-a-zombie's graves come up.
        406 => {
            board.enable_grave_stones = true;
            board.add_grave_stone_particles(sounds);
        }
        _ => {}
    }
    false
}

/// `CutScene::AnimateBoard`.
fn animate_board(
    board: &mut Board,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
    music: &mut Music,
    sounds: &mut Sounds,
) {
    update_dave_timeline(board, effects, res, images, sounds);
    let cs = board.cut_scene.clone();
    let time = cs.cutscene_time;
    let dave = cs.crazy_dave_time;
    let pan_right_start = TIME_PAN_RIGHT_START + dave;
    let pan_right_end = TIME_PAN_RIGHT_END + dave;
    let pan_left_start = TIME_PAN_LEFT_START + dave;
    let pan_left_end = TIME_PAN_LEFT_END + dave;
    let pan_right_to = BOARD_IMAGE_WIDTH_OFFSET - BOARD_WIDTH;

    // Pan right to the street, and back.
    let board_offset = if cs.is_scrolled_left_at_start() { BOARD_OFFSET } else { 0 };
    if time <= pan_right_start {
        board.x = board_offset;
    }
    if time > pan_right_start && time <= pan_right_end {
        board.x = -cs.calc_position(pan_right_start, pan_right_end, -board_offset, pan_right_to);
    }
    if time > pan_left_start {
        board.x = -cs.calc_position(pan_left_start, pan_left_end, pan_right_to, 0);
    }

    // The seed chooser slides up, and away again once the seeds are picked.
    if board.choose_seeds_on_current_level()
        && let Some(offset) = board.seed_chooser.as_ref().map(|_| super::seed_chooser::offset_y(res))
    {
        let on_start = TIME_SEED_CHOOSER_SLIDE_ON_START + dave;
        let on_end = TIME_SEED_CHOOSER_SLIDE_ON_END + dave;
        let off_start = TIME_SEED_CHOOSER_SLIDE_OFF_START + dave;
        let off_end = TIME_SEED_CHOOSER_SLIDE_OFF_END + dave;
        let y = if time > on_start && time <= on_end {
            Some(cs.calc_position(on_start, on_end, offset, 0))
        } else if time > off_start && time <= off_end {
            Some(cs.calc_position(off_start, off_end, 0, offset))
        } else {
            None
        };
        if let (Some(y), Some(chooser)) = (y, board.seed_chooser.as_mut()) {
            chooser.y = y;
        }
        if time > on_start {
            board.menu_button.no_draw = false;
        }
    }

    // The seed bank drops in, after the sod when there's no seed choosing.
    let mut prepare_end = 0;
    if !board.choose_seeds_on_current_level() {
        prepare_end = cs.boss_time + cs.fog_time + cs.grave_stone_time + cs.sod_time - TIME_SEED_CHOOSER_SLIDE_ON_START
            + TIME_PAN_LEFT_END;
    }
    let bank_on_start = TIME_SEED_BANK_ON_START + prepare_end + dave;
    let bank_on_end = TIME_SEED_BANK_ON_END + prepare_end + dave;
    if time > bank_on_start && time <= bank_on_end {
        let height = res.image("IMAGE_SEEDBANK").map_or(0, |i| i.height as i32);
        board.seed_bank.x = SEED_BANK_OFFSET_X;
        board.seed_bank.y = cs.calc_position(bank_on_start, bank_on_end, -height, 0);
    }
    let bank_right_start = TIME_SEED_BANK_RIGHT_START + dave;
    let bank_right_end = TIME_SEED_BANK_RIGHT_END + dave;
    if time > bank_right_start {
        board.seed_bank.x = cs.calc_position(bank_right_start, bank_right_end, SEED_BANK_OFFSET_X, SEED_BANK_OFFSET_X_END);
        board.seed_bank.cut_scene_darken =
            animate_curve(bank_right_start, bank_right_end, time, 255, 128, TodCurve::EaseOut);
    }

    // The first levels roll out new rows of sod.
    if cs.sod_time > 0 {
        let sod_start = TIME_ROLL_SOD_START + dave;
        let sod_end = TIME_ROLL_SOD_END + dave;
        board.sod_position = animate_curve(sod_start, sod_end, time, 0, 1000, TodCurve::Linear);
        if time == sod_start {
            sounds.play_foley(FoleyType::Digger);
            let top = make_render_order(render_layer::TOP, 0, 0);
            let rolls: &[(f32, f32)] = match board.level {
                1 => &[(0.0, 0.0)],
                2 => &[(0.0, -102.0), (0.0, 111.0)],
                4 => &[(-3.0, -198.0), (-3.0, 203.0)],
                _ => &[],
            };
            for &(x, y) in rolls {
                match effects.add_reanimation(x, y, ReanimationType::SodRoll, res, images) {
                    Ok((_, roll)) => roll.render_order = top,
                    Err(e) => error!("{e}"),
                }
            }
            let dirt: &[(f32, f32)] = match board.level {
                1 => &[(35.0, 348.0)],
                2 => &[(35.0, 246.0), (35.0, 459.0)],
                4 => &[(32.0, 150.0), (32.0, 511.0)],
                _ => &[],
            };
            let order = make_render_order(render_layer::TOP, 0, 1);
            for &(x, y) in dirt {
                effects.add_particle(x, y, order, ParticleEffect::SodRoll, res, images, sounds.rng());
            }
        }
        if time == sod_end {
            sounds.stop_foley(FoleyType::Digger);
        }
    }

    // The night lawn's graves come up.
    if cs.grave_stone_time > 0 && time == cs.sod_time + TIME_GRAVE_STONE_START + dave {
        board.enable_grave_stones = true;
        board.add_grave_stone_particles(sounds);
    }

    // The final boss level sets up its lawn before anything else.
    if time == pan_left_start || (board.is_final_boss_level() && time > 0) {
        place_lawn_items(board, effects, res, images, sounds);
        add_flower_pots(board, effects, res, images, sounds);
    }

    // Dr. Zomboss steps onto the lawn; the challenge version starts without
    // his cinematic (`boss_time` is adventure-only).
    if board.is_final_boss_level() && time == TIME_READY_SET_PLANT_START + cs.lawn_mower_time + dave {
        let mut fx = super::zombie::Fx { effects: &mut *effects, res: &mut *res, images: &mut *images, sounds: &mut *sounds };
        board.play_boss_enter(&mut fx);
    }
    if board.is_final_boss_level() && time == bank_on_start {
        music.make_sure_music_is_playing(board.game_music());
    }

    // The fog rolls back in as the scene is revealed.
    if board.fog.blown_count_down > 0 && time > TIME_FOG_ROLL_IN + cs.sod_time + cs.grave_stone_time + dave {
        if board.fog.blown_count_down > 200 {
            board.fog.blown_count_down = 200;
        }
        board.fog.blown_count_down -= 1;
    }

    // The mowers roll in, bottom row first.
    for (row, mower_start) in TIME_LAWN_MOWER_START.iter().enumerate() {
        let start = mower_start + cs.sod_time + cs.grave_stone_time + dave;
        if time > start {
            let x = cs.calc_position(start, start + TIME_LAWN_MOWER_DURATION, -80, -21);
            if let Some(mower) = board.find_lawn_mower_in_row(row as i32) {
                mower.visible = true;
                mower.pos_x = x as f32;
            }
        }
    }

    // "Ready, Set, Plant!", or just the music fading out before play.
    let ready_set_plant = TIME_READY_SET_PLANT_START
        + cs.lawn_mower_time
        + cs.sod_time
        + cs.grave_stone_time
        + dave
        + cs.fog_time
        + cs.boss_time;
    if cs.ready_set_plant_time > 0 && time == ready_set_plant {
        let order = make_render_order(render_layer::SCREEN_FADE, 0, 0);
        match effects.add_reanimation(400.0, 324.0, ReanimationType::ReadySetPlant, res, images) {
            Ok((_, reanim)) => reanim.render_order = order,
            Err(e) => error!("{e}"),
        }
        sounds.play_sample("SOUND_READYSETPLANT");
        if !board.is_final_boss_level() {
            music.fade_out(150);
        }
    }
    if cs.ready_set_plant_time == 0 && time == ready_set_plant - 2000 && !board.is_final_boss_level() {
        music.fade_out(200);
    }
}

const TIME_INTRO_PRESENTS_FADE_IN: i32 = 1000;
const TIME_INTRO_LOGO_START: i32 = 5500;
const TIME_INTRO_LOGO_END: i32 = 5900;
const TIME_INTRO_PAN_RIGHT_START: i32 = 5890;
const TIME_INTRO_PAN_RIGHT_END: i32 = 11890;
const TIME_INTRO_FADE_OUT: i32 = 10890;
const TIME_INTRO_FADE_OUT_END: i32 = 11890;
const TIME_INTRO_CINEMATIC_END: i32 = 13890;

/// The first-ever adventure's cinematic: `CutScene::StartLevelIntro` for
/// `GAMEMODE_INTRO`.
pub fn start_intro(board: &mut Board, music: &mut Music) {
    board.cut_scene = CutScene { crazy_dave_dialog_start: -1, scrolled_left: false, ..CutScene::default() };
    board.menu_button.no_draw = true;
    board.show_shovel = false;
    board.x = 0;
    music.make_sure_music_is_playing(MusicTune::PoolWaterygraves);
}

/// `CutScene::LoadIntroBoard`: the lawn the logo is shown over. The zombies
/// the pool levels bring (football, snorkel, catapult) aren't ported, so
/// cone and bucket heads and plain zombies stand in for them.
fn load_intro_board(
    board: &mut Board,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
    sounds: &mut Sounds,
) {
    use crate::plant::SeedType::*;
    use crate::zombie::ZombieType;
    #[rustfmt::skip]
    let plants = [
        (0, 1, Threepeater), (0, 2, Lilypad), (0, 2, Peashooter), (0, 3, Lilypad), (0, 3, Peashooter), (0, 4, Sunflower),
        (1, 0, Threepeater), (1, 1, Sunflower), (1, 2, Lilypad), (1, 2, Sunflower), (1, 4, Threepeater), (1, 5, Threepeater),
        (2, 0, Sunflower), (2, 1, Peashooter), (2, 3, Lilypad), (2, 3, Peashooter), (2, 4, Sunflower), (2, 5, Sunflower),
        (3, 0, Torchwood), (3, 4, Threepeater), (4, 2, Lilypad), (4, 2, Torchwood), (5, 1, Torchwood), (5, 4, Torchwood),
        (5, 5, Torchwood), (6, 0, Spikeweed), (6, 4, Spikeweed), (7, 1, Spikeweed),
    ];
    for (col, row, seed) in plants {
        let (x, y) = (board.grid_to_pixel_x(col), board.grid_to_pixel_y(col, row));
        let plant = super::plant::Plant::new(col, row, x, y, seed, false, effects, res, images, sounds.rng());
        board.plants.alloc(plant);
    }
    use ZombieType::{Normal as N, Pail as P, TrafficCone as C};
    #[rustfmt::skip]
    let zombies = [
        (N, 460.0, 0), (P, 680.0, 0), (C, 730.0, 0), (N, 810.0, 0), (C, 670.0, 1), (N, 740.0, 1), (N, 880.0, 1),
        (N, 500.0, 2), (C, 680.0, 2), (P, 604.0, 3), (N, 880.0, 3), (N, 600.0, 4), (P, 690.0, 4), (N, 780.0, 4),
        (C, 730.0, 5), (N, 590.0, 5),
    ];
    let mut fx = super::zombie::Fx { effects, res, images, sounds };
    for (ty, x, row) in zombies {
        board.add_zombie_in_row(ty, row, 0, &mut fx);
        if let Some((_, zombie)) = board.zombies.iter_mut().last() {
            zombie.place_x(x);
        }
    }
}

/// `CutScene::UpdateIntro`, every tick. Returns true when the cinematic is
/// over and the adventure's first level should start.
pub fn update_intro(
    board: &mut Board,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
    music: &mut Music,
    sounds: &mut Sounds,
) -> bool {
    if board.paused || board.draw_count == 0 {
        return false;
    }
    board.cut_scene.cutscene_time += 10;
    let time = board.cut_scene.cutscene_time;
    board.x = animate_curve(TIME_INTRO_PAN_RIGHT_START, TIME_INTRO_PAN_RIGHT_END, time, -100, 100, TodCurve::Linear);

    if time == 10 {
        load_intro_board(board, effects, res, images, sounds);
    }
    if time == TIME_INTRO_FADE_OUT {
        music.fade_out(250);
    }
    if time == TIME_INTRO_LOGO_END {
        let order = make_render_order(render_layer::TOP, 0, 0);
        effects.add_particle(400.0, 300.0, order, ParticleEffect::ScreenFlash, res, images, sounds.rng());
        sounds.play_sample("SOUND_HUGE_WAVE");
    }
    if time == TIME_INTRO_FADE_OUT - 200 {
        sounds.play_sample("SOUND_SIREN");
    }
    time >= TIME_INTRO_CINEMATIC_END
}

/// `CutScene::DrawIntro`, in screen coordinates: the black screen, "PopCap
/// Games Presents", the logo and the fade out.
pub fn draw_intro(board: &Board, res: &GameResources, g: &mut crate::graphics::Graphics) {
    use crate::graphics::{Justify, SexyColor};
    let time = board.cut_scene.cutscene_time;
    g.push_state();
    g.translate(-board.x as f32, -board.y as f32);
    if time <= TIME_INTRO_PAN_RIGHT_START || time > TIME_INTRO_FADE_OUT_END {
        g.set_color(SexyColor::BLACK);
        g.fill_rect(0.0, 0.0, BOARD_WIDTH as f32, super::BOARD_HEIGHT as f32);
    }
    let presents_end = TIME_INTRO_PAN_RIGHT_START - TIME_INTRO_PRESENTS_FADE_IN;
    if time > TIME_INTRO_PRESENTS_FADE_IN
        && time <= presents_end
        && let Some(font) = res.font("FONT_BRIANNETOD16")
    {
        let alpha = if time < presents_end - 600 {
            animate_curve(TIME_INTRO_PRESENTS_FADE_IN, TIME_INTRO_PRESENTS_FADE_IN + 300, time, 0, 255, TodCurve::Linear)
        } else {
            animate_curve(presents_end - 600, presents_end - 300, time, 255, 0, TodCurve::Linear)
        };
        let text = res.translate("[INTRO_PRESENTS]");
        g.tod_draw_string(font, 400, 310, &text, SexyColor::rgba(255, 255, 255, alpha.clamp(0, 255) as u8), Justify::Center);
    }
    if time > TIME_INTRO_LOGO_START
        && time <= TIME_INTRO_PAN_RIGHT_END
        && let Some(logo) = res.image("IMAGE_PVZ_LOGO")
    {
        let scale = sexy::tod::animate_curve_float(TIME_INTRO_LOGO_START, TIME_INTRO_LOGO_END, time, 5.0, 1.0, TodCurve::EaseOut);
        let (w, h) = (BOARD_WIDTH as f32, 150.0);
        g.set_color(SexyColor::rgba(0, 0, 0, 128));
        g.fill_rect(400.0 - w * scale * 0.5, 300.0 - h * 0.5 * scale, w * scale, h * scale);
        g.draw_image_matrix(
            logo,
            bevy::math::Affine2::from_cols_array(&[scale, 0.0, 0.0, scale, 400.0, 300.0]),
        );
    }
    if time > TIME_INTRO_FADE_OUT && time <= TIME_INTRO_FADE_OUT_END {
        let alpha = animate_curve(TIME_INTRO_FADE_OUT, TIME_INTRO_FADE_OUT_END, time, 0, 255, TodCurve::Linear);
        g.set_color(SexyColor::rgba(0, 0, 0, alpha.clamp(0, 255) as u8));
        g.fill_rect(0.0, 0.0, BOARD_WIDTH as f32, super::BOARD_HEIGHT as f32);
    }
    g.pop_state();
}
