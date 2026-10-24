//! Level-specific features: the night levels' Crazy Dave talks and seed
//! slot offer, 2-10's conveyor belt, and 2-5's whack-a-zombie.

use crate::plant::SeedType;
use crate::player::PlayerInfo;
use crate::zombie::ZombieType;

use super::super::conveyor::mini_boss_seeds;
use super::super::cursor::CursorType;
use super::super::zombie::ZombiePhase;
use super::super::{STORE_ITEM_PACKET_UPGRADE, cutscene};
use super::test_board;

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
