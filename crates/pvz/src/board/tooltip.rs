//! The yellow tooltip box (`ToolTipWidget`): a bold title, an optional red
//! warning and a few lines of text, as the seed chooser shows for a plant.
//!
//! Behaviour per ruslan831 decomp, `Lawn/ToolTipWidget.cpp`: `GetLines`,
//! `CalculateSize` and `Draw`.

use crate::graphics::{Graphics, Justify, SexyColor};
use crate::resources::{FontRes, GameResources};

const TITLE_FONT: &str = "FONT_TINYBOLD";
const TEXT_FONT: &str = "FONT_PICO129";

#[derive(Debug, Clone)]
pub struct ToolTip {
    title: String,
    label: String,
    warning: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// How wide a line gets before wrapping (`mGetsLinesWidth`).
    lines_width: i32,
}

impl ToolTip {
    /// A tooltip with its size worked out (`SetTitle`/`SetLabel`/`SetWarningText`
    /// + `CalculateSize`). The texts are `[KEYS]` or plain text.
    pub fn new(res: &GameResources, title: &str, label: &str, warning: &str) -> Option<Self> {
        let (title_font, text_font) = (res.font(TITLE_FONT)?, res.font(TEXT_FONT)?);
        let translate = |s: &str| if s.is_empty() { String::new() } else { res.translate(s) };
        let mut tip = Self {
            title: translate(title),
            label: translate(label),
            warning: translate(warning),
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            lines_width: 0,
        };
        let mut max_width = title_font.font.string_width(&tip.title).max(text_font.font.string_width(&tip.warning));
        tip.lines_width = (max_width - 30).max(100);
        let lines = tip.lines(text_font);
        for line in &lines {
            max_width = max_width.max(text_font.font.string_width(line));
        }
        let mut height = if tip.title.is_empty() { 6 } else { title_font.font.ascent + 8 };
        if !tip.warning.is_empty() {
            height += title_font.font.ascent + 2;
        }
        height += lines.len() as i32 * text_font.font.ascent;
        tip.width = max_width + 10;
        tip.height = height + lines.len() as i32 * 2 - 2;
        Some(tip)
    }

    /// `ToolTipWidget::GetLines`: the label broken into lines a bit narrower
    /// than the title, at spaces (or where it says to).
    fn lines(&self, font: &FontRes) -> Vec<String> {
        let chars: Vec<char> = self.label.chars().collect();
        let mut lines = Vec::new();
        let (mut start, mut i, mut width) = (0, 0, 0);
        while i != chars.len() {
            while i < chars.len() && chars[i] != ' ' && chars[i] != '\n' {
                width += font.font.char_width_kern(chars[i], None);
                i += 1;
            }
            if i != chars.len() && width < self.lines_width && chars[i] != '\n' {
                width += font.font.char_width_kern(chars[i], None);
                i += 1;
            } else {
                lines.push(chars[start..i].iter().collect());
                width = 0;
                if i < chars.len() && chars[i] == '\n' {
                    i += 1;
                }
                while i < chars.len() && chars[i] == ' ' {
                    i += 1;
                }
                start = i;
            }
        }
        lines
    }

    /// `ToolTipWidget::Draw`: kept on the screen.
    pub fn draw(&self, g: &mut Graphics, res: &GameResources) {
        let (Some(title_font), Some(text_font)) = (res.font(TITLE_FONT), res.font(TEXT_FONT)) else { return };
        let t = g.translation();
        let x = self.x.clamp(-t.x as i32, (800.0 - t.x) as i32 - self.width);
        let mut y = self.y.clamp(-t.y as i32, (600.0 - t.y) as i32 - self.height);
        g.push_state();
        g.set_color(SexyColor::rgba(255, 255, 200, 255));
        g.fill_rect(x as f32, y as f32, self.width as f32, self.height as f32);
        g.set_color(SexyColor::BLACK);
        g.draw_rect(x as f32, y as f32, (self.width - 1) as f32, (self.height - 1) as f32);
        g.pop_state();
        y += 1;
        let centered = |font: &FontRes, text: &str| x + (self.width - font.font.string_width(text)) / 2;
        if !self.title.is_empty() {
            let ascent = title_font.font.ascent;
            g.tod_draw_string(title_font, centered(title_font, &self.title), y + ascent, &self.title, SexyColor::BLACK, Justify::Left);
            y += ascent + 2;
        }
        if !self.warning.is_empty() {
            let ascent = text_font.font.ascent;
            let red = SexyColor::rgba(255, 0, 0, 255);
            g.tod_draw_string(text_font, centered(text_font, &self.warning), y + ascent, &self.warning, red, Justify::Left);
            y += ascent + 2;
        }
        for line in self.lines(text_font) {
            let ascent = text_font.font.ascent;
            g.tod_draw_string(text_font, centered(text_font, &line), y + ascent, &line, SexyColor::BLACK, Justify::Left);
            y += ascent + 2;
        }
    }
}
