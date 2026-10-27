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
