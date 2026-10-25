//! The kernel-pult's butter: who it sticks to, and that it's drawn on the
//! zombie's head (`Zombie::DrawButter`).

use sexy::data_array::Id;

use crate::zombie::ZombieType;

use super::super::zombie::{Fx, Zombie};
use super::{TestBoard, test_board};

/// Butters a zombie the way a pat of butter landing on it does.
fn butter(t: &mut TestBoard, id: Id<Zombie>) {
    t.with(|board, effects, res, images, sounds| {
        let mut fx = Fx { effects, res, images, sounds };
        board.zombies.get_mut(id).unwrap().apply_butter(&mut fx);
    });
}

/// How far the middle of the butter is from the middle of the image on the
/// zombie's `track` (its head), worked out from the body reanimation as drawn,
/// and how big that image is.
fn butter_offset(t: &mut TestBoard, id: Id<Zombie>, track: &str) -> ((f32, f32), (f32, f32)) {
    t.with(|board, effects, res, _, _| {
        let z = board.zombies.get(id).unwrap();
        let butter = res.image("IMAGE_REANIM_CORNPULT_BUTTER_SPLAT").unwrap();
        let (bx, by, scale) = z.butter_position(effects, res).unwrap_or_else(|| panic!("{:?}: no butter drawn", z.zombie_type));
        let body = effects.get(z.body_reanim().unwrap()).unwrap();
        let track = body.definition.find_track(track).unwrap_or_else(|| panic!("{:?}: no {track} track", z.zombie_type));
        let head = res.image(&body.current_transform(track).image.expect("the head track shows an image")).unwrap();
        // The track's matrix applied to the middle of its image (it may be mirrored).
        let m = body.track_matrix(track);
        let (w, h) = (head.cel_width() as f32 / 2.0, head.cel_height() as f32 / 2.0);
        let head_center = (m.m00 * w + m.m01 * h + m.m02, m.m10 * w + m.m11 * h + m.m12);
        let butter_center = (bx + butter.cel_width() as f32 * scale / 2.0, by + butter.cel_height() as f32 * scale / 2.0);
        let offset = (butter_center.0 - head_center.0, butter_center.1 - head_center.1);
        (offset, (head.cel_width() as f32, head.cel_height() as f32))
    })
}

/// DrawButter puts it on the crown: centred across the head, in its top half.
fn assert_on_head(ty: ZombieType, ((dx, dy), (_, h)): ((f32, f32), (f32, f32))) {
    assert!(dx.abs() < 6.0 && dy < 0.0 && dy > -h / 2.0, "{ty:?}: butter {dx}, {dy} off the crown of its head ({h} high)");
}

#[test]
fn butter_lands_on_every_zombies_head() {
    use ZombieType::*;
    let types = [
        Normal, Flag, TrafficCone, Pail, Door, Newspaper, Football, Dancer, BackupDancer, Polevaulter, Snorkel, DolphinRider, Ladder, Imp,
        Gargantuar, Catapult,
    ];
    for (i, ty) in types.into_iter().enumerate() {
        let mut t = test_board!("1-6");
        let row = i as i32 % 5;
        let id = t.zombie(ty, row, 450.0);
        t.ticks(3);
        butter(&mut t, id);
        assert!(t.zombie_ref(id).unwrap().is_buttered(), "{ty:?} takes butter");
        t.ticks(3);
        // The catapult's butter goes on its driver's head.
        let track = if ty == Catapult { "Zombie_catapult_driver_head" } else { "anim_head1" };
        assert_on_head(ty, butter_offset(&mut t, id, track));
    }
}

#[test]
fn butter_sits_on_the_head_of_a_zombie_on_the_roof() {
    for (row, x) in [(0, 300.0), (2, 520.0), (4, 700.0)] {
        let mut t = test_board!("5-4");
        let id = t.zombie(ZombieType::Pail, row, x);
        butter(&mut t, id);
        t.ticks(3);
        assert_on_head(ZombieType::Pail, butter_offset(&mut t, id, "anim_head1"));
    }
}

#[test]
fn butter_does_not_stick_to_a_zamboni_or_a_bobsled_team() {
    for ty in [ZombieType::Zamboni, ZombieType::Bobsled] {
        let mut t = test_board!("3-6");
        let id = t.zombie(ty, 0, 600.0);
        butter(&mut t, id);
        assert!(!t.zombie_ref(id).unwrap().is_buttered(), "{ty:?}");
    }
}

#[test]
fn butter_does_not_stick_to_a_pole_vaulter_in_mid_jump() {
    let mut t = test_board!("1-6");
    t.plant(crate::plant::SeedType::Wallnut, 4, 2);
    let id = t.zombie(ZombieType::Polevaulter, 2, 600.0);
    assert!(t.run_until(2000, |b| b.zombies.get(id).is_some_and(|z| z.is_vaulting() && z.phase == super::super::zombie::ZombiePhase::PoleInVault)));
    butter(&mut t, id);
    assert!(!t.zombie_ref(id).unwrap().is_buttered());
}

#[test]
fn the_butter_is_drawn_where_it_belongs() {
    use crate::graphics::Graphics;
    for ty in [ZombieType::Normal, ZombieType::Gargantuar] {
        let mut t = test_board!("5-4");
        let id = t.zombie(ty, 2, 400.0);
        butter(&mut t, id);
        t.ticks(3);
        t.with(|board, effects, res, _, _| {
            let z = board.zombies.get(id).unwrap();
            let butter = res.image("IMAGE_REANIM_CORNPULT_BUTTER_SPLAT").unwrap();
            let (bx, by, scale) = z.butter_position(effects, res).unwrap();
            let mut g = Graphics::default();
            z.draw(&mut g, effects, res, None, false, true);
            let drawn = g.image_rects().into_iter().find(|(image, _)| *image == butter.handle).map(|(_, rect)| rect).expect("butter drawn");
            // DrawButter: the top-left corner at the butter's position, in the zombie's frame.
            let (x, y) = (z.x as f32 + bx, z.y as f32 + by);
            let (w, h) = (butter.width as f32 * scale, butter.height as f32 * scale);
            assert!(
                (drawn.x - x).abs() < 1.0 && (drawn.y - y).abs() < 1.0 && (drawn.w - w).abs() < 1.0 && (drawn.h - h).abs() < 1.0,
                "{ty:?}: butter drawn at {drawn:?}, should be at ({x}, {y}) {w}x{h}"
            );
        });
    }
}
