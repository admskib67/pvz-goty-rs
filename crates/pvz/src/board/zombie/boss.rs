//! Dr. Zomboss, the last zombie (level 5-10): a giant robot that stands at the
//! right of the lawn and, one attack at a time, drops a recreational vehicle
//! on a block of plants, stomps the back rows, lowers three bungee zombies,
//! sends zombies down the lawn, and spits fireballs and iceballs along a row.
//! Only while his head is lowered can plants hurt him.
//!
//! The boss never walks. What he needs from the board (spawning zombies,
//! bungees, the mowers his fireballs flatten, the zombies he takes with him
//! when he dies) goes out as [`BossRequest`]s, which the board carries out.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Zombie.cpp`: `UpdateBoss`,
//! `BossPlayIdle`, `BossSetupReanim`, `BossRVAttack`/`BossRVLanding`,
//! `BossSpawnAttack`/`BossSpawnContact`, `BossStompAttack`/`BossCanStompRow`/
//! `BossStompContact`, `BossBungeeAttack`/`BossBungeeSpawn`/`BossBungeeLeave`/
//! `BossAreBungeesDone`, `BossHeadAttack`/`BossHeadSpit`/`BossHeadSpitEffect`/
//! `BossHeadSpitContact`, `UpdateBossFireball`, `BossDestroyIceballInRow`,
//! `BossDestroyFireball`, `BossStartDeath`, `BossDie`, `DrawBossPart`,
//! `DrawBossBackArm`, `DrawBossFireBall`, `ApplyBossSmokeParticles` and the
//! boss's parts of `TakeBodyDamage`, `GetBodyDamageIndex`,
//! `EffectedByDamage`, `PlayDeathAnim` and `UpdateDeath`; `Lawn/Board.cpp`:
//! `AddBossRenderItem`.

use super::*;

/// `NUM_BOSS_BUNGEES`.
pub const NUM_BOSS_BUNGEES: usize = 3;
/// `BOSS_FLASH_HEALTH_FRACTION`: below a tenth of his health he flashes.
const BOSS_FLASH_HEALTH_FRACTION: i32 = 10;

pub(crate) const RENDER_GROUP_BOSS_BACK_LEG: i32 = 4;
pub(crate) const RENDER_GROUP_BOSS_FRONT_LEG: i32 = 5;
pub(crate) const RENDER_GROUP_BOSS_BACK_ARM: i32 = 6;
const RENDER_GROUP_BOSS_FIREBALL_ADDITIVE: i32 = 7;
const RENDER_GROUP_BOSS_FIREBALL_TOP: i32 = 8;

/// The zombies he sends once he's been about a while (`gBossZombieList`).
const BOSS_ZOMBIE_LIST: [ZombieType; 12] = [
    ZombieType::TrafficCone,
    ZombieType::Pail,
    ZombieType::Football,
    ZombieType::Polevaulter,
    ZombieType::JackInTheBox,
    ZombieType::Ladder,
    ZombieType::Zamboni,
    ZombieType::Catapult,
    ZombieType::Pogo,
    ZombieType::Newspaper,
    ZombieType::Door,
    ZombieType::Gargantuar,
];

/// What the boss is doing, besides the zombie's own phase.
#[derive(Debug, Clone)]
pub struct BossState {
    /// `mBossMode`: how hurt he is (0 to 2), which attacks he adds.
    pub mode: i32,
    /// `mBossStompCounter`, `mBossBungeeCounter`, `mBossHeadCounter`: ticks
    /// until each attack is due (-1: not yet started).
    pub stomp_counter: i32,
    pub bungee_counter: i32,
    pub head_counter: i32,
    /// The fireball or iceball rolling along `fireball_row` (`mBossFireBallReanimID`).
    pub fireball: Option<ReanimationId>,
    pub fireball_row: i32,
    pub is_fireball: bool,
    /// `mTargetRow` (and the zombie's `target_col`): where the vehicle lands,
    /// which row he stomps or sends a zombie down.
    pub target_row: i32,
    /// `mFollowerZombieID`: the bungees he dropped.
    pub followers: [Option<Id<Zombie>>; NUM_BOSS_BUNGEES],
    /// The smoke on his head, with where it sits on the head's track.
    smoke: Vec<(ParticleSystemId, f32, f32)>,
}

impl Default for BossState {
    fn default() -> Self {
        Self {
            mode: 0,
            stomp_counter: -1,
            bungee_counter: -1,
            head_counter: -1,
            fireball: None,
            fireball_row: 0,
            is_fireball: false,
            target_row: 0,
            followers: [None; NUM_BOSS_BUNGEES],
            smoke: Vec::new(),
        }
    }
}

/// What the boss asks of the board.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BossRequest {
    /// A zombie walks in from `x` = 600 in `row` (`BossSpawnContact`).
    SpawnZombie { ty: ZombieType, row: i32 },
    /// Three bungee zombies drop on three columns starting at `target_col`.
    SpawnBungees { target_col: i32 },
    /// `BossBungeeLeave`: buttered bungees die as they're pulled up.
    KillButteredBungees,
    /// `UpdateBossFireball`: mowers in `row` between the two x are flattened.
    SquishMowers { row: i32, from_x: f32, to_x: f32 },
    /// `BossDie`: every other zombie dies with him, and the music fades.
    Died,
}

/// The parts the board draws separately, each at its own place among the
/// objects of the lawn (`BossPart`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BossPart {
    BackLeg,
    FrontLeg,
    Main,
    BackArm,
    Fireball,
}

impl Zombie {
    pub fn is_boss(&self) -> bool {
        self.zombie_type == ZombieType::Boss
    }

    /// Whether the zombie counts for plants in `row`: the boss is in every row.
    pub fn in_row(&self, row: i32) -> bool {
        self.row == row || self.is_boss()
    }

    /// The boss's health bar, as the progress meter shows it (0 to 150).
    pub fn boss_progress(&self) -> i32 {
        150 * (self.body_max_health - self.body_health) / self.body_max_health.max(1)
    }

    /// `Zombie::GetBodyDamageIndex` for the boss.
    pub(super) fn boss_damage_index(&self) -> i32 {
        if self.body_health < self.body_max_health / 2 {
            2
        } else if self.body_health < self.body_max_health * 4 / 5 {
            1
        } else {
            0
        }
    }

    /// `Zombie::BossSetupReanim`: the legs and arms are drawn apart from the
    /// body, and the driver rides in his head.
    pub(super) fn boss_setup_reanim(&mut self, fx: &mut Fx) {
        let Some(body_id) = self.body else { return };
        if let Some(body) = fx.effects.get_mut(body_id) {
            body.assign_render_group_to_prefix("Boss_innerleg", RENDER_GROUP_BOSS_BACK_LEG);
            body.assign_render_group_to_prefix("Boss_outerleg", RENDER_GROUP_BOSS_FRONT_LEG);
            body.assign_render_group_to_prefix("Boss_body2", RENDER_GROUP_BOSS_FRONT_LEG);
            body.assign_render_group_to_prefix("Boss_innerarm", RENDER_GROUP_BOSS_BACK_ARM);
            body.assign_render_group_to_prefix("Boss_RV", RENDER_GROUP_BOSS_BACK_ARM);
        }
        if let Ok((driver, reanim)) = fx.effects.add_reanimation(0.0, 0.0, ReanimationType::BossDriver, fx.res, fx.images) {
            reanim.play("anim_idle", LoopType::Loop, 0, 18.0);
            reanim.dont_draw_if_parent_hidden = true;
            reanim.attach_offset = sexy::matrix::Matrix3 { m00: 1.2, m11: 1.2, m02: 28.0, m12: -84.0, ..sexy::matrix::Matrix3::IDENTITY };
            self.special_head = Some(driver);
            fx.effects.attach_to_another(driver, body_id, "Boss_head2");
        }
        if let Some(body) = fx.effects.get_mut(body_id) {
            body.frame_base_pose = 0;
        }
    }

    /// `Zombie::BossPlayIdle`.
    fn boss_play_idle(&mut self, fx: &mut Fx) {
        self.phase = ZombiePhase::BossIdle;
        self.phase_counter = fx.sounds.rng().range_int(100, 200);
        self.play_zombie_reanim(fx.effects, "anim_idle", LoopType::Loop, 0, 6.0);
    }

    fn body_event(&self, fx: &Fx, time: f32) -> bool {
        self.body.and_then(|b| fx.effects.get(b)).is_some_and(|b| b.should_trigger_timed_event(time))
    }

    fn body_looped(&self, fx: &Fx) -> bool {
        self.body.and_then(|b| fx.effects.get(b)).is_some_and(|b| b.loop_count > 0)
    }

    pub(super) fn driver_play(&self, fx: &mut Fx, track: &str, loop_type: LoopType, blend: i32, rate: f32) {
        if let Some(driver) = self.special_head.and_then(|d| fx.effects.get_mut(d)) {
            driver.play(track, loop_type, blend, rate);
        }
    }

    /// `Zombie::UpdateBoss`.
    pub(super) fn update_boss(&mut self, plants: &mut DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        if env.scene_intro {
            // He stamps in as the level begins.
            if self.body_event(fx, 0.24) || self.body_event(fx, 0.79) {
                fx.sounds.play_foley(FoleyType::Thump);
                self.shake = Some((1, 4));
            }
            return;
        }
        self.update_boss_fireball(plants, env, fx);
        if self.ice_trap_counter == 0 {
            for counter in [&mut self.summon_counter, &mut self.boss.bungee_counter, &mut self.boss.stomp_counter, &mut self.boss.head_counter] {
                if *counter > 0 {
                    *counter -= 1;
                }
            }
            let chilled = self.chilled_counter > 0;
            if let Some(driver) = self.special_head.and_then(|d| fx.effects.get_mut(d)) {
                if chilled {
                    driver.anim_rate = 6.0;
                } else if driver.anim_rate == 0.0 {
                    driver.anim_rate = 12.0;
                }
            }
        } else if let Some(driver) = self.special_head.and_then(|d| fx.effects.get_mut(d)) {
            driver.anim_rate = 0.0;
        }

        match self.phase {
            ZombiePhase::BossEnter => self.boss_play_idle(fx),
            ZombiePhase::BossIdle => {
                if self.body_health == 1 {
                    self.play_death_anim(0, env, fx);
                    return;
                }
                if self.phase_counter > 0 {
                    return;
                }
                let index = self.boss_damage_index();
                if index != self.boss.mode {
                    self.boss.mode = index;
                    if index == 1 {
                        // The first time he's hurt he lowers the bungees.
                        self.boss_bungee_attack(fx);
                    } else {
                        self.boss_rv_attack(fx);
                    }
                } else if self.boss.stomp_counter == 0 {
                    self.boss_stomp_attack(plants, fx);
                } else if self.boss.bungee_counter == 0 {
                    // Adventure mode: one time in four he drops the vehicle instead.
                    if fx.sounds.rng().next_range(4) == 0 {
                        self.boss.bungee_counter = fx.sounds.rng().range_int(4000, 5000);
                        self.boss_rv_attack(fx);
                    } else {
                        self.boss_bungee_attack(fx);
                    }
                } else if self.boss.head_counter == 0 {
                    self.boss_head_attack(fx);
                } else if self.summon_counter == 0 {
                    self.boss_spawn_attack(env, fx);
                } else {
                    self.phase_counter = fx.sounds.rng().range_int(100, 200);
                }
            }
            ZombiePhase::BossSpawning => {
                if self.body_event(fx, 0.6) {
                    self.boss_spawn_contact(fx);
                }
                if self.body_looped(fx) {
                    self.boss_play_idle(fx);
                }
            }
            ZombiePhase::BossStomping => {
                let trigger = if self.boss.target_row >= 2 { 0.55 } else { 0.5 };
                if self.body_event(fx, trigger) {
                    self.boss_stomp_contact(plants, fx);
                }
                if self.body_looped(fx) {
                    self.boss_play_idle(fx);
                }
            }
            ZombiePhase::BossBungeesEnter => {
                if self.body_event(fx, 0.4) {
                    self.phase = ZombiePhase::BossBungeesDrop;
                    self.boss_requests.push(BossRequest::SpawnBungees { target_col: self.target_col });
                }
            }
            ZombiePhase::BossBungeesDrop => {
                if env.boss_bungees_done {
                    self.boss_bungee_leave(fx);
                }
            }
            ZombiePhase::BossBungeesLeave => {
                if self.body_looped(fx) {
                    self.boss_play_idle(fx);
                }
            }
            ZombiePhase::BossDropRv => {
                if self.body_event(fx, 0.65) {
                    self.boss_rv_landing(plants, fx);
                }
                if self.body_looped(fx) {
                    self.boss_play_idle(fx);
                }
            }
            ZombiePhase::BossHeadEnter => {
                if self.boss_damage_index() == 2 && self.body_event(fx, 0.37) {
                    self.boss_smoke(true, fx);
                }
                if self.body_event(fx, 0.55) {
                    fx.sounds.play_foley(FoleyType::Hydraulic);
                }
                if self.body_looped(fx) {
                    self.phase = ZombiePhase::BossHeadIdleBeforeSpit;
                    self.play_zombie_reanim(fx.effects, "anim_head_idle", LoopType::Loop, 0, 12.0);
                    self.phase_counter = 500;
                }
            }
            ZombiePhase::BossHeadIdleBeforeSpit => {
                if self.body_health == 1 {
                    self.boss_start_death(fx);
                } else if self.phase_counter == 0 {
                    self.boss_head_spit(fx);
                }
            }
            ZombiePhase::BossHeadSpit => {
                if self.body_event(fx, 0.37) {
                    self.boss_head_spit_effect(fx);
                }
                if self.body_event(fx, 0.42) {
                    self.boss_head_spit_contact(env, fx);
                }
                if self.body_looped(fx) {
                    self.driver_play(fx, "anim_idle", LoopType::Loop, 20, 18.0);
                    self.phase = ZombiePhase::BossHeadIdleAfterSpit;
                    self.play_zombie_reanim(fx.effects, "anim_head_idle", LoopType::Loop, 0, 12.0);
                    self.phase_counter = 300;
                }
            }
            ZombiePhase::BossHeadIdleAfterSpit => {
                if self.body_health == 1 {
                    self.boss_start_death(fx);
                } else if self.phase_counter == 0 {
                    self.phase = ZombiePhase::BossHeadLeave;
                    self.play_zombie_reanim(fx.effects, "anim_head_leave", LoopType::PlayOnceAndHold, 0, 12.0);
                }
            }
            ZombiePhase::BossHeadLeave => {
                if self.body_event(fx, 0.23) {
                    self.chilled_counter = 0;
                    self.update_anim_speed(fx.effects);
                }
                if self.body_event(fx, 0.48) || self.body_event(fx, 0.8) {
                    fx.sounds.play_foley(FoleyType::Thump);
                }
                if self.body_looped(fx) {
                    self.boss_smoke(false, fx);
                    self.boss_play_idle(fx);
                }
            }
            _ => {}
        }
        self.move_boss_smoke(fx);
    }

    /// `Zombie::BossRVAttack`: the vehicle drops on a block of plants.
    pub fn boss_rv_attack(&mut self, fx: &mut Fx) {
        self.remove_cold_effects(fx.effects);
        self.phase = ZombiePhase::BossDropRv;
        let rng = fx.sounds.rng();
        // The vehicle covers rows target to target + 1, columns target to target + 2.
        self.boss.target_row = rng.range_int(0, 3);
        self.target_col = rng.range_int(0, 2);
        self.play_zombie_reanim(fx.effects, "anim_RV_1", LoopType::PlayOnceAndHold, 20, 16.0);
        fx.sounds.play_foley(FoleyType::HydraulicShort);
    }

    /// `Zombie::BossRVLanding`.
    fn boss_rv_landing(&mut self, plants: &mut DataArray<Plant>, fx: &mut Fx) {
        let (row, col) = (self.boss.target_row, self.target_col);
        for (_, plant) in plants.iter_mut() {
            if !plant.dead && (row..=row + 1).contains(&plant.row) && (col..=col + 2).contains(&plant.col) && plant.squish(fx.effects) {
                fx.sounds.play_foley(FoleyType::Squish);
            }
        }
        self.shake = Some((1, 2));
        fx.sounds.play_sample("SOUND_RVTHROW");
        self.summon_counter = 500;
        self.boss.head_counter = 5000;
        if self.boss.mode >= 1 {
            self.boss.stomp_counter = 4000;
        }
        if self.boss.mode >= 2 {
            self.boss.bungee_counter = 6500;
        }
    }

    /// `Zombie::BossSpawnAttack`: he reaches down to a row and drops a zombie.
    fn boss_spawn_attack(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        self.remove_cold_effects(fx.effects);
        self.phase = ZombiePhase::BossSpawning;
        let rng = fx.sounds.rng();
        self.summon_counter = match self.boss.mode {
            0 => rng.range_int(450, 550),
            1 => rng.range_int(350, 450),
            _ => rng.range_int(150, 250),
        };
        // PickRowForNewZombie: any row zombies can walk down.
        let rows: Vec<i32> = (0..env.open_rows.len() as i32).filter(|&r| env.open_rows[r as usize]).collect();
        self.boss.target_row = if rows.is_empty() { 2 } else { rows[fx.sounds.rng().next_range(rows.len() as u32) as usize] };
        let track = format!("anim_spawn_{}", self.boss.target_row.clamp(0, 4) + 1);
        self.play_zombie_reanim(fx.effects, &track, LoopType::PlayOnceAndHold, 20, 12.0);
        fx.sounds.play_foley(FoleyType::HydraulicShort);
    }

    /// `Zombie::BossSpawnContact`: the older he is, the worse the zombie.
    fn boss_spawn_contact(&mut self, fx: &mut Fx) {
        let ty = if self.age < 3500 {
            ZombieType::Normal
        } else if self.age < 8000 {
            ZombieType::TrafficCone
        } else if self.age < 12500 {
            ZombieType::Pail
        } else {
            // A gargantuar can't come down the top row.
            let list: Vec<ZombieType> = BOSS_ZOMBIE_LIST
                .iter()
                .copied()
                .filter(|ty| !(self.boss.target_row == 0 && *ty == ZombieType::Gargantuar))
                .collect();
            list[fx.sounds.rng().next_range(list.len() as u32) as usize]
        };
        self.boss_requests.push(BossRequest::SpawnZombie { ty, row: self.boss.target_row });
    }

    /// `Zombie::BossStompAttack`.
    fn boss_stomp_attack(&mut self, plants: &DataArray<Plant>, fx: &mut Fx) {
        self.remove_cold_effects(fx.effects);
        self.phase = ZombiePhase::BossStomping;
        self.boss.stomp_counter = fx.sounds.rng().range_int(5500, 6500);
        let rows: Vec<i32> = (0..4).filter(|&row| Self::boss_can_stomp_row(plants, row)).collect();
        if rows.is_empty() {
            return;
        }
        self.boss.target_row = rows[fx.sounds.rng().next_range(rows.len() as u32) as usize];
        let track = format!("anim_stomp_{}", self.boss.target_row + 1);
        self.play_zombie_reanim(fx.effects, &track, LoopType::PlayOnceAndHold, 20, 12.0);
        fx.sounds.play_foley(FoleyType::HydraulicShort);
    }

    /// `Zombie::BossCanStompRow`: something to flatten in rows `row` and
    /// `row + 1`, from the sixth column on.
    fn boss_can_stomp_row(plants: &DataArray<Plant>, row: i32) -> bool {
        plants.iter().any(|(_, p)| !p.not_on_ground() && (row..=row + 1).contains(&p.row) && p.col >= 5)
    }

    /// `Zombie::BossStompContact`.
    fn boss_stomp_contact(&mut self, plants: &mut DataArray<Plant>, fx: &mut Fx) {
        let row = self.boss.target_row;
        for (_, plant) in plants.iter_mut() {
            if !plant.dead && (row..=row + 1).contains(&plant.row) && plant.col >= 5 && plant.squish(fx.effects) {
                fx.sounds.play_foley(FoleyType::Squish);
            }
        }
        self.shake = Some((1, 4));
        fx.sounds.play_foley(FoleyType::Thump);
    }

    /// `Zombie::BossBungeeAttack`.
    pub fn boss_bungee_attack(&mut self, fx: &mut Fx) {
        self.remove_cold_effects(fx.effects);
        self.phase = ZombiePhase::BossBungeesEnter;
        self.boss.bungee_counter = fx.sounds.rng().range_int(4000, 5000);
        self.target_col = fx.sounds.rng().range_int(0, 2);
        self.play_zombie_reanim(fx.effects, "anim_bungee_1_enter", LoopType::PlayOnceAndHold, 20, 12.0);
        fx.sounds.play_foley(FoleyType::HydraulicShort);
        fx.sounds.play_foley(FoleyType::BungeeScream);
    }

    /// `Zombie::BossBungeeLeave`.
    fn boss_bungee_leave(&mut self, fx: &mut Fx) {
        self.phase = ZombiePhase::BossBungeesLeave;
        self.boss_requests.push(BossRequest::KillButteredBungees);
        self.play_zombie_reanim(fx.effects, "anim_bungee_1_leave", LoopType::PlayOnceAndHold, 20, 18.0);
    }

    /// `Zombie::BossHeadAttack`: his head comes down.
    fn boss_head_attack(&mut self, fx: &mut Fx) {
        self.phase = ZombiePhase::BossHeadEnter;
        self.boss.head_counter = fx.sounds.rng().range_int(4000, 5000);
        self.play_zombie_reanim(fx.effects, "anim_head_enter", LoopType::PlayOnceAndHold, 20, 12.0);
        fx.sounds.play_foley(FoleyType::HydraulicShort);
    }

    /// `Zombie::BossHeadSpit`: a fireball (or an iceball) along a row.
    fn boss_head_spit(&mut self, fx: &mut Fx) {
        if let Some(ball) = self.boss.fireball.take() {
            fx.effects.kill(ball);
        }
        self.phase = ZombiePhase::BossHeadSpit;
        let rng = fx.sounds.rng();
        self.boss.fireball_row = rng.range_int(0, 4);
        self.boss.is_fireball = rng.range_int(0, 1) == 0;
        let track = format!("anim_head_attack_{}", self.boss.fireball_row + 1);
        self.play_zombie_reanim(fx.effects, &track, LoopType::PlayOnceAndHold, 20, 12.0);
        let fireball = self.boss.is_fireball;
        if let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b)) {
            let (eyes, mouth) = if fireball {
                (None, None)
            } else {
                (Some("IMAGE_REANIM_ZOMBIE_BOSS_EYEGLOW_BLUE"), Some("IMAGE_REANIM_ZOMBIE_BOSS_MOUTHGLOW_BLUE"))
            };
            body.set_image_override("Boss_eyeglow_red", eyes);
            body.set_image_override("Boss_mouthglow_red", mouth);
        }
        self.driver_play(fx, "anim_drive", LoopType::Loop, 20, 36.0);
    }

    /// `Zombie::BossDestroyIceballInRow`: a jalapeno melts the iceball.
    pub fn boss_destroy_iceball_in_row(&mut self, row: i32, fx: &mut Fx) {
        if self.boss.fireball_row != row {
            return;
        }
        let Some(ball) = self.boss.fireball.filter(|_| !self.boss.is_fireball) else { return };
        if let Some(m) = fx.effects.get(ball).map(|b| b.overlay) {
            let rng = fx.sounds.rng();
            fx.effects.add_particle(m.m02 + 80.0, m.m12 + 80.0, render_layer::TOP, ParticleEffect::IceballDeath, fx.res, fx.images, rng);
        }
        fx.effects.kill(ball);
        self.boss.fireball = None;
    }

    /// `Zombie::BossDestroyFireball`: an ice-shroom puts out the fireball.
    pub fn boss_destroy_fireball(&mut self, fx: &mut Fx) {
        let Some(ball) = self.boss.fireball.filter(|_| self.boss.is_fireball) else { return };
        if let Some(m) = fx.effects.get(ball).map(|b| b.overlay) {
            let (x, y) = (m.m02 + 80.0, m.m12 + 40.0);
            for i in 0..6 {
                let angle = 2.0 * std::f32::consts::PI * i as f32 / 6.0 + std::f32::consts::FRAC_PI_2;
                let rate = fx.sounds.rng().range_f32(20.0, 25.0);
                if let Ok((_, flame)) =
                    fx.effects.add_reanimation(x + 60.0 * angle.sin(), y + 60.0 * angle.cos(), ReanimationType::JalapenoFire, fx.res, fx.images)
                {
                    flame.anim_time = 0.2;
                    flame.loop_type = LoopType::PlayOnceFullLastFrame;
                    flame.anim_rate = rate;
                    flame.render_order = render_layer::TOP;
                }
            }
        }
        fx.effects.kill(ball);
        self.boss.fireball = None;
    }

    /// `Zombie::BossHeadSpitEffect`: the flames at his jaw.
    fn boss_head_spit_effect(&mut self, fx: &mut Fx) {
        let (x, y) = {
            let Some(body) = self.body.and_then(|b| fx.effects.get(b)) else { return };
            let jaw = body.current_transform(body.find_track("Boss_jaw"));
            (self.pos_x + jaw.x + 100.0, self.pos_y + jaw.y + 50.0)
        };
        let rng = fx.sounds.rng();
        let id = fx.effects.add_particle(x, y, self.render_order + 2, ParticleEffect::ZombieBossFireball, fx.res, fx.images, rng);
        if !self.boss.is_fireball
            && let Some(system) = id.and_then(|p| fx.effects.particle_mut(p))
        {
            system.override_image(None, Some("IMAGE_ZOMBIE_BOSS_ICEBALL_PARTICLES"));
        }
        fx.sounds.play_foley(FoleyType::BossBoulderAttack);
    }

    /// `Zombie::BossHeadSpitContact`: the ball forms in front of the lawn and rolls left.
    fn boss_head_spit_contact(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        let y = super::super::pos_y_based_on_row(env.background, 550.0, self.boss.fireball_row) - 90.0;
        let fireball = self.boss.is_fireball;
        let ty = if fireball { ReanimationType::BossFireball } else { ReanimationType::BossIceball };
        let order = self.render_order + 1;
        if let Ok((id, ball)) = fx.effects.add_reanimation(455.0, y, ty, fx.res, fx.images) {
            ball.play("anim_form", LoopType::PlayOnceAndHold, 0, 16.0);
            ball.is_attachment = true;
            ball.render_order = order;
            if fireball {
                ball.assign_render_group_to_track("additive", RENDER_GROUP_BOSS_FIREBALL_ADDITIVE);
                ball.assign_render_group_to_track("superglow", RENDER_GROUP_BOSS_FIREBALL_ADDITIVE);
            } else {
                ball.assign_render_group_to_track("ice_highlight", RENDER_GROUP_BOSS_FIREBALL_ADDITIVE);
            }
            self.boss.fireball = Some(id);
        }
        self.driver_play(fx, "anim_laugh", LoopType::Loop, 20, 18.0);
        fx.sounds.play_foley(FoleyType::HydraulicShort);
    }

    /// `Zombie::UpdateBossFireball`: it rolls along its row flattening whatever
    /// it passes.
    fn update_boss_fireball(&mut self, plants: &mut DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        let Some(ball_id) = self.boss.fireball else { return };
        let row = self.boss.fireball_row;
        let Some(ball) = fx.effects.get_mut(ball_id) else {
            self.boss.fireball = None;
            return;
        };
        let speed = ball.track_velocity("_ground");
        ball.overlay.m02 -= speed;
        let pos_x = ball.overlay.m02;
        let pos_y = super::super::pos_y_based_on_row(env.background, pos_x + 75.0, row) - 90.0;
        ball.overlay.m12 = pos_y;
        let gone = pos_x < -180.0;
        if gone {
            fx.effects.kill(ball_id);
            self.boss.fireball = None;
        }

        // SquishAllInSquare(.., ATTACKTYPE_DRIVE_OVER): spikes aren't flattened.
        let x = (pos_x + 75.0) as i32;
        let col = if x < 40 { -1 } else { ((x - 40) / 80).clamp(0, 8) };
        for (_, plant) in plants.iter_mut() {
            if plant.row == row
                && plant.col == col
                && !plant.dead
                && !matches!(plant.seed, crate::plant::SeedType::Spikeweed | crate::plant::SeedType::Spikerock)
                && plant.seed != crate::plant::SeedType::Spikerock
                && plant.squish(fx.effects)
            {
                fx.sounds.play_foley(FoleyType::Squish);
            }
        }
        self.boss_requests.push(BossRequest::SquishMowers { row, from_x: pos_x, to_x: pos_x + 50.0 });
        if gone {
            return;
        }

        let fireball = self.boss.is_fireball;
        let rolling = {
            let Some(ball) = fx.effects.get_mut(ball_id) else { return };
            if ball.loop_type == LoopType::PlayOnceAndHold && ball.loop_count > 0 {
                ball.play("anim_role", LoopType::Loop, 0, 2.0);
                ball.render_order = make_render_order(render_layer::PARTICLE, row, 0);
            }
            ball.loop_type == LoopType::Loop
        };
        if rolling && fx.sounds.rng().next_range(10) == 0 {
            let rng = fx.sounds.rng();
            let ball_x = pos_x + 100.0 + rng.range_f32(0.0, 20.0);
            let ball_y = super::super::pos_y_based_on_row(env.background, ball_x - 40.0, row) + 90.0 + rng.range_f32(-50.0, 0.0);
            let order = make_render_order(render_layer::GRAVE_STONE, row, 6);
            let effect = if fireball { ParticleEffect::FireballTrail } else { ParticleEffect::IceballTrail };
            fx.effects.add_particle(ball_x, ball_y, order, effect, fx.res, fx.images, rng);
        }
        fx.effects.update_reanimation(ball_id);
    }

    /// `Zombie::BossStartDeath`: his head goes back up and he blows.
    fn boss_start_death(&mut self, fx: &mut Fx) {
        self.phase = ZombiePhase::BossHeadLeave;
        self.play_zombie_reanim(fx.effects, "anim_head_leave", LoopType::PlayOnceAndHold, 0, 24.0);
        let rng = fx.sounds.rng();
        fx.effects.add_particle(700.0, 150.0, render_layer::TOP, ParticleEffect::BossExplosion, fx.res, fx.images, rng);
        fx.sounds.play_sample("SOUND_BOSSEXPLOSION");
        fx.sounds.play_foley(FoleyType::GargantuDeath);
        self.boss_die(fx);
    }

    /// `Zombie::BossDie`: the ball stops, the music fades and the rest of the
    /// zombies go down with him.
    pub(super) fn boss_die(&mut self, fx: &mut Fx) {
        if !self.is_on_board() {
            return;
        }
        if self.boss.fireball.is_some() {
            let row = self.boss.fireball_row;
            self.boss_destroy_iceball_in_row(row, fx);
            self.boss_destroy_fireball(fx);
            if let Some(ball) = self.boss.fireball.take() {
                fx.effects.kill(ball);
            }
        }
        self.boss_requests.push(BossRequest::Died);
        self.remove_cold_effects(fx.effects);
    }

    /// `Zombie::ApplyBossSmokeParticles`: smoke rises from his head once he's
    /// badly hurt.
    fn boss_smoke(&mut self, on: bool, fx: &mut Fx) {
        for (id, _, _) in self.boss.smoke.drain(..) {
            fx.effects.remove_particle(id);
        }
        if !on {
            return;
        }
        let mut spots = vec![(120.0, 30.0), (205.0, 58.0)];
        if self.body_health < self.body_max_health / BOSS_FLASH_HEALTH_FRACTION {
            spots.push((193.0, 27.0));
        }
        for (dx, dy) in spots {
            let rng = fx.sounds.rng();
            if let Some(id) = fx.effects.add_particle(0.0, 0.0, 0, ParticleEffect::ZamboniSmoke, fx.res, fx.images, rng) {
                self.boss.smoke.push((id, dx, dy));
            }
        }
    }

    /// The smoke follows his head.
    fn move_boss_smoke(&self, fx: &mut Fx) {
        if self.boss.smoke.is_empty() {
            return;
        }
        let (x, y) = self.track_position(fx.effects, "Boss_head");
        for &(id, dx, dy) in &self.boss.smoke {
            if let Some(system) = fx.effects.particle_mut(id) {
                system.system_move(x + dx, y + dy);
            }
        }
    }

    /// The boss's part of `Zombie::TakeBodyDamage`: his armour clanks, he
    /// cracks as he's hurt, and he can't be killed outright — at the last
    /// point of health he starts to go (`UpdateBoss`).
    pub(super) fn boss_take_damage(&mut self, health_before: i32, index_before: i32, flags: u32, fx: &mut Fx) {
        if flags & damage::DOESNT_CAUSE_FLASH == 0 {
            fx.sounds.play_foley(FoleyType::ShieldHit);
        }
        let index_after = self.boss_damage_index();
        if index_before != index_after
            && let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b))
        {
            let n = index_after;
            if n == 1 || n == 2 {
                let id = |part: &str| format!("IMAGE_REANIM_ZOMBIE_BOSS_{part}_DAMAGE{n}");
                body.set_image_override("Boss_head", Some(&id("HEAD")));
                body.set_image_override("Boss_jaw", Some(&id("JAW")));
                body.set_image_override("Boss_outerarm_hand", Some(&id("OUTERARM_HAND")));
                body.set_image_override("Boss_outerarm_thumb2", Some(&id("OUTERARM_THUMB")));
                body.set_image_override(if n == 1 { "Boss_innerleg_foot" } else { "Boss_outerleg_foot" }, Some(&id("FOOT")));
            }
            if n == 2 {
                self.boss_smoke(true, fx);
            }
        }
        let low = self.body_max_health / BOSS_FLASH_HEALTH_FRACTION;
        if health_before >= low && self.body_health < low {
            let order = make_render_order(render_layer::TOP, 0, 0);
            let rng = fx.sounds.rng();
            fx.effects.add_particle(770.0, 260.0, order, ParticleEffect::BossExplosion, fx.res, fx.images, rng);
            fx.sounds.play_foley(FoleyType::BossExplosionSmall);
            self.boss_smoke(true, fx);
        }
        if self.body_health <= 0 {
            self.body_health = 1;
        }
    }

    /// The boss's part of `Zombie::UpdateDeath`: explosions all over him,
    /// then the driver waves a flag and the award drops.
    pub(super) fn update_boss_death(&mut self, fx: &mut Fx) {
        const BOOMS: [f32; 13] = [0.1, 0.12, 0.15, 0.19, 0.2, 0.26, 0.3, 0.4, 0.42, 0.5, 0.58, 0.61, 0.71];
        if BOOMS.iter().any(|&t| self.body_event(fx, t)) {
            let rng = fx.sounds.rng();
            let (x, y) = (rng.range_f32(600.0, 750.0), rng.range_f32(50.0, 300.0));
            fx.effects.add_particle(x, y, render_layer::TOP, ParticleEffect::BossExplosion, fx.res, fx.images, rng);
            fx.sounds.play_foley(FoleyType::BossExplosionSmall);
        }
        if self.body_event(fx, 0.93) {
            self.shake = Some((1, 2));
            fx.sounds.play_foley(FoleyType::BossExplosionSmall);
            fx.sounds.play_foley(FoleyType::Thump);
        }
        if self.body_event(fx, 0.99) {
            self.driver_play(fx, "anim_flag", LoopType::PlayOnceAndHold, 20, 30.0);
        }
        if let Some(driver) = self.special_head.and_then(|d| fx.effects.get_mut(d))
            && driver.is_anim_playing("anim_flag")
            && driver.loop_count > 0
        {
            driver.play("anim_flag_loop", LoopType::Loop, 20, 17.0);
        }
        if self.body_looped(fx) {
            self.drop_loot();
        }
    }

    /// `Zombie::DrawBossPart`.
    pub fn draw_boss_part(&self, g: &mut Graphics, effects: &EffectSystem, res: &GameResources, part: BossPart) {
        g.push_state();
        g.translate(self.x as f32, self.y as f32);
        match part {
            BossPart::BackLeg => self.draw_boss_group(g, effects, res, RENDER_GROUP_BOSS_BACK_LEG),
            BossPart::FrontLeg => self.draw_boss_group(g, effects, res, RENDER_GROUP_BOSS_FRONT_LEG),
            BossPart::Main => self.draw_boss_group(g, effects, res, RENDER_GROUP_NORMAL),
            BossPart::BackArm => {
                // The back arm reaches with the vehicle and the bungees.
                let (dx, dy) = match self.phase {
                    ZombiePhase::BossDropRv => (self.target_col as f32 * 80.0, (self.boss.target_row - 1) as f32 * 85.0 - self.target_col as f32 * 20.0),
                    ZombiePhase::BossBungeesEnter | ZombiePhase::BossBungeesDrop | ZombiePhase::BossBungeesLeave => {
                        (self.target_col as f32 * 80.0 - 23.0, 0.0)
                    }
                    _ => (0.0, 0.0),
                };
                g.translate(dx, dy);
                self.draw_boss_group(g, effects, res, RENDER_GROUP_BOSS_BACK_ARM);
            }
            BossPart::Fireball => {
                if let Some(ball) = self.boss.fireball.and_then(|b| effects.get(b)) {
                    // The ball is placed in the lawn's own coordinates.
                    g.translate(-(self.x as f32), -(self.y as f32));
                    draw_render_group(g, ball, RENDER_GROUP_NORMAL, res);
                    g.push_state();
                    g.set_additive(true);
                    draw_render_group(g, ball, RENDER_GROUP_BOSS_FIREBALL_ADDITIVE, res);
                    g.pop_state();
                    draw_render_group(g, ball, RENDER_GROUP_BOSS_FIREBALL_TOP, res);
                }
            }
        }
        g.pop_state();
    }

    fn draw_boss_group(&self, g: &mut Graphics, effects: &EffectSystem, res: &GameResources, group: i32) {
        if let Some(body) = self.body {
            effects.draw_group(g, body, group, res);
        }
    }

    /// Where the board draws a part among its objects (`AddBossRenderItem`).
    pub fn boss_part_orders(&self, effects: &EffectSystem) -> Vec<(i32, BossPart)> {
        let mut back_leg_row = 1;
        let mut front_leg_row = 3;
        let mut back_arm_row = 4;
        if self.is_dead_or_dying() {
            back_arm_row = 1;
        } else if self.phase == ZombiePhase::BossStomping
            && let Some(time) = self.body.and_then(|b| effects.get(b)).map(|b| b.anim_time)
            && time > 0.25
            && time < 0.75
        {
            if self.boss.target_row == 1 {
                back_leg_row = 2;
            } else if self.boss.target_row == 3 {
                front_leg_row = 4;
            }
        }
        let mut parts = vec![
            (make_render_order(render_layer::BOSS, back_leg_row, 2), BossPart::BackLeg),
            (make_render_order(render_layer::BOSS, front_leg_row, 2), BossPart::FrontLeg),
            (make_render_order(render_layer::BOSS, 4, 2), BossPart::Main),
            (make_render_order(render_layer::BOSS, back_arm_row, 3), BossPart::BackArm),
        ];
        if let Some(ball) = self.boss.fireball.and_then(|b| effects.get(b)) {
            parts.push((ball.render_order, BossPart::Fireball));
        }
        parts
    }

    /// Whether the bungees he dropped are all gone, or one is on its way up
    /// (`Zombie::BossAreBungeesDone`), given each one's state.
    pub fn boss_bungees_done(bungees: impl Iterator<Item = Option<ZombiePhase>>) -> bool {
        let mut remaining = 0;
        for phase in bungees.flatten() {
            if phase == ZombiePhase::BungeeRising {
                return true;
            }
            remaining += 1;
        }
        remaining == 0
    }

    pub fn boss_followers(&self) -> [Option<Id<Zombie>>; NUM_BOSS_BUNGEES] {
        self.boss.followers
    }

    pub fn set_boss_followers(&mut self, followers: [Option<Id<Zombie>>; NUM_BOSS_BUNGEES]) {
        self.boss.followers = followers;
    }

    /// `BossBungeeSpawn`: the bungee starts its drop from `pos_y - 30`.
    pub fn drop_from_boss(&mut self) {
        self.altitude = self.pos_y - 30.0;
    }

    /// Where the cords of the bungees he holds are cut off (`DrawBungeeCord`).
    pub fn boss_cord_clip(&self, effects: &EffectSystem, bungee_col: i32) -> Option<f32> {
        let mut amount = 55.0;
        if self.phase == ZombiePhase::BossBungeesLeave {
            let time = self.body.and_then(|b| effects.get(b)).map_or(0.0, |b| b.anim_time);
            amount = sexy::tod::animate_curve_float_time(0.0, 0.2, time, 55.0, 0.0, TodCurve::Linear);
        }
        (bungee_col > self.target_col).then_some(amount)
    }

    pub fn set_cord_clip(&mut self, clip: Option<f32>) {
        self.cord_clip = clip;
    }

    pub fn take_boss_requests(&mut self) -> Vec<BossRequest> {
        std::mem::take(&mut self.boss_requests)
    }

    /// Sets when each attack is due (`None` keeps it): for tests and the
    /// development scene.
    pub(crate) fn boss_set_counters(&mut self, stomp: Option<i32>, bungee: Option<i32>, head: Option<i32>, summon: Option<i32>) {
        self.boss.stomp_counter = stomp.unwrap_or(self.boss.stomp_counter);
        self.boss.bungee_counter = bungee.unwrap_or(self.boss.bungee_counter);
        self.boss.head_counter = head.unwrap_or(self.boss.head_counter);
        self.summon_counter = summon.unwrap_or(self.summon_counter);
        self.phase_counter = 0;
    }

    /// Where the vehicle lands or the next stomp or zombie goes: (row, column).
    #[cfg(test)]
    pub(crate) fn boss_target(&self) -> (i32, i32) {
        (self.boss.target_row, self.target_col)
    }

    /// The ball he spat: its row and whether it's a fireball.
    #[cfg(test)]
    pub(crate) fn boss_ball(&self) -> Option<(i32, bool)> {
        self.boss.fireball.map(|_| (self.boss.fireball_row, self.boss.is_fireball))
    }

    #[cfg(test)]
    pub(crate) fn boss_mode(&self) -> i32 {
        self.boss.mode
    }
}
