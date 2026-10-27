//! Survival: the five lawns, three difficulties, and the endless run of
//! stages within one lawn.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Challenge.cpp`
//! (`InitZombieWaves`, `InitZombieWavesSurvival`, `SurvivalCompletedStage`),
//! `Lawn/Board.cpp` (`GetNumWavesPerSurvivalStage`, `GetSurvivalFlagsCompleted`,
//! `IsFinalSurvivalStage`, `PickZombieWaves`'s survival points, the
//! "more zombies" pause in `FadeOutLevel`) and `LawnApp.cpp`
//! (`IsSurvivalMode`, `IsSurvivalNormal/Hard/Endless`).
//!
//! A survival run is five stages on normal and hard (ten and twenty waves per
//! stage, so each stage is one or two flags) and never ends on endless.
//! Clearing a stage shows the "more zombies" advice, then the next stage's
//! waves begin on the same lawn with the plants left standing. The original
//! also reopens the seed chooser for a repick between stages; this port keeps
//! the seed bank as it is.
//!
//! Not yet: `RepickSeedPackets` (the between-stage seed chooser), the stage
//! advice's music fade, and the "start onslaught" button.

use bevy::prelude::*;
use sexy::rand::MtRand;

use crate::audio::Sounds;
use crate::particle::ParticleEffect;
use crate::reanim::EffectSystem;
use crate::resources::GameResources;
use crate::zombie::ZombieType;

use super::message::MessageStyle;
use super::waves::{WaveState, ZOMBIE_COUNTDOWN_FIRST_WAVE};
use super::zombie::Fx;
use super::{BOARD_HEIGHT, BOARD_WIDTH, Board, GameMode, MAX_GRID_SIZE_Y, SurvivalDifficulty, SurvivalLawn, make_render_order, render_layer};

/// `mNextSurvivalStageCounter` when a stage is cleared.
const NEXT_STAGE_COUNTDOWN: i32 = 500;
/// How many zombie kinds `Challenge::InitZombieWavesSurvival` may add.
const MAX_EXTRA_ZOMBIE_TYPES: i32 = 9;

impl Board {
    /// The survival lawn and difficulty, if this is a survival run.
    pub fn survival(&self) -> Option<(SurvivalLawn, SurvivalDifficulty)> {
        match self.mode {
            GameMode::Survival { lawn, difficulty } => Some((lawn, difficulty)),
            _ => None,
        }
    }

    pub fn is_survival(&self) -> bool {
        self.survival().is_some()
    }

    pub fn survival_difficulty(&self) -> Option<SurvivalDifficulty> {
        self.survival().map(|(_, difficulty)| difficulty)
    }

    /// `Board::GetSurvivalFlagsCompleted`: the flags behind the player,
    /// counting the one the current wave is raising.
    pub fn survival_flags_completed(&self) -> i32 {
        let Some(difficulty) = self.survival_difficulty() else { return 0 };
        let per_flag = self.num_waves_per_flag().max(1);
        let finished_stages = self.survival_stage * difficulty.waves_per_stage() / per_flag;
        let mut wave = self.waves.current_wave;
        if wave > 0
            && self.is_flag_wave(wave - 1)
            && self.board_fade_out_counter < 0
            && self.next_survival_stage_counter == 0
        {
            wave -= 1;
        }
        wave / per_flag + finished_stages
    }

    /// `Board::IsFinalSurvivalStage`: the last stage of a normal or hard
    /// run; an endless run has no last stage.
    pub fn is_final_survival_stage(&self) -> bool {
        let Some(difficulty) = self.survival_difficulty() else { return false };
        let flags = difficulty.waves_per_stage() * (self.survival_stage + 1) / self.num_waves_per_flag().max(1);
        match difficulty {
            SurvivalDifficulty::Normal => flags >= 5,
            SurvivalDifficulty::Hard => flags >= 10,
            SurvivalDifficulty::Endless => false,
        }
    }

    /// `Challenge::InitZombieWaves` for survival: stage zero has a fixed
    /// table, later stages add random kinds the lawn allows.
    pub(super) fn init_survival_waves(&mut self, rng: &mut MtRand) {
        let Some((lawn, difficulty)) = self.survival() else { return };
        self.waves = WaveState::default();
        let num_waves = difficulty.waves_per_stage();
        self.waves.num_waves = num_waves;
        self.waves.zombies_in_wave = vec![Vec::new(); num_waves as usize];

        let stage = self.survival_stage;
        if stage == 0 {
            for ty in [ZombieType::Normal, ZombieType::TrafficCone] {
                self.waves.zombie_allowed[ty as usize] = true;
            }
            if difficulty != SurvivalDifficulty::Normal {
                self.waves.zombie_allowed[ZombieType::Pail as usize] = true;
            }
        } else {
            self.waves.zombie_allowed[ZombieType::Normal as usize] = true;
            // A fifth of the runs start with the newspaper instead of a cone.
            let second = if rng.next_range(5) == 0 { ZombieType::Newspaper } else { ZombieType::TrafficCone };
            self.waves.zombie_allowed[second as usize] = true;
            let mut capacity = (stage + 1).min(MAX_EXTRA_ZOMBIE_TYPES);
            // The reference picks uniformly and retries; a bounded loop does
            // the same without risking a spin when few kinds are allowed.
            let mut attempts = 0;
            while capacity > 0 && attempts < 200 {
                attempts += 1;
                let ty = ZombieType::from_index(rng.next_range(crate::zombie::NUM_ZOMBIE_TYPES as u32) as usize);
                if self.waves.zombie_allowed[ty as usize] || !survival_zombie_allowed(ty, lawn, difficulty) {
                    continue;
                }
                self.waves.zombie_allowed[ty as usize] = true;
                capacity -= 1;
            }
        }
        self.pick_zombie_waves(rng);
        self.waves.zombie_count_down = ZOMBIE_COUNTDOWN_FIRST_WAVE;
        self.waves.zombie_count_down_start = self.waves.zombie_count_down;
    }

    /// `Board::FadeOutLevel`'s survival branch: the stage is over, the
    /// "more zombies" advice shows, and the pause begins.
    pub(super) fn begin_next_survival_stage(&mut self, effects: &mut EffectSystem, res: &mut GameResources, sounds: &mut Sounds) {
        self.next_survival_stage_counter = NEXT_STAGE_COUNTDOWN;
        self.clear_advice(None);
        self.display_advice(res, "[ADVICE_MORE_ZOMBIES]", MessageStyle::BigMiddle, None);
        sounds.play_sample("SOUND_HUGE_WAVE");
        for row in 0..MAX_GRID_SIZE_Y {
            self.ice_timer[row] = NEXT_STAGE_COUNTDOWN;
        }
        // SurvivalCompletedStage: whatever is left leaves with the stage.
        for (_, zombie) in self.zombies.iter_mut() {
            if !zombie.is_dead_or_dying() {
                zombie.die_no_loot(effects);
            }
        }
    }

    /// The ticks of the pause between two stages; at the end the next
    /// stage's waves are set up on the same lawn.
    pub(super) fn update_survival_stage(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        if self.next_survival_stage_counter == 0 {
            return;
        }
        self.next_survival_stage_counter -= 1;
        if self.next_survival_stage_counter > 0 {
            return;
        }
        let fx = Fx { effects, res, images, sounds };
        self.survival_stage += 1;
        self.clear_advice(None);
        self.waves.progress_meter_width = 0;
        self.level_award_spawned = false;
        self.init_survival_waves(fx.sounds.rng());
        let order = make_render_order(render_layer::TOP, 0, 0);
        fx.effects.add_particle(
            BOARD_WIDTH as f32 / 2.0,
            BOARD_HEIGHT as f32 / 2.0,
            order,
            ParticleEffect::ScreenFlash,
            fx.res,
            fx.images,
            fx.sounds.rng(),
        );
    }
}

/// `Challenge::InitZombieWavesSurvival`'s filters, limited to the zombies
/// this port plays: which kinds a lawn and difficulty allow beyond the
/// starting table.
fn survival_zombie_allowed(ty: ZombieType, lawn: SurvivalLawn, difficulty: SurvivalDifficulty) -> bool {
    use ZombieType::*;
    let background = lawn.background();
    if !is_ported_wave_zombie(ty) || ty.definition().pick_weight == 0 {
        return false;
    }
    // Normal survival stops at the snorkel; hard and endless go further.
    if difficulty == SurvivalDifficulty::Normal && (ty as i32) > ZombieType::Snorkel as i32 {
        return false;
    }
    // Pool-only zombies need a pool.
    if ty.is_pool_only() && !background.has_pool() {
        return false;
    }
    // Diggers and dancers don't dig or dance on a roof.
    if background.has_roof() && matches!(ty, Digger | Dancer) {
        return false;
    }
    // Bungees only come to endless runs, or over a roof.
    if !background.has_roof() && difficulty != SurvivalDifficulty::Endless && ty == Bungee {
        return false;
    }
    // The Zamboni ploughs a graveyard's graves away; only the lawns without
    // graves see it.
    if self_stage_has_graves(lawn) && ty == Zamboni {
        return false;
    }
    true
}

/// The zombies that appear in waves and that this port simulates.
fn is_ported_wave_zombie(ty: ZombieType) -> bool {
    use ZombieType::*;
    matches!(
        ty,
        Normal
            | TrafficCone
            | Polevaulter
            | Pail
            | Newspaper
            | Door
            | Football
            | Dancer
            | Snorkel
            | Zamboni
            | Bobsled
            | DolphinRider
            | Balloon
            | Bungee
            | Ladder
            | Catapult
            | Gargantuar
    )
}

/// `StageHasGraveStones`: only the night lawn has graves.
fn self_stage_has_graves(lawn: SurvivalLawn) -> bool {
    matches!(lawn, SurvivalLawn::Night)
}
