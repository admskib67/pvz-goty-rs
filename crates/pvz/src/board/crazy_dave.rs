//! Crazy Dave: he walks in during some level intros, talks in a speech
//! bubble that the player clicks through, and leaves.
//!
//! Behaviour per ruslan831 decomp, `LawnApp.cpp`: `CrazyDaveEnter`,
//! `CrazyDaveDie`, `CrazyDaveLeave`, `CrazyDaveTalkIndex`,
//! `CrazyDaveDoneHanding`, `CrazyDaveStopSound`, `CrazyDaveTalkMessage`,
//! `AdvanceCrazyDaveText`, `GetCrazyDaveText`, `UpdateCrazyDave` and
//! `DrawCrazyDave`. Not yet: handing over fertilizer, tree food and the
//! money bag, and the store's and upsell's speech bubbles.

use bevy::prelude::*;
use sexy::rand::MtRand;
use sexy::reanim::{LoopType, RENDER_GROUP_HIDDEN, RENDER_GROUP_NORMAL};

use crate::audio::{FoleyType, Sounds};
use crate::graphics::{Graphics, Justify, SexyColor, SexyRect};
use crate::reanim::{EffectSystem, ReanimationId, ReanimationType};
use crate::resources::GameResources;

/// `CrazyDaveState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DaveState {
    #[default]
    Off,
    Entering,
    Idling,
    Talking,
    Leaving,
    HandingTalking,
    HandingIdling,
}

/// What Dave's messages fill in (`GetCrazyDaveText`).
#[derive(Debug, Clone, Default)]
pub struct DaveVars {
    /// `{PLAYER_NAME}`.
    pub name: String,
    /// `{MONEY}`: the player's coins.
    pub money: i32,
    /// `{UPGRADE_COST}`: the next seed slot's price, in coins.
    pub upgrade_cost: i32,
}

#[derive(Debug, Clone, Default)]
pub struct CrazyDave {
    pub state: DaveState,
    reanim: Option<ReanimationId>,
    blink_counter: i32,
    blink: Option<ReanimationId>,
    /// What he's holding out (the wall-nut), attached to his hand.
    held: Option<ReanimationId>,
    /// `mCrazyDaveMessageIndex`, or -1.
    pub message_index: i32,
    message_text: String,
}

impl CrazyDave {
    pub fn new() -> Self {
        Self { message_index: -1, ..Self::default() }
    }

    /// `LawnApp::CrazyDaveEnter`.
    pub fn enter(&mut self, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>, rng: &mut MtRand) {
        match effects.add_reanimation(0.0, 0.0, ReanimationType::CrazyDave, res, images) {
            Ok((id, dave)) => {
                dave.is_attachment = true;
                let (start, _) = dave.definition.frames_for_layer("anim_idle_handing");
                dave.frame_base_pose = start as i32;
                dave.play("anim_enter", LoopType::PlayOnceAndHold, 0, 24.0);
                self.reanim = Some(id);
            }
            Err(e) => {
                error!("{e}");
                return;
            }
        }
        self.state = DaveState::Entering;
        self.message_index = -1;
        self.message_text.clear();
        self.blink_counter = rng.range_int(400, 800);
    }

    /// `LawnApp::CrazyDaveDie`.
    pub fn die(&mut self, effects: &mut EffectSystem, sounds: &mut Sounds) {
        if let Some(id) = self.reanim.take() {
            effects.kill(id);
            if let Some(blink) = self.blink.take() {
                effects.kill(blink);
            }
            self.state = DaveState::Off;
            self.message_index = -1;
            self.message_text.clear();
            stop_sound(sounds);
        }
    }

    /// `LawnApp::CrazyDaveLeave`.
    pub fn leave(&mut self, effects: &mut EffectSystem, sounds: &mut Sounds) {
        let Some(id) = self.reanim else { return };
        if matches!(self.state, DaveState::HandingTalking | DaveState::HandingIdling) {
            self.done_handing(effects);
        }
        if let Some(dave) = effects.get_mut(id) {
            dave.play("anim_leave", LoopType::PlayOnceAndHold, 20, 24.0);
            dave.set_image_override("Dave_mouths", None);
        }
        self.state = DaveState::Leaving;
        self.message_index = -1;
        self.message_text.clear();
        stop_sound(sounds);
    }

    /// `LawnApp::CrazyDaveDoneHanding`: what he held out goes away.
    fn done_handing(&mut self, effects: &mut EffectSystem) {
        if let Some(held) = self.held.take() {
            effects.kill(held);
        }
    }

    /// `GetCrazyDaveText`; `None` when there's no such message.
    fn text(res: &GameResources, index: i32, vars: &DaveVars) -> Option<String> {
        let text = res.translate(&format!("[CRAZY_DAVE_{index}]"));
        (!text.starts_with("<Missing")).then(|| {
            text.replace("{PLAYER_NAME}", &vars.name)
                .replace("{MONEY}", &super::coin::money_string(vars.money))
                .replace("{UPGRADE_COST}", &super::coin::money_string(vars.upgrade_cost))
        })
    }

    /// The message he's saying now (with its control codes).
    pub fn message_text(&self) -> &str {
        &self.message_text
    }

    /// `LawnApp::CrazyDaveTalkIndex`.
    #[allow(clippy::too_many_arguments)]
    pub fn talk_index(
        &mut self,
        index: i32,
        vars: &DaveVars,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        self.message_index = index;
        let text = Self::text(res, index, vars).unwrap_or_default();
        self.talk_message(&text, effects, res, images, sounds);
    }

    /// `LawnApp::AdvanceCrazyDaveText`: the next message, if there is one.
    #[allow(clippy::too_many_arguments)]
    pub fn advance_text(
        &mut self,
        vars: &DaveVars,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) -> bool {
        if Self::text(res, self.message_index + 1, vars).is_none() {
            return false;
        }
        self.talk_index(self.message_index + 1, vars, effects, res, images, sounds);
        true
    }

    /// `LawnApp::CrazyDaveTalkMessage`.
    pub fn talk_message(
        &mut self,
        message: &str,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        let Some(id) = self.reanim else { return };
        let has = |code: &str| message.contains(code);
        let do_handing = has("{HANDING}");
        if matches!(self.state, DaveState::HandingTalking | DaveState::HandingIdling) && !do_handing {
            self.done_handing(effects);
        }
        let do_sound = !has("{NO_SOUND}");
        if do_sound {
            stop_sound(sounds);
        }
        // Words that aren't control codes.
        let mut words = 0;
        let mut in_code = false;
        for c in message.chars() {
            match c {
                '{' => in_code = true,
                '}' => in_code = false,
                _ if !in_code => words += 1,
                _ => {}
            }
        }
        if let Some(dave) = effects.get_mut(id) {
            dave.set_image_override("Dave_mouths", None);
        }
        if self.state != DaveState::Talking || do_sound {
            let play = |effects: &mut EffectSystem, track: &str, loop_type: LoopType| {
                if let Some(dave) = effects.get_mut(id) {
                    dave.play(track, loop_type, 50, 12.0);
                }
            };
            let foley: FoleyType;
            if do_handing {
                play(effects, "anim_talk_handing", LoopType::Loop);
                foley = if has("{SHORT_SOUND}") {
                    FoleyType::CrazyDaveShort
                } else if has("{SCREAM}") {
                    FoleyType::CrazyDaveScream
                } else {
                    FoleyType::CrazyDaveLong
                };
                self.state = DaveState::HandingTalking;
            } else if has("{SHAKE}") {
                play(effects, "anim_crazy", LoopType::PlayOnceAndHold);
                foley = FoleyType::CrazyDaveCrazy;
                self.state = DaveState::Talking;
            } else if has("{SCREAM}") {
                play(effects, "anim_smalltalk", LoopType::PlayOnceAndHold);
                foley = FoleyType::CrazyDaveScream;
                self.state = DaveState::Talking;
            } else if has("{SCREAM2}") {
                play(effects, "anim_mediumtalk", LoopType::PlayOnceAndHold);
                foley = FoleyType::CrazyDaveScream2;
                self.state = DaveState::Talking;
            } else if has("{SHOW_WALLNUT}") {
                play(effects, "anim_talk_handing", LoopType::Loop);
                self.hand_wallnut(id, effects, res, images);
                foley = FoleyType::CrazyDaveScream2;
                self.state = DaveState::HandingTalking;
            } else if has("{SHOW_HAMMER}") {
                play(effects, "anim_talk_handing", LoopType::Loop);
                self.hand_hammer(id, effects, res, images);
                foley = FoleyType::CrazyDaveScream2;
                self.state = DaveState::HandingTalking;
            } else {
                let (track, ty) = if words < 23 {
                    ("anim_smalltalk", FoleyType::CrazyDaveShort)
                } else if words < 52 {
                    ("anim_mediumtalk", FoleyType::CrazyDaveLong)
                } else {
                    ("anim_blahblah", FoleyType::CrazyDaveExtraLong)
                };
                play(effects, track, LoopType::PlayOnceAndHold);
                foley = ty;
                self.state = DaveState::Talking;
            }
            if do_sound {
                sounds.play_foley(foley);
            }
        }
        self.message_text = message.to_owned();
    }

    /// The wall-nut he holds out (`{SHOW_WALLNUT}`).
    fn hand_wallnut(&mut self, dave: ReanimationId, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>) {
        let Ok((wallnut, nut)) = effects.add_reanimation(0.0, 0.0, ReanimationType::Wallnut, res, images) else { return };
        nut.play("anim_idle", LoopType::Loop, 0, 12.0);
        let offset = sexy::matrix::Matrix3::translation(100.0, 393.0);
        nut.attach_offset = offset * sexy::matrix::Matrix3 { m00: 1.2, m11: 1.2, ..sexy::matrix::Matrix3::IDENTITY };
        effects.attach_to_another(wallnut, dave, "Dave_handinghand");
        self.held = Some(wallnut);
        effects.update_reanimation(dave);
    }

    /// The mallet he holds out for whack-a-zombie (`{SHOW_HAMMER}`).
    fn hand_hammer(&mut self, dave: ReanimationId, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>) {
        let Ok((hammer, mallet)) = effects.add_reanimation(0.0, 0.0, ReanimationType::Hammer, res, images) else { return };
        mallet.play("anim_whack_zombie", LoopType::PlayOnceAndHold, 0, 24.0);
        mallet.anim_time = 1.0;
        let offset = sexy::matrix::Matrix3::translation(62.0, 445.0);
        mallet.attach_offset = offset * sexy::matrix::Matrix3 { m00: 1.5, m11: 1.5, ..sexy::matrix::Matrix3::IDENTITY };
        effects.attach_to_another(hammer, dave, "Dave_handinghand");
        self.held = Some(hammer);
        effects.update_reanimation(dave);
    }

    /// `LawnApp::UpdateCrazyDave`.
    pub fn update(&mut self, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>, sounds: &mut Sounds) {
        let Some(id) = self.reanim else { return };
        let Some(dave) = effects.get_mut(id) else { return };
        let looped = dave.loop_count > 0;
        match self.state {
            DaveState::Entering | DaveState::Talking if looped => {
                dave.play("anim_idle", LoopType::Loop, 20, 12.0);
                self.state = DaveState::Idling;
            }
            DaveState::HandingTalking if looped => {
                dave.play("anim_idle_handing", LoopType::Loop, 20, 12.0);
                self.state = DaveState::HandingIdling;
            }
            DaveState::Leaving if looped => {
                self.die(effects, sounds);
                return;
            }
            _ => {}
        }

        if matches!(self.state, DaveState::Idling | DaveState::HandingIdling) {
            let mouth = [
                ("{MOUTH_BIG_SMILE}", "IMAGE_REANIM_CRAZYDAVE_MOUTH1"),
                ("{MOUTH_SMALL_SMILE}", "IMAGE_REANIM_CRAZYDAVE_MOUTH5"),
                ("{MOUTH_BIG_OH}", "IMAGE_REANIM_CRAZYDAVE_MOUTH4"),
                ("{MOUTH_SMALL_OH}", "IMAGE_REANIM_CRAZYDAVE_MOUTH6"),
            ]
            .into_iter()
            .find(|(code, _)| self.message_text.contains(code));
            if let (Some((_, image)), Some(dave)) = (mouth, effects.get_mut(id)) {
                dave.set_image_override("Dave_mouths", Some(image));
            }
        }

        // Blinking.
        if matches!(
            self.state,
            DaveState::Idling | DaveState::Talking | DaveState::HandingTalking | DaveState::HandingIdling
        ) {
            self.blink_counter -= 1;
            if self.blink_counter <= 0 {
                self.blink_counter = sounds.rng().range_int(400, 800);
                if let Ok((blink_id, blink)) = effects.add_reanimation(0.0, 0.0, ReanimationType::CrazyDave, res, images) {
                    blink.set_frames_for_layer("anim_blink");
                    blink.loop_type = LoopType::PlayOnceFullLastFrameAndHold;
                    blink.anim_rate = 15.0;
                    effects.attach_to_another(blink_id, id, "Dave_head");
                    if let Some(dave) = effects.get_mut(id) {
                        dave.assign_render_group_to_track("Dave_eye", RENDER_GROUP_HIDDEN);
                    }
                    self.blink = Some(blink_id);
                }
            }
        }
        if let Some(blink) = self.blink
            && effects.get(blink).is_none_or(|b| b.loop_count > 0)
        {
            if let Some(dave) = effects.get_mut(id) {
                dave.assign_render_group_to_track("Dave_eye", RENDER_GROUP_NORMAL);
            }
            effects.kill(blink);
            self.blink = None;
        }
        effects.update_reanimation(id);
    }

    /// `LawnApp::DrawCrazyDave`, in screen coordinates.
    pub fn draw(&self, effects: &EffectSystem, res: &GameResources, g: &mut Graphics) {
        let Some(id) = self.reanim else { return };
        if !self.message_text.is_empty() {
            let (x, y) = (285.0, 20.0);
            if let Some(bubble) = res.image("IMAGE_STORE_SPEECHBUBBLE2") {
                g.draw_image(bubble, x, y);
            }
            let mut text = self.message_text.clone();
            let mut rect = SexyRect::new(x + 25.0, y + 6.0, 233.0, 144.0);
            if text.contains("{SHAKE}") {
                rect.x += rand_bit();
                rect.y += rand_bit();
            }
            let click_to_continue = !text.contains("{NO_CLICK}");
            // Control codes aren't shown.
            while let (Some(open), Some(close)) = (text.find('{'), text.find('}')) {
                if close < open {
                    break;
                }
                text.replace_range(open..=close, "");
            }
            if let Some(font) = res.font("FONT_BRIANNETOD16") {
                g.tod_draw_string_wrapped(font, rect, &text, SexyColor::BLACK, Justify::CenterMiddle);
            }
            if click_to_continue && let Some(font) = res.font("FONT_PICO129") {
                g.tod_draw_string(font, (x + 139.0) as i32, (y + 140.0) as i32, "click to continue", SexyColor::BLACK, Justify::Center);
            }
        }
        effects.draw(g, id, res);
    }
}

/// `rand() % 2`, for the shaking bubble.
fn rand_bit() -> f32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEED: AtomicU32 = AtomicU32::new(12345);
    let next = SEED.load(Ordering::Relaxed).wrapping_mul(1_103_515_245).wrapping_add(12345);
    SEED.store(next, Ordering::Relaxed);
    ((next >> 16) & 1) as f32
}

/// `LawnApp::CrazyDaveStopSound`.
fn stop_sound(sounds: &mut Sounds) {
    for foley in [FoleyType::CrazyDaveShort, FoleyType::CrazyDaveLong, FoleyType::CrazyDaveExtraLong, FoleyType::CrazyDaveCrazy] {
        sounds.stop_foley(foley);
    }
}
