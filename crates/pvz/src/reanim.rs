//! Playing and drawing reanimations, plus the effect system that owns them.
//!
//! The animation model lives in `sexy::reanim`; this module maps the game's
//! reanimation types to files and draws tracks through [`Graphics`].
//! Behaviour per ruslan831 decomp, `Reanimation::DrawTrack`/`DrawRenderGroup`,
//! `ReanimatorEnsureDefinitionLoaded` and `LawnApp::AddReanimation`.

use std::sync::Arc;

use bevy::math::Affine2;
use bevy::prelude::*;
use sexy::matrix::Matrix3;
use sexy::reanim::ReanimDefinition;
use sexy::tod::float_round_to_int;

use crate::graphics::{Graphics, SexyColor, SexyRect};
use crate::resources::GameResources;

pub type Reanimation = sexy::reanim::Reanimation<Arc<ReanimDefinition>>;

/// Reanimation types (`ReanimationType`). Only the ones ported screens use
/// so far; the original's table lists ~150.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReanimationType {
    LoadbarSprout,
    LoadbarZombieHead,
}

impl ReanimationType {
    /// File from `gLawnReanimationArray`.
    pub fn file(self) -> &'static str {
        match self {
            Self::LoadbarSprout => "reanim/LoadBar_sprout.reanim",
            Self::LoadbarZombieHead => "reanim/LoadBar_Zombiehead.reanim",
        }
    }
}

/// The render group tracks draw in unless reassigned (`RENDER_GROUP_NORMAL`).
pub const RENDER_GROUP_NORMAL: i32 = 0;

/// Running effects. Mirrors `EffectSystem`'s reanimation holder; particles
/// and trails will join it as they're ported.
#[derive(Resource, Default)]
pub struct EffectSystem {
    reanimations: Vec<Reanimation>,
}

impl EffectSystem {
    /// `LawnApp::AddReanimation`. Loads the definition on first use.
    pub fn add_reanimation(
        &mut self,
        x: f32,
        y: f32,
        ty: ReanimationType,
        resources: &mut GameResources,
        images: &mut Assets<Image>,
    ) -> Result<&mut Reanimation, String> {
        let definition = resources.reanim_definition(ty.file(), images)?;
        self.reanimations.push(Reanimation::new(x, y, definition));
        Ok(self.reanimations.last_mut().unwrap())
    }

    /// One 100 Hz tick; finished reanimations are removed.
    pub fn update(&mut self) {
        for reanim in &mut self.reanimations {
            reanim.update();
        }
        self.reanimations.retain(|r| !r.dead);
    }

    pub fn reanimations(&self) -> &[Reanimation] {
        &self.reanimations
    }
}

fn to_affine(m: Matrix3) -> Affine2 {
    Affine2::from_cols_array(&[m.m00, m.m10, m.m01, m.m11, m.m02, m.m12])
}

/// `Reanimation::Draw`: every track in the normal render group.
pub fn draw_reanimation(g: &mut Graphics, reanim: &Reanimation, resources: &GameResources) {
    if reanim.dead {
        return;
    }
    for (i, inst) in reanim.tracks.iter().enumerate() {
        if inst.render_group == RENDER_GROUP_NORMAL {
            draw_track(g, reanim, i, resources);
        }
    }
}

/// `Reanimation::DrawTrack` for image tracks.
fn draw_track(g: &mut Graphics, reanim: &Reanimation, track: usize, resources: &GameResources) {
    let transform = reanim.current_transform(track);
    let mut frame = transform.frame();
    if frame < 0 {
        return;
    }

    let inst = &reanim.tracks[track];
    let [r, gr, b, a] = inst.track_color;
    let mut color = SexyColor::rgba(r, gr, b, a);
    if !inst.ignore_color_override {
        let [r, gr, b, a] = reanim.color_override;
        color = color.multiply(SexyColor::rgba(r, gr, b, a));
    }
    if g.colorize_images() {
        color = color.multiply(g.color());
    }
    let alpha = float_round_to_int(transform.a * color.a as f32).clamp(0, 255);
    if alpha <= 0 {
        return;
    }
    color.a = alpha as u8;

    // TODO: text tracks (`font`/`text`) and the "fullscreen" fill track.
    let Some(image) = transform.image.as_deref().and_then(|id| resources.image(id)) else { return };

    let (cel_w, cel_h) = (image.cel_width() as f32, image.cel_height() as f32);
    let matrix = (reanim.overlay * transform.matrix() * Matrix3::translation(cel_w * 0.5, cel_h * 0.5))
        .translated(-0.5, -0.5);
    while frame >= image.cols as i32 {
        frame -= image.cols as i32;
    }
    let src = SexyRect::new(frame as f32 * cel_w, 0.0, cel_w, cel_h);

    g.push_state();
    g.set_colorize_images(true);
    g.set_color(color);
    g.draw_image_matrix_src(image, to_affine(matrix), src);
    g.pop_state();
}
