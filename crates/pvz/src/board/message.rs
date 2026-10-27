//! The board's text banner (`MessageWidget`): the "<name>'s House" title in
//! the level intro, tutorial hints and warnings, and the huge wave warning
//! (one `TextFadeOn` animation per letter, `MESSAGE_STYLE_HUGE_WAVE`).
//!
//! Behaviour per ruslan831 decomp, `Lawn/MessageWidget.cpp`.

use bevy::prelude::*;
use sexy::data_array::Id;
use sexy::reanim::LoopType;
use sexy::tod::{TodCurve, animate_curve, animate_curve_float, float_round_to_int};

use crate::graphics::{Graphics, Justify, SexyColor, SexyRect};
use crate::reanim::{EffectSystem, Reanimation, ReanimationType, to_affine};
use crate::resources::GameResources;

use super::{BOARD_HEIGHT, BOARD_WIDTH, Board};

/// `MessageStyle`.
#[allow(dead_code, reason = "the full set; the board uses more of it as it's ported")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageStyle {
    Off,
    TutorialLevel1,
    TutorialLevel1Stay,
    TutorialLevel2,
    TutorialLater,
    TutorialLaterStay,
    HintLong,
    HintFast,
    HintStay,
    HintTallFast,
    HintTallUnlockMessage,
    HintTallLong,
    BigMiddle,
    BigMiddleFast,
    HouseName,
    /// "A huge wave of zombies is approaching!": each letter is its own
    /// `TextFadeOn` animation (`LayoutReanimText`).
    HugeWave,
    SlotMachine,
    ZenGardenLong,
}

/// One letter of an animated label and where it sits from the screen centre.
#[derive(Debug, Clone, Copy)]
struct LetterText {
    reanim: Id<Reanimation>,
    x: f32,
    y: f32,
}

#[derive(Debug, Clone)]
pub struct MessageWidget {
    label: String,
    style: MessageStyle,
    /// Ticks left on screen; 10000 or more stays until cleared.
    duration: i32,
    /// `mDisplayTime`: the duration the label started with.
    display_time: i32,
    /// `mSlideOffTime`: when the letters start sliding away (label length + 100).
    slide_off_time: i32,
    /// `mReanimType`: what the letters animate with (only the huge wave has one).
    reanim_type: Option<ReanimationType>,
    /// The letters laid out for the current label.
    letters: Vec<LetterText>,
    /// The label changed: lay the letters out on the next update.
    layout_pending: bool,
}

impl Default for MessageWidget {
    fn default() -> Self {
        Self {
            label: String::new(),
            style: MessageStyle::Off,
            duration: 0,
            display_time: 0,
            slide_off_time: 0,
            reanim_type: None,
            letters: Vec::new(),
            layout_pending: false,
        }
    }
}

impl MessageWidget {
    /// `SetLabel`. `text` is already translated.
    pub fn set_label(&mut self, text: String, style: MessageStyle) {
        use MessageStyle::*;
        self.label = text;
        self.style = style;
        self.reanim_type = (style == HugeWave).then_some(ReanimationType::TextFadeOn);
        self.slide_off_time = self.label.chars().count() as i32 + 100;
        self.layout_pending = self.reanim_type.is_some();
        self.duration = match style {
            HintLong | BigMiddle | ZenGardenLong | HintTallLong => 1500,
            HintTallUnlockMessage => 500,
            HintFast | HintTallFast | BigMiddleFast | TutorialLevel1 | TutorialLevel2 | TutorialLater => 500,
            HintStay | TutorialLevel1Stay | TutorialLaterStay => 10000,
            HouseName => 250,
            HugeWave => 750,
            SlotMachine => 750,
            Off => 0,
        };
        self.display_time = self.duration;
    }

    /// `ClearLabel`.
    pub fn clear_label(&mut self) {
        self.duration = 0;
    }

    /// How many animated letters the banner has laid out (tests).
    #[cfg(test)]
    pub(super) fn letters_len(&self) -> usize {
        self.letters.len()
    }

    /// The first animated letter's current alpha (tests).
    #[cfg(test)]
    pub(super) fn first_letter_alpha(&self, effects: &EffectSystem) -> Option<f32> {
        self.letters.first().and_then(|l| effects.get(l.reanim)).map(|r| r.current_transform(2).a)
    }

    /// `MessageWidget::Update`: ticks the banner and its letters (which are
    /// attachments, so the effect system leaves them alone).
    pub fn update(&mut self, effects: &mut EffectSystem, resources: &mut GameResources, images: &mut Assets<Image>) {
        if self.layout_pending {
            self.layout_pending = false;
            for letter in self.letters.drain(..) {
                effects.kill(letter.reanim);
            }
            if let Some(ty) = self.reanim_type {
                self.layout_text(effects, resources, images, ty);
            }
        }
        if self.duration > 0 && self.duration < 10000 {
            self.duration -= 1;
            if self.duration == 0 {
                self.style = MessageStyle::Off;
            }
        }
        if self.duration <= 0 || self.letters.is_empty() {
            return;
        }
        let fading_on = self.reanim_type == Some(ReanimationType::TextFadeOn);
        let text_speed = if fading_on { 100 } else { 1 };
        for (pos, letter) in self.letters.iter().enumerate() {
            let Some(reanim) = effects.get_mut(letter.reanim) else { continue };
            let rate = if self.duration > self.slide_off_time {
                if fading_on {
                    60.0
                } else {
                    animate_curve_float(0, 50, (self.display_time - self.duration) * text_speed - pos as i32, 0.0, 40.0, TodCurve::Linear)
                }
            } else {
                if self.duration == self.slide_off_time {
                    reanim.play("anim_leave", LoopType::PlayOnceAndHold, 0, 0.0);
                }
                animate_curve_float(0, 50, (self.slide_off_time - self.duration) * text_speed - pos as i32, 0.0, 40.0, TodCurve::Linear)
            };
            reanim.anim_rate = rate;
            effects.update_reanimation(letter.reanim);
        }
    }

    /// `LayoutReanimText`: one `TextFadeOn` animation per letter, centred on
    /// the screen (its overlay is the letter's place in the line).
    fn layout_text(
        &mut self,
        effects: &mut EffectSystem,
        resources: &mut GameResources,
        images: &mut Assets<Image>,
        ty: ReanimationType,
    ) {
        let font_id = if self.style == MessageStyle::SlotMachine { "FONT_HOUSEOFTERROR16" } else { "FONT_HOUSEOFTERROR28" };
        let (line_widths, line_spacing, advances) = {
            let Some(font) = resources.font(font_id) else { return };
            let lines: Vec<&str> = self.label.split('\n').collect();
            let widths: Vec<f32> = lines.iter().map(|line| font.font.string_width(line) as f32).collect();
            let advances: Vec<f32> = self.label.chars().map(|c| font.font.char_width_kern(c, None) as f32).collect();
            (widths, font.font.line_spacing() as f32, advances)
        };
        let widths = line_widths;
        let mut line = 0;
        let mut x = -widths.first().copied().unwrap_or(0.0) * 0.5;
        let mut y = 0.0;
        for (pos, ch) in self.label.chars().enumerate() {
            if ch == '\n' {
                line += 1;
                x = -widths.get(line).copied().unwrap_or(0.0) * 0.5;
                y += line_spacing;
                continue;
            }
            let Ok((id, reanim)) = effects.add_reanimation(x, y, ty, resources, images) else { break };
            reanim.is_attachment = true;
            reanim.play("anim_enter", LoopType::PlayOnceAndHold, 0, 0.0);
            self.letters.push(LetterText { reanim: id, x, y });
            x += advances.get(pos).copied().unwrap_or(0.0);
        }
    }

    /// `MessageWidget::Draw`.
    pub fn draw(&self, board: &Board, res: &GameResources, g: &mut Graphics, effects: &EffectSystem) {
        use MessageStyle::*;
        if self.duration <= 0 {
            return;
        }
        if self.reanim_type.is_some() {
            self.draw_reanimated_text(res, g, effects);
            return;
        }
        let font_id = if self.style == SlotMachine { "FONT_HOUSEOFTERROR16" } else { "FONT_HOUSEOFTERROR28" };
        let Some(font) = res.font(font_id) else { return };
        let hint = SexyColor::rgba(253, 245, 173, 255);
        // (x, y, background height, text offset, colour, minimum alpha, fades out)
        let (x, y, rect_height, text_offset, mut color, min_alpha, fade_out) = match self.style {
            TutorialLevel1 | TutorialLevel1Stay => (400, 400, 110, -4, hint, 192, false),
            TutorialLevel2 | TutorialLater | TutorialLaterStay | HintTallFast | HintTallUnlockMessage | HintTallLong => {
                (400, 476, 100, -4, hint, 192, false)
            }
            HintLong | HintFast | HintStay => (400, 527, 55, -4, hint, 192, false),
            BigMiddle | BigMiddleFast => (400, 300, 110, 0, hint, 192, false),
            HouseName => (400, 550, 0, 0, SexyColor::WHITE, 255, true),
            HugeWave => (400, 330, 0, 0, SexyColor::rgba(255, 0, 0, 255), 255, false),
            SlotMachine => (340, 93, 0, 0, SexyColor::rgba(250, 250, 0, 255), 64, false),
            ZenGardenLong => (400, 514, 55, -4, hint, 192, false),
            Off => return,
        };

        if min_alpha != 255 {
            let alpha = animate_curve(75, 0, board.main_counter % 75, min_alpha, 255, TodCurve::BounceSlowMiddle);
            color = color.with_alpha(alpha);
        }
        if fade_out {
            color = color.with_alpha(self.duration * 15);
        }

        if rect_height > 0 {
            g.push_state();
            g.set_color(SexyColor::rgba(0, 0, 0, 128));
            g.fill_rect(0.0, y as f32, BOARD_WIDTH as f32, rect_height as f32);
            g.pop_state();
            let rect = SexyRect::new(0.0, (y + text_offset) as f32, BOARD_WIDTH as f32, rect_height as f32);
            g.tod_draw_string_wrapped(font, rect, &self.label, color, Justify::CenterMiddle);
        } else {
            // Positioned on the screen, not the (possibly panning) board.
            let left = x - board.x - BOARD_WIDTH / 2;
            let rect = SexyRect::new(left as f32, (y - font.font.ascent) as f32, BOARD_WIDTH as f32, BOARD_HEIGHT as f32);
            g.tod_draw_string_wrapped(font, rect, &self.label, color, Justify::Center);
        }
    }

    /// `DrawReanimatedText`: each letter through its `TextFadeOn` transform
    /// (track 2), with the fade-on stretch the original applies while the
    /// letters are still assembling.
    fn draw_reanimated_text(&self, res: &GameResources, g: &mut Graphics, effects: &EffectSystem) {
        let Some(font) = res.font("FONT_HOUSEOFTERROR28") else { return };
        let color = match self.style {
            MessageStyle::HugeWave => SexyColor::rgba(255, 0, 0, 255),
            MessageStyle::SlotMachine => SexyColor::rgba(250, 250, 0, 255),
            _ => SexyColor::WHITE,
        };
        let pos_y = if self.style == MessageStyle::HugeWave { 330.0 } else { 596.0 };
        for (ch, letter) in self.label.chars().zip(&self.letters) {
            let Some(reanim) = effects.get(letter.reanim) else { break };
            let mut transform = reanim.current_transform(2);
            let alpha = float_round_to_int(color.a as f32 * transform.a).clamp(0, 255);
            if alpha <= 0 {
                break;
            }
            transform.x += letter.x;
            transform.y += letter.y + pos_y - BOARD_HEIGHT as f32 / 2.0;
            if self.reanim_type == Some(ReanimationType::TextFadeOn)
                && self.display_time - self.duration < self.slide_off_time
            {
                transform.x += (1.0 - reanim.anim_time) * letter.x;
            }
            g.push_state();
            g.set_color(color.with_alpha(alpha));
            g.draw_string_matrix(font, to_affine(transform.matrix()), &ch.to_string());
            g.pop_state();
        }
    }
}
