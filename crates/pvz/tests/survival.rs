//! Survival (`board::survival`): the stage tables, the flag arithmetic and
//! the "more zombies" run from one stage into the next.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Challenge.cpp`
//! (`InitZombieWaves`, `InitZombieWavesSurvival`), `Lawn/Board.cpp`
//! (`GetSurvivalFlagsCompleted`, `IsFinalSurvivalStage`, `FadeOutLevel`) and
//! `Lawn/Widget/ChallengeScreen.cpp` (`AccomplishmentsNeeded`).

use sexy::rand::MtRand;

use crate::board::{SurvivalDifficulty, SurvivalLawn};
use crate::zombie::ZombieType;

use super::TestBoard;

/// Every survival mode `TestBoard::survival` supports here.
fn day_normal() -> Option<TestBoard> {
    TestBoard::survival(SurvivalLawn::Day, SurvivalDifficulty::Normal)
}

#[test]
fn a_normal_run_is_ten_waves_of_plain_and_cone_zombies() {
    let Some(t) = day_normal() else { return };
    assert_eq!(t.board.waves.num_waves, 10, "ten waves to a stage");
    assert_eq!(t.board.survival_stage, 0);
    assert!(!t.board.is_final_survival_stage(), "five stages make a run");
    assert_eq!(t.board.survival_flags_completed(), 0);
    // Stage zero's table: normals and cones, plus pails on hard and endless.
    let allowed: Vec<bool> = t.board.waves.zombie_allowed.to_vec();
    assert!(allowed[ZombieType::Normal as usize]);
    assert!(allowed[ZombieType::TrafficCone as usize]);
    assert!(!allowed[ZombieType::Pail as usize], "normal starts without pails");
    assert!(!allowed[ZombieType::Bungee as usize], "no bungees before endless");
    assert!(!allowed[ZombieType::Gargantuar as usize]);
}

#[test]
fn hard_runs_have_twenty_waves_and_start_with_pails() {
    let Some(t) = TestBoard::survival(SurvivalLawn::Pool, SurvivalDifficulty::Hard) else { return };
    assert_eq!(t.board.waves.num_waves, 20);
    assert!(t.board.waves.zombie_allowed[ZombieType::Pail as usize]);
    assert!(!t.board.is_final_survival_stage());
}

#[test]
fn later_stages_add_kinds_the_stage_allows() {
    let Some(mut t) = day_normal() else { return };
    t.board.survival_stage = 2;
    t.with(|board, effects, res, images, sounds| {
        let fx = crate::board::zombie::Fx { effects, res, images, sounds };
        board.init_survival_waves(fx.sounds.rng());
    });
    let allowed = &t.board.waves.zombie_allowed;
    assert_eq!(t.board.waves.num_waves, 10, "the difficulty still sets the stage length");
    // Normal survival never goes past the snorkel.
    for (i, &is_allowed) in allowed.iter().enumerate().skip(ZombieType::Zamboni as usize) {
        assert!(!is_allowed, "{:?} is beyond normal survival", ZombieType::from_index(i));
    }
    assert!(allowed[ZombieType::Normal as usize]);
    // At least the fixed pair and something extra.
    assert!(allowed.iter().filter(|&&a| a).count() >= 3);
}

#[test]
fn endless_keeps_going_after_five_stages() {
    let Some(mut t) = TestBoard::survival(SurvivalLawn::Roof, SurvivalDifficulty::Endless) else { return };
    t.board.survival_stage = 7;
    assert!(!t.board.is_final_survival_stage(), "an endless run has no last stage");
    assert_eq!(t.board.survival_flags_completed(), 14, "two flags per twenty-wave stage");
    assert_eq!(t.board.waves.num_waves, 20);
}

#[test]
fn the_flags_are_counted_per_wave_and_per_finished_stage() {
    let Some(mut t) = day_normal() else { return };
    t.board.waves.current_wave = 5;
    assert_eq!(t.board.survival_flags_completed(), 0);
    // The tenth wave raises the flag; counting it as done waits for the
    // stage to clear.
    t.board.waves.current_wave = 10;
    assert_eq!(t.board.survival_flags_completed(), 0);
    t.board.survival_stage = 1;
    t.board.waves.current_wave = 0;
    assert_eq!(t.board.survival_flags_completed(), 1);
}

#[test]
fn clearing_a_stage_brings_the_next_one_after_the_advice() {
    let Some(mut t) = day_normal() else { return };
    // As if all ten waves had come and gone.
    t.board.waves.current_wave = t.board.waves.num_waves;
    let zombie = t.zombie(ZombieType::Normal, 2, 0.0);
    assert!(
        t.run_until(300, |b| b.next_survival_stage_counter > 0),
        "the mower killing the last zombie clears the stage"
    );
    assert!(t.board.zombies.get(zombie).is_none_or(|z| z.is_dead_or_dying()));
    assert!(
        t.run_until(700, |b| b.survival_stage == 1 && b.survival_repick),
        "the next stage opens the seed chooser"
    );
    assert!(t.board.seed_chooser.is_some(), "the chooser is up");
    // Pick the first packets and press Start, as the player would.
    t.with(|board, _, _, _, _| {
        let count = board.seed_bank.num_packets;
        board.seed_chooser.as_mut().expect("repick chooser").fill_bank(count);
        assert!(board.seed_chooser_start().is_none(), "no warnings for the default picks");
    });
    t.tick();
    assert!(!t.board.survival_repick, "the chooser closed");
    assert_eq!(t.board.waves.current_wave, 0, "its waves start over");
    assert_eq!(t.board.waves.num_waves, 10);
    assert!(!t.board.has_level_award_dropped(), "no award between stages");
}

#[test]
fn the_fifth_stage_is_the_last_of_a_normal_run() {
    let Some(mut t) = day_normal() else { return };
    t.board.survival_stage = 4;
    t.board.waves.current_wave = t.board.waves.num_waves;
    assert!(t.board.is_final_survival_stage());
    t.zombie(ZombieType::Normal, 2, 0.0);
    assert!(t.run_until(300, |b| b.has_level_award_dropped()), "the run is won");
    // The trophy coin must be picked up before the fade starts, as in the
    // adventure's endings.
    let award = t.board.coins.iter().find(|(_, c)| c.is_level_award()).map(|(_, c)| (c.pos_x, c.pos_y));
    let Some((x, y)) = award else { panic!("the trophy coin dropped") };
    t.click((x + 30.0) as i32, (y + 30.0) as i32);
    assert!(t.run_until(1000, |b| b.level_complete), "picking it up wins the run");
}

#[test]
fn the_graveyard_never_gets_the_zamboni() {
    // `InitZombieWavesSurvival`: the Zamboni would plough a graveyard's
    // graves away, so night survival never picks it.
    for difficulty in [SurvivalDifficulty::Hard, SurvivalDifficulty::Endless] {
        let Some(mut t) = TestBoard::survival(SurvivalLawn::Night, difficulty) else { return };
        t.board.survival_stage = 6;
        let mut rng = MtRand::new(7);
        t.board.init_survival_waves(&mut rng);
        assert!(!t.board.waves.zombie_allowed[ZombieType::Zamboni as usize], "night has graves");
    }
}

#[test]
fn the_save_file_name_follows_the_original_modes() {
    use crate::board::save::save_file_name;
    assert_eq!(save_file_name(3, crate::board::GameMode::Adventure), "game3_0.dat");
    assert_eq!(
        save_file_name(3, crate::board::GameMode::Survival { lawn: SurvivalLawn::Day, difficulty: SurvivalDifficulty::Normal }),
        "game3_1.dat"
    );
    assert_eq!(
        save_file_name(3, crate::board::GameMode::Survival { lawn: SurvivalLawn::Roof, difficulty: SurvivalDifficulty::Hard }),
        "game3_10.dat"
    );
}
