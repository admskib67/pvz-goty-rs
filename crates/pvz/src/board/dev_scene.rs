//! Development aid: `PVZ_DEV_SCENE=butter` skips the level intro and lines up
//! buttered zombies of every ported type, so `PVZ_SCREENSHOT` can show where
//! the butter is drawn on each of them. `PVZ_DEV_SCENE=pool` skips the intro
//! and the seed chooser, and plants a few plants on the pool lawn of 3-1.

use crate::audio::Music;
use crate::zombie::ZombieType;

use super::zombie::Fx;
use super::{Board, SEED_BANK_OFFSET_X_END};

const ENV: &str = "PVZ_DEV_SCENE";

/// Sets up the scene `PVZ_DEV_SCENE` names, if any, on a board fresh from its
/// intro setup.
pub(super) fn set_up(board: &mut Board, music: &mut Music, fx: &mut Fx) {
    match std::env::var(ENV).as_deref() {
        Ok("butter") => butter(board, music, fx),
        Ok("pool") => pool(board, music, fx),
        Ok(other) => bevy::log::warn!("{ENV}: no scene called {other:?}"),
        Err(_) => {}
    }
}

/// Play starts at once, the way the intro would leave it.
fn skip_intro(board: &mut Board, music: &mut Music, fx: &mut Fx) {
    for (_, zombie) in board.zombies.iter_mut() {
        zombie.die_no_loot(fx.effects);
    }
    board.init_lawn_mowers();
    for mower in &mut board.lawn_mowers {
        mower.visible = true;
        mower.pos_x = -21.0;
    }
    board.x = 0;
    board.enable_grave_stones = true;
    board.seed_bank.x = SEED_BANK_OFFSET_X_END;
    board.seed_bank.y = 0;
    board.seed_bank.cut_scene_darken = 0;
    board.menu_button.no_draw = false;
    board.show_shovel = !board.is_whack_a_zombie_level();
    board.start_playing(music);
}

/// Plants on the water (lily pads under two of them, and one planted from
/// the bank: the splash plays) and on the lawn, with a zombie wading in.
fn pool(board: &mut Board, music: &mut Music, fx: &mut Fx) {
    use crate::plant::SeedType::*;
    skip_intro(board, music, fx);
    board.seed_chooser = None;
    board.add_plant(3, 2, Lilypad, fx.effects, fx.res, fx.images, fx.sounds);
    board.add_plant(3, 2, Peashooter, fx.effects, fx.res, fx.images, fx.sounds);
    board.add_plant(4, 3, Lilypad, fx.effects, fx.res, fx.images, fx.sounds);
    board.add_plant(4, 3, Snowpea, fx.effects, fx.res, fx.images, fx.sounds);
    board.add_plant(2, 0, Sunflower, fx.effects, fx.res, fx.images, fx.sounds);
    board.add_plant(2, 1, Wallnut, fx.effects, fx.res, fx.images, fx.sounds);
    let id = board.add_zombie_in_row(ZombieType::Normal, 3, 0, fx);
    if let Some(zombie) = board.zombies.get_mut(id) {
        zombie.place_x(560.0);
    }
}

fn butter(board: &mut Board, music: &mut Music, fx: &mut Fx) {
    use ZombieType::*;
    skip_intro(board, music, fx);
    let lineup = [
        [Normal, Pail, Polevaulter],
        [TrafficCone, Flag, Football],
        [Door, Newspaper, Dancer],
        [BackupDancer, Snorkel, DolphinRider],
        [Ladder, Imp, Gargantuar],
    ];
    for (row, types) in lineup.into_iter().enumerate() {
        for (i, ty) in types.into_iter().enumerate() {
            let id = board.add_zombie_in_row(ty, row as i32, 0, fx);
            if let Some(zombie) = board.zombies.get_mut(id) {
                zombie.place_x(260.0 + i as f32 * 190.0);
                zombie.apply_butter(fx);
                zombie.dev_hold_butter();
            }
        }
    }
}
