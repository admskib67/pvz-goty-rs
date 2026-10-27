//! Buttons, text boxes and lists, and the bit of SexyFramework's
//! `WidgetManager` the screens need: hover, press and click, with the topmost
//! widget winning.
//!
//! Behaviour per ruslan831 decomp: `NewLawnButton`, `LawnStoneButton` and
//! `DrawStoneButton` (`Lawn/Widget/GameButton.cpp`), `ButtonWidget`'s mouse
//! handling, `EditWidget`/`LawnEditWidget`, `ListWidget`, `Slider` and
//! `Checkbox` (`SexyAppFramework/Slider.cpp`, `Checkbox.cpp`).

use bevy::prelude::*;
use sexy::font::Font;

use crate::graphics::{Graphics, SexyColor, SexyRect};
use crate::input::GameCursor;
use crate::resources::{FontRes, GameResources};

/// A `NewLawnButton` without a label.
#[derive(Debug, Clone)]
pub struct GameButton<Id> {
    pub id: Id,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub button_image: String,
    pub over_image: String,
    pub down_image: String,
    pub button_offset: (i32, i32),
    /// How far the image shifts while held down.
    pub translate: (i32, i32),
    /// Tint (`COLOR_BKG`), e.g. grey for locked modes.
    pub color: SexyColor,
    pub btn_no_draw: bool,
    pub mouse_visible: bool,
    pub visible: bool,
    pub disabled: bool,
    /// Hit area in local coordinates, instead of the bounding box.
    pub polygon: Option<[Vec2; 4]>,
    pub is_over: bool,
    pub is_down: bool,
}

impl<Id> GameButton<Id> {
    /// `MakeNewButton`: sized to its normal image.
    pub fn new(id: Id, resources: &GameResources, normal: &str, over: &str, down: &str) -> Self {
        let (width, height) = resources.image(normal).map_or((0, 0), |i| (i.width as i32, i.height as i32));
        Self {
            id,
            x: 0,
            y: 0,
            width,
            height,
            button_image: normal.to_owned(),
            over_image: over.to_owned(),
            down_image: down.to_owned(),
            button_offset: (0, 0),
            translate: (1, 1),
            color: SexyColor::WHITE,
            btn_no_draw: false,
            mouse_visible: true,
            visible: true,
            disabled: false,
            polygon: None,
            is_over: false,
            is_down: false,
        }
    }

    pub fn resize(&mut self, x: i32, y: i32, width: i32, height: i32) {
        (self.x, self.y, self.width, self.height) = (x, y, width, height);
    }

    fn hit(&self, p: Vec2) -> bool {
        let local = p - Vec2::new(self.x as f32, self.y as f32);
        if local.x < 0.0 || local.y < 0.0 || local.x >= self.width as f32 || local.y >= self.height as f32 {
            return false;
        }
        match &self.polygon {
            Some(poly) => point_in_polygon(poly, local),
            None => true,
        }
    }

    /// `NewLawnButton::Draw` (image part).
    pub fn draw(&self, g: &mut Graphics, resources: &GameResources) {
        if self.btn_no_draw || !self.visible {
            return;
        }
        let is_down = self.is_down && self.is_over && !self.disabled;
        let (image, (dx, dy)) = if is_down {
            (&self.down_image, (self.button_offset.0 + self.translate.0, self.button_offset.1 + self.translate.1))
        } else if self.is_over || self.is_down {
            (&self.over_image, self.button_offset)
        } else {
            (&self.button_image, self.button_offset)
        };
        let Some(image) = resources.image(image) else { return };
        g.push_state();
        g.set_colorize_images(true);
        g.set_color(self.color);
        g.draw_image(image, (self.x + dx) as f32, (self.y + dy) as f32);
        g.pop_state();
    }
}

/// `TodIsPointInPolygon` for a convex, clockwise (on screen) polygon.
fn point_in_polygon(poly: &[Vec2; 4], p: Vec2) -> bool {
    (0..poly.len()).all(|i| {
        let (cur, next) = (poly[i], poly[(i + 1) % poly.len()]);
        // SexyVector2::Perp is (-y, x).
        let edge = next - cur;
        Vec2::new(-edge.y, edge.x).dot(p - cur) >= 0.0
    })
}

/// What a button did this frame (`ButtonListener` callbacks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonEvent<Id> {
    MouseEnter(Id),
    Press(Id),
    /// Released over the button: a click.
    Depress(Id),
}

/// Result of routing the mouse through a set of buttons.
pub struct MouseResult<Id> {
    pub events: Vec<ButtonEvent<Id>>,
    /// Pressed where no button is: the screen behind gets `MouseDown`.
    pub background_press: Option<Vec2>,
}

/// Routes the mouse through `buttons`, given front to back.
pub fn update_buttons<Id: Copy>(
    buttons: &mut [&mut GameButton<Id>],
    cursor: &GameCursor,
    mouse: &ButtonInput<MouseButton>,
) -> MouseResult<Id> {
    let mut events = Vec::new();
    let hovered = cursor.0.and_then(|p| {
        buttons.iter().position(|b| b.visible && b.mouse_visible && !b.disabled && b.hit(p))
    });

    for (i, b) in buttons.iter_mut().enumerate() {
        let over = hovered == Some(i);
        if over && !b.is_over {
            events.push(ButtonEvent::MouseEnter(b.id));
        }
        b.is_over = over;
    }

    let mut background_press = None;
    if mouse.just_pressed(MouseButton::Left) {
        match hovered {
            Some(i) => {
                buttons[i].is_down = true;
                events.push(ButtonEvent::Press(buttons[i].id));
            }
            None => background_press = cursor.0,
        }
    }
    if mouse.just_released(MouseButton::Left) {
        for b in buttons.iter_mut().filter(|b| b.is_down) {
            b.is_down = false;
            if b.is_over {
                events.push(ButtonEvent::Depress(b.id));
            }
        }
    }
    MouseResult { events, background_press }
}

/// A dialog's labelled button: a `LawnStoneButton` (`MakeButton`), a stone
/// slab stretched to its width with a centered green label, or, with an
/// [`ImageLook`], a labelled `NewLawnButton` (`MakeNewButton`). Coordinates
/// are relative to its parent.
#[derive(Debug, Clone)]
pub struct StoneButton<Id> {
    pub id: Id,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub label: String,
    pub disabled: bool,
    /// Hidden buttons are neither drawn nor clickable (`SetVisible(false)`).
    pub visible: bool,
    /// Drawn as a `NewLawnButton` instead of a stone slab.
    pub image_look: Option<Box<ImageLook>>,
    pub is_over: bool,
    pub is_down: bool,
}

/// How a labelled `NewLawnButton` looks: its images and label fonts.
#[derive(Debug, Clone)]
pub struct ImageLook {
    pub normal: &'static str,
    pub over: &'static str,
    pub down: &'static str,
    pub font: &'static str,
    pub hilite_font: &'static str,
    /// `mTranslateX/Y`: how far the image moves while held down.
    pub translate: (i32, i32),
    pub text_offset: (i32, i32),
    pub text_down_offset: (i32, i32),
}

impl<Id> StoneButton<Id> {
    /// `MakeButton`. `label` is already translated.
    pub fn new(id: Id, label: String) -> Self {
        Self {
            id,
            x: 0,
            y: 0,
            width: 0,
            height: 33,
            label,
            disabled: false,
            visible: true,
            image_look: None,
            is_over: false,
            is_down: false,
        }
    }

    /// `MakeNewButton`: sized to its normal image.
    pub fn new_image(id: Id, label: String, look: ImageLook, resources: &GameResources) -> Self {
        let (width, height) = resources.image(look.normal).map_or((0, 0), |i| (i.width as i32, i.height as i32));
        Self { width, height, image_look: Some(Box::new(look)), ..Self::new(id, label) }
    }

    pub fn resize(&mut self, x: i32, y: i32, width: i32, height: i32) {
        (self.x, self.y, self.width, self.height) = (x, y, width, height);
    }

    pub fn contains(&self, p: IVec2) -> bool {
        self.visible && p.x >= self.x && p.y >= self.y && p.x < self.x + self.width && p.y < self.y + self.height
    }

    /// `LawnStoneButton::Draw`, or `NewLawnButton::Draw` with an image look.
    pub fn draw(&self, g: &mut Graphics, resources: &GameResources) {
        if !self.visible {
            return;
        }
        let is_down = self.is_down && self.is_over && !self.disabled;
        match &self.image_look {
            None => {
                draw_stone_button(g, resources, self.x, self.y, self.width, self.height, is_down, self.is_over, &self.label)
            }
            Some(look) => self.draw_image_look(g, resources, look, is_down),
        }
    }

    /// `NewLawnButton::Draw` with a centered label.
    fn draw_image_look(&self, g: &mut Graphics, resources: &GameResources, look: &ImageLook, is_down: bool) {
        let (image, offset) = if is_down {
            (look.down, look.translate)
        } else if self.is_over || self.is_down {
            (look.over, (0, 0))
        } else {
            (look.normal, (0, 0))
        };
        if let Some(image) = resources.image(image) {
            g.draw_image(image, (self.x + offset.0) as f32, (self.y + offset.1) as f32);
        }
        let Some(metrics) = resources.font(look.font) else { return };
        let ascent = metrics.font.ascent;
        let mut font_x = self.x + look.text_offset.0 + look.translate.0;
        let mut font_y = self.y + look.text_offset.1 + look.translate.1;
        font_x += (self.width - metrics.font.string_width(&self.label)) / 2;
        font_y += (self.height - ascent / 6 + ascent - 1) / 2;
        if is_down {
            font_x += look.text_down_offset.0;
            font_y += look.text_down_offset.1;
        }
        let font = if is_down || self.is_over { resources.font(look.hilite_font).unwrap_or(metrics) } else { metrics };
        g.push_state();
        g.set_color(SexyColor::WHITE);
        g.draw_string(font, font_x, font_y, &self.label);
        g.pop_state();
    }
}

/// A labelled Lawn `GameButton` (the almanac's and award screen's): an
/// image, a brighter one while the mouse is over it, an optional glow on
/// top, and a centered label. Coordinates are relative to its parent.
#[derive(Debug, Clone)]
pub struct LabelButton {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub label: String,
    pub font: &'static str,
    pub color: SexyColor,
    pub hilite_color: SexyColor,
    pub image: &'static str,
    pub over_image: Option<&'static str>,
    /// `mDownImage`, when it differs from the over image.
    pub down_image: Option<&'static str>,
    /// `mOverOverlayImage`: drawn over the button while the mouse is on it.
    pub over_overlay: Option<&'static str>,
    pub text_offset: (i32, i32),
    /// `!mBtnNoDraw`: hidden buttons are neither drawn nor clickable.
    pub visible: bool,
    pub is_over: bool,
    pub is_down: bool,
}

impl LabelButton {
    /// `label` is already translated.
    pub fn new(label: String, font: &'static str, color: SexyColor, image: &'static str) -> Self {
        Self {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            label,
            font,
            color,
            hilite_color: color,
            image,
            over_image: None,
            down_image: None,
            over_overlay: None,
            text_offset: (0, 0),
            visible: true,
            is_over: false,
            is_down: false,
        }
    }

    pub fn resize(&mut self, x: i32, y: i32, width: i32, height: i32) {
        (self.x, self.y, self.width, self.height) = (x, y, width, height);
    }

    pub fn contains(&self, p: IVec2) -> bool {
        self.visible && p.x >= self.x && p.y >= self.y && p.x < self.x + self.width && p.y < self.y + self.height
    }

    /// `GameButton::Draw`.
    pub fn draw(&self, g: &mut Graphics, resources: &GameResources) {
        if !self.visible {
            return;
        }
        let is_down = self.is_down && self.is_over;
        let Some(font) = resources.font(self.font) else { return };
        let ascent = font.font.ascent;
        let mut font_x = self.x + self.text_offset.0 + (self.width - font.font.string_width(&self.label)) / 2;
        let mut font_y = self.y + self.text_offset.1 + (self.height - ascent / 6 + ascent - 1) / 2;
        // Held down, the over image (or the plain one) and the label move a pixel.
        let (image, shift, color) = if is_down {
            (self.down_image.or(self.over_image).unwrap_or(self.image), 1, self.hilite_color)
        } else if self.is_over {
            (self.over_image.unwrap_or(self.image), 0, self.hilite_color)
        } else {
            (self.image, 0, self.color)
        };
        if let Some(image) = resources.image(image) {
            g.draw_image(image, (self.x + shift) as f32, (self.y + shift) as f32);
        }
        font_x += shift;
        font_y += shift;
        g.push_state();
        g.set_color(color);
        g.draw_string(font, font_x, font_y, &self.label);
        g.pop_state();
        if self.is_over
            && let Some(overlay) = self.over_overlay.and_then(|id| resources.image(id))
        {
            g.draw_image(overlay, self.x as f32, self.y as f32);
        }
    }
}

/// A horizontal `Slider`: a track image in three cels (left end, middle
/// tiled, right end) and a thumb to drag. `value` runs from 0 to 1.
/// Coordinates are relative to its parent. Not ported: vertical sliders and
/// the dragging cursor.
#[derive(Debug, Clone)]
pub struct Slider {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub value: f32,
    /// `mDragging`: the thumb is held.
    pub dragging: bool,
    /// Where on the thumb the mouse holds it (`mRelX`).
    rel_x: i32,
    track_image: &'static str,
    thumb_image: &'static str,
    thumb_width: i32,
}

impl Slider {
    pub fn new(track_image: &'static str, thumb_image: &'static str, resources: &GameResources) -> Self {
        let thumb_width = resources.image(thumb_image).map_or(0, |i| i.width as i32);
        Self::with_thumb_width(track_image, thumb_image, thumb_width)
    }

    fn with_thumb_width(track_image: &'static str, thumb_image: &'static str, thumb_width: i32) -> Self {
        Self { x: 0, y: 0, width: 0, height: 0, value: 0.0, dragging: false, rel_x: 0, track_image, thumb_image, thumb_width }
    }

    pub fn resize(&mut self, x: i32, y: i32, width: i32, height: i32) {
        (self.x, self.y, self.width, self.height) = (x, y, width, height);
    }

    pub fn contains(&self, p: IVec2) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.x + self.width && p.y < self.y + self.height
    }

    /// `SetValue`, clamped to 0..1.
    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(0.0, 1.0);
    }

    fn thumb_x(&self) -> i32 {
        (self.value * (self.width - self.thumb_width) as f32) as i32
    }

    /// `MouseDown` at `local` (relative to the slider): grabs the thumb, or
    /// jumps to where the track was clicked. Neither tells the listener yet;
    /// the release does.
    pub fn mouse_down(&mut self, local: IVec2) {
        let thumb_x = self.thumb_x();
        if local.x >= thumb_x && local.x < thumb_x + self.thumb_width {
            self.dragging = true;
            self.rel_x = local.x - thumb_x;
        } else {
            self.set_value(local.x as f32 / self.width as f32);
        }
    }

    /// `MouseDrag`: moves a held thumb. Returns the new value when it changed.
    pub fn mouse_drag(&mut self, local: IVec2) -> Option<f32> {
        if !self.dragging {
            return None;
        }
        let old = self.value;
        self.set_value((local.x - self.rel_x) as f32 / (self.width - self.thumb_width) as f32);
        (self.value != old).then_some(self.value)
    }

    /// `MouseUp`: lets go. The listener always hears the value now.
    pub fn mouse_up(&mut self) -> f32 {
        self.dragging = false;
        self.value
    }

    /// `Slider::Draw`.
    pub fn draw(&self, g: &mut Graphics, resources: &GameResources) {
        if let Some(track) = resources.image(self.track_image) {
            let (cw, ch) = (track.width as i32 / 3, track.height as i32);
            let ty = (self.height - ch) / 2;
            let cel = |i: i32| SexyRect::new((cw * i) as f32, 0.0, cw as f32, ch as f32);
            g.draw_image_src(track, self.x as f32, (self.y + ty) as f32, cel(0));
            g.push_state();
            g.clip_rect((self.x + cw) as f32, (self.y + ty) as f32, (self.width - cw * 2) as f32, ch as f32);
            for i in 0..(self.width - cw * 2 + cw - 1) / cw.max(1) {
                g.draw_image_src(track, (self.x + cw + i * cw) as f32, (self.y + ty) as f32, cel(1));
            }
            g.pop_state();
            g.draw_image_src(track, (self.x + self.width - cw) as f32, (self.y + ty) as f32, cel(2));
        }
        if let Some(thumb) = resources.image(self.thumb_image) {
            let y = self.y + (self.height - thumb.height as i32) / 2;
            g.draw_image(thumb, (self.x + self.thumb_x()) as f32, y as f32);
        }
    }
}

/// A `Checkbox` drawn with a checked and an unchecked image (`MakeNewCheckbox`).
/// Coordinates are relative to its parent.
#[derive(Debug, Clone)]
pub struct Checkbox {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub checked: bool,
    unchecked_image: &'static str,
    checked_image: &'static str,
}

impl Checkbox {
    pub fn new(unchecked_image: &'static str, checked_image: &'static str, checked: bool) -> Self {
        Self { x: 0, y: 0, width: 0, height: 0, checked, unchecked_image, checked_image }
    }

    pub fn resize(&mut self, x: i32, y: i32, width: i32, height: i32) {
        (self.x, self.y, self.width, self.height) = (x, y, width, height);
    }

    pub fn contains(&self, p: IVec2) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.x + self.width && p.y < self.y + self.height
    }

    /// `MouseDown`: flips the box. Returns whether it's now checked.
    pub fn mouse_down(&mut self) -> bool {
        self.checked = !self.checked;
        self.checked
    }

    /// `Checkbox::Draw`.
    pub fn draw(&self, g: &mut Graphics, resources: &GameResources) {
        let id = if self.checked { self.checked_image } else { self.unchecked_image };
        if let Some(image) = resources.image(id) {
            g.draw_image(image, self.x as f32, self.y as f32);
        }
    }
}

/// `DrawStoneButton`.
#[allow(clippy::too_many_arguments)]
pub fn draw_stone_button(
    g: &mut Graphics,
    resources: &GameResources,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    is_down: bool,
    highlighted: bool,
    label: &str,
) {
    let [Some(left), Some(middle), Some(right)] = (if is_down {
        ["IMAGE_BUTTON_DOWN_LEFT", "IMAGE_BUTTON_DOWN_MIDDLE", "IMAGE_BUTTON_DOWN_RIGHT"]
    } else {
        ["IMAGE_BUTTON_LEFT", "IMAGE_BUTTON_MIDDLE", "IMAGE_BUTTON_RIGHT"]
    })
    .map(|id| resources.image(id)) else {
        return;
    };
    let shift = is_down as i32;
    let mut image_x = x + shift;
    let repeat = (width - left.width as i32 - right.width as i32) / middle.width.max(1) as i32;
    g.draw_image(left, image_x as f32, y as f32);
    image_x += left.width as i32;
    for _ in 0..repeat.max(0) {
        g.draw_image(middle, image_x as f32, y as f32);
        image_x += middle.width as i32;
    }
    g.draw_image(right, image_x as f32, y as f32);

    let (Some(metrics), Some(font)) = (
        resources.font("FONT_DWARVENTODCRAFT18GREENINSET"),
        resources.font(if highlighted {
            "FONT_DWARVENTODCRAFT18BRIGHTGREENINSET"
        } else {
            "FONT_DWARVENTODCRAFT18GREENINSET"
        }),
    ) else {
        return;
    };
    let ascent = metrics.font.ascent;
    let font_x = x + shift + (width - metrics.font.string_width(label)) / 2 + 1;
    let font_y = y + shift + (height - ascent / 6 - 1 + ascent) / 2 - 4;
    g.push_state();
    g.set_color(SexyColor::WHITE);
    g.draw_string(font, font_x, font_y, label);
    g.pop_state();
}

/// Editing keys an [`EditWidget`] understands (`ProcessKey`'s `KeyCode`s).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKey {
    Left,
    Right,
    Backspace,
    Delete,
    Home,
    End,
    Return,
    /// `Ctrl+Z`.
    Undo,
}

/// A one-line text box (`EditWidget`, with `LawnEditWidget`'s
/// capitalized first letter). Coordinates are relative to its parent.
/// Not ported: clipboard (no OS clipboard here) and password characters
/// (nothing asks for a password).
#[derive(Debug, Clone)]
pub struct EditWidget {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub text: Vec<char>,
    pub cursor_pos: i32,
    /// The other end of the selection, or -1.
    pub hilite_pos: i32,
    left_pos: i32,
    pub max_chars: i32,
    /// `AddWidthCheckFont`: the text may not get wider than this in its font.
    pub max_pixels: i32,
    pub blink_delay: i32,
    blink_acc: i32,
    showing_cursor: bool,
    pub has_focus: bool,
    auto_cap_first_letter: bool,
    had_double_click: bool,
    /// `mUndoText`/`mUndoCursor`: what the last edit replaced.
    undo_text: Vec<char>,
    undo_cursor: i32,
}

/// `gLawnEditWidgetColors`: no background or outline.
const EDIT_TEXT: SexyColor = SexyColor::rgba(240, 240, 255, 255);
const EDIT_HILITE: SexyColor = SexyColor::WHITE;
const EDIT_HILITE_TEXT: SexyColor = SexyColor::BLACK;

impl EditWidget {
    /// `CreateEditWidget`.
    pub fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            text: Vec::new(),
            cursor_pos: 0,
            hilite_pos: -1,
            left_pos: 0,
            max_chars: -1,
            max_pixels: -1,
            blink_delay: 14,
            blink_acc: 0,
            showing_cursor: false,
            has_focus: false,
            auto_cap_first_letter: true,
            had_double_click: false,
            undo_text: Vec::new(),
            undo_cursor: 0,
        }
    }

    pub fn text(&self) -> String {
        self.text.iter().collect()
    }

    pub fn contains(&self, p: IVec2) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.x + self.width && p.y < self.y + self.height
    }

    pub fn resize(&mut self, x: i32, y: i32, width: i32, height: i32, font: &Font) {
        (self.x, self.y, self.width, self.height) = (x, y, width, height);
        self.focus_cursor(false, font);
    }

    /// `SetText`: puts the cursor at the end with everything selected.
    pub fn set_text(&mut self, text: &str, font: &Font) {
        self.text = text.chars().collect();
        self.cursor_pos = self.text.len() as i32;
        self.hilite_pos = 0;
        self.left_pos = 0;
        self.focus_cursor(true, font);
    }

    fn width_to(&self, font: &Font, end: i32) -> i32 {
        let end = (end.max(0) as usize).min(self.text.len());
        font.string_width(&self.text[..end].iter().collect::<String>())
    }

    pub fn got_focus(&mut self) {
        self.has_focus = true;
        self.showing_cursor = true;
        self.blink_acc = 0;
    }

    pub fn lost_focus(&mut self) {
        self.has_focus = false;
        self.showing_cursor = false;
    }

    /// `EditWidget::Update`: blinks the cursor while focused.
    pub fn update(&mut self) {
        if self.has_focus {
            self.blink_acc += 1;
            if self.blink_acc > self.blink_delay {
                self.blink_acc = 0;
                self.showing_cursor = !self.showing_cursor;
            }
        }
    }

    /// `LawnEditWidget::KeyChar`: types a character, capitalizing the first letter.
    pub fn key_char(&mut self, mut c: char, font: &Font, allow: impl Fn(char) -> bool) {
        if self.auto_cap_first_letter && c.is_ascii_alphabetic() {
            c = c.to_ascii_uppercase();
            self.auto_cap_first_letter = false;
        }
        self.process_key(None, Some(c), false, false, font, allow);
    }

    /// `EditWidget::KeyDown` for an editing key. Returns true for Return
    /// (`EditWidgetText`).
    pub fn key_down(&mut self, key: EditKey, shift: bool, ctrl: bool, font: &Font) -> bool {
        self.process_key(Some(key), None, shift, ctrl, font, |_| false)
    }

    /// `EditWidget::ProcessKey`.
    fn process_key(
        &mut self,
        key: Option<EditKey>,
        c: Option<char>,
        shift: bool,
        ctrl: bool,
        font: &Font,
        allow: impl Fn(char) -> bool,
    ) -> bool {
        // `Ctrl+Z` puts back what the last edit changed.
        if matches!(key, Some(EditKey::Undo)) {
            std::mem::swap(&mut self.text, &mut self.undo_text);
            self.cursor_pos = std::mem::replace(&mut self.undo_cursor, self.cursor_pos);
            self.hilite_pos = -1;
            self.focus_cursor(false, font);
            return false;
        }
        // Remember the text before it changes (`mUndoText`).
        let before = self.text.clone();
        let mut remove_hilite = !shift;
        if shift && self.hilite_pos == -1 {
            self.hilite_pos = self.cursor_pos;
        }
        let old_cursor = self.cursor_pos;
        let selection = (self.hilite_pos != -1 && self.hilite_pos != self.cursor_pos)
            .then(|| (self.cursor_pos.min(self.hilite_pos) as usize, self.cursor_pos.max(self.hilite_pos) as usize));
        let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_' || (c as u32) >= 0xC0;
        let mut entered = false;

        match key {
            Some(EditKey::Left) => {
                if ctrl {
                    while self.cursor_pos > 0 && !is_word(self.text[self.cursor_pos as usize - 1]) {
                        self.cursor_pos -= 1;
                    }
                    while self.cursor_pos > 0 && is_word(self.text[self.cursor_pos as usize - 1]) {
                        self.cursor_pos -= 1;
                    }
                } else if shift || self.hilite_pos == -1 {
                    self.cursor_pos -= 1;
                } else {
                    self.cursor_pos = self.cursor_pos.min(self.hilite_pos);
                }
            }
            Some(EditKey::Right) => {
                let len = self.text.len() as i32;
                if ctrl {
                    while self.cursor_pos < len - 1 && is_word(self.text[self.cursor_pos as usize + 1]) {
                        self.cursor_pos += 1;
                    }
                    while self.cursor_pos < len - 1 && !is_word(self.text[self.cursor_pos as usize + 1]) {
                        self.cursor_pos += 1;
                    }
                }
                if shift || self.hilite_pos == -1 {
                    self.cursor_pos += 1;
                } else {
                    self.cursor_pos = self.cursor_pos.max(self.hilite_pos);
                }
            }
            Some(EditKey::Backspace) if !self.text.is_empty() => match selection {
                Some((lo, hi)) => {
                    self.text.drain(lo..hi);
                    self.cursor_pos = lo as i32;
                    self.hilite_pos = -1;
                }
                None => {
                    if self.cursor_pos > 0 {
                        self.text.remove(self.cursor_pos as usize - 1);
                    }
                    self.cursor_pos -= 1;
                    self.hilite_pos = -1;
                }
            },
            Some(EditKey::Delete) if !self.text.is_empty() => match selection {
                Some((lo, hi)) => {
                    self.text.drain(lo..hi);
                    self.cursor_pos = lo as i32;
                    self.hilite_pos = -1;
                }
                None => {
                    if (self.cursor_pos as usize) < self.text.len() {
                        self.text.remove(self.cursor_pos as usize);
                    }
                }
            },
            Some(EditKey::Home) => self.cursor_pos = 0,
            Some(EditKey::End) => self.cursor_pos = self.text.len() as i32,
            Some(EditKey::Return) => entered = true,
            Some(_) => {}
            None => {
                let c = c.unwrap_or('\0');
                if (' '..='\u{7f}').contains(&c) && font.string_width(&c.to_string()) > 0 && allow(c) {
                    match selection {
                        Some((lo, hi)) => {
                            self.text.splice(lo..hi, [c]);
                            self.cursor_pos = lo as i32;
                            self.hilite_pos = -1;
                        }
                        None => {
                            self.text.insert(self.cursor_pos as usize, c);
                            self.hilite_pos = -1;
                        }
                    }
                    self.cursor_pos += 1;
                    self.focus_cursor(false, font);
                } else {
                    remove_hilite = false;
                }
            }
        }

        if self.text != before {
            self.undo_text = before;
            self.undo_cursor = old_cursor;
        }
        if self.max_chars != -1 && self.text.len() as i32 > self.max_chars {
            self.text.truncate(self.max_chars as usize);
        }
        // EnforceMaxPixels.
        if self.max_pixels > 0 {
            while font.string_width(&self.text()) > self.max_pixels {
                self.text.pop();
            }
        }
        self.cursor_pos = self.cursor_pos.clamp(0, self.text.len() as i32);
        if old_cursor != self.cursor_pos {
            self.blink_acc = 0;
            self.showing_cursor = true;
        }
        self.focus_cursor(true, font);
        if remove_hilite || self.hilite_pos == self.cursor_pos {
            self.hilite_pos = -1;
        }
        entered
    }

    /// `FocusCursor`: scrolls the text so the cursor stays visible.
    fn focus_cursor(&mut self, big_jump: bool, font: &Font) {
        let step = if big_jump { 10 } else { 1 };
        while self.cursor_pos < self.left_pos {
            self.left_pos = (self.left_pos - step).max(0);
        }
        let last = self.text.len() as i32 - 1;
        while self.width - 8 > 0
            && self.width_to(font, self.cursor_pos) - self.width_to(font, self.left_pos) >= self.width - 8
            && self.left_pos < last
        {
            self.left_pos = (self.left_pos + step).min(last);
        }
    }

    /// `GetCharAt`: the cursor position nearest local x.
    fn char_at(&self, x: i32, font: &Font) -> i32 {
        let mut pos = 0;
        let left = self.left_pos.max(0) as usize;
        for i in left..self.text.len() {
            let lo = font.string_width(&self.text[left..i].iter().collect::<String>());
            let hi = font.string_width(&self.text[left..=i].iter().collect::<String>());
            if x >= (lo + hi) / 2 + 5 {
                pos = i as i32 + 1;
            }
        }
        pos
    }

    /// `HiliteWord`: selects the word under the cursor.
    fn hilite_word(&mut self) {
        let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
        let len = self.text.len() as i32;
        if self.cursor_pos < len {
            self.hilite_pos = self.cursor_pos;
            while self.hilite_pos > 0 && is_word(self.text[self.hilite_pos as usize - 1]) {
                self.hilite_pos -= 1;
            }
            while self.cursor_pos < len - 1 && is_word(self.text[self.cursor_pos as usize + 1]) {
                self.cursor_pos += 1;
            }
            if self.cursor_pos < len {
                self.cursor_pos += 1;
            }
        }
    }

    /// `MouseDown`, at a point local to the widget.
    pub fn mouse_down(&mut self, local: IVec2, click_count: u32, font: &Font) {
        self.hilite_pos = -1;
        self.cursor_pos = self.char_at(local.x, font);
        if click_count > 1 {
            self.had_double_click = true;
            self.hilite_word();
        }
        self.focus_cursor(false, font);
    }

    pub fn mouse_drag(&mut self, local: IVec2, font: &Font) {
        if self.hilite_pos == -1 {
            self.hilite_pos = self.cursor_pos;
        }
        self.cursor_pos = self.char_at(local.x, font);
        self.focus_cursor(false, font);
    }

    pub fn mouse_up(&mut self, local: IVec2, font: &Font) {
        if self.hilite_pos == self.cursor_pos {
            self.hilite_pos = -1;
        }
        if self.had_double_click {
            self.hilite_pos = -1;
            self.cursor_pos = self.char_at(local.x, font);
            self.had_double_click = false;
            self.hilite_word();
        }
    }

    /// `EditWidget::Draw`: the text, then the selection (or the cursor, a
    /// 2-pixel selection) redrawn inverted on top.
    pub fn draw(&self, g: &mut Graphics, font: &FontRes) {
        let f = &font.font;
        let shown: String = self.text[(self.left_pos.max(0) as usize).min(self.text.len())..].iter().collect();
        let text_y = (self.height - f.height) / 2 + f.ascent;
        g.push_state();
        g.translate(self.x as f32, self.y as f32);
        for pass in 0..2 {
            g.push_state();
            if pass == 1 {
                let left = self.width_to(f, self.left_pos);
                let mut cursor_x = self.width_to(f, self.cursor_pos) - left;
                let mut hilite_x = cursor_x + 2;
                if self.hilite_pos != -1 && self.cursor_pos != self.hilite_pos {
                    hilite_x = self.width_to(f, self.hilite_pos) - left;
                }
                if !self.showing_cursor {
                    cursor_x += 2;
                }
                cursor_x = cursor_x.max(0).min(self.width - 8);
                hilite_x = hilite_x.max(0).min(self.width - 8);
                g.clip_rect(
                    (4 + cursor_x.min(hilite_x)) as f32,
                    ((self.height - f.height) / 2) as f32,
                    (hilite_x - cursor_x).abs() as f32,
                    f.height as f32,
                );
            } else {
                g.clip_rect(4.0, 0.0, (self.width - 8) as f32, self.height as f32);
            }
            if pass == 1 && self.has_focus {
                g.set_color(EDIT_HILITE);
                g.fill_rect(0.0, 0.0, self.width as f32, self.height as f32);
            }
            g.set_color(if pass == 0 || !self.has_focus { EDIT_TEXT } else { EDIT_HILITE_TEXT });
            g.draw_string(font, 4, text_y, &shown);
            g.pop_state();
        }
        g.pop_state();
    }
}

impl Default for EditWidget {
    fn default() -> Self {
        Self::new()
    }
}

/// `ListWidget` colors, in `SetColors` order.
#[derive(Debug, Clone, Copy)]
pub struct ListColors {
    pub bkg: SexyColor,
    pub outline: SexyColor,
    pub text: SexyColor,
    pub hilite: SexyColor,
    pub select: SexyColor,
}

/// A list of text lines with a hover highlight and a selected line
/// (`ListWidget`), without the scrollbar. Coordinates are relative to its parent.
#[derive(Debug, Clone)]
pub struct ListWidget {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub lines: Vec<String>,
    line_colors: Vec<SexyColor>,
    pub colors: ListColors,
    pub font: &'static str,
    pub item_height: i32,
    pub select_idx: i32,
    pub hilite_idx: i32,
    page_size: f64,
    pub draw_outline: bool,
}

impl ListWidget {
    /// A centered list (the only kind the game uses).
    pub fn new(font: &'static str, colors: ListColors, item_height: i32) -> Self {
        Self {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            lines: Vec::new(),
            line_colors: Vec::new(),
            colors,
            font,
            item_height,
            select_idx: -1,
            hilite_idx: -1,
            page_size: 0.0,
            draw_outline: true,
        }
    }

    pub fn resize(&mut self, x: i32, y: i32, width: i32, height: i32) {
        (self.x, self.y, self.width, self.height) = (x, y, width, height);
        self.page_size = if height > self.item_height + 8 { (height as f64 - 8.0) / self.item_height as f64 } else { 1.0 };
    }

    pub fn contains(&self, p: IVec2) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.x + self.width && p.y < self.y + self.height
    }

    /// Appends a line (`AddLine` without sorting), returning its index.
    pub fn add_line(&mut self, line: String) -> i32 {
        self.lines.push(line);
        self.line_colors.push(self.colors.text);
        self.lines.len() as i32 - 1
    }

    pub fn remove_line(&mut self, idx: i32) {
        if idx >= 0 && (idx as usize) < self.lines.len() {
            self.lines.remove(idx as usize);
            self.line_colors.remove(idx as usize);
        }
    }

    /// `MouseMove`, at a point local to the list, or `None` when the mouse left it.
    pub fn mouse_move(&mut self, local: Option<IVec2>) {
        self.hilite_idx = match local {
            Some(p) => {
                let idx = ((p.y - 4) as f64 / self.item_height as f64) as i32;
                if idx < 0 || idx as usize >= self.lines.len() { -1 } else { idx }
            }
            None => -1,
        };
    }

    /// `ListWidget::Draw`.
    pub fn draw(&self, g: &mut Graphics, resources: &GameResources) {
        let Some(font) = resources.font(self.font) else { return };
        let f = &font.font;
        let (w, h) = (self.width as f32, self.height as f32);
        g.push_state();
        g.translate(self.x as f32, self.y as f32);
        g.set_color(self.colors.bkg);
        g.fill_rect(0.0, 0.0, w, h);

        let item_offset = (self.item_height - f.height) / 2;
        let last = (self.lines.len() as i32 - 1).min(self.page_size as i32 + 1);
        for i in 0..=last {
            let draw_y = 4 + i * self.item_height;
            if i == self.select_idx {
                g.push_state();
                g.clip_rect(0.0, 4.0, w, h - 8.0);
                g.set_color(self.colors.select);
                g.fill_rect(0.0, draw_y as f32, w, self.item_height as f32);
                g.pop_state();
            }
            let line = &self.lines[i as usize];
            g.push_state();
            g.clip_rect(4.0, 4.0, w - 8.0, h - 8.0);
            g.set_color(if i == self.hilite_idx { self.colors.hilite } else { self.line_colors[i as usize] });
            g.draw_string(font, (self.width - f.string_width(line)) / 2, draw_y + f.ascent + item_offset, line);
            g.pop_state();
        }
        if self.draw_outline {
            g.set_color(self.colors.outline);
            g.draw_rect(0.0, 0.0, w - 1.0, h - 1.0);
        }
        g.pop_state();
    }
}

/// `DrawEditBox`: the frame behind a dialog's text box.
pub fn draw_edit_box(g: &mut Graphics, resources: &GameResources, edit: &EditWidget) {
    if let Some(image) = resources.image("IMAGE_EDITBOX") {
        let dest = SexyRect::new((edit.x - 8) as f32, (edit.y - 4) as f32, (edit.width + 16) as f32, (edit.height + 8) as f32);
        g.draw_image_box(image, dest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slider() -> Slider {
        let mut slider = Slider::with_thumb_width("TRACK", "THUMB", 20);
        slider.resize(199, 116, 135, 40);
        slider
    }

    #[test]
    fn slider_jumps_to_a_click_on_the_track() {
        let mut s = slider();
        s.set_value(0.0);
        s.mouse_down(IVec2::new(108, 20));
        assert!(!s.dragging);
        assert!((s.value - 0.8).abs() < 1e-6);
        assert_eq!(s.mouse_up(), s.value);
    }

    #[test]
    fn slider_thumb_drags_and_clamps() {
        let mut s = slider();
        s.set_value(0.4);
        // The thumb sits at 0.4 * (135 - 20) = 46; grab it 5 px in.
        s.mouse_down(IVec2::new(51, 20));
        assert!(s.dragging);
        assert_eq!(s.mouse_drag(IVec2::new(51, 20)), None);
        let v = s.mouse_drag(IVec2::new(28, 20)).unwrap();
        assert!((v - 23.0 / 115.0).abs() < 1e-6);
        assert_eq!(s.mouse_drag(IVec2::new(-50, 20)), Some(0.0));
        assert_eq!(s.mouse_drag(IVec2::new(500, 20)), Some(1.0));
        assert_eq!(s.mouse_up(), 1.0);
        assert!(!s.dragging);
        assert_eq!(s.mouse_drag(IVec2::new(0, 20)), None);
    }

    #[test]
    fn checkbox_flips_on_click() {
        let mut c = Checkbox::new("OFF", "ON", false);
        assert!(c.mouse_down());
        assert!(!c.mouse_down());
    }

    #[test]
    fn hidden_buttons_take_no_clicks() {
        let mut b = StoneButton::new(0, "Hi".into());
        b.resize(10, 10, 50, 30);
        assert!(b.contains(IVec2::new(20, 20)));
        b.visible = false;
        assert!(!b.contains(IVec2::new(20, 20)));
    }

    #[test]
    fn polygon_hit_test_matches_adventure_button_shape() {
        let poly = [Vec2::new(7.0, 1.0), Vec2::new(328.0, 30.0), Vec2::new(314.0, 125.0), Vec2::new(1.0, 78.0)];
        assert!(point_in_polygon(&poly, Vec2::new(150.0, 60.0)));
        assert!(!point_in_polygon(&poly, Vec2::new(320.0, 5.0)));
        assert!(!point_in_polygon(&poly, Vec2::new(3.0, 120.0)));
    }
}
