//! The almanac's cards: `TodDrawStringWrapped` with `{FORMAT}` codes.

use crate::graphics::{Graphics, Justify, SexyColor, SexyRect};

use super::{Shared, shared};

fn with_fonts(shared: &mut Shared) -> &crate::resources::GameResources {
    let shared = &mut *shared;
    shared.resources.load_group_now("LoadingFonts", &mut shared.images).expect("fonts load");
    &shared.resources
}

/// The glyphs drawn for `text` in a wide rectangle, top to bottom.
fn glyph_tops(res: &crate::resources::GameResources, text: &str) -> Vec<f32> {
    let font = res.font("FONT_BRIANNETOD12").expect("loaded with the game");
    let mut g = Graphics::default();
    g.tod_draw_string_wrapped(font, SexyRect::new(0.0, 0.0, 400.0, 300.0), text, SexyColor::WHITE, Justify::Left);
    g.image_rects().into_iter().map(|(_, rect)| rect.y).collect()
}

#[test]
fn format_codes_are_not_drawn() {
    let Some(mut shared) = shared() else {
        eprintln!("skipped: no main.pak (set PVZ_GAME_PATH or pick the game in the app)");
        return;
    };
    let res = with_fonts(&mut shared);
    // Codes, known or not (a zombie's hidden toughness), draw nothing.
    assert_eq!(glyph_tops(res, "{KEYWORD}Damage:{STAT} low {270}").len(), glyph_tops(res, "Damage: low ").len());
}

#[test]
fn shortline_pulls_the_next_line_up() {
    let Some(mut shared) = shared() else { return };
    let res = with_fonts(&mut shared);
    let plain = glyph_tops(res, "a\nb");
    let short = glyph_tops(res, "a{SHORTLINE}\nb");
    let (plain_b, short_b) = (plain.last().unwrap(), short.last().unwrap());
    assert_eq!(plain_b - short_b, 9.0);
    // Flavour text runs its lines together.
    assert_eq!(glyph_tops(res, "{FLAVOR}a\nb").iter().fold(f32::MIN, |a, &b| a.max(b)), plain[0]);
}
