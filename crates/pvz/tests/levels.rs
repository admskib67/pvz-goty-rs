//! Level-specific features: the night levels' Crazy Dave talks and seed
//! slot offer, 2-10's conveyor belt, and 2-5's whack-a-zombie.

use crate::plant::SeedType;
use crate::player::PlayerInfo;
use crate::zombie::ZombieType;

use super::super::conveyor::mini_boss_seeds;
use super::super::cursor::CursorType;
use super::super::message::MessageStyle;
use super::super::tutorial::AdviceType;
use super::super::zombie::ZombiePhase;
use super::super::{STORE_ITEM_PACKET_UPGRADE, cutscene};
use super::{TestBoard, test_board};

#[test]
fn the_whack_a_zombie_tutorial_flashes_the_packets_once_there_is_sun() {
    use super::super::tutorial::TutorialState;
    let mut t = test_board!("2-5");
    assert!(t.board.sun_money > 0, "the level starts with sun");
    assert!(
        t.run_until(50, |b| b.tutorial_state == TutorialState::WhackBeforePickSeed),
        "the first sun starts the lesson"
    );
    assert!(
        t.run_until(2000, |b| b.tutorial_state == TutorialState::WhackPickSeed),
        "then the packets flash"
    );
    assert!(t.run_until(1000, |b| b.tutorial_state == TutorialState::WhackCompleted), "and it's over");
}

#[test]
fn the_huge_wave_banner_animates_its_letters() {
    let mut t = test_board!("1-1");
    let letters = t.with(|board, _effects, res, images, _sounds| {
        let _ = res.load_group_now("LoadingFonts", images);
        board.display_advice_again(res, "[ADVICE_HUGE_WAVE]", MessageStyle::HugeWave, AdviceType::HugeWave);
        res.translate("[ADVICE_HUGE_WAVE]").chars().count()
    });
    t.tick();
    assert_eq!(t.board.advice.letters_len(), letters, "one animation per letter");
    t.ticks(20);
    let alpha = t.board.advice.first_letter_alpha(&t.effects).expect("the first letter");
    assert!(alpha > 0.0, "the letters fade in, alpha {alpha}");
}

#[test]
fn night_mini_boss_belt_brings_night_plants() {
    let t = test_board!("2-10");
    assert!(t.board.has_conveyor_belt_seed_bank());
    let seeds: Vec<_> = mini_boss_seeds(20).iter().map(|&(seed, _)| seed).collect();
    for seed in [SeedType::Gravebuster, SeedType::Iceshroom, SeedType::Doomshroom, SeedType::Puffshroom] {
        assert!(seeds.contains(&seed), "2-10's belt has {seed:?}");
    }
    assert!(!seeds.contains(&SeedType::Peashooter));
}

#[test]
fn crazy_dave_talks_at_the_start_of_the_night() {
    let mut t = test_board!("2-1");
    assert_eq!(cutscene::crazy_dave_dialog_start(&t.board), 201, "2-1: fighting at night");
    t.board.level = 12;
    assert_eq!(cutscene::crazy_dave_dialog_start(&t.board), 1401, "2-2: the money talk");
    t.board.level = 13;
    t.board.player_coins = 0;
    assert_eq!(cutscene::crazy_dave_dialog_start(&t.board), -1, "2-3: nothing to say if you can't afford a slot");
    t.board.player_coins = 75;
    assert_eq!(cutscene::crazy_dave_dialog_start(&t.board), 1501, "2-3: a seed slot for $750");
    t.board.level = 15;
    assert_eq!(cutscene::crazy_dave_dialog_start(&t.board), 401, "2-5: whack-a-zombie");
}

#[test]
fn buying_a_seed_slot_adds_a_packet() {
    let mut t = test_board!("2-3");
    let packets = t.board.seed_bank.num_packets;
    t.board.player_coins = 100;
    t.board.packet_offer_answer = Some(true);
    t.tick();
    assert_eq!(t.board.seed_bank.num_packets, packets + 1, "one more seed slot");
    assert_eq!(t.board.player_coins, 25, "for 75 coins");
    assert!(!t.board.can_get_packet_upgrade(0), "and he doesn't offer it again");
}

#[test]
fn bought_seed_slots_carry_over_to_new_levels() {
    let mut player = PlayerInfo { level: 13, ..Default::default() };
    player.purchases[STORE_ITEM_PACKET_UPGRADE] = 1;
    let t = test_board!("2-3");
    let without = t.board.num_seeds_in_bank(Some(&PlayerInfo { level: 13, ..Default::default() }));
    assert_eq!(t.board.num_seeds_in_bank(Some(&player)), without + 1);
}

#[test]
fn whack_a_zombie_gives_a_mallet_and_three_packets() {
    let t = test_board!("2-5");
    assert!(t.board.is_whack_a_zombie_level());
    assert_eq!(t.board.cursor.cursor_type, CursorType::Hammer, "the mallet's in hand");
    let seeds: Vec<_> = t.board.seed_bank.packets[..t.board.seed_bank.num_packets].iter().map(|p| p.packet_type).collect();
    assert_eq!(seeds, [Some(SeedType::Potatomine), Some(SeedType::Gravebuster), Some(SeedType::Cherrybomb)]);
    assert!(!t.board.choose_seeds_on_current_level(), "no seed chooser");
    let graves = t.board.grid_items.len();
    assert_eq!(graves, 9, "nine graves to start");
    assert!(t.board.grid_items.iter().all(|(_, g)| g.grid_x >= 3), "from the fourth column on");
}

/// Clicks the mallet down on a zombie.
fn whack(t: &mut super::TestBoard, id: sexy::data_array::Id<super::super::zombie::Zombie>) {
    let rect = t.zombie_ref(id).unwrap().zombie_rect();
    t.click(rect.x + rect.w / 2, rect.y + rect.h / 2 + 20);
}

#[test]
fn the_mallet_knocks_out_zombies_but_not_helmets() {
    let mut t = test_board!("2-5");
    let zombie = t.zombie(ZombieType::Normal, 2, 400.0);
    t.tick();
    whack(&mut t, zombie);
    assert!(t.zombie_ref(zombie).is_none(), "one whack and it's gone");

    let pail = t.zombie(ZombieType::Pail, 3, 400.0);
    t.tick();
    whack(&mut t, pail);
    assert!(t.zombie_ref(pail).is_some_and(|z| z.helm_health() == 200), "a bucket takes 900 of the blow");
    whack(&mut t, pail);
    whack(&mut t, pail);
    assert!(t.zombie_ref(pail).is_none(), "then the bare head");
}

#[test]
fn a_missed_whack_hurts_nobody() {
    let mut t = test_board!("2-5");
    let zombie = t.zombie(ZombieType::Normal, 2, 400.0);
    t.tick();
    let (x, y) = t.square_center(0, 0);
    t.click(x, y);
    assert!(t.zombie_ref(zombie).is_some_and(|z| z.body_health == 270));
}

#[test]
fn zombies_pop_out_of_the_graves() {
    let mut t = test_board!("2-5").with_waves();
    assert!(
        t.run_until(1000, |b| b.zombies.iter().any(|(_, z)| z.phase == ZombiePhase::RisingFromGrave)),
        "the first zombie climbs out of a grave"
    );
    let (col, row, x) = {
        let (_, z) = t.board.zombies.iter().next().unwrap();
        (t.board.pixel_to_grid_x(z.x + 25), z.row, z.x)
    };
    assert!(t.board.grave_stone_at(col, row).is_some(), "where there's a grave");
    t.ticks(100);
    let z = t.board.zombies.iter().next().unwrap().1;
    assert_eq!(z.phase, ZombiePhase::Normal, "it's up quickly");
    assert!(z.x < x, "and walks towards the house");
}

#[test]
fn planting_in_the_wrong_spot_puts_the_plant_back() {
    let mut t = test_board!("2-5");
    t.give_sun(500);
    // The grave buster only goes on a grave: anywhere else it goes back to the bank.
    t.plant_from_bank(1, 0, 0);
    assert!(t.plant_at(0, 0).is_none());
    assert_eq!(t.board.sun_money, 500);
    t.tick();
    assert_eq!(t.board.cursor.cursor_type, CursorType::Hammer, "the mallet's back in hand");
}

#[test]
fn the_seed_chooser_flags_plants_that_wont_help() {
    let day = test_board!("1-8");
    assert_eq!(day.board.seed_not_recommended(SeedType::Puffshroom), Some("[NOCTURNAL_WARNING]"), "mushrooms sleep in the day");
    assert_eq!(day.board.seed_not_recommended(SeedType::Gravebuster), Some("[NOT_RECOMMENDED_FOR_LEVEL]"), "no graves");
    assert_eq!(day.board.seed_not_recommended(SeedType::Peashooter), None);
    drop(day);
    let night = test_board!("2-2");
    assert_eq!(night.board.seed_not_recommended(SeedType::Puffshroom), None);
    assert_eq!(night.board.seed_not_recommended(SeedType::Gravebuster), None);
}

#[test]
fn the_seed_chooser_warns_about_missing_sun_plants() {
    let t = test_board!("1-8");
    let warnings = t.board.chooser_warnings(|seed| seed == SeedType::Peashooter);
    assert_eq!(warnings, ["[SEED_CHOOSER_SUN_WARNING]"], "no sunflower");
    assert!(t.board.chooser_warnings(|seed| seed == SeedType::Sunflower).is_empty(), "a sunflower's enough");
    drop(t);
    let t = test_board!("2-1");
    let warnings = t.board.chooser_warnings(|seed| seed == SeedType::Peashooter);
    assert_eq!(warnings, ["[SEED_CHOOSER_PUFFSHROOM_WARNING]", "[SEED_CHOOSER_NIGHT_SUN_WARNING]"], "2-1 asks twice");
}

#[test]
fn the_seed_chooser_warns_about_pool_and_roof() {
    let t = test_board!("3-2");
    let warnings = t.board.chooser_warnings(|seed| seed == SeedType::Sunflower);
    assert_eq!(warnings, ["[SEED_CHOOSER_POOL_WARNING]"], "water and no lily pad");
    assert!(t.board.chooser_warnings(|seed| matches!(seed, SeedType::Sunflower | SeedType::Lilypad)).is_empty());
    drop(t);
    let t = test_board!("5-2");
    let warnings = t.board.chooser_warnings(|seed| seed == SeedType::Sunflower);
    assert!(warnings.contains(&"[SEED_CHOOSER_ROOF_WARNING]".into()), "{warnings:?}");
    assert!(!t.board.chooser_warnings(|seed| matches!(seed, SeedType::Sunflower | SeedType::Flowerpot)).contains(&"[SEED_CHOOSER_ROOF_WARNING]".into()));
}

#[test]
fn the_seed_chooser_warns_about_upgrades_without_their_base_plant() {
    use crate::board::seed_chooser::ChooserWarning;
    let t = test_board!("1-8");
    let picked = |seed| matches!(seed, SeedType::Sunflower | SeedType::Gatlingpea);
    let warnings = t.board.chooser_warnings(picked);
    assert_eq!(warnings, [ChooserWarning::Upgrade { to: SeedType::Gatlingpea, from: SeedType::Repeater }]);
    let picked = |seed| matches!(seed, SeedType::Sunflower | SeedType::Gatlingpea | SeedType::Repeater);
    assert!(t.board.chooser_warnings(picked).is_empty());
}

#[test]
fn the_seed_chooser_offers_store_upgrades_and_lets_the_imitater_copy_a_plant() {
    use super::super::seed_chooser::SeedChooser;
    use bevy::math::IVec2;
    let Some(mut t) = TestBoard::with_player("2-1", |p| {
        p.purchases[0] = 1; // gatling pea
        p.purchases[8] = 1; // imitater
    }) else {
        return;
    };
    let owned = &t.board.owned_seeds;
    assert!(owned[SeedType::Gatlingpea as usize] && owned[SeedType::Imitater as usize], "the store's plants count");
    assert!(!owned[SeedType::Twinsunflower as usize]);
    assert!(t.board.has_seven_chooser_rows(), "an upgrade makes a seventh row");
    let mut chooser = SeedChooser::new(&t.board.owned_seeds, true, 0);
    chooser.mouse_visible = true;
    t.board.seed_chooser = Some(chooser);
    // The gatling pea sits first on the sixth row.
    t.with(|board, _, _, _, sounds| {
        board.seed_chooser_mouse_down(IVec2::new(30, 480), sounds);
    });
    assert_eq!(t.board.seed_chooser.as_ref().unwrap().picked(), [(SeedType::Gatlingpea as usize, None)]);
    // The imitater's button opens its panel, and a plant picked from it is copied.
    t.with(|board, _, _, _, sounds| {
        board.seed_chooser_mouse_down(IVec2::new(470, 520), sounds);
    });
    assert!(t.board.seed_chooser.as_ref().unwrap().imitater_panel_open());
    // The panel's first plant, the peashooter.
    t.with(|board, _, _, _, sounds| {
        board.seed_chooser_mouse_down(IVec2::new(195, 195), sounds);
    });
    let picked = t.board.seed_chooser.as_ref().unwrap().picked();
    assert!(picked.contains(&(SeedType::Imitater as usize, Some(SeedType::Peashooter))), "{picked:?}");
}

/// Wins the level by killing its last zombie with a cherry bomb.
fn win_level(t: &mut TestBoard) {
    t.board.waves.current_wave = t.board.waves.num_waves;
    t.plant(SeedType::Cherrybomb, 4, 2);
    t.zombie(ZombieType::Normal, 2, 450.0);
}

#[test]
fn a_replayed_level_ends_with_a_money_bag_that_pays_out_with_the_mowers() {
    use super::super::coin::CoinType;
    let Some(mut t) = TestBoard::with_player("1-3", |p| p.finished_adventure = 1) else { return };
    win_level(&mut t);
    assert!(t.run_until(600, |b| b.coins.iter().any(|(_, c)| c.ty == CoinType::MoneyBag)), "the last zombie drops a money bag");
    // Let it land, then click it.
    t.ticks(200);
    let (x, y) = t.board.coins.iter().find(|(_, c)| c.ty == CoinType::MoneyBag).map(|(_, c)| (c.pos_x as i32 + 20, c.pos_y as i32 + 20)).unwrap();
    t.click(x, y);
    assert!(t.board.has_level_award_dropped());
    t.ticks(700);
    assert!(t.money >= 25, "the bag's five gold coins are {}", t.money);
    assert!(t.money >= 25 + 5 * 5, "and each of the five mowers turned into a gold coin: {}", t.money);
}

#[test]
fn a_first_time_level_still_ends_with_its_award() {
    use super::super::coin::CoinType;
    let Some(mut t) = TestBoard::with_player("1-3", |_| {}) else { return };
    win_level(&mut t);
    assert!(t.run_until(600, |b| b.coins.iter().any(|(_, c)| c.ty == CoinType::FinalSeedPacket)));
    assert!(!t.board.coins.iter().any(|(_, c)| c.ty == CoinType::MoneyBag));
}
