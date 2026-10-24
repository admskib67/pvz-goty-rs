//! The board's text banner (`MessageWidget`): the "<name>'s House" title in
//! the level intro, tutorial hints and warnings.
//!
//! Behaviour per ruslan831 decomp, `Lawn/MessageWidget.cpp`. Not yet ported:
//! the letter-by-letter animated styles (the huge wave warning).

use sexy::tod::{TodCurve, animate_curve};

use crate::graphics::{Graphics, Justify, SexyColor, SexyRect};
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
    /// "A huge wave of zombies is approaching!". TODO: the original lays
    /// the letters out as separate animations (REANIM_TEXT_FADE_ON).
    HugeWave,
    SlotMachine,
    ZenGardenLong,
}

#[derive(Debug, Clone)]
pub struct MessageWidget {
    label: String,
    style: MessageStyle,
    /// Ticks left on screen; 10000 or more stays until cleared.
    duration: i32,
}

impl Default for MessageWidget {
    fn default() -> Self {
        Self { label: String::new(), style: MessageStyle::Off, duration: 0 }
    }
}

impl MessageWidget {
    /// `SetLabel`. `text` is already translated.
    pub fn set_label(&mut self, text: String, style: MessageStyle) {
        use MessageStyle::*;
        self.label = text;
        self.style = style;
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
    }

    /// `ClearLabel`.
    pub fn clear_label(&mut self) {
        self.duration = 0;
    }

    /// `MessageWidget::Update`.
    pub fn update(&mut self) {
        if self.duration > 0 && self.duration < 10000 {
            self.duration -= 1;
            if self.duration == 0 {
                self.style = MessageStyle::Off;
            }
        }
    }

    /// `MessageWidget::Draw`.
    pub fn draw(&self, board: &Board, res: &GameResources, g: &mut Graphics) {
        use MessageStyle::*;
        if self.duration <= 0 {
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
}
