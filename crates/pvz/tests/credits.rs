//! The credits movie, run through its three parts to the names at the end.

use crate::credits::{CreditScreen, CreditsPhase, prepare_credits, tick_movie};

use super::test_board;

#[test]
fn the_credits_play_through_to_the_end() {
    let mut t = test_board!("1-1");
    let (phases, brain_seen, stage_seen, buttons) = t.with(|_, effects, res, images, sounds| {
        effects.clear();
        prepare_credits(res, images);
        let mut screen = CreditScreen::new(res);
        screen.play_reanim(1, effects, res, images);
        let (mut phases, mut brain_seen, mut stage_seen) = (vec![screen.phase], false, false);
        let mut ticks = 0;
        for _ in 0..30_000 {
            ticks += 1;
            tick_movie(&mut screen, effects, res, images, sounds);
            if phases.last() != Some(&screen.phase) {
                phases.push(screen.phase);
            }
            brain_seen |= screen.brain().is_some();
            stage_seen |= screen.reanim_id().and_then(|r| effects.find_attached(r, "Credits_stage")).is_some();
            if screen.phase == CreditsPhase::End && screen.buttons_shown() {
                break;
            }
        }
        eprintln!("the credits took {ticks} ticks");
        (phases, brain_seen, stage_seen, screen.buttons_shown())
    });
    assert_eq!(phases, [CreditsPhase::Main1, CreditsPhase::Main2, CreditsPhase::Main3, CreditsPhase::End], "the three parts, then the end");
    assert!(stage_seen, "the stage's attacher tracks bring in their own animations");
    assert!(brain_seen, "the brain bounces along the words");
    assert!(buttons, "Main Menu and Replay appear at the end");
}
