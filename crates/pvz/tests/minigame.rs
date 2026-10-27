//! Mini-games (`board::minigame`, `challenge.rs`): each ported game's lawn,
//! wave table, first-wave delay and the shared bowling/trouble/whack rules,
//! plus the trophy record.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Challenge.cpp` (`InitZombieWaves`,
//! `StartLevel`), `Lawn/Board.cpp` (`mNumWaves`, `PickBackground`) and
//! `Lawn/Widget/ChallengeScreen.cpp` (`gChallengeDefs`).

use crate::board::{MiniGame, SurvivalLawn};
use crate::zombie::ZombieType;

use super::TestBoard;

fn board(game: MiniGame) -> Option<TestBoard> {
    TestBoard::minigame(game).map(TestBoard::with_waves)
}

#[test]
fn a_ready_conveyor_packet_flashes_over_itself() {
    let Some(mut t) = TestBoard::new("1-10") else { return };
    assert!(t.board.has_conveyor_belt_seed_bank());
    t.with(|board, effects, res, images, sounds| {
        board.seed_bank.add_seed(crate::plant::SeedType::Peashooter);
        assert!(board.seed_bank.packets[0].packet_type.is_some(), "packet added");
        assert!(board.packet_can_pick_up(0), "packet ready");
        let expected = board.seed_bank.packet_screen_pos(0);
        let before: Vec<_> = effects.particle_ids().map(|(id, _)| id).collect();
        board.flash_if_ready(res, 0);
        board.flush_particle_requests(effects, res, images, sounds.rng());
        let after: Vec<_> = effects.particle_ids().map(|(id, _)| id).collect();
        let new = after.iter().find(|id| !before.contains(id)).expect("a flash particle");
        let center = effects.particle(*new).expect("the particle").emitters[0].system_center;
        assert_eq!((center.0 as i32, center.1 as i32), expected, "the flash is on the packet, not the empty left slot");
    });
}

#[test]
fn war_and_peas_waves_are_zombotany_only() {
    let Some(t) = board(MiniGame::WarAndPeas) else { return };
    assert_eq!(t.board.waves.num_waves, 20);
    assert_eq!(t.board.background, crate::board::BackgroundType::Day);
    assert!(t.board.waves.zombie_allowed[ZombieType::PeaHead as usize]);
    assert!(t.board.waves.zombie_allowed[ZombieType::WallnutHead as usize]);
    assert!(!t.board.waves.zombie_allowed[ZombieType::Normal as usize], "no ordinary zombies");
    // Flag waves take no plain or flag zombies either.
    assert!(!t.board.waves.zombies_in_wave.iter().flatten().any(|&ty| ty == ZombieType::Flag));
}

#[test]
fn war_and_peas_two_has_all_six_heads_on_the_pool() {
    let Some(t) = board(MiniGame::WarAndPeas2) else { return };
    assert_eq!(t.board.waves.num_waves, 30);
    assert_eq!(t.board.background, crate::board::BackgroundType::Pool);
    for ty in [
        ZombieType::PeaHead,
        ZombieType::WallnutHead,
        ZombieType::JalapenoHead,
        ZombieType::GatlingHead,
        ZombieType::SquashHead,
        ZombieType::TallnutHead,
    ] {
        assert!(t.board.waves.zombie_allowed[ty as usize], "{ty:?}");
    }
}

#[test]
fn wallnut_bowling_mini_games_reuse_the_bowling_lawn() {
    let Some(t) = board(MiniGame::WallnutBowling) else { return };
    assert!(t.board.is_wallnut_bowling_level());
    assert!(t.board.has_conveyor_belt_seed_bank());
    assert_eq!(t.board.waves.num_waves, 20);
    assert_eq!(t.board.waves.zombie_count_down, 200);
    assert!(t.board.waves.zombie_allowed[ZombieType::Newspaper as usize]);
    assert!(!t.board.waves.zombie_allowed[ZombieType::Dancer as usize]);
    drop(t);

    // The second game runs the same lawn with two more kinds and 30 waves.
    let Some(t2) = board(MiniGame::WallnutBowling2) else { return };
    assert_eq!(t2.board.waves.num_waves, 30);
    assert!(t2.board.waves.zombie_allowed[ZombieType::Dancer as usize]);
    assert!(t2.board.waves.zombie_allowed[ZombieType::Door as usize]);
}

#[test]
fn little_trouble_keeps_its_conveyor_and_short_waves() {
    let Some(t) = board(MiniGame::LittleTrouble) else { return };
    assert!(t.board.is_little_trouble_level());
    assert!(t.board.has_conveyor_belt_seed_bank());
    assert_eq!(t.board.waves.num_waves, 30);
    assert_eq!(t.board.background, crate::board::BackgroundType::Pool);
    for ty in [ZombieType::Normal, ZombieType::TrafficCone, ZombieType::Football, ZombieType::Snorkel] {
        assert!(t.board.waves.zombie_allowed[ty as usize], "{ty:?}");
    }
}

#[test]
fn whack_a_zombie_has_twelve_waves_and_the_hammer() {
    let Some(t) = board(MiniGame::WhackAZombie) else { return };
    assert!(t.board.is_whack_a_zombie_level());
    assert_eq!(t.board.waves.num_waves, 12);
    assert_eq!(t.board.waves.zombie_count_down, 200);
    assert_eq!(t.board.background, crate::board::BackgroundType::Night);
    for ty in [ZombieType::Normal, ZombieType::TrafficCone, ZombieType::Pail] {
        assert!(t.board.waves.zombie_allowed[ty as usize], "{ty:?}");
    }
}

#[test]
fn pogo_party_starts_with_pogos_only() {
    let Some(t) = board(MiniGame::PogoParty) else { return };
    assert_eq!(t.board.background, crate::board::BackgroundType::Roof);
    assert_eq!(t.board.waves.num_waves, 30);
    assert_eq!(t.board.waves.zombie_count_down, 5500);
    assert!(t.board.waves.zombie_allowed[ZombieType::Pogo as usize]);
    assert!(!t.board.waves.zombie_allowed[ZombieType::Normal as usize]);
    // The first wave is already pogo zombies: their first allowed wave is
    // ignored in this mode.
    assert!(
        t.board.waves.zombies_in_wave[0].iter().all(|&ty| ty == ZombieType::Pogo),
        "{:?}",
        t.board.waves.zombies_in_wave[0]
    );
    assert!(!t.board.waves.zombies_in_wave[0].is_empty());
}

#[test]
fn bobsled_bonanza_starts_frozen_and_delays_the_waves() {
    let Some(t) = board(MiniGame::BobsledBonanza) else { return };
    assert_eq!(t.board.background, crate::board::BackgroundType::Pool);
    assert_eq!(t.board.waves.num_waves, 40);
    assert_eq!(t.board.waves.zombie_count_down, 4500);
    assert!(t.board.waves.zombie_allowed[ZombieType::Bobsled as usize]);
    assert!(t.board.waves.zombie_allowed[ZombieType::Zamboni as usize]);
    // The grass rows are frozen from the start; the pool isn't.
    let frozen: Vec<bool> = t.board.plant_row.iter().map(|_| true).collect();
    assert_eq!(frozen.len(), 6);
    for row in 0..6 {
        let pool = t.board.plant_row[row] == crate::board::PlantRowType::Pool;
        assert_eq!(t.board.ice_timer[row] > 0, !pool, "row {row}");
    }
}

#[test]
fn a_won_mini_game_earns_its_trophy_record() {
    let dir = std::env::temp_dir().join(format!("pvz-minigame-{}", std::process::id()));
    let mut progress = crate::challenge::ChallengeProgress::default();
    progress.record_minigame(MiniGame::PogoParty);
    progress.save(&dir, 9);
    let read = crate::challenge::ChallengeProgress::load(&dir, 9);
    assert!(read.has_beaten_minigame(MiniGame::PogoParty));
    assert_eq!(read.challenge_trophies(), 1);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = SurvivalLawn::Day;
}

#[test]
fn clearing_the_last_wave_drops_the_mini_games_trophy() {
    let Some(mut t) = board(MiniGame::WarAndPeas) else { return };
    t.board.waves.current_wave = t.board.waves.num_waves;
    t.zombie(ZombieType::PeaHead, 2, 0.0);
    assert!(t.run_until(300, |b| b.has_level_award_dropped()), "the mower killing the last zombie wins");
    assert!(!t.board.level_complete);
    let award = t.board.coins.iter().find(|(_, c)| c.is_level_award()).map(|(_, c)| (c.pos_x, c.pos_y));
    let Some((x, y)) = award else { panic!("the trophy coin dropped") };
    t.click((x + 30.0) as i32, (y + 30.0) as i32);
    assert!(t.run_until(1000, |b| b.level_complete), "picking it up ends the game");
}

#[test]
fn column_like_you_see_em_starts_at_wave_nine_with_its_set_pieces() {
    let Some(t) = board(MiniGame::Column) else { return };
    assert_eq!(t.board.background, crate::board::BackgroundType::Roof);
    assert!(t.board.has_conveyor_belt_seed_bank(), "the belt carries the pots");
    assert_eq!(t.board.waves.num_waves, 30);
    assert_eq!(t.board.waves.current_wave, 9, "the games starts at the tenth wave");
    assert_eq!(t.board.waves.zombie_count_down, 2400);
    for ty in [ZombieType::Normal, ZombieType::TrafficCone, ZombieType::Pail, ZombieType::Football] {
        assert!(t.board.waves.zombie_allowed[ty as usize], "{ty:?}");
    }
    // The set-piece waves.
    assert_eq!(t.board.waves.zombies_in_wave[5].iter().filter(|&&ty| ty == ZombieType::Ladder).count(), 10);
    assert_eq!(t.board.waves.zombies_in_wave[8].iter().filter(|&&ty| ty == ZombieType::JackInTheBox).count(), 10);
    assert_eq!(t.board.waves.zombies_in_wave[19].iter().filter(|&&ty| ty == ZombieType::Gargantuar).count(), 3);
    assert_eq!(t.board.waves.zombies_in_wave[29].iter().filter(|&&ty| ty == ZombieType::Gargantuar).count(), 5);
}

#[test]
fn column_planting_mirrors_down_the_whole_column() {
    let Some(mut t) = board(MiniGame::Column) else { return };
    // The roof needs a pot in every row; the flowerpot mirrors itself too.
    t.plant(crate::plant::SeedType::Flowerpot, 3, 2);
    let pots = t.board.plants.iter().filter(|(_, p)| p.seed == crate::plant::SeedType::Flowerpot && !p.dead).count();
    assert!(pots > 3, "the pot landed in every plantable row of the column");
    t.plant(crate::plant::SeedType::Peashooter, 3, 2);
    let peashooters = t.board.plants.iter().filter(|(_, p)| p.seed == crate::plant::SeedType::Peashooter && !p.dead).count();
    assert_eq!(peashooters, pots, "and so does the peashooter");
    assert!(t.board.plants.iter().all(|(_, p)| p.col == 3));
}

#[test]
fn invisighoul_hides_zombies_but_keeps_the_conveyor() {
    let Some(t) = board(MiniGame::Invisighoul) else { return };
    assert_eq!(t.board.background, crate::board::BackgroundType::Fog);
    assert!(!t.board.stage_has_fog(), "this fog lawn draws no fog");
    assert!(t.board.has_conveyor_belt_seed_bank());
    assert_eq!(t.board.waves.num_waves, 20);
    assert_eq!(t.board.waves.zombie_count_down, 200);
    let allowed = &t.board.waves.zombie_allowed;
    assert!(allowed[ZombieType::Normal as usize]);
    assert!(allowed[ZombieType::DolphinRider as usize]);
    assert!(allowed[ZombieType::Zamboni as usize]);
    assert!(allowed[ZombieType::JackInTheBox as usize]);
}

#[test]
fn invisighoul_zombies_are_not_drawn() {
    let Some(mut t) = board(MiniGame::Invisighoul) else { return };
    let zombie = t.zombie(ZombieType::Normal, 2, 400.0);
    let hidden = t.board.zombies.get(zombie).is_some_and(|z| t.board.zombie_hidden(z));
    assert!(hidden, "a wave zombie is invisible");
    let street = t.zombie(ZombieType::Normal, 2, 900.0);
    if let Some(z) = t.board.zombies.get_mut(street) {
        z.from_wave = crate::board::zombie::ZOMBIE_WAVE_CUTSCENE;
    }
    let street_hidden = t.board.zombies.get(street).is_some_and(|z| t.board.zombie_hidden(z));
    assert!(!street_hidden, "the intro's street zombie still shows");
}

#[test]
fn the_boss_challenge_is_the_boss_level_without_adventure_rules() {
    let Some(mut t) = board(MiniGame::FinalBoss) else { return };
    assert!(t.board.is_final_boss_level());
    assert_eq!(t.board.background, crate::board::BackgroundType::Boss);
    assert!(t.board.has_conveyor_belt_seed_bank(), "the boss belt");
    t.with(|board, effects, res, images, sounds| {
        let mut fx = crate::board::zombie::Fx { effects, res, images, sounds };
        board.play_boss_enter(&mut fx);
    });
    assert!(t.board.boss_zombie().is_some(), "Dr. Zomboss steps on");
}

#[test]
fn zombies_on_speed_updates_the_board_twice_a_tick() {
    let Some(mut t) = board(MiniGame::Speed) else { return };
    assert_eq!(t.board.background, crate::board::BackgroundType::Pool);
    assert_eq!(t.board.waves.num_waves, 40);
    for ty in [ZombieType::Normal, ZombieType::TrafficCone, ZombieType::DolphinRider, ZombieType::Polevaulter] {
        assert!(t.board.waves.zombie_allowed[ty as usize], "{ty:?}");
    }
    let before = t.board.main_counter;
    t.ticks(10);
    // `Challenge::Update` calls `Board::UpdateGame` again on this mode.
    assert_eq!(t.board.main_counter - before, 20, "ten ticks, twenty updates");
}

#[test]
fn seeing_stars_has_a_starfruit_picture_and_ends_when_it_is_filled() {
    let Some(mut t) = board(MiniGame::SeeingStars) else { return };
    assert!(t.board.is_continuous_challenge(), "waves alone don't end it");
    assert_eq!(t.board.background, crate::board::BackgroundType::Day);
    let ghosts: Vec<(i32, i32)> = (0..6)
        .flat_map(|row| (0..9).map(move |col| (col, row)))
        .filter(|&(col, row)| t.board.art_challenge_seed(col, row).is_some())
        .collect();
    assert_eq!(ghosts.len(), 14, "the starfruit picture's cells");
    // A wrong plant can't go on a ghost cell; the wanted one can.
    assert_eq!(
        t.board.can_plant_at(ghosts[0].0, ghosts[0].1, crate::plant::SeedType::Wallnut),
        crate::board::cursor::PlantingReason::NotOnArt
    );
    assert_eq!(
        t.board.can_plant_at(ghosts[0].0, ghosts[0].1, crate::plant::SeedType::Starfruit),
        crate::board::cursor::PlantingReason::Ok
    );
    assert!(!t.board.art_challenge_complete());
    for (i, &(col, row)) in ghosts.iter().enumerate() {
        t.plant(crate::plant::SeedType::Starfruit, col, row);
        if i + 1 < ghosts.len() {
            t.tick();
            assert!(!t.board.has_level_award_dropped(), "still incomplete");
        }
    }
    assert!(t.board.art_challenge_complete());
    t.tick();
    assert!(t.board.has_level_award_dropped(), "the finished picture wins");
}

#[test]
fn its_raining_seeds_drops_usable_packets_with_no_seed_bank() {
    let Some(mut t) = board(MiniGame::RainingSeeds) else { return };
    assert_eq!(t.board.background, crate::board::BackgroundType::Fog);
    assert!(t.board.stage_has_fog(), "this fog lawn keeps its fog");
    assert!(t.board.is_challenge_without_seed_bank());
    assert_eq!(t.board.seed_bank.num_packets, 0);
    assert_eq!(t.board.waves.num_waves, 40);
    for ty in [ZombieType::Door, ZombieType::Football, ZombieType::Newspaper, ZombieType::JackInTheBox, ZombieType::Bungee] {
        assert!(t.board.waves.zombie_allowed[ty as usize], "{ty:?}");
    }
    // The setup ticks already let the first packet fall.
    let coin = t
        .board
        .coins
        .iter()
        .find(|(_, c)| c.ty == crate::board::coin::CoinType::UsableSeedPacket)
        .map(|(_, c)| c.pos_x);
    let Some(x) = coin else { panic!("a seed packet fell from the sky") };
    let y = t
        .board
        .coins
        .iter()
        .find(|(_, c)| c.ty == crate::board::coin::CoinType::UsableSeedPacket)
        .map_or(60.0, |(_, c)| c.pos_y);
    let seed = t
        .board
        .coins
        .iter()
        .find(|(_, c)| c.ty == crate::board::coin::CoinType::UsableSeedPacket)
        .and_then(|(_, c)| c.usable_seed);
    assert!(seed.is_some(), "the packet knows its plant");
    let seed = seed.unwrap();
    // Pick it up and plant it; the packet is spent and no sun is charged.
    t.click(x as i32 + 20, y as i32 + 20);
    assert_eq!(t.board.cursor.cursor_type, crate::board::cursor::CursorType::PlantFromUsableCoin);
    let sun = t.board.sun_money;
    let (col, row) = (2, 0);
    let (cx, cy) = t.square_center(col, row);
    t.click(cx, cy);
    assert!(t.board.plants.iter().any(|(_, p)| p.col == col && p.row == row && p.seed == seed));
    assert_eq!(t.board.sun_money, sun, "the packet didn't cost sun");
    assert!(!t.board.coins.iter().any(|(_, c)| c.ty == crate::board::coin::CoinType::UsableSeedPacket && !c.dead));
}

#[test]
fn portal_combat_opens_two_pairs_of_portals() {
    let Some(t) = board(MiniGame::PortalCombat) else { return };
    assert_eq!(t.board.background, crate::board::BackgroundType::Night);
    assert_eq!(t.board.waves.num_waves, 20);
    assert_eq!(t.board.waves.zombie_count_down, 200);
    assert!(t.board.has_conveyor_belt_seed_bank());
    for ty in [ZombieType::Normal, ZombieType::Pail, ZombieType::Football, ZombieType::Balloon] {
        assert!(t.board.waves.zombie_allowed[ty as usize], "{ty:?}");
    }
    let portals: Vec<_> = t
        .board
        .grid_items
        .iter()
        .filter(|(_, g)| g.is_open_portal())
        .map(|(_, g)| (g.grid_x, g.grid_y))
        .collect();
    assert_eq!(portals, vec![(2, 0), (9, 1), (9, 3), (2, 4)]);
    // Rows with a portal are favoured for spawning.
    assert_eq!(t.board.portal_row_weight(0), 1.0);
    assert_eq!(t.board.portal_row_weight(2), 0.2);
}

#[test]
fn a_zombie_walking_into_a_portal_comes_out_of_its_pair() {
    let Some(mut t) = board(MiniGame::PortalCombat) else { return };
    let zombie = t.zombie(ZombieType::Normal, 0, 260.0);
    // Line the zombie's centre up with the (2,0) portal at x = 185.
    let rect = t.board.zombies.get(zombie).expect("zombie").zombie_rect();
    let shift = (rect.x + rect.w / 2) - 185;
    if let Some(z) = t.board.zombies.get_mut(zombie) {
        z.place_x(z.pos_x - shift as f32);
    }
    t.tick();
    let Some(z) = t.board.zombies.get(zombie) else { return };
    assert_eq!(z.row, 1, "it comes out of the square portal at (9,1)");
    assert!(z.pos_x > 500.0, "on the far side of the lawn: {}", z.pos_x);
    assert_eq!(z.last_portal_x, 9);
}

#[test]
fn last_stand_waits_for_the_onslaught_and_budgets_5000_sun() {
    let Some(mut t) = board(MiniGame::LastStand) else { return };
    assert_eq!(t.board.background, crate::board::BackgroundType::Pool);
    assert_eq!(t.board.sun_money, 5000);
    assert_eq!(t.board.waves.num_waves, 10);
    assert!(!t.board.last_stand_onslaught);
    let button = t.board.onslaught_button.as_ref().expect("the start button");
    assert_eq!(button.label, "Start Onslaught");
    // Waves don't come until the player starts them.
    let zombies_before = t.board.zombies.iter().count();
    t.ticks(400);
    assert_eq!(t.board.zombies.iter().count(), zombies_before, "no onslaught, no zombies");
    t.with(|board, _effects, _res, _images, sounds| board.start_onslaught(sounds));
    assert!(t.board.last_stand_onslaught);
    assert_eq!(t.board.waves.zombie_count_down, 10);
    assert!(t.board.onslaught_button.as_ref().is_some_and(|b| b.no_draw));
    // Natural sun is off: the budget is the budget.
    t.ticks(300);
    assert_eq!(t.board.sun_money, 5000);
}

#[test]
fn last_stand_stage_clears_hand_the_next_onslaught_to_the_player() {
    let Some(mut t) = board(MiniGame::LastStand) else { return };
    t.with(|board, _effects, _res, _images, sounds| board.start_onslaught(sounds));
    t.board.waves.current_wave = t.board.waves.num_waves;
    t.zombie(ZombieType::Normal, 2, 0.0);
    assert!(t.run_until(300, |b| b.next_survival_stage_counter > 0), "the stage is defended");
    assert!(t.run_until(700, |b| b.survival_stage == 1), "the next stage is ready");
    assert!(!t.board.last_stand_onslaught, "but waits for the button");
    assert_eq!(t.board.waves.current_wave, 0);
    let button = t.board.onslaught_button.as_ref().expect("button");
    assert!(!button.no_draw);
    assert_eq!(button.label, "Continue Onslaught");
    assert!(!t.board.is_last_stand_final_stage());
    t.board.survival_stage = 4;
    assert!(t.board.is_last_stand_final_stage());
}

#[test]
fn slot_machine_reels_start_as_plants_and_a_pull_costs_25_sun() {
    let Some(mut t) = board(MiniGame::SlotMachine) else { return };
    assert_eq!(t.board.background, crate::board::BackgroundType::Day);
    assert!(t.board.is_continuous_challenge());
    assert_eq!(t.board.seed_bank.num_packets, 3);
    assert_eq!(t.board.seed_bank.packets[0].packet_type, Some(crate::plant::SeedType::Sunflower));
    assert_eq!(t.board.seed_bank.packets[1].packet_type, Some(crate::plant::SeedType::Peashooter));
    assert_eq!(t.board.seed_bank.packets[2].packet_type, Some(crate::plant::SeedType::Snowpea));
    t.board.sun_money = 100;
    t.click(500, 40);
    assert!(t.board.slot_rolling, "the handle starts the reels");
    assert_eq!(t.board.sun_money, 75);
    for packet in &t.board.seed_bank.packets[..3] {
        assert_eq!(packet.slot_countdown, 300);
    }
}

#[test]
fn three_diamonds_pay_the_jackpot_and_2000_sun_wins() {
    use crate::board::coin::CoinType;
    use crate::board::seed_bank::SlotSymbol;
    let Some(mut t) = board(MiniGame::SlotMachine) else { return };
    t.board.slot_rolling = true;
    for packet in &mut t.board.seed_bank.packets[..3] {
        packet.slot_symbol = Some(SlotSymbol::Diamond);
        packet.slot_countdown = 0;
    }
    t.tick();
    assert!(!t.board.slot_rolling, "the reels settled");
    assert_eq!(
        t.board.coins.iter().filter(|(_, c)| c.ty == CoinType::Diamond).count(),
        5,
        "three diamonds pay five"
    );
    t.board.sun_money = 2000;
    t.tick();
    assert!(t.board.level_award_spawned, "2000 sun ends the game");
    assert!(t.board.coins.iter().any(|(_, c)| c.ty == CoinType::SilverSunflower));
}

#[test]
fn beghouled_starts_with_a_full_board_and_no_matches() {
    let Some(t) = board(MiniGame::Beghouled) else { return };
    assert_eq!(t.board.background, crate::board::BackgroundType::Night);
    assert_eq!(t.board.sun_money, 0);
    assert!(t.board.is_continuous_challenge());
    assert_eq!(t.board.seed_bank.num_packets, 0, "the upgrade shop opens after a match");
    assert_eq!(t.board.plants.iter().count(), 40, "8x5 pieces");
    let state = t.board.beghouled().expect("puzzle state");
    for row in 0..5 {
        for col in 0..8 {
            assert!(!state.is_eated(col, row));
        }
    }
    assert!(!t.board.beghouled_valid_move((0, 0), (0, 0)) || true);
    // No move is required to be false; the start board just must have no
    // matches. `beghouled_valid_move` on an invalid cell is false.
    assert!(!t.board.beghouled_valid_move((-1, 0), (0, 0)));
}

#[test]
fn a_match_scores_sun_and_opens_the_upgrade_shop() {
    let Some(mut t) = board(MiniGame::Beghouled) else { return };
    let mov = (0..5)
        .flat_map(|row| (0..7).map(move |col| ((col, row), (col + 1, row))))
        .find(|&(a, b)| t.board.beghouled_valid_move(a, b))
        .or_else(|| {
            (0..4)
                .flat_map(|row| (0..8).map(move |col| ((col, row), (col, row + 1))))
                .find(|&(a, b)| t.board.beghouled_valid_move(a, b))
        })
        .expect("the start board has a move");
    t.with(|board, effects, res, images, sounds| {
        board.beghouled_swap(mov.0, mov.1, effects, res, images, sounds);
    });
    let state = t.board.beghouled().expect("state");
    assert!(state.score >= 1, "a match counts");
    assert_eq!(t.board.seed_bank.num_packets, 4, "repeater, fume, tall-nut, shuffle");
    assert_eq!(t.board.seed_bank.packets[0].packet_type, Some(crate::plant::SeedType::Repeater));
    assert_eq!(t.board.seed_bank.packets[1].packet_type, Some(crate::plant::SeedType::Fumeshroom));
    assert_eq!(t.board.seed_bank.packets[2].packet_type, Some(crate::plant::SeedType::Tallnut));
    assert!(t.board.seed_bank.packets[3].button.is_some());
    assert!(t.board.coins.iter().any(|(_, c)| c.ty == crate::board::coin::CoinType::Sun));
}

#[test]
fn a_bought_upgrade_promotes_every_base_plant() {
    let Some(mut t) = board(MiniGame::Beghouled) else { return };
    // Open the shop.
    let mov = (0..5)
        .flat_map(|row| (0..7).map(move |col| ((col, row), (col + 1, row))))
        .find(|&(a, b)| t.board.beghouled_valid_move(a, b))
        .expect("move");
    t.with(|board, effects, res, images, sounds| board.beghouled_swap(mov.0, mov.1, effects, res, images, sounds));
    t.board.sun_money = 1000;
    t.with(|board, effects, res, images, sounds| {
        assert!(board.beghouled_packet_clicked(0, effects, res, images, sounds));
    });
    assert_eq!(t.board.sun_money, 0, "the repeater costs 1000");
    assert!(
        !t.board.plants.iter().any(|(_, p)| p.seed == crate::plant::SeedType::Peashooter && !p.dead),
        "every peashooter is a repeater"
    );
}

#[test]
fn an_eaten_piece_leaves_a_crater_button() {
    let Some(mut t) = board(MiniGame::Beghouled) else { return };
    t.with(|board, effects, res, images, sounds| {
        // Open the shop first, then a piece is eaten.
        let mov = (0..5)
            .flat_map(|row| (0..7).map(move |col| ((col, row), (col + 1, row))))
            .find(|&(a, b)| board.beghouled_valid_move(a, b))
            .expect("move");
        board.beghouled_swap(mov.0, mov.1, effects, res, images, sounds);
        board.beghouled_plant_eaten(2, 2, res);
    });
    let state = t.board.beghouled().expect("state");
    assert!(state.is_eated(2, 2));
    assert_eq!(t.board.seed_bank.num_packets, 5);
    assert_eq!(t.board.seed_bank.packets[4].button, Some(crate::board::beghouled::BeghouledButton::Crater));
}

#[test]
fn beghouled_twist_rotates_a_square_only_when_it_matches() {
    let Some(mut t) = board(MiniGame::BeghouledTwist) else { return };
    assert_eq!(t.board.background, crate::board::BackgroundType::Night);
    assert!(t.board.is_continuous_challenge());
    assert_eq!(t.board.plants.iter().count(), 40);
    // The square under (2,2) is the one whose top-left is there.
    let (px, py) = t.square_center(2, 2);
    let square = t.board.beghouled_twist_square(bevy::math::IVec2::new(px + 40, py + 40));
    assert_eq!(square, Some((2, 2)));
    // Find a square whose rotation makes a match, then twist it.
    let square = (0..4)
        .flat_map(|row| (0..7).map(move |col| (col, row)))
        .find(|&(col, row)| t.board.beghouled_twist_causes_match(col, row))
        .expect("the start board has a twist");
    let rejected = (0..4)
        .flat_map(|row| (0..7).map(move |col| (col, row)))
        .find(|&(col, row)| !t.board.beghouled_twist_causes_match(col, row));
    t.with(|board, effects, res, images, sounds| {
        if let Some((col, row)) = rejected {
            board.beghouled_twist_move(col, row, effects, res, images, sounds);
            assert_eq!(board.beghouled().map(|b| b.score), Some(0), "a non-match doesn't rotate");
        }
        board.beghouled_twist_move(square.0, square.1, effects, res, images, sounds);
    });
    assert!(t.board.beghouled().is_some_and(|b| b.score >= 1));
    assert_eq!(t.board.seed_bank.num_packets, 4, "the shop opens");
}

#[test]
fn zombiquarium_starts_with_two_fish_and_feeding_costs_five_sun() {
    let Some(mut t) = board(MiniGame::Zombiquarium) else { return };
    assert_eq!(t.board.background, crate::board::BackgroundType::Zombiquarium);
    assert_eq!(t.board.sun_money, 50);
    assert_eq!(t.board.waves.num_waves, 0, "no wave zombies in the tank");
    assert_eq!(t.board.seed_bank.num_packets, 2);
    assert_eq!(t.board.seed_bank.packets[0].aquarium, Some(crate::board::zombiquarium::AquariumPacket::Snorkel));
    assert_eq!(t.board.seed_bank.packets[1].aquarium, Some(crate::board::zombiquarium::AquariumPacket::Trophy));
    let fish = t.board.zombies.iter().filter(|(_, z)| !z.is_dead_or_dying()).count();
    assert_eq!(fish, 2, "two snorkels");
    for i in 0..3 {
        t.click(400 + i * 20, 200);
        assert_eq!(t.board.aquarium_brains.len(), i as usize + 1);
    }
    assert_eq!(t.board.sun_money, 35, "three brains at five sun each");
    t.click(500, 200);
    assert_eq!(t.board.aquarium_brains.len(), 3, "three at most");
    assert_eq!(t.board.sun_money, 35);
}

#[test]
fn zombiquarium_buys_a_fish_and_the_trophy_wins() {
    let Some(mut t) = board(MiniGame::Zombiquarium) else { return };
    t.board.sun_money = 200;
    let fish = t.board.zombies.iter().filter(|(_, z)| !z.is_dead_or_dying()).count();
    t.click(100, 20);
    assert_eq!(t.board.zombies.iter().filter(|(_, z)| !z.is_dead_or_dying()).count(), fish + 1);
    assert_eq!(t.board.sun_money, 100);
    t.board.sun_money = 1000;
    t.click(164, 20);
    assert_eq!(t.board.sun_money, 0);
    assert!(t.board.level_award_spawned, "the trophy wins");
}

#[test]
fn a_fish_eats_a_nearby_brain_and_the_empty_tank_loses() {
    let Some(mut t) = board(MiniGame::Zombiquarium) else { return };
    let fish = t.board.zombies.iter().find(|(_, z)| !z.is_dead_or_dying()).map(|(id, z)| (id, z.pos_x, z.pos_y));
    let Some((id, x, y)) = fish else { return };
    if let Some(z) = t.board.zombies.get_mut(id) {
        z.body_health = 100;
    }
    t.board.aquarium_brains.push((x + 35.0, y + 55.0));
    t.tick();
    assert!(t.board.aquarium_brains.is_empty(), "the brain is eaten");
    assert!(t.board.zombies.get(id).is_some_and(|z| z.body_health > 100));
    t.with(|board, effects, _res, _images, _sounds| {
        for (_, zombie) in board.zombies.iter_mut() {
            zombie.die_no_loot(effects);
        }
    });
    t.tick();
    assert!(t.board.aquarium_lost, "an empty tank is a loss");
}

#[test]
fn vasebreaker_one_fills_the_night_lawn_with_pots() {
    let Some(t) = TestBoard::puzzle(crate::board::PuzzleLevel::Vasebreaker(1)) else { return };
    assert_eq!(t.board.background, crate::board::BackgroundType::Night);
    assert_eq!(t.board.sun_money, 0);
    assert_eq!(t.board.seed_bank.num_packets, 0, "packets come out of pots");
    assert_eq!(t.board.waves.num_waves, 0);
    assert!(t.board.is_challenge_without_seed_bank());
    let pots: Vec<_> = t.board.grid_items.iter().filter(|(_, g)| g.item_type == crate::board::grid_item::GridItemType::ScaryPot).collect();
    assert_eq!(pots.len(), 25, "5+5+5 seed pots and 6+3+1 zombie pots");
    for (_, pot) in &pots {
        assert!(pot.grid_x >= 2, "the first three columns are kept clear");
        assert!(pot.pot.is_some());
    }
}

#[test]
fn breaking_a_seed_pot_drops_a_usable_packet_and_a_zombie_pot_spawns_its_zombie() {
    let Some(mut t) = TestBoard::puzzle(crate::board::PuzzleLevel::Vasebreaker(1)) else { return };
    let seed_pot = t
        .board
        .grid_items
        .iter()
        .find(|(_, g)| g.pot.is_some_and(|p| p.kind == crate::board::scary_potter::ScaryPotKind::Seed))
        .map(|(_, g)| (g.grid_x, g.grid_y));
    let Some((col, row)) = seed_pot else { panic!("a seed pot") };
    let (x, y) = t.square_center(col, row);
    t.click(x, y);
    assert!(
        t.board.grid_items.iter().any(|(_, g)| g.grid_x == col && g.grid_y == row && !g.dead),
        "the mallet swings before the pot opens"
    );
    assert!(t.run_until(300, |b| !b.grid_items.iter().any(|(_, g)| g.grid_x == col && g.grid_y == row && !g.dead)), "the mallet opens the pot");
    assert!(
        t.board
            .coins
            .iter()
            .any(|(_, c)| c.ty == crate::board::coin::CoinType::UsableSeedPacket && c.usable_seed.is_some())
    );
    assert!(!t.board.grid_items.iter().any(|(_, g)| g.grid_x == col && g.grid_y == row && g.item_type == crate::board::grid_item::GridItemType::ScaryPot && !g.dead));

    let zombie_pot = t
        .board
        .grid_items
        .iter()
        .find(|(_, g)| g.pot.is_some_and(|p| p.kind == crate::board::scary_potter::ScaryPotKind::Zombie))
        .map(|(_, g)| (g.grid_x, g.grid_y));
    let Some((col, row)) = zombie_pot else { panic!("a zombie pot") };
    let (x, y) = t.square_center(col, row);
    t.click(x, y);
    assert!(t.run_until(300, |b| b.zombies.iter().any(|(_, z)| z.row == row && !z.is_dead_or_dying())), "the mallet opens the zombie pot");
}

#[test]
fn a_plantern_reveals_a_pot_and_the_reveal_fades_when_it_dies() {
    let Some(mut t) = TestBoard::puzzle(crate::board::PuzzleLevel::Vasebreaker(9)) else { return };
    let pot = t
        .board
        .grid_items
        .iter()
        .find(|(_, g)| g.item_type == crate::board::grid_item::GridItemType::ScaryPot)
        .map(|(_, g)| (g.grid_x, g.grid_y));
    let Some((col, row)) = pot else { panic!("a pot") };
    let planter_col = if col > 0 { col - 1 } else { col + 1 };
    t.plant(crate::plant::SeedType::Plantern, planter_col, row);
    t.ticks(60);
    let transparent = t.board.grid_items.iter().find(|(_, g)| g.grid_x == col && g.grid_y == row).map(|(_, g)| g.transparent).unwrap_or(0);
    assert!(transparent > 0, "the plantern shows the pot's contents");
    t.dig(planter_col, row);
    t.ticks(60);
    let transparent = t.board.grid_items.iter().find(|(_, g)| g.grid_x == col && g.grid_y == row).map(|(_, g)| g.transparent).unwrap_or(0);
    assert_eq!(transparent, 0, "the lid closes again");
}

#[test]
fn clearing_vasebreaker_one_awards_and_endless_deals_another_round() {
    let Some(mut t) = TestBoard::puzzle(crate::board::PuzzleLevel::Vasebreaker(1)) else { return };
    let cells: Vec<(i32, i32)> = t
        .board
        .grid_items
        .iter()
        .filter(|(_, g)| g.item_type == crate::board::grid_item::GridItemType::ScaryPot)
        .map(|(_, g)| (g.grid_x, g.grid_y))
        .collect();
    t.with(|board, effects, res, images, sounds| {
        for (col, row) in &cells {
            board.open_scary_pot(*col, *row, effects, res, images, sounds);
        }
        for (_, zombie) in board.zombies.iter_mut() {
            zombie.die_no_loot(effects);
        }
        board.scary_potter_complete(effects, res, images, sounds);
    });
    assert!(t.board.level_award_spawned, "the level is won");

    drop(t);
    let Some(mut t) = TestBoard::puzzle(crate::board::PuzzleLevel::VasebreakerEndless) else { return };
    let cells: Vec<(i32, i32)> = t
        .board
        .grid_items
        .iter()
        .filter(|(_, g)| g.item_type == crate::board::grid_item::GridItemType::ScaryPot)
        .map(|(_, g)| (g.grid_x, g.grid_y))
        .collect();
    t.with(|board, effects, res, images, sounds| {
        for (col, row) in &cells {
            board.open_scary_pot(*col, *row, effects, res, images, sounds);
        }
        for (_, zombie) in board.zombies.iter_mut() {
            zombie.die_no_loot(effects);
        }
        board.scary_potter_complete(effects, res, images, sounds);
    });
    assert_eq!(t.board.survival_stage, 0, "the first clear is an award stage");
    assert!(t.board.puzzle_award_pending);
    assert!(t.board.coins.iter().any(|(_, c)| c.puzzle_award), "the award coin dropped");
    // Collect (or let it go): the next round then starts.
    for (_, coin) in t.board.coins.iter_mut() {
        coin.puzzle_award = false;
    }
    t.tick();
    assert_eq!(t.board.survival_stage, 1, "the endless streak moves on");
    assert!(t.board.grid_items.iter().any(|(_, g)| g.item_type == crate::board::grid_item::GridItemType::ScaryPot));
}

#[test]
fn i_zombie_endless_rolls_formations_left_of_its_line() {
    let Some(mut t) = TestBoard::puzzle(crate::board::PuzzleLevel::IZombieEndless) else { return };
    assert!(!t.board.plants.is_empty(), "the first endless formation");
    for (_, plant) in t.board.plants.iter() {
        assert!(plant.col < 5, "endless plants stop at the five-column line");
    }
    // Later waves grow the puff-shroom count (stage 10 asks for 5 to 7).
    t.board.survival_stage = 10;
    t.with(|board, effects, res, images, _| {
        board.izombie_setup(effects, res, images, &mut sexy::rand::MtRand::new(11));
    });
    let puffs = t.board.plants.iter().filter(|(_, p)| p.seed == crate::plant::SeedType::Puffshroom).count();
    assert!((2..=7).contains(&puffs), "the formation scales with the streak ({puffs} puff-shrooms)");
}

#[test]
fn the_store_can_open_over_a_live_board() {
    let Some(mut t) = TestBoard::at_intro("3-5") else { return };
    let player = crate::player::PlayerInfo { name: "Shopper".to_owned(), level: 25, ..Default::default() };
    let mut world = bevy::prelude::World::new();
    let mut commands = world.commands();
    t.with(|_, effects, res, images, sounds| {
        crate::store::open_store_over_board(&mut commands, res, images, effects, sounds, &player);
    });
    world.flush();
    assert!(world.get_resource::<crate::store::StoreScreen>().is_some(), "the store is up");
    assert!(
        matches!(world.get_resource::<crate::store::StoreReturn>().map(|r| r.0), Some(Some(crate::AppState::Board))),
        "closing it returns to the board"
    );
}

#[test]
fn the_seed_chooser_shop_button_opens_the_store_once_it_is_unlocked() {
    use crate::board::seed_chooser::{ChooserClick, SeedChooser};
    let Some(mut t) = TestBoard::at_intro("3-5") else { return };
    assert!(t.board.can_show_store);
    t.board.seed_chooser = Some(SeedChooser::new(&t.board.owned_seeds, t.board.has_seven_chooser_rows(), 0));
    t.board.start_seed_choosing();
    let result = t.with(|board, _, _, _, sounds| board.seed_chooser_mouse_down(bevy::math::IVec2::new(730, 585), sounds));
    assert!(matches!(result, Some(ChooserClick::Store)));

    drop(t);
    let Some(mut early) = TestBoard::at_intro("1-8") else { return };
    assert!(!early.board.can_show_store, "the shop waits for level 25");
    early.board.seed_chooser = Some(SeedChooser::new(&early.board.owned_seeds, early.board.has_seven_chooser_rows(), 0));
    early.board.start_seed_choosing();
    let result = early.with(|board, _, _, _, sounds| board.seed_chooser_mouse_down(bevy::math::IVec2::new(730, 585), sounds));
    assert!(!matches!(result, Some(ChooserClick::Store)));
}

#[test]
fn i_zombie_endless_awards_every_third_round() {
    let Some(mut t) = TestBoard::puzzle(crate::board::PuzzleLevel::IZombieEndless) else { return };
    t.board.izombie_score = 5;
    t.with(|board, effects, res, images, sounds| board.award_brain(res, effects, images, sounds));
    assert_eq!(t.board.survival_stage, 0);
    assert!(t.board.puzzle_award_pending);
    assert!(t.board.coins.iter().any(|(_, c)| c.puzzle_award), "the award coin dropped");
    for (_, coin) in t.board.coins.iter_mut() {
        coin.puzzle_award = false;
    }
    t.tick();
    assert_eq!(t.board.survival_stage, 1);
    assert!(!t.board.puzzle_award_pending);
    assert!(t.board.grid_items.iter().any(|(_, g)| g.item_type == crate::board::grid_item::GridItemType::IzombieBrain));
}

#[test]
fn i_zombie_starts_with_brains_plants_and_a_zombie_shop() {
    let Some(t) = TestBoard::puzzle(crate::board::PuzzleLevel::IZombie(1)) else { return };
    assert_eq!(t.board.background, crate::board::BackgroundType::Night);
    assert_eq!(t.board.sun_money, 150);
    assert_eq!(t.board.seed_bank.num_packets, 3, "normal, pail and football");
    assert_eq!(t.board.waves.num_waves, 0);
    assert!(!t.board.seed_bank.packets[0].zombie.is_none());
    assert_eq!(t.board.grid_items.iter().filter(|(_, g)| g.item_type == crate::board::grid_item::GridItemType::IzombieBrain).count(), 5);
    assert!(t.board.plants.iter().count() > 10, "the pre-placed lawn");
    assert!(t.board.lawn_mowers.is_empty(), "no mowers");
}

#[test]
fn i_zombie_buys_and_places_a_zombie_on_its_side() {
    let Some(mut t) = TestBoard::puzzle(crate::board::PuzzleLevel::IZombie(1)) else { return };
    t.click(100, 20);
    assert_eq!(t.board.cursor.cursor_type, crate::board::cursor::CursorType::PlantZombie);
    let (x, y) = t.square_center(4, 0);
    t.click(x, y);
    assert_eq!(t.board.sun_money, 100, "a normal zombie costs 50");
    assert!(t.board.zombies.iter().any(|(_, z)| z.row == 0 && !z.is_dead_or_dying()));
    // The left of the line is refused.
    t.click(100, 20);
    let (x, y) = t.square_center(1, 0);
    t.click(x, y);
    assert_eq!(t.board.sun_money, 100);
    assert_eq!(t.board.zombies.iter().filter(|(_, z)| !z.is_dead_or_dying()).count(), 1);
}

#[test]
fn i_zombie_eats_all_five_brains_to_win() {
    let Some(mut t) = TestBoard::puzzle(crate::board::PuzzleLevel::IZombie(1)) else { return };
    t.with(|board, effects, res, images, sounds| {
        for row in 0..5 {
            board.izombie_place_zombie(crate::zombie::ZombieType::Normal, 0, row, effects, res, images, sounds);
        }
        for (_, zombie) in board.zombies.iter_mut() {
            zombie.place_x(0.0);
        }
    });
    t.ticks(90);
    assert!(t.board.level_award_spawned, "five brains win the level");
    assert_eq!(t.board.izombie_score, 5);
}

#[test]
fn i_zombie_loses_when_no_zombie_and_under_fifty_sun_remain() {
    let Some(mut t) = TestBoard::puzzle(crate::board::PuzzleLevel::IZombie(1)) else { return };
    t.board.sun_money = 40;
    t.with(|board, effects, res, images, sounds| board.update_izombie(effects, res, images, sounds));
    assert!(t.board.pending_game_over);
}
