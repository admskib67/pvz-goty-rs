//! The options dialog as the main menu opens it (`NewOptionsDialog`).

use crate::dialog::{DialogButton, LawnDialog, OptionsFrom, OptionsValues, SFX_SLIDER_SCALE};

use super::shared;

const VALUES: OptionsValues =
    OptionsValues { music_volume: 0.85, sfx_volume: 0.5525, fullscreen: false, hardware_acceleration: true };

#[test]
fn options_from_the_main_menu() {
    let Some(shared) = shared() else {
        eprintln!("skipped: no main.pak (set PVZ_GAME_PATH or pick the game in the app)");
        return;
    };
    let res = &shared.resources;
    let dialog = LawnDialog::new_options(res, VALUES, OptionsFrom::GameSelector { finished_adventure: false });
    let back = res.image("IMAGE_OPTIONS_MENUBACK").expect("loaded with the game");
    // Centered on the screen at the background's size.
    assert_eq!((dialog.width, dialog.height), (back.width as i32, back.height as i32));
    assert_eq!(dialog.x, (800 - dialog.width) / 2);
    let o = dialog.options().unwrap();
    // The sound slider shows the volume over its 0.65 scale.
    assert!((o.sfx.value - 0.5525 / SFX_SLIDER_SCALE).abs() < 1e-6);
    assert_eq!(o.music.value, 0.85);
    assert!(!o.fullscreen.checked && o.hardware_acceleration.checked);
    // From the main menu everything sits lower, and only OK shows.
    assert_eq!((o.music.y, o.sfx.y, o.hardware_acceleration.y, o.fullscreen.y), (121, 153, 190, 226));
    assert!(!o.almanac.visible && !o.restart.visible && !o.main_menu.visible);
    let ok = dialog.yes_button.as_ref().unwrap();
    assert_eq!((ok.id, ok.x, ok.y), (DialogButton::Yes, 30, 381));
    assert_eq!(ok.label, res.translate("[DIALOG_BUTTON_OK]"));
}

#[test]
fn finished_adventure_offers_the_credits() {
    let Some(shared) = shared() else { return };
    let res = &shared.resources;
    let dialog = LawnDialog::new_options(res, VALUES, OptionsFrom::GameSelector { finished_adventure: true });
    let o = dialog.options().unwrap();
    assert!(o.main_menu.visible);
    assert_eq!(o.main_menu.label, res.translate("[CREDITS]"));
    assert_eq!((o.main_menu.x, o.main_menu.y), (107, 241 + 86));
}

#[test]
fn options_from_a_level() {
    let Some(shared) = shared() else { return };
    let res = &shared.resources;
    let dialog = LawnDialog::new_options(res, VALUES, OptionsFrom::Board { level_intro: false, can_show_almanac: true });
    let o = dialog.options().unwrap();
    assert!(o.almanac.visible && o.restart.visible && o.main_menu.visible);
    assert_eq!(o.music.y, 116);
    assert_eq!((o.restart.y, o.main_menu.y), (241 + 43, 241 + 86));
    assert_eq!(dialog.yes_button.as_ref().unwrap().label, res.translate("[BACK_TO_GAME]"));

    // No restarting or almanac during the level intro.
    let dialog = LawnDialog::new_options(res, VALUES, OptionsFrom::Board { level_intro: true, can_show_almanac: true });
    let o = dialog.options().unwrap();
    assert!(!o.restart.visible && !o.almanac.visible && o.main_menu.visible);
    // Nor before the almanac is found (level 15).
    let dialog = LawnDialog::new_options(res, VALUES, OptionsFrom::Board { level_intro: false, can_show_almanac: false });
    assert!(!dialog.options().unwrap().almanac.visible);
}
