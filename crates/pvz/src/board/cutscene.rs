//! The level intro: the view starts on the house, pans right to the street
//! and back, then the seed bank slides in, the sod rolls out on the first
//! levels and the lawn mowers roll in before play starts.
//!
//! Times are in hundredths of a second (`mCutsceneTime` advances by 10 per
//! tick). Not yet ported: Crazy Dave, the seed chooser, the zombies waiting
//! on the street, grave stones, fog and the boss.
//!
//! Behaviour per ruslan831 decomp, `Lawn/CutScene.cpp`: `StartLevelIntro`,
//! `Update`, `AnimateBoard`, `PlaceLawnItems`, `ShowShovel`,
//! `IsAfterSeedChooser` and `CalcPosition`.

use bevy::prelude::*;
use sexy::tod::{TodCurve, animate_curve};

use crate::audio::{FoleyType, Music, MusicTune, Sounds};
use crate::particle::ParticleEffect;
use crate::reanim::{EffectSystem, ReanimationType};
use crate::resources::GameResources;

use super::message::MessageStyle;
use super::{
    BOARD_IMAGE_WIDTH_OFFSET, BOARD_OFFSET, BOARD_WIDTH, BackgroundType, Board, GameScene, SEED_BANK_OFFSET_X,
    SEED_BANK_OFFSET_X_END, make_render_order, render_layer,
};

const TIME_PAN_RIGHT_START: i32 = 1500;
const TIME_PAN_RIGHT_END: i32 = 3500;
const TIME_SEED_CHOOSER_SLIDE_ON_START: i32 = 4000;
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
const TIME_READY_SET_PLANT_END: i32 = 7830;
const TIME_INTRO_END: i32 = 6000;
const TIME_LAWN_MOWER_DURATION: i32 = 250;
const LOST_TIME_BRAIN_GRAPHIC_START: i32 = 6000;
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
    lawn_mower_time: i32,
    placed_lawn_items: bool,
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
    pub fn show_zombie_walking(&self) -> bool {
        self.cutscene_time > TIME_PAN_RIGHT_START
    }

    /// `CutScene::StartZombiesWon`.
    pub fn start_zombies_won(&mut self) {
        self.cutscene_time = 0;
    }

    /// `IsAfterSeedChooser`.
    pub fn is_after_seed_chooser(&self) -> bool {
        self.cutscene_time > TIME_SEED_CHOOSER_SLIDE_OFF_START + self.crazy_dave_time
    }

    /// `IsScrolledLeftAtStart`: every level ported so far starts on the house.
    fn is_scrolled_left_at_start(&self) -> bool {
        true
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

/// `CutScene::StartLevelIntro`.
pub fn start_level_intro(board: &mut Board, res: &GameResources, music: &mut Music, player_name: &str) {
    let seed_bank_height = res.image("IMAGE_SEEDBANK").map_or(0, |i| i.height as i32);
    board.seed_bank.x = SEED_BANK_OFFSET_X;
    board.seed_bank.y = -seed_bank_height;
    board.menu_button.no_draw = true;
    board.show_shovel = false;
    board.seed_bank.cut_scene_darken = 255;

    let level = board.level;
    let first_time = board.first_time_adventure;
    let cs = &mut board.cut_scene;
    *cs = CutScene::default();
    if first_time && matches!(level, 1 | 2 | 4) {
        cs.sod_time = TIME_ROLL_SOD_END - TIME_ROLL_SOD_START;
        board.sod_position = 0;
    } else {
        cs.sod_time = 0;
        board.sod_position = 1000;
    }
    // TODO: grave stones (TimeGraveStoneEnd - TimeGraveStoneStart on night levels).
    cs.grave_stone_time = 0;
    cs.ready_set_plant_time =
        if first_time && level <= 2 { 0 } else { TIME_READY_SET_PLANT_END - TIME_READY_SET_PLANT_START };
    cs.lawn_mower_time = 550;
    // TODO: Crazy Dave's talks (mCrazyDaveDialogStart), fog and the boss.
    cs.crazy_dave_time = 0;
    cs.fog_time = 0;
    cs.boss_time = 0;

    if cs.is_scrolled_left_at_start() {
        board.x = BOARD_OFFSET;
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
    // TODO: the seed chooser, which stops the clock while seeds are picked.
    board.cut_scene.cutscene_time += 10;

    if board.cut_scene.cutscene_time >= board.cut_scene.time_to_start() {
        // TODO: RemoveCutsceneZombies once street zombies exist.
        board.menu_button.no_draw = false;
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
    // TODO: the brains graphic shakes between 7000 and 8000 (SetShakeOverride).
    if time == LOST_TIME_BRAIN_GRAPHIC_END
        && let Some(reanim) = cs.zombies_won_reanim.and_then(|id| effects.get_mut(id))
    {
        reanim.set_frames_for_layer("anim_screen");
    }
    time == LOST_TIME_END
}

/// `CutScene::ShowShovel`: not until the shovel is introduced in 1-5.
fn show_shovel(board: &mut Board) {
    if !board.first_time_adventure || board.level > 4 {
        board.show_shovel = true;
    }
}

/// `CutScene::PlaceLawnItems`.
fn place_lawn_items(board: &mut Board) {
    if board.cut_scene.placed_lawn_items {
        return;
    }
    board.cut_scene.placed_lawn_items = true;
    board.init_lawn_mowers();
    // TODO: flower pots on the roof, and the rake.
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

    if time == pan_left_start {
        place_lawn_items(board);
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
        music.fade_out(150);
    }
    if cs.ready_set_plant_time == 0 && time == ready_set_plant - 2000 {
        music.fade_out(200);
    }
}
