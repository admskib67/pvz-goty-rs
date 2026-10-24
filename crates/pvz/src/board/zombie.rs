//! Zombies on the lawn (`Zombie`). So far the plain family: the normal,
//! flag, conehead and buckethead zombies. They walk in from the right, eat
//! plants that are in their way, lose their helmet, arm and head as they're
//! hurt, and fall over when they die.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Zombie.cpp`: `ZombieInitialize`,
//! `LoadReanim`, `LoadPlainZombieReanim`, `SetupReanimLayers`,
//! `PickRandomSpeed`, `UpdateAnimSpeed`, `Update`, `UpdatePlaying`,
//! `UpdateZombiePosition`, `UpdateZombieWalking`, `CheckIfPreyCaught`,
//! `EatPlant`, `StartEating`/`StopEating`/`StartWalkAnim`, `Animate`,
//! `UpdateReanim`, `GetDrawPos`, `GetZombieRect`/`GetZombieAttackRect`,
//! `TakeDamage`, `TakeHelmDamage`, `TakeBodyDamage`, `UpdateDamageStates`,
//! `DropHead`/`DropArm`/`DropHelm`/`DropFlag`, `PlayDeathAnim`,
//! `UpdateDeath`, `DieNoLoot`, `EffectedByDamage`, `DrawShadow`.
//!
//! Not yet: the other zombie types, the chilled/frozen/buttered/yucky
//! states, pools and ladders, and mind control.

use bevy::prelude::*;
use sexy::data_array::DataArray;
use sexy::reanim::{LoopType, RENDER_GROUP_HIDDEN, RENDER_GROUP_NORMAL};

use crate::audio::{FoleyType, Sounds};
use crate::graphics::Graphics;
use crate::particle::ParticleEffect;
use crate::reanim::{EffectSystem, ParticleSystemId, ReanimationId, ReanimationType};
use crate::resources::GameResources;
use crate::zombie::ZombieType;

use super::plant::Plant;
use super::{BackgroundType, make_render_order, render_layer};

/// `ZOMBIE_START_RANDOM_OFFSET`.
const START_RANDOM_OFFSET: u32 = 40;
/// `TICKS_BETWEEN_EATS` and `DAMAGE_PER_EAT`.
const TICKS_BETWEEN_EATS: i32 = 4;
const DAMAGE_PER_EAT: i32 = 4;
/// `BOARD_EDGE`: zombies whose x gets this far left have reached the house.
const BOARD_EDGE: i32 = -100;
const WIDE_BOARD_WIDTH: f32 = 800.0;
/// `Zombie::ZOMBIE_WAVE_WINNER`: the zombie that walks into the house.
pub const ZOMBIE_WAVE_WINNER: i32 = -4;
/// `Zombie::ZOMBIE_WAVE_CUTSCENE`: a zombie waiting on the street during the
/// level intro.
pub const ZOMBIE_WAVE_CUTSCENE: i32 = -2;

/// `DamageFlags`, as bit masks.
#[allow(dead_code, reason = "the full set; zombies use more of it as they're ported")]
pub mod damage {
    pub const BYPASSES_SHIELD: u32 = 1 << 0;
    pub const HITS_SHIELD_AND_BODY: u32 = 1 << 1;
    pub const FREEZE: u32 = 1 << 2;
    pub const DOESNT_CAUSE_FLASH: u32 = 1 << 3;
    pub const DOESNT_LEAVE_BODY: u32 = 1 << 4;
    pub const SPIKE: u32 = 1 << 5;
    /// What a zombie that's dying of its lost head takes (`9U`).
    pub const HEADLESS: u32 = BYPASSES_SHIELD | DOESNT_CAUSE_FLASH;
}

/// `DamageRangeFlags`, as bit masks.
#[allow(dead_code, reason = "the full set; zombies use more of it as they're ported")]
pub mod damage_range {
    pub const GROUND: u32 = 1 << 0;
    pub const FLYING: u32 = 1 << 1;
    pub const SUBMERGED: u32 = 1 << 2;
    pub const OFF_GROUND: u32 = 1 << 4;
    pub const DYING: u32 = 1 << 5;
    pub const UNDERGROUND: u32 = 1 << 6;
    pub const ONLY_MINDCONTROLLED: u32 = 1 << 7;
}

/// `Rect` (integer), for hit boxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl IRect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    fn offset(mut self, dx: i32, dy: i32) -> Self {
        self.x += dx;
        self.y += dy;
        self
    }
}

/// `GetRectOverlap`: how far two rectangles overlap horizontally (negative
/// when there's a gap).
pub fn rect_overlap(a: IRect, b: IRect) -> i32 {
    let (mut rmin, rmax, xmax) = if a.x < b.x { (a.x + a.w, b.x + b.w, b.x) } else { (b.x + b.w, a.x + a.w, a.x) };
    if rmin > xmax && rmin > rmax {
        rmin = rmax;
    }
    rmin - xmax
}

/// `HelmType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelmType {
    None,
    TrafficCone,
    Pail,
}

/// `ZombiePhase`, for what's ported so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZombiePhase {
    Normal,
    Dying,
    /// Run over by a lawn mower.
    Mowered,
    /// Burned to a crisp (the ones that have lost their head stay standing).
    Burned,
    /// The pole vaulter running up to the first plant it meets.
    PolePreVault,
    /// The pole vaulter in mid-jump, untouchable.
    PoleInVault,
}

/// What the board tells its zombies each tick.
#[derive(Debug, Clone, Copy)]
pub struct ZombieEnv {
    pub background: BackgroundType,
    pub level: i32,
    pub finished_adventure: bool,
    /// `mZombies.mSize`.
    pub zombie_count: usize,
    /// `CountZombiesOnScreen`.
    pub zombies_on_screen: usize,
    /// `IsWallnutBowlingLevel`: zombies don't eat, and pole vaulters jump on the spot.
    pub bowling: bool,
}

/// The things a zombie touches besides itself and the plants.
pub struct Fx<'a, 'w, 's> {
    pub effects: &'a mut EffectSystem,
    pub res: &'a mut GameResources,
    pub images: &'a mut Assets<Image>,
    pub sounds: &'a mut Sounds<'w, 's>,
}

/// `ZombieDrawPosition`.
#[derive(Debug, Clone, Copy)]
struct DrawPos {
    head_x: i32,
    head_y: i32,
    body_y: f32,
    image_offset_x: f32,
    image_offset_y: f32,
}

#[derive(Debug)]
pub struct Zombie {
    pub zombie_type: ZombieType,
    pub row: i32,
    pub from_wave: i32,
    pub pos_x: f32,
    pub pos_y: f32,
    vel_x: f32,
    /// `mX`/`mY`: the integer position hit tests and drawing use.
    pub x: i32,
    pub y: i32,
    width: i32,
    frame: i32,
    variant: bool,
    pub is_eating: bool,
    just_got_shot_counter: i32,
    /// Ticks left of being slowed by cold (`mChilledCounter`).
    chilled_counter: i32,
    phase_counter: i32,
    pub has_head: bool,
    has_arm: bool,
    /// The flag zombie still has its flag.
    has_object: bool,
    helm_type: HelmType,
    helm_health: i32,
    helm_max_health: i32,
    pub body_health: i32,
    body_max_health: i32,
    pub phase: ZombiePhase,
    anim_counter: i32,
    anim_ticks_per_frame: i32,
    anim_frames: i32,
    age: i32,
    groan_counter: i32,
    zombie_rect: IRect,
    attack_rect: IRect,
    body: Option<ReanimationId>,
    /// The flag zombie's flag.
    special_head: Option<ReanimationId>,
    /// Counts down to 0 as a dead zombie fades out, or -1.
    zombie_fade: i32,
    original_anim_rate: f32,
    /// The mower's chopping animation a mowed-down zombie follows.
    mowered_reanim: Option<ReanimationId>,
    dropped_loot: bool,
    /// The zombie died and the board should see whether this was the one
    /// that ends the level (`DropLoot` → `TrySpawnLevelAward`).
    pub loot_pending: bool,
    pub dead: bool,
    pub render_order: i32,
    /// Particles that ride along with the zombie, and where (`mAttachmentID`).
    attached: Vec<(ParticleSystemId, f32, f32)>,
}

impl Zombie {
    /// `Zombie::ZombieInitialize`, for a zombie on the board.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        row: i32,
        zombie_type: ZombieType,
        variant: bool,
        from_wave: i32,
        is_flag_wave: bool,
        env: &ZombieEnv,
        fx: &mut Fx,
    ) -> Self {
        let rng = fx.sounds.rng();
        let mut pos_x = 780.0 + rng.next_range(START_RANDOM_OFFSET) as f32;
        let groan_counter = rng.range_int(300, 400);
        if is_flag_wave {
            pos_x += 40.0;
        }
        let mut z = Self {
            zombie_type,
            row,
            from_wave,
            pos_x,
            pos_y: 0.0,
            vel_x: 0.0,
            x: 0,
            y: 0,
            width: 120,
            frame: 0,
            variant,
            is_eating: false,
            just_got_shot_counter: 0,
            chilled_counter: 0,
            phase_counter: 0,
            has_head: true,
            has_arm: true,
            has_object: false,
            helm_type: HelmType::None,
            helm_health: 0,
            helm_max_health: 0,
            body_health: 270,
            body_max_health: 270,
            phase: ZombiePhase::Normal,
            anim_counter: 0,
            anim_ticks_per_frame: 12,
            anim_frames: 12,
            age: 0,
            groan_counter,
            zombie_rect: IRect::new(36, 0, 42, 115),
            attack_rect: IRect::new(50, 0, 20, 115),
            body: None,
            special_head: None,
            zombie_fade: -1,
            original_anim_rate: 0.0,
            mowered_reanim: None,
            dropped_loot: false,
            loot_pending: false,
            dead: false,
            render_order: 0,
            attached: Vec::new(),
        };
        z.pos_y = if z.is_on_board() { z.pos_y_based_on_row(row, env) } else { 0.0 };
        z.pick_random_speed(fx);

        if let Some(ty) = zombie_type.reanim() {
            z.load_reanim(ty, env, fx);
        }
        match zombie_type {
            ZombieType::Normal => z.load_plain_zombie_reanim(fx),
            ZombieType::TrafficCone => {
                z.load_plain_zombie_reanim(fx);
                z.reanim_show_prefix(fx, "anim_cone", RENDER_GROUP_NORMAL);
                z.reanim_show_prefix(fx, "anim_hair", RENDER_GROUP_HIDDEN);
                z.helm_type = HelmType::TrafficCone;
                z.helm_health = 370;
            }
            ZombieType::Pail => {
                z.load_plain_zombie_reanim(fx);
                z.reanim_show_prefix(fx, "anim_bucket", RENDER_GROUP_NORMAL);
                z.reanim_show_prefix(fx, "anim_hair", RENDER_GROUP_HIDDEN);
                z.helm_type = HelmType::Pail;
                z.helm_health = 1100;
            }
            ZombieType::Polevaulter => {
                z.body_health = 500;
                z.anim_ticks_per_frame = 6;
                z.phase = ZombiePhase::PolePreVault;
                z.has_object = true;
                z.variant = false;
                z.pos_x = WIDE_BOARD_WIDTH + 70.0 + fx.sounds.rng().next_range(10) as f32;
                z.play_zombie_reanim(fx.effects, "anim_run", LoopType::Loop, 0, 0.0);
                z.pick_random_speed(fx);
                z.attack_rect = if env.bowling { IRect::new(-229, 0, 270, 115) } else { IRect::new(-29, 0, 70, 115) };
            }
            ZombieType::Flag => {
                z.has_object = true;
                z.load_plain_zombie_reanim(fx);
                if let Ok((flag, reanim)) = fx.effects.add_reanimation(0.0, 0.0, ReanimationType::Flag, fx.res, fx.images) {
                    reanim.play("Zombie_flag", LoopType::Loop, 0, 15.0);
                    z.special_head = Some(flag);
                    if let Some(body) = z.body {
                        fx.effects.attach_to_another(flag, body, "Zombie_flaghand");
                        if let Some(b) = fx.effects.get_mut(body) {
                            b.frame_base_pose = 0;
                        }
                    }
                }
                z.pos_x = WIDE_BOARD_WIDTH;
            }
            _ => {}
        }

        z.update_anim_speed(fx.effects);
        if z.variant {
            z.reanim_show_prefix(fx, "anim_tongue", RENDER_GROUP_NORMAL);
        }
        z.body_max_health = z.body_health;
        z.helm_max_health = z.helm_health;
        z.x = z.pos_x as i32;
        z.y = z.pos_y as i32;
        z.render_order = make_render_order(render_layer::ZOMBIE, row, 4);
        z.update_reanim(fx.effects);
        z
    }

    /// `Zombie::GetPosYBasedOnRow`.
    fn pos_y_based_on_row(&self, row: i32, env: &ZombieEnv) -> f32 {
        super::pos_y_based_on_row(env.background, self.pos_x + 40.0, row) - 30.0
    }

    /// `Zombie::LoadReanim`: the body reanimation, walking.
    fn load_reanim(&mut self, ty: ReanimationType, env: &ZombieEnv, fx: &mut Fx) {
        match fx.effects.add_reanimation(0.0, 0.0, ty, fx.res, fx.images) {
            Ok((id, body)) => {
                body.loop_type = LoopType::Loop;
                body.is_attachment = true;
                self.body = Some(id);
            }
            Err(e) => {
                error!("{e}");
                return;
            }
        }
        if self.is_on_board() {
            self.start_walk_anim(0, env, fx);
        } else {
            // Waiting on the street: shuffles about, out of step with the others.
            let rng = fx.sounds.rng();
            let idle2 = rng.next_range(4) > 0;
            let (track, rate) = if idle2 { ("anim_idle2", rng.range_f32(12.0, 24.0)) } else { ("anim_idle", rng.range_f32(12.0, 18.0)) };
            let time = rng.range_f32(0.0, 0.99);
            let has = |name: &str| fx.effects.get(self.body.unwrap()).is_some_and(|b| b.definition.find_track(name).is_some());
            let track = if has(track) { Some(track) } else if has("anim_idle") { Some("anim_idle") } else { None };
            if let Some(track) = track {
                self.play_zombie_reanim(fx.effects, track, LoopType::Loop, 0, rate);
            }
            if let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b)) {
                body.anim_time = time;
            }
        }
    }

    /// Moves the zombie to `x` along its row (the intro's lineup).
    pub fn place_x(&mut self, x: f32) {
        self.pos_x = x;
        self.x = x as i32;
    }

    /// Puts the zombie somewhere (for the street zombies).
    pub fn place(&mut self, x: f32, y: f32) {
        self.pos_x = x;
        self.pos_y = y;
        self.x = x as i32;
        self.y = y as i32;
        self.render_order = make_render_order(render_layer::ZOMBIE, self.row, 4);
    }

    /// `Zombie::IsOnBoard`.
    pub fn is_on_board(&self) -> bool {
        self.from_wave != ZOMBIE_WAVE_CUTSCENE
    }

    /// `Zombie::Update` for a zombie waiting on the street: it only
    /// animates.
    pub fn update_street(&mut self, effects: &mut EffectSystem) {
        self.age += 1;
        self.anim_counter += 1;
        if self.anim_counter >= self.anim_frames * self.anim_ticks_per_frame {
            self.anim_counter = 0;
        }
        self.frame = self.anim_counter / self.anim_ticks_per_frame;
        self.x = self.pos_x as i32;
        self.y = self.pos_y as i32;
        self.update_reanim(effects);
    }

    /// `Zombie::LoadPlainZombieReanim` + `SetupReanimLayers`.
    fn load_plain_zombie_reanim(&mut self, fx: &mut Fx) {
        self.attack_rect = IRect::new(20, 0, 50, 115);
        for prefix in [
            "anim_cone",
            "anim_bucket",
            "anim_screendoor",
            "Zombie_flaghand",
            "Zombie_duckytube",
            "anim_tongue",
            "Zombie_mustache",
        ] {
            self.reanim_show_prefix(fx, prefix, RENDER_GROUP_HIDDEN);
        }
        self.setup_door_arms(fx, false);
        match self.zombie_type {
            ZombieType::TrafficCone => {
                self.reanim_show_prefix(fx, "anim_cone", RENDER_GROUP_NORMAL);
                self.reanim_show_prefix(fx, "anim_hair", RENDER_GROUP_HIDDEN);
            }
            ZombieType::Pail => {
                self.reanim_show_prefix(fx, "anim_bucket", RENDER_GROUP_NORMAL);
                self.reanim_show_prefix(fx, "anim_hair", RENDER_GROUP_HIDDEN);
            }
            ZombieType::Flag => {
                self.reanim_show_prefix(fx, "anim_innerarm", RENDER_GROUP_HIDDEN);
                self.reanim_show_track(fx, "Zombie_flaghand", RENDER_GROUP_NORMAL);
                self.reanim_show_track(fx, "Zombie_innerarm_screendoor", RENDER_GROUP_NORMAL);
            }
            _ => {}
        }
    }

    /// `Zombie::SetupDoorArms`: the screen door zombie holds its door with
    /// arms of its own; everyone else has the plain ones.
    fn setup_door_arms(&self, fx: &mut Fx, show: bool) {
        let (arm, door) = if show { (RENDER_GROUP_HIDDEN, RENDER_GROUP_NORMAL) } else { (RENDER_GROUP_NORMAL, RENDER_GROUP_HIDDEN) };
        for prefix in ["Zombie_outerarm_hand", "Zombie_outerarm_lower", "Zombie_outerarm_upper", "anim_innerarm"] {
            self.reanim_show_prefix(fx, prefix, arm);
        }
        for prefix in ["Zombie_outerarm_screendoor", "Zombie_innerarm_screendoor", "Zombie_innerarm_screendoor_hand"] {
            self.reanim_show_prefix(fx, prefix, door);
        }
    }

    fn reanim_show_prefix(&self, fx: &mut Fx, prefix: &str, group: i32) {
        if let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b)) {
            body.assign_render_group_to_prefix(prefix, group);
        }
    }

    fn reanim_show_track(&self, fx: &mut Fx, track: &str, group: i32) {
        if let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b)) {
            body.assign_render_group_to_track(track, group);
        }
    }

    /// `Zombie::PickRandomSpeed`, for the plain zombies.
    fn pick_random_speed(&mut self, fx: &mut Fx) {
        let rng = fx.sounds.rng();
        if self.zombie_type == ZombieType::Flag {
            self.vel_x = 0.45;
        } else if self.phase == ZombiePhase::PolePreVault {
            self.vel_x = rng.range_f32(0.66, 0.68);
        } else {
            self.vel_x = rng.range_f32(0.23, 0.37);
            self.anim_ticks_per_frame = if self.vel_x < 0.3 { 12 } else { 15 };
        }
        self.update_anim_speed(fx.effects);
    }

    /// `Zombie::PlayZombieReanim`.
    fn play_zombie_reanim(&mut self, effects: &mut EffectSystem, track: &str, loop_type: LoopType, blend: i32, rate: f32) {
        let Some(body) = self.body.and_then(|b| effects.get_mut(b)) else { return };
        body.play(track, loop_type, blend, rate);
        if rate != 0.0 {
            self.original_anim_rate = rate;
        }
        self.update_anim_speed(effects);
    }

    /// `Zombie::StartWalkAnim`.
    fn start_walk_anim(&mut self, blend: i32, _env: &ZombieEnv, fx: &mut Fx) {
        let Some(body) = self.body.and_then(|b| fx.effects.get(b)) else { return };
        let has_walk2 = body.definition.find_track("anim_walk2").is_some();
        let has_walk = body.definition.find_track("anim_walk").is_some();
        self.pick_random_speed(fx);
        // TODO: the dance mode's "anim_dance".
        let mut variant = fx.sounds.rng().next_range(2);
        if self.zombie_type == ZombieType::Flag {
            variant = 0;
        }
        if variant == 0 && has_walk2 {
            self.play_zombie_reanim(fx.effects, "anim_walk2", LoopType::Loop, blend, 0.0);
        } else if has_walk {
            self.play_zombie_reanim(fx.effects, "anim_walk", LoopType::Loop, blend, 0.0);
        }
    }

    /// `Zombie::StartEating`.
    fn start_eating(&mut self, effects: &mut EffectSystem) {
        if self.is_eating {
            return;
        }
        self.is_eating = true;
        self.play_zombie_reanim(effects, "anim_eat", LoopType::Loop, 20, 0.0);
    }

    /// `Zombie::StopEating`.
    fn stop_eating(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        if !self.is_eating {
            return;
        }
        self.is_eating = false;
        if self.body.is_some() {
            self.start_walk_anim(20, env, fx);
        }
        self.update_anim_speed(fx.effects);
    }

    fn apply_anim_rate(&self, effects: &mut EffectSystem, rate: f32) {
        let rate = if self.chilled_counter > 0 { rate * 0.5 } else { rate };
        if let Some(body) = self.body.and_then(|b| effects.get_mut(b)) {
            body.anim_rate = rate;
        }
    }

    /// `Zombie::ApplyChill`: frozen zombies move and bite at half speed.
    fn apply_chill(&mut self, fx: &mut Fx) {
        if self.is_dead_or_dying() {
            return;
        }
        if self.chilled_counter == 0 {
            fx.sounds.play_foley(FoleyType::Frozen);
        }
        self.chilled_counter = self.chilled_counter.max(1000);
        self.update_anim_speed(fx.effects);
    }

    /// `Zombie::ApplyBurn`: cherry bombs and jalapenos.
    pub fn apply_burn(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        if self.dead || self.phase == ZombiePhase::Burned {
            return;
        }
        if self.body_health >= 1800 {
            self.take_damage(1800, 18, env, fx);
            return;
        }
        if matches!(self.phase, ZombiePhase::Dying | ZombiePhase::Mowered | ZombiePhase::PoleInVault) {
            self.die_with_loot(fx);
        } else if !self.has_head {
            // Headless zombies stay standing, charred, for a few moments.
            self.apply_anim_rate(fx.effects, 0.0);
            self.original_anim_rate = 0.0;
            self.phase = ZombiePhase::Burned;
            self.phase_counter = 300;
            self.just_got_shot_counter = 0;
            self.drop_loot();
        } else {
            if let Ok((_, charred)) =
                fx.effects.add_reanimation(self.pos_x + 22.0, self.pos_y - 10.0, ReanimationType::ZombieCharred, fx.res, fx.images)
            {
                charred.anim_rate *= fx.sounds.rng().range_f32(0.9, 1.1);
                charred.render_order = self.render_order;
            }
            self.die_with_loot(fx);
        }
    }

    /// `Zombie::UpdateBurn`.
    fn update_burn(&mut self, fx: &mut Fx) {
        self.phase_counter -= 1;
        if self.phase_counter == 0 {
            self.die_with_loot(fx);
        }
    }

    /// `Zombie::UpdateAnimSpeed`: the walk animation's speed follows the
    /// zombie's, so its feet don't slide.
    fn update_anim_speed(&self, effects: &mut EffectSystem) {
        if !self.is_on_board() {
            return;
        }
        let Some(body) = self.body.and_then(|b| effects.get(b)) else { return };
        if self.is_dead_or_dying() {
            self.apply_anim_rate(effects, self.original_anim_rate);
        } else if self.is_eating {
            self.apply_anim_rate(effects, 36.0);
        } else if let Some(track) = body.definition.find_track("_ground") {
            let frames = &body.definition.tracks[track].transforms;
            let last = (body.frame_start + body.frame_count).saturating_sub(1);
            let distance = frames[last].x - frames[body.frame_start].x;
            if distance >= 1e-6 {
                let one_over_speed = body.frame_count as f32 / distance;
                let rate = self.vel_x * one_over_speed * 47.0;
                self.apply_anim_rate(effects, rate);
            }
        } else {
            self.apply_anim_rate(effects, self.original_anim_rate);
        }
    }

    /// What `TotalZombiesHealthInWave` counts for this zombie.
    pub fn total_health(&self) -> i32 {
        self.body_health + self.helm_health
    }

    pub fn helm_type(&self) -> HelmType {
        self.helm_type
    }

    pub fn is_dead_or_dying(&self) -> bool {
        self.dead || matches!(self.phase, ZombiePhase::Dying | ZombiePhase::Burned | ZombiePhase::Mowered)
    }

    pub fn is_winner(&self) -> bool {
        self.from_wave == ZOMBIE_WAVE_WINNER
    }

    pub fn dropped_loot(&self) -> bool {
        self.dropped_loot
    }

    pub fn set_dropped_loot(&mut self) {
        self.dropped_loot = true;
    }

    /// `Zombie::IsWalkingBackwards`: not for the types so far.
    fn is_walking_backwards(&self) -> bool {
        false
    }

    /// `Zombie::GetDrawPos`.
    fn draw_pos(&self) -> DrawPos {
        let (head_x, head_y) = if self.is_eating {
            (47, 4)
        } else {
            match self.frame {
                0 => (50, 2),
                1 => (49, 1),
                2 => (49, 2),
                3 => (48, 4),
                4 => (48, 5),
                5 => (48, 4),
                6 => (48, 2),
                7 => (49, 1),
                8 => (49, 2),
                9 => (50, 4),
                10 => (50, 5),
                _ => (50, 4),
            }
        };
        let (mut image_offset_x, mut image_offset_y) = (self.pos_x - self.x as f32, self.pos_y - self.y as f32);
        if self.zombie_type == ZombieType::Polevaulter {
            image_offset_x -= 6.0;
            image_offset_y -= 11.0;
        }
        DrawPos { head_x, head_y, body_y: 0.0, image_offset_x, image_offset_y }
    }

    /// `Zombie::GetZombieRect`.
    pub fn zombie_rect(&self) -> IRect {
        let mut rect = self.zombie_rect;
        if self.is_walking_backwards() {
            rect.x = self.width - rect.x - rect.w;
        }
        rect.offset(self.x, self.y + self.draw_pos().body_y as i32)
    }

    /// `Zombie::GetZombieAttackRect`.
    fn attack_rect(&self) -> IRect {
        let mut rect = self.attack_rect;
        if self.is_walking_backwards() {
            rect.x = self.width - rect.x - rect.w;
        }
        rect.offset(self.x, self.y + self.draw_pos().body_y as i32)
    }

    /// `Zombie::EffectedByDamage`.
    pub fn effected_by_damage(&self, damage_range: u32) -> bool {
        if damage_range & damage_range::DYING == 0 && self.is_dead_or_dying() {
            return false;
        }
        if damage_range & damage_range::ONLY_MINDCONTROLLED != 0 {
            return false;
        }
        if self.zombie_rect().x > WIDE_BOARD_WIDTH as i32 {
            return false;
        }
        if self.phase == ZombiePhase::PoleInVault {
            return damage_range & damage_range::OFF_GROUND != 0;
        }
        // Nothing's flying, submerged or underground yet.
        damage_range & damage_range::GROUND != 0
    }

    /// `Zombie::ZombieTargetLeadX`, for lobbed projectiles.
    #[allow(dead_code, reason = "used by the pult plants once they're ported")]
    pub fn target_lead_x(&self, time: f32) -> f32 {
        let speed = if self.is_eating { 0.0 } else { self.vel_x };
        let rect = self.zombie_rect();
        (rect.x + rect.w / 2) as f32 - speed * time
    }

    /// `Zombie::GetTrackPosition`.
    fn track_position(&self, effects: &EffectSystem, track: &str) -> (f32, f32) {
        let Some(body) = self.body.and_then(|b| effects.get(b)) else { return (self.pos_x, self.pos_y) };
        let m = body.track_matrix(body.find_track(track));
        (m.m02 + self.pos_x, m.m12 + self.pos_y)
    }

    /// `Zombie::Update`, while playing.
    pub fn update(&mut self, plants: &mut DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        self.age += 1;
        match self.phase {
            ZombiePhase::Dying => {
                self.update_death(fx);
                self.update_zombie_walking(fx.effects);
            }
            ZombiePhase::Mowered => self.update_mowered(fx),
            ZombiePhase::Burned => self.update_burn(fx),
            ZombiePhase::Normal | ZombiePhase::PolePreVault | ZombiePhase::PoleInVault => {
                self.update_playing(plants, env, fx);
                self.animate(plants, fx);
            }
        }
        self.just_got_shot_counter -= 1;
        if self.zombie_fade > 0 {
            self.zombie_fade -= 1;
            if self.zombie_fade == 0 {
                self.die_no_loot(fx.effects);
            }
        }
        self.x = self.pos_x as i32;
        self.y = self.pos_y as i32;
        // AttachmentUpdateAndMove.
        for &(id, dx, dy) in &self.attached {
            if let Some(system) = fx.effects.particle_mut(id) {
                system.system_move(self.pos_x + dx, self.pos_y + dy);
            }
        }
        self.update_reanim(fx.effects);
    }

    /// `Zombie::Update` for the zombie that's walking into the house after
    /// the level was lost: the others stand still.
    pub fn update_winner(&mut self, show_walking: bool, plants: &mut DataArray<Plant>, fx: &mut Fx) {
        self.age += 1;
        if show_walking {
            self.update_zombie_walking(fx.effects);
        }
        self.animate(plants, fx);
        self.just_got_shot_counter -= 1;
        self.x = self.pos_x as i32;
        self.y = self.pos_y as i32;
        self.update_reanim(fx.effects);
    }

    /// `Zombie::WalkIntoHouse`, for the day and night lawns.
    pub fn walk_into_house(&mut self) {
        self.from_wave = ZOMBIE_WAVE_WINNER;
        // TODO: the pool, roof and boss lawns end differently.
        self.pos_y = 290.0;
        self.render_order = make_render_order(render_layer::ZOMBIE, 2, 0);
    }

    /// `Zombie::UpdateMowered`: once the mower's done, it's over.
    fn update_mowered(&mut self, fx: &mut Fx) {
        let done = self.mowered_reanim.and_then(|m| fx.effects.get(m)).is_none_or(|m| m.loop_count > 0);
        if done {
            self.drop_head(0, fx);
            self.drop_arm(0, fx);
            self.die_with_loot(fx);
        }
    }

    /// `Zombie::MowDown`.
    pub fn mow_down(&mut self, fx: &mut Fx) {
        if self.dead || self.phase == ZombiePhase::Mowered {
            return;
        }
        if self.phase == ZombiePhase::Dying {
            // Already falling over: just a puff of smoke.
            if let Ok((_, puff)) = fx.effects.add_reanimation(self.pos_x - 73.0, self.pos_y - 56.0, ReanimationType::Puff, fx.res, fx.images) {
                puff.render_order = self.render_order + 2;
                puff.set_frames_for_layer("anim_puff");
            }
            let rng = fx.sounds.rng();
            fx.effects.add_particle(self.pos_x + 110.0, self.pos_y, self.render_order + 1, ParticleEffect::MowerCloud, fx.res, fx.images, rng);
            self.drop_head(0, fx);
            self.drop_arm(0, fx);
            self.drop_helm(0, fx);
            self.die_with_loot(fx);
            return;
        }
        self.drop_helm(0, fx);
        if self.zombie_type == ZombieType::Flag {
            self.drop_flag(fx);
        }
        if let Ok((id, reanim)) =
            fx.effects.add_reanimation(0.0, 0.0, ReanimationType::LawnMoweredZombie, fx.res, fx.images)
        {
            reanim.anim_rate = 8.0;
            reanim.is_attachment = false;
            reanim.loop_type = LoopType::PlayOnceAndHold;
            reanim.render_order = self.render_order;
            self.mowered_reanim = Some(id);
        }
        self.phase = ZombiePhase::Mowered;
        self.drop_loot();
    }

    /// `Zombie::DropLoot`: asks the board whether this zombie ends the level.
    fn drop_loot(&mut self) {
        // TODO: coins for the zombie's value (DropLootPiece), from level 1-11 on.
        self.loot_pending = true;
    }

    /// `Zombie::DieWithLoot`.
    pub fn die_with_loot(&mut self, fx: &mut Fx) {
        self.die_no_loot(fx.effects);
        self.drop_loot();
    }

    /// `Zombie::UpdatePlaying`.
    fn update_playing(&mut self, plants: &mut DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        if self.chilled_counter > 0 {
            self.chilled_counter -= 1;
            if self.chilled_counter == 0 {
                self.update_anim_speed(fx.effects);
            }
        }
        self.groan_counter -= 1;
        if self.groan_counter == 0
            && fx.sounds.rng().next_range(env.zombie_count.max(1) as u32) == 0
            && self.has_head
        {
            fx.sounds.play_foley_pitch(FoleyType::Groan, 0.0);
            self.groan_counter = fx.sounds.rng().next_range(1000) as i32 + 500;
        }

        if self.zombie_type == ZombieType::Polevaulter {
            self.update_zombie_polevaulter(plants, env, fx);
        }
        self.update_zombie_position(fx.effects, env);
        self.check_if_prey_caught(plants, env, fx);

        // A zombie that's lost its head dies slowly.
        if !self.is_dead_or_dying() && !self.has_head && fx.sounds.rng().next_range(5) == 0 {
            let damage = if self.body_max_health >= 500 { 3 } else { 1 };
            self.take_damage(damage, damage::HEADLESS, env, fx);
        }
    }

    /// `Zombie::UpdateZombiePolevaulter`: runs until a plant's in reach,
    /// then vaults over it and walks on.
    fn update_zombie_polevaulter(&mut self, plants: &DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        if self.phase == ZombiePhase::PolePreVault && self.has_head {
            if self.find_plant_target(plants).is_some() {
                let plant_x = self.find_plant_target(plants).and_then(|id| plants.get(id)).map_or(0, |p| p.x);
                self.phase = ZombiePhase::PoleInVault;
                self.attack_rect = IRect::new(-40, 0, 100, 115);
                self.play_zombie_reanim(fx.effects, "anim_jump", LoopType::PlayOnceAndHold, 20, 24.0);
                let duration = self
                    .body
                    .and_then(|b| fx.effects.get(b))
                    .map_or(1.0, |b| b.frame_count as f32 / b.anim_rate * 100.0);
                let distance = if env.bowling { 0 } else { self.x - plant_x - 80 };
                self.vel_x = distance as f32 / duration;
                self.has_object = false;
            }
        } else if self.phase == ZombiePhase::PoleInVault {
            let Some(body) = self.body.and_then(|b| fx.effects.get(b)) else { return };
            let ended = body.loop_count > 0;
            if body.should_trigger_timed_event(0.2) {
                fx.sounds.play_sample("SOUND_GRASSSTEP");
            }
            if body.should_trigger_timed_event(0.4) {
                fx.sounds.play_sample("SOUND_POLEVAULT");
            }
            // TODO: tall-nuts stop the jump short (FOLEY_BONK).
            if ended {
                self.pos_x -= 150.0;
                self.x = self.pos_x as i32;
                self.phase = ZombiePhase::Normal;
                self.attack_rect = IRect::new(50, 0, 20, 115);
                self.start_walk_anim(0, env, fx);
            }
        }
    }

    /// `Zombie::UpdateZombiePosition`.
    fn update_zombie_position(&mut self, effects: &mut EffectSystem, env: &ZombieEnv) {
        self.update_zombie_walking(effects);
        let desired = self.pos_y_based_on_row(self.row, env);
        if self.pos_y < desired {
            self.pos_y += (desired - self.pos_y).min(1.0);
        } else if self.pos_y > desired {
            self.pos_y -= (self.pos_y - desired).min(1.0);
        }
    }

    /// `Zombie::UpdateZombieWalking`: the walk animation's `_ground` track
    /// says how far to move each tick.
    fn update_zombie_walking(&mut self, effects: &EffectSystem) {
        if self.is_eating {
            return;
        }
        let speed = match self.body.and_then(|b| effects.get(b)) {
            _ if self.phase == ZombiePhase::PoleInVault => self.vel_x,
            Some(body) if body.definition.find_track("_ground").is_some() => body.track_velocity("_ground"),
            _ if self.chilled_counter > 0 => self.vel_x * 0.4,
            _ => self.vel_x,
        };
        self.pos_x -= speed;
    }

    /// `Zombie::CheckIfPreyCaught`: every few ticks, bite whatever's in reach.
    fn check_if_prey_caught(&mut self, plants: &mut DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        let ticks_between_eats = if self.chilled_counter > 0 { TICKS_BETWEEN_EATS * 2 } else { TICKS_BETWEEN_EATS };
        if !self.has_head || self.age % ticks_between_eats != 0 || self.is_vaulting() {
            return;
        }
        // Zombies ignore the rolling nuts on the bowling level (CanTargetPlant).
        let target = if env.bowling { None } else { self.find_plant_target(plants) };
        if let Some(id) = target {
            self.eat_plant(plants, id, fx);
        } else if self.is_eating {
            self.stop_eating(env, fx);
        }
    }

    /// `Zombie::FindPlantTarget`, for chewing.
    fn find_plant_target(&self, plants: &DataArray<Plant>) -> Option<sexy::data_array::Id<Plant>> {
        let attack = self.attack_rect();
        plants
            .iter()
            .find(|(_, plant)| {
                plant.row == self.row && !plant.dead && rect_overlap(attack, plant.rect()) >= 20 && can_target(plant)
            })
            .map(|(id, _)| id)
    }

    /// `Zombie::EatPlant`.
    fn eat_plant(&mut self, plants: &mut DataArray<Plant>, id: sexy::data_array::Id<Plant>, fx: &mut Fx) {
        self.start_eating(fx.effects);
        if self.chilled_counter > 0 && self.age % 2 == 1 {
            return;
        }
        let Some(plant) = plants.get_mut(id) else { return };
        // TODO: instant plants and others in a special state aren't eaten
        // (cherry bomb, jalapeno, squash, ...).
        plant.health -= DAMAGE_PER_EAT;
        plant.recently_eaten_countdown = 50;
        if plant.health <= 0 {
            fx.sounds.play_sample("SOUND_GULP");
            plant.die(fx.effects);
            // TODO: the plants-eaten count and "peashooter died" advice.
        }
    }

    /// `Zombie::Animate`.
    fn animate(&mut self, plants: &mut DataArray<Plant>, fx: &mut Fx) {
        self.anim_counter += 1;
        if self.is_eating && self.has_head {
            let frame_length = 6;
            if self.anim_counter >= self.anim_frames * frame_length {
                self.anim_counter = frame_length;
            }
            self.frame = self.anim_counter / frame_length;
            let (left, right) = if self.zombie_type == ZombieType::Polevaulter { (0.38, 0.8) } else { (0.14, 0.68) };
            let bite = self
                .body
                .and_then(|b| fx.effects.get(b))
                .is_some_and(|b| b.should_trigger_timed_event(left) || b.should_trigger_timed_event(right));
            if bite {
                self.animate_chew(plants, fx);
            }
        } else {
            if self.anim_counter >= self.anim_frames * self.anim_ticks_per_frame {
                self.anim_counter = 0;
            }
            self.frame = self.anim_counter / self.anim_ticks_per_frame;
        }
    }

    /// `Zombie::AnimateChewSound` and `AnimateChewEffect`: the bite's sound,
    /// the plant flashing, and nuts spitting crumbs.
    fn animate_chew(&self, plants: &mut DataArray<Plant>, fx: &mut Fx) {
        let target = self.find_plant_target(plants);
        let nut = target
            .and_then(|id| plants.get(id))
            .is_some_and(|p| matches!(p.seed, crate::plant::SeedType::Wallnut | crate::plant::SeedType::Tallnut));
        fx.sounds.play_foley(if nut { FoleyType::ChompSoft } else { FoleyType::Chomp });
        if let Some(plant) = target.and_then(|id| plants.get_mut(id)) {
            plant.eaten_flash_countdown = plant.eaten_flash_countdown.max(25);
        }
        if nut {
            let order = make_render_order(render_layer::PROJECTILE, self.row, 0);
            let rng = fx.sounds.rng();
            fx.effects.add_particle(self.pos_x + 37.0, self.pos_y + 40.0, order, ParticleEffect::WallnutEatSmall, fx.res, fx.images, rng);
        }
    }

    /// `Zombie::UpdateReanim`: puts the body where the zombie is.
    fn update_reanim(&self, effects: &mut EffectSystem) {
        let Some(body_id) = self.body else { return };
        let pos = self.draw_pos();
        let offset_x = pos.image_offset_x + 15.0;
        let offset_y = pos.image_offset_y + pos.body_y - 28.0 + 20.0;
        let fade = if self.zombie_fade >= 0 { (255 * self.zombie_fade / 10).clamp(0, 255) as u8 } else { 255 };
        // DrawReanim's colors: chilled zombies are blue, burned ones black,
        // and a zombie that was just hit flashes white.
        let (tint, mut extra): ([u8; 3], Option<[u8; 3]>) = if self.phase == ZombiePhase::Burned {
            ([0, 0, 0], None)
        } else if self.chilled_counter > 0 {
            ([75, 75, 255], Some([75, 75, 255]))
        } else {
            ([255, 255, 255], None)
        };
        if self.just_got_shot_counter > 0 {
            let flash = (self.just_got_shot_counter * 10).clamp(0, 255) as u8;
            let base = extra.unwrap_or([0, 0, 0]);
            extra = Some([base[0].saturating_add(flash), base[1].saturating_add(flash), base[2].saturating_add(flash)]);
        }
        if let Some(body) = effects.get_mut(body_id) {
            body.overlay.m01 = 0.0;
            body.overlay.m10 = 0.0;
            body.override_scale(1.0, 1.0);
            body.set_position(offset_x, offset_y);
            body.color_override = [tint[0], tint[1], tint[2], fade];
            body.extra_additive = extra.map(|c| [c[0], c[1], c[2], 255]);
        }
        // The mower's animation carries the zombie along.
        if let Some(mowered) = self.mowered_reanim {
            effects.update_reanimation(mowered);
            let matrix = effects.get(mowered).map(|m| m.attachment_overlay_matrix(0));
            if let (Some(mut m), Some(body)) = (matrix, effects.get_mut(body_id)) {
                m.m02 += body.overlay.m02;
                m.m12 += body.overlay.m12;
                body.overlay = m;
            }
        }
        effects.update_reanimation(body_id);
    }

    /// `Zombie::TakeDamage`.
    pub fn take_damage(&mut self, damage: i32, flags: u32, env: &ZombieEnv, fx: &mut Fx) {
        if self.is_dead_or_dying() {
            return;
        }
        let mut remaining = damage;
        // TODO: shields (screen door, newspaper, ladder).
        if remaining > 0 && self.helm_type != HelmType::None {
            remaining = self.take_helm_damage(remaining, flags, fx);
        }
        if remaining > 0 {
            self.take_body_damage(remaining, flags, env, fx);
        }
    }

    /// A rolling wall-nut's hit (`Plant::UpdateBowling`): a helmet takes the
    /// blow, otherwise the zombie is flattened.
    pub fn bowling_damage(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        match self.helm_type {
            HelmType::Pail => {
                fx.sounds.play_foley(FoleyType::ShieldHit);
                self.take_helm_damage(900, 0, fx);
            }
            HelmType::TrafficCone => {
                fx.sounds.play_foley(FoleyType::PlasticHit);
                self.take_helm_damage(900, 0, fx);
            }
            HelmType::None => self.take_damage(1800, 0, env, fx),
        }
    }

    /// `Zombie::GetHelmDamageIndex`.
    fn helm_damage_index(&self) -> i32 {
        if self.helm_health < self.helm_max_health / 3 {
            2
        } else if self.helm_health < self.helm_max_health * 2 / 3 {
            1
        } else {
            0
        }
    }

    /// `Zombie::GetBodyDamageIndex`.
    #[allow(dead_code, reason = "damage states of the bigger zombies")]
    fn body_damage_index(&self) -> i32 {
        if self.body_health < self.body_max_health / 3 {
            2
        } else if self.body_health < self.body_max_health * 2 / 3 {
            1
        } else {
            0
        }
    }

    /// `Zombie::TakeHelmDamage`: returns the damage the helmet didn't absorb.
    fn take_helm_damage(&mut self, damage: i32, flags: u32, fx: &mut Fx) -> i32 {
        if flags & damage::DOESNT_CAUSE_FLASH == 0 {
            self.just_got_shot_counter = 25;
        }
        let index_before = self.helm_damage_index();
        let absorbed = self.helm_health.min(damage);
        let remaining = damage - absorbed;
        self.helm_health -= absorbed;
        if flags & damage::FREEZE != 0 {
            self.apply_chill(fx);
        }
        if self.helm_health == 0 {
            self.drop_helm(flags, fx);
            return remaining;
        }
        let index_after = self.helm_damage_index();
        if index_before != index_after
            && let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b))
        {
            let (track, images) = match self.helm_type {
                HelmType::TrafficCone => ("anim_cone", ["IMAGE_REANIM_ZOMBIE_CONE2", "IMAGE_REANIM_ZOMBIE_CONE3"]),
                HelmType::Pail => ("anim_bucket", ["IMAGE_REANIM_ZOMBIE_BUCKET2", "IMAGE_REANIM_ZOMBIE_BUCKET3"]),
                HelmType::None => return remaining,
            };
            if index_after >= 1 {
                body.set_image_override(track, Some(images[index_after as usize - 1]));
            }
        }
        remaining
    }

    /// `Zombie::DropHelm`.
    fn drop_helm(&mut self, flags: u32, fx: &mut Fx) {
        let (track, effect) = match self.helm_type {
            HelmType::TrafficCone => ("anim_cone", ParticleEffect::ZombieTrafficCone),
            HelmType::Pail => ("anim_bucket", ParticleEffect::ZombiePail),
            HelmType::None => return,
        };
        let (x, y) = self.track_position(fx.effects, track);
        self.reanim_show_prefix(fx, track, RENDER_GROUP_HIDDEN);
        self.reanim_show_prefix(fx, "anim_hair", RENDER_GROUP_NORMAL);
        if flags & damage::DOESNT_LEAVE_BODY == 0 {
            let rng = fx.sounds.rng();
            fx.effects.add_particle(x, y, self.render_order + 1, effect, fx.res, fx.images, rng);
        }
        self.helm_type = HelmType::None;
    }

    /// `Zombie::TakeBodyDamage`.
    fn take_body_damage(&mut self, damage: i32, flags: u32, env: &ZombieEnv, fx: &mut Fx) {
        if flags & damage::DOESNT_CAUSE_FLASH == 0 {
            self.just_got_shot_counter = 25;
        }
        if flags & damage::FREEZE != 0 {
            self.apply_chill(fx);
        }
        self.body_health -= damage;
        self.update_damage_states(flags, fx);
        if self.body_health <= 0 {
            self.body_health = 0;
            self.play_death_anim(flags, env, fx);
            self.drop_loot();
        }
    }

    /// `Zombie::UpdateDamageStates`: arm off at two thirds, head at one third.
    fn update_damage_states(&mut self, flags: u32, fx: &mut Fx) {
        if self.has_arm && self.body_health < 2 * self.body_max_health / 3 && self.body_health > 0 {
            self.drop_arm(flags, fx);
        }
        if self.has_head && self.body_health < self.body_max_health / 3 {
            self.drop_head(flags, fx);
            self.drop_loot();
            // TODO: dying right away once the level's award has dropped.
        }
    }

    /// `Zombie::DropArm`.
    fn drop_arm(&mut self, flags: u32, fx: &mut Fx) {
        if !self.has_arm {
            return;
        }
        self.has_arm = false;
        let (lower, upper, upper_image) = if self.zombie_type == ZombieType::Polevaulter {
            (
                "Zombie_polevaulter_outerarm_lower",
                "Zombie_polevaulter_outerarm_upper",
                "IMAGE_REANIM_ZOMBIE_POLEVAULTER_OUTERARM_UPPER2",
            )
        } else {
            ("Zombie_outerarm_lower", "Zombie_outerarm_upper", "IMAGE_REANIM_ZOMBIE_OUTERARM_UPPER2")
        };
        self.reanim_show_prefix(fx, lower, RENDER_GROUP_HIDDEN);
        self.reanim_show_prefix(fx, "Zombie_outerarm_hand", RENDER_GROUP_HIDDEN);
        let pos = self.draw_pos();
        let (mut x, mut y) = (self.pos_x + pos.image_offset_x + 45.0, self.pos_y + pos.image_offset_y + pos.body_y + 78.0);
        if self.body.is_some() {
            (x, y) = self.track_position(fx.effects, lower);
            if let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b)) {
                body.set_image_override(upper, Some(upper_image));
            }
            if self.zombie_type == ZombieType::Flag
                && let Some(flag) = self.special_head.and_then(|f| fx.effects.get_mut(f))
            {
                flag.set_image_override("Zombie_flag", Some("IMAGE_REANIM_ZOMBIE_FLAG3"));
            }
        }
        if flags & damage::DOESNT_LEAVE_BODY == 0 {
            let rng = fx.sounds.rng();
            let particle = fx.effects.add_particle(x, y, self.render_order + 1, ParticleEffect::ZombieArm, fx.res, fx.images, rng);
            if self.zombie_type == ZombieType::Polevaulter
                && let Some(system) = particle.and_then(|p| fx.effects.particle_mut(p))
            {
                system.override_image(None, Some("IMAGE_REANIM_ZOMBIE_OUTERARM_HAND"));
            }
        }
        fx.sounds.play_foley(FoleyType::LimbsPop);
    }

    /// `Zombie::DropHead`.
    fn drop_head(&mut self, flags: u32, fx: &mut Fx) {
        if !self.has_head {
            return;
        }
        self.has_head = false;
        self.reanim_show_prefix(fx, "anim_head", RENDER_GROUP_HIDDEN);
        self.reanim_show_prefix(fx, "anim_hair", RENDER_GROUP_HIDDEN);
        self.reanim_show_prefix(fx, "anim_tongue", RENDER_GROUP_HIDDEN);
        if flags & damage::DOESNT_LEAVE_BODY != 0 {
            return;
        }
        if self.zombie_type == ZombieType::Flag {
            self.drop_flag(fx);
        } else if self.zombie_type == ZombieType::Polevaulter {
            // DropPole.
            for prefix in ["Zombie_polevaulter_innerarm", "Zombie_polevaulter_innerhand", "Zombie_polevaulter_pole"] {
                self.reanim_show_prefix(fx, prefix, RENDER_GROUP_HIDDEN);
            }
        }
        let pos = self.draw_pos();
        let (mut x, mut y) = (
            self.pos_x + pos.image_offset_x + pos.head_x as f32 + 11.0,
            self.pos_y + pos.image_offset_y + pos.head_y as f32 + pos.body_y + 21.0,
        );
        if self.body.is_some() {
            (x, y) = self.track_position(fx.effects, "anim_head1");
        }
        let rng = fx.sounds.rng();
        let particle = fx.effects.add_particle(x, y, self.render_order + 1, ParticleEffect::ZombieHead, fx.res, fx.images, rng);
        if self.zombie_type == ZombieType::Polevaulter
            && let Some(system) = particle.and_then(|p| fx.effects.particle_mut(p))
        {
            system.override_image(None, Some("IMAGE_ZOMBIEPOLEVAULTERHEAD"));
        }
        fx.sounds.play_foley(FoleyType::LimbsPop);
    }

    /// `Zombie::DropFlag`.
    fn drop_flag(&mut self, fx: &mut Fx) {
        if self.zombie_type != ZombieType::Flag || !self.has_object {
            return;
        }
        if let Some(flag) = self.special_head.take() {
            fx.effects.kill(flag);
        }
        self.reanim_show_prefix(fx, "anim_innerarm", RENDER_GROUP_NORMAL);
        self.reanim_show_track(fx, "Zombie_flaghand", RENDER_GROUP_HIDDEN);
        self.reanim_show_track(fx, "Zombie_innerarm_screendoor", RENDER_GROUP_HIDDEN);
        self.has_object = false;
        let (x, y) = self.track_position(fx.effects, "Zombie_flaghand");
        let rng = fx.sounds.rng();
        fx.effects.add_particle(x + 6.0, y - 45.0, self.render_order + 1, ParticleEffect::ZombieFlag, fx.res, fx.images, rng);
    }

    /// `Zombie::PlayDeathAnim`.
    fn play_death_anim(&mut self, flags: u32, env: &ZombieEnv, fx: &mut Fx) {
        if self.phase == ZombiePhase::Dying {
            return;
        }
        let has_death = self
            .body
            .and_then(|b| fx.effects.get(b))
            .is_some_and(|b| b.definition.find_track("anim_death").is_some());
        if !has_death {
            self.die_no_loot(fx.effects);
            return;
        }
        if flags & damage::DOESNT_LEAVE_BODY != 0 {
            self.die_no_loot(fx.effects);
            return;
        }
        self.stop_eating(env, fx);
        self.vel_x = 0.0;
        self.phase = ZombiePhase::Dying;

        let rng = fx.sounds.rng();
        let mut rate = rng.range_f32(24.0, 30.0);
        let hit = rng.next_range(100);
        let can_super_long = env.finished_adventure || env.level > 5;
        let body = self.body.and_then(|b| fx.effects.get(b));
        let has_track = |name: &str| body.is_some_and(|b| b.definition.find_track(name).is_some());
        let mut track = "anim_death";
        if hit == 99 && has_track("anim_superlongdeath") && can_super_long && env.zombies_on_screen <= 5 {
            rate = 14.0;
            track = "anim_superlongdeath";
        } else if hit > 50 && has_track("anim_death2") {
            track = "anim_death2";
        }
        self.play_zombie_reanim(fx.effects, track, LoopType::PlayOnceAndHold, 20, rate);
        self.reanim_show_prefix(fx, "anim_tongue", RENDER_GROUP_HIDDEN);
    }

    /// `Zombie::UpdateDeath`.
    fn update_death(&mut self, fx: &mut Fx) {
        let Some(body) = self.body.and_then(|b| fx.effects.get(b)) else {
            self.die_no_loot(fx.effects);
            return;
        };
        let fall_time = if self.zombie_type == ZombieType::Polevaulter {
            0.68
        } else if body.is_anim_playing("anim_superlongdeath") {
            0.788
        } else if body.is_anim_playing("anim_death2") {
            0.71
        } else {
            0.77
        };
        if body.should_trigger_timed_event(fall_time) {
            fx.sounds.play_foley(FoleyType::ZombieFalling);
        }
        if self.zombie_fade == -1 && body.loop_count > 0 {
            self.zombie_fade = 100;
        }
    }

    /// `Zombie::DieNoLoot`.
    pub fn die_no_loot(&mut self, effects: &mut EffectSystem) {
        for id in [self.body.take(), self.special_head.take(), self.mowered_reanim.take()].into_iter().flatten() {
            effects.kill(id);
        }
        for &(id, _, _) in &self.attached {
            effects.remove_particle(id);
        }
        self.attached.clear();
        self.dead = true;
    }

    /// `Zombie::AddAttachedParticle`: the effect moves along with the zombie.
    pub fn add_attached_particle(&mut self, x: f32, y: f32, effect: ParticleEffect, fx: &mut Fx) {
        let rng = fx.sounds.rng();
        let render_order = self.render_order + 1;
        if let Some(id) =
            fx.effects.add_particle(self.pos_x + x, self.pos_y + y, render_order, effect, fx.res, fx.images, rng)
        {
            self.attached.push((id, x, y));
        }
    }

    /// `Zombie::CheckForBoardEdge`: the zombie has got into the house.
    pub fn reached_house(&self) -> bool {
        let edge = if self.zombie_type == ZombieType::Polevaulter { -150 } else { BOARD_EDGE };
        self.has_head && self.x <= edge && !self.is_dead_or_dying()
    }

    /// Running up to a plant, or in mid-jump: plants that bite or explode
    /// under a zombie can't get it now.
    pub fn is_vaulting(&self) -> bool {
        matches!(self.phase, ZombiePhase::PolePreVault | ZombiePhase::PoleInVault)
    }

    /// `Zombie::Draw`: the body (with its flag), in the zombie's own
    /// coordinates (its `mX`, `mY`).
    pub fn draw(&self, g: &mut Graphics, effects: &EffectSystem, res: &GameResources) {
        g.push_state();
        g.translate(self.x as f32, self.y as f32);
        if self.is_winner() {
            // Winners vanish into the house (SetupDrawZombieWon).
            g.clip_rect(-123.0 - self.x as f32, -self.y as f32, 800.0, 600.0);
        }
        if let Some(body) = self.body {
            effects.draw(g, body, res);
        }
        g.pop_state();
    }

    /// `Zombie::DrawShadow`.
    pub fn draw_shadow(&self, g: &mut Graphics, night: bool, res: &GameResources) {
        if matches!(self.phase, ZombiePhase::Dying | ZombiePhase::PoleInVault) {
            return;
        }
        let pos = self.draw_pos();
        let vaulter = self.zombie_type == ZombieType::Polevaulter;
        let offset_x = pos.image_offset_x + if vaulter { 36.0 } else { 23.0 };
        let offset_y = pos.image_offset_y + pos.body_y + if vaulter { 11.0 } else { 0.0 };
        let Some(shadow) = res.image(if night { "IMAGE_PLANTSHADOW2" } else { "IMAGE_PLANTSHADOW" }) else { return };
        g.push_state();
        g.translate(self.x as f32, self.y as f32);
        g.draw_image_cel_center_scaled(shadow, offset_x, offset_y + 92.0, 0, 1.0, 1.0);
        g.pop_state();
    }
}

/// `Zombie::CanTargetPlant`, for chewing: spiky plants and the like aren't
/// ported yet, so everything on the ground is food.
fn can_target(plant: &Plant) -> bool {
    !plant.dead && plant.seed != crate::plant::SeedType::Tanglekelp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_overlap_is_horizontal_and_negative_for_gaps() {
        let a = IRect::new(0, 0, 50, 10);
        assert_eq!(rect_overlap(a, IRect::new(30, 500, 50, 10)), 20);
        assert_eq!(rect_overlap(a, IRect::new(50, 0, 10, 10)), 0);
        assert_eq!(rect_overlap(a, IRect::new(70, 0, 10, 10)), -20);
        // One inside the other: the smaller one's width.
        assert_eq!(rect_overlap(a, IRect::new(10, 0, 5, 10)), 5);
    }
}
