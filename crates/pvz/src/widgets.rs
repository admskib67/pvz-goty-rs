//! Image buttons and the bit of SexyFramework's `WidgetManager` they need:
//! hover, press and click, with the topmost button winning.
//!
//! Behaviour per ruslan831 decomp, `NewLawnButton` (`Lawn/Widget/GameButton.cpp`:
//! `Draw`, `IsPointVisible`, `MakeNewButton`) and `ButtonWidget`'s mouse
//! handling.

use bevy::prelude::*;

use crate::graphics::{Graphics, SexyColor};
use crate::input::GameCursor;
use crate::resources::GameResources;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_hit_test_matches_adventure_button_shape() {
        let poly = [Vec2::new(7.0, 1.0), Vec2::new(328.0, 30.0), Vec2::new(314.0, 125.0), Vec2::new(1.0, 78.0)];
        assert!(point_in_polygon(&poly, Vec2::new(150.0, 60.0)));
        assert!(!point_in_polygon(&poly, Vec2::new(320.0, 5.0)));
        assert!(!point_in_polygon(&poly, Vec2::new(3.0, 120.0)));
    }
}
