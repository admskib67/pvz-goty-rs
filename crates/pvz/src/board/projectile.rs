//! Things plants shoot (`Projectile`). So far the pea, the snow pea, the
//! puff-shroom's short-range puff, the torchwood's fire pea, and the
//! catapult plants' lobbed cabbages, kernels, butter and melons, and the
//! catapult zombie's basketballs, which come down on plants.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Projectile.cpp`:
//! `ProjectileInitialize`, `Update`, `UpdateMotion`, `UpdateNormalMotion`,
//! `CheckForCollision`, `FindCollisionTarget`, `PlayImpactSound`,
//! `DoImpact`, `Draw`, `DrawShadow`, `Die` and `GetProjectileRect`.

use bevy::math::{Affine2, Vec2};
use sexy::data_array::DataArray;
use sexy::rand::MtRand;
use sexy::tod::{TodCurve, animate_curve_float};

use crate::audio::FoleyType;
use crate::graphics::Graphics;
use crate::particle::ParticleEffect;
use crate::reanim::{ParticleSystemId, ReanimationId, ReanimationType};
use crate::resources::GameResources;

use super::plant::{Plant, PlantState, find_umbrella_plant, top_plant_any};
use super::zombie::{Basketball, Fx, HelmType, IRect, Zombie, ZombieEnv, damage, damage_range, rect_overlap};
use super::{make_render_order, render_layer};

/// `ProjectileType`, for what's ported so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectileType {
    Pea,
    Snowpea,
    /// A zombotany head's pea, fired backwards at the plants
    /// (`PROJECTILE_ZOMBIE_PEA`).
    ZombiePea,
    Puff,
    /// A pea that has passed a torchwood (`PROJECTILE_FIREBALL`).
    Fire,
    /// The cabbage-pult's cabbage, the kernel-pult's kernel and butter, the
    /// melon-pult's melon: lobbed over the lawn.
    Cabbage,
    Kernel,
    Butter,
    Melon,
    /// The winter melon's melon: a melon whose splash chills.
    Wintermelon,
    /// The catapult zombie's basketball, thrown at plants.
    Basketball,
    /// The cactus's and cattail's spike (`PROJECTILE_SPIKE`).
    Spike,
    /// The starfruit's star (`PROJECTILE_STAR`).
    Star,
    /// The cob cannon's cob (`PROJECTILE_COBBIG`): up and out of sight, then
    /// down on the spot it was aimed at.
    Cob,
}

impl ProjectileType {
    /// `ProjectileDefinition::mDamage`.
    fn damage(self) -> i32 {
        match self {
            Self::Pea | Self::Snowpea | Self::Puff | Self::Spike | Self::Star | Self::ZombiePea => 20,
            Self::Fire | Self::Cabbage | Self::Butter => 40,
            Self::Kernel => 20,
            Self::Melon | Self::Wintermelon => 80,
            Self::Basketball => 75,
            Self::Cob => 300,
        }
    }
}

/// `ProjectileMotion`, for the shots that don't fly straight right.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProjectileMotion {
    Straight,
    /// A split pea's back head: it flies left (`MOTION_BACKWARDS`).
    Backwards,
    /// A starfruit's star, at this speed (`MOTION_STAR`).
    Star { vel_x: f32, vel_y: f32 },
    /// A cattail's spike, steering towards a zombie (`MOTION_HOMING`).
    Homing(sexy::data_array::Id<Zombie>),
}

#[derive(Debug)]
pub struct Projectile {
    pub ty: ProjectileType,
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
    shadow_y: f32,
    pub row: i32,
    pub render_order: i32,
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    age: i32,
    damage_range_flags: u32,
    /// A puff-shroom's puff only goes so far (`MOTION_PUFF`).
    pub short_range: bool,
    motion: ProjectileMotion,
    /// The trail that follows it (`mAttachmentID`), and its offset.
    trail: Option<(ParticleSystemId, f32, f32)>,
    /// A threepeater's side pea drifts towards its lane at this speed
    /// (`mVelY`, `MOTION_THREEPEATER`).
    vel_y: f32,
    /// The flames around a fire pea (`mAttachmentID`).
    fire: Option<ReanimationId>,
    /// The last torchwood column it passed (`mHitTorchwoodGridX`), so one
    /// torchwood changes it only once.
    hit_torchwood_col: Option<i32>,
    /// Lobbed in an arc (`MOTION_LOBBED`): `vel_x` along, `vel_z` up and down
    /// (negative is up), pulled by `acc_z` each tick.
    lobbed: bool,
    vel_x: f32,
    vel_z: f32,
    acc_z: f32,
    rotation: f32,
    rotation_speed: f32,
    /// Where a cob lands (`mCobTargetX`, `mCobTargetRow`) and its square's y.
    cob_target: Option<(f32, i32, f32)>,
    /// A cob has just come down: the board shakes.
    pub landed: bool,
    pub dead: bool,
}

impl Projectile {
    /// `Projectile::ProjectileInitialize`. `shadow_y` is where the shadow
    /// lies (the row's ground, found by the board).
    pub fn new(x: i32, y: i32, render_order: i32, row: i32, shadow_y: f32, ty: ProjectileType) -> Self {
        Self {
            ty,
            pos_x: x as f32,
            pos_y: y as f32,
            pos_z: 0.0,
            shadow_y,
            row,
            render_order,
            width: 40,
            height: 40,
            x,
            y,
            age: 0,
            damage_range_flags: damage_range::GROUND,
            short_range: false,
            motion: ProjectileMotion::Straight,
            trail: None,
            vel_y: 0.0,
            fire: None,
            hit_torchwood_col: None,
            lobbed: false,
            vel_x: 0.0,
            vel_z: 0.0,
            acc_z: 0.0,
            rotation: 0.0,
            rotation_speed: 0.0,
            cob_target: None,
            landed: false,
            dead: false,
        }
    }

    /// The plant throws it in an arc that lands `range_x` along and `range_y`
    /// down in 120 ticks (`Plant::Fire`), reaching what `range_flags` allows.
    pub fn launch_lob(&mut self, range_x: f32, range_y: f32, range_flags: u32, rng: &mut MtRand) {
        self.lobbed = true;
        self.vel_x = range_x / 120.0;
        self.vel_z = range_y / 120.0 - 7.0;
        self.acc_z = 0.115;
        self.damage_range_flags = range_flags;
        (self.rotation, self.rotation_speed) = match self.ty {
            ProjectileType::Cabbage | ProjectileType::Butter => (-7.0 * std::f32::consts::PI / 25.0, rng.range_f32(-0.08, -0.02)),
            ProjectileType::Melon | ProjectileType::Wintermelon => (-2.0 * std::f32::consts::PI / 5.0, rng.range_f32(-0.08, -0.02)),
            ProjectileType::Kernel => (0.0, rng.range_f32(-0.2, -0.08)),
            _ => (0.0, 0.0),
        };
    }

    /// `Plant::Fire` for a cob cannon: the cob shoots straight up out of sight
    /// and comes back down at `target_x` in `target_row`, whose square is
    /// `land_y` down. `size` is the cob's picture (its hit box is a 230 px square around it).
    pub fn launch_cob(&mut self, target_x: f32, target_row: i32, land_y: f32, size: (i32, i32)) {
        self.lobbed = true;
        self.vel_x = 0.001;
        self.vel_y = 0.0;
        self.acc_z = 0.0;
        self.vel_z = -8.0;
        self.damage_range_flags = damage_range::EXPLOSION;
        self.cob_target = Some((target_x, target_row, land_y));
        (self.width, self.height) = size;
    }

    /// `Zombie::ZombieCatapultFire`: a catapult zombie's basketball, lobbed
    /// back towards the plants, spinning.
    pub fn launch_basketball(&mut self, shot: &Basketball, rng: &mut MtRand) {
        self.lobbed = true;
        self.vel_x = shot.vel_x;
        self.vel_z = shot.vel_z;
        self.acc_z = 0.115;
        self.rotation = rng.range_f32(0.0, 2.0 * std::f32::consts::PI);
        self.rotation_speed = rng.range_f32(0.05, 0.1);
    }

    /// The trail particles a snow pea or puff leaves (`ProjectileInitialize`),
    /// and where they sit on it.
    pub fn trail_effect(ty: ProjectileType) -> Option<(ParticleEffect, f32, f32)> {
        match ty {
            ProjectileType::Snowpea => Some((ParticleEffect::SnowpeaTrail, 8.0, 13.0)),
            ProjectileType::Puff => Some((ParticleEffect::PuffshroomTrail, 13.0, 13.0)),
            ProjectileType::Pea
            | ProjectileType::Fire
            | ProjectileType::Cabbage
            | ProjectileType::Kernel
            | ProjectileType::Butter
            | ProjectileType::Melon
            | ProjectileType::Wintermelon
            | ProjectileType::Cob
            | ProjectileType::Spike
            | ProjectileType::Star
            | ProjectileType::ZombiePea
            | ProjectileType::Basketball => None,
        }
    }

    /// `mDamageRangeFlags`: which zombies it can hit.
    pub fn set_damage_range(&mut self, flags: u32) {
        self.damage_range_flags = flags;
    }

    /// `Plant::Fire`: how the shot flies.
    pub fn set_motion(&mut self, motion: ProjectileMotion, rng: &mut MtRand) {
        self.motion = motion;
        match motion {
            ProjectileMotion::Star { vel_x, vel_y } => {
                self.vel_x = vel_x;
                self.vel_y = vel_y;
                self.shadow_y += 15.0;
                self.rotation_speed = rng.range_f32(0.05, 0.1) * if rng.next_range(2) == 0 { -1.0 } else { 1.0 };
            }
            ProjectileMotion::Homing(_) => {
                self.vel_x = 2.0;
                self.vel_y = 0.0;
            }
            ProjectileMotion::Straight | ProjectileMotion::Backwards => {}
        }
    }

    /// A threepeater's side pea: it starts in the plant's row and drifts up
    /// or down (`vel_y` is -3 or 3) into the row it's meant for.
    pub fn set_lane_drift(&mut self, vel_y: f32) {
        self.vel_y = vel_y;
        self.shadow_y += if vel_y < 0.0 { 80.0 } else { -80.0 };
    }

    /// `AttachParticle`: `trail` follows the projectile from now on.
    pub fn attach_trail(&mut self, trail: ParticleSystemId, dx: f32, dy: f32) {
        self.trail = Some((trail, dx, dy));
    }

    /// Where it is (`mPosX`, `mPosY`).
    #[cfg(test)]
    pub(super) fn position(&self) -> (f32, f32) {
        (self.pos_x, self.pos_y)
    }

    /// `Projectile::GetProjectileRect`.
    pub(super) fn rect(&self) -> IRect {
        match self.ty {
            ProjectileType::Pea | ProjectileType::Snowpea | ProjectileType::ZombiePea => IRect::new(self.x - 15, self.y, self.width + 15, self.height),
            ProjectileType::Spike | ProjectileType::Star => IRect::new(self.x, self.y, self.width, self.height),
            ProjectileType::Melon | ProjectileType::Wintermelon => IRect::new(self.x + 20, self.y, 60, self.height),
            ProjectileType::Fire => IRect::new(self.x, self.y, self.width - 10, self.height),
            ProjectileType::Cob => IRect::new(self.x + self.width / 2 - 115, self.y + self.height / 2 - 115, 230, 230),
            ProjectileType::Puff | ProjectileType::Cabbage | ProjectileType::Kernel | ProjectileType::Butter | ProjectileType::Basketball => {
                IRect::new(self.x, self.y, self.width, self.height)
            }
        }
    }

    /// `Projectile::Die`: the trail fades out where it is.
    fn die(&mut self, fx: &mut Fx) {
        self.dead = true;
        self.drop_attachments(fx);
    }

    /// Lets the trail fade out and puts the flames out.
    fn drop_attachments(&mut self, fx: &mut Fx) {
        if let Some((id, _, _)) = self.trail.take() {
            let rng = fx.sounds.rng();
            if let Some(trail) = fx.effects.particle_mut(id) {
                trail.cross_fade("FadeOut", rng);
            }
        }
        if let Some(fire) = self.fire.take() {
            fx.effects.kill(fire);
        }
    }

    /// `Projectile::ConvertToFireball`: a pea passing the torchwood in `col`
    /// catches fire.
    pub fn convert_to_fireball(&mut self, col: i32, fx: &mut Fx) {
        if self.hit_torchwood_col == Some(col) {
            return;
        }
        self.ty = ProjectileType::Fire;
        self.hit_torchwood_col = Some(col);
        fx.sounds.play_foley(FoleyType::FirePea);
        let rng = fx.sounds.rng();
        let rate = rng.range_f32(50.0, 80.0);
        if let Ok((id, flames)) = fx.effects.add_reanimation(self.pos_x - 25.0, self.pos_y - 25.0, ReanimationType::FirePea, fx.res, fx.images) {
            flames.loop_type = sexy::reanim::LoopType::Loop;
            flames.anim_rate = rate;
            flames.render_order = self.render_order;
            self.fire = Some(id);
        }
    }

    /// `Projectile::ConvertToPea`: a snow pea thaws at the torchwood in `col`.
    pub fn convert_to_pea(&mut self, col: i32, fx: &mut Fx) {
        if self.hit_torchwood_col == Some(col) {
            return;
        }
        self.drop_attachments(fx);
        self.ty = ProjectileType::Pea;
        self.hit_torchwood_col = Some(col);
        fx.sounds.play_foley(FoleyType::Throw);
    }

    /// `Projectile::Update`.
    pub fn update(&mut self, zombies: &mut DataArray<Zombie>, plants: &mut DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        self.age += 1;
        // Peas start under the plants' heads and come out on top after a tick.
        if self.age > 0 {
            self.render_order = make_render_order(render_layer::PROJECTILE, self.row, 0);
        }
        self.rotation += self.rotation_speed;
        self.update_motion(zombies, plants, env, fx);
        // AttachmentUpdateAndMove.
        if let Some((id, dx, dy)) = self.trail
            && let Some(trail) = fx.effects.particle_mut(id)
        {
            trail.system_move(self.pos_x + dx, self.pos_y + self.pos_z + dy);
        }
        if let Some(id) = self.fire
            && let Some(flames) = fx.effects.get_mut(id)
        {
            flames.set_position(self.pos_x - 25.0, self.pos_y + self.pos_z - 25.0);
            flames.render_order = self.render_order;
        }
    }

    /// `Projectile::UpdateMotion`: moves, then follows the roof's slope.
    fn update_motion(&mut self, zombies: &mut DataArray<Zombie>, plants: &mut DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        let old_row = self.row;
        let old_y = super::pos_y_based_on_row(env.background, self.pos_x, self.row);
        if self.lobbed {
            self.update_lob_motion(zombies, plants, env, fx);
        } else {
            self.update_normal_motion(zombies, plants, env, fx);
        }
        let slope = super::pos_y_based_on_row(env.background, self.pos_x, old_row) - old_y;
        if self.lobbed {
            self.pos_y += slope;
            self.pos_z -= slope;
        } else {
            // The original lets straight shots fly level and moves only their
            // shadow, which on the sloped roof looks as if they hit the wrong
            // row; here they climb the slope with the zombies.
            self.pos_y += slope;
        }
        self.shadow_y += slope;
        self.x = self.pos_x as i32;
        self.y = (self.pos_y + self.pos_z) as i32;
    }

    /// `Projectile::UpdateNormalMotion`.
    fn update_normal_motion(&mut self, zombies: &mut DataArray<Zombie>, plants: &mut DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        match self.motion {
            ProjectileMotion::Backwards => self.pos_x -= 3.33,
            ProjectileMotion::Homing(target) => {
                if let Some(zombie) = zombies.get(target).filter(|z| z.effected_by_damage(self.damage_range_flags)) {
                    let rect = zombie.zombie_rect();
                    let to_target = Vec2::new(zombie.target_lead_x(0.0), (rect.y + rect.h / 2) as f32)
                        - Vec2::new(self.pos_x + self.width as f32 / 2.0, self.pos_y + self.height as f32 / 2.0);
                    let steer = to_target.normalize_or_zero() * (0.001 * self.age as f32);
                    let motion = (Vec2::new(self.vel_x, self.vel_y) + steer).normalize_or_zero() * 2.0;
                    (self.vel_x, self.vel_y) = (motion.x, motion.y);
                    self.rotation = -self.vel_y.atan2(self.vel_x);
                }
                self.pos_y += self.vel_y;
                self.pos_x += self.vel_x;
                self.shadow_y += self.vel_y;
                self.row = super::row_at_pixel(env.background, self.pos_x as i32, self.pos_y as i32);
            }
            ProjectileMotion::Star { .. } => {
                self.pos_y += self.vel_y;
                self.pos_x += self.vel_x;
                self.shadow_y += self.vel_y;
                if self.vel_y != 0.0 {
                    self.row = super::row_at_pixel(env.background, self.pos_x as i32, self.pos_y as i32);
                }
            }
            ProjectileMotion::Straight => {
                self.pos_x += 3.33;
                if self.vel_y != 0.0 {
                    self.pos_y += self.vel_y;
                    self.vel_y *= 0.97;
                    self.shadow_y += self.vel_y;
                }
            }
        }
        self.check_for_collision(zombies, plants, env, fx);
    }

    /// `Projectile::UpdateLobMotion`: up and over, landing on the first zombie
    /// it comes down on, or the ground.
    fn update_lob_motion(&mut self, zombies: &mut DataArray<Zombie>, plants: &mut DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Projectile::UpdateLobMotion: a cob
        // that has left the screen reappears above its target.
        if let (ProjectileType::Cob, Some((x, row, y))) = (self.ty, self.cob_target)
            && self.pos_z < -700.0
        {
            self.vel_z = 8.0;
            self.row = row;
            self.pos_x = x;
            self.pos_y = y;
            self.shadow_y = y + 67.0;
            self.rotation = -std::f32::consts::FRAC_PI_2;
        }
        self.vel_z += self.acc_z;
        self.pos_x += self.vel_x;
        self.pos_z += self.vel_z;
        let rising = self.vel_z < 0.0;
        if self.age <= 20 || rising {
            return;
        }
        let mut min_collision_z = match self.ty {
            ProjectileType::Basketball => 60.0,
            ProjectileType::Butter => -32.0,
            ProjectileType::Melon | ProjectileType::Wintermelon => -35.0,
            ProjectileType::Cabbage | ProjectileType::Kernel => -30.0,
            ProjectileType::Cob => -60.0,
            _ => 0.0,
        };
        if env.pool_rows.get(self.row as usize).copied().unwrap_or(false) {
            min_collision_z += 40.0;
        }
        if self.pos_z <= min_collision_z {
            return;
        }
        if self.ty == ProjectileType::Basketball {
            let plant = self.find_collision_target_plant(plants);
            if plant.is_some() || self.pos_z > 80.0 {
                self.basketball_impact(plants, plant, zombies, env, fx);
            }
            return;
        }
        let target = self.find_collision_target(zombies);
        let hit_ground = self.pos_z > if self.ty == ProjectileType::Cob { -40.0 } else { 80.0 };
        if target.is_none() && !hit_ground {
            return;
        }
        if self.ty == ProjectileType::Cob {
            self.do_cob_impact(zombies, env, fx);
            return;
        }
        self.do_lob_impact(zombies, target, env, fx);
    }

    /// `Projectile::CheckForCollision`.
    fn check_for_collision(&mut self, zombies: &mut DataArray<Zombie>, plants: &mut DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        // Puffs only go so far.
        if self.short_range && self.age >= 75 {
            self.die(fx);
            return;
        }
        if self.pos_x > super::BOARD_WIDTH as f32 || self.pos_x + (self.width as f32) < 0.0 {
            self.die(fx);
            return;
        }
        // A homing spike hits its zombie when it reaches it.
        if let ProjectileMotion::Homing(target) = self.motion {
            if let Some(zombie) = zombies.get(target).filter(|z| z.effected_by_damage(self.damage_range_flags)) {
                let rect = zombie.zombie_rect();
                if rect_overlap(self.rect(), rect) >= 0 && self.pos_y > rect.y as f32 && self.pos_y < (rect.y + rect.h) as f32 {
                    self.do_impact(zombies, target, env, fx);
                }
            }
            return;
        }
        // Stars fall off the top and bottom of the lawn.
        if self.ty == ProjectileType::Star && (self.pos_y > 600.0 || self.pos_y < 40.0) {
            self.die(fx);
            return;
        }
        // A zombotany head's pea flies backwards into the plants (or, when
        // hypnotized, into the zombies like any pea).
        if self.ty == ProjectileType::ZombiePea && self.motion == ProjectileMotion::Backwards {
            if let Some(id) = self.find_collision_target_plant(plants) {
                self.zombie_pea_impact(plants, id, fx);
            }
            return;
        }
        // Peas lifted well off the ground go over everyone.
        if matches!(self.ty, ProjectileType::Pea | ProjectileType::Star) && self.shadow_y - self.pos_y > 90.0 {
            return;
        }
        if let Some(id) = self.find_collision_target(zombies) {
            self.do_impact(zombies, id, env, fx);
        }
    }

    /// `Projectile::FindCollisionTarget`: the leftmost zombie it's touching.
    fn find_collision_target(&self, zombies: &DataArray<Zombie>) -> Option<sexy::data_array::Id<Zombie>> {
        let rect = self.rect();
        let mut best: Option<(i32, sexy::data_array::Id<Zombie>)> = None;
        for (id, zombie) in zombies.iter() {
            if !zombie.in_row(self.row) || !zombie.effected_by_damage(self.damage_range_flags) {
                continue;
            }
            // A snorkel swimming under the water is only hit by shots coming down on it.
            if zombie.phase == super::zombie::ZombiePhase::SnorkelWalkingInPool && self.pos_z <= 45.0 {
                continue;
            }
            if rect_overlap(rect, zombie.zombie_rect()) >= 0 && best.is_none_or(|(x, _)| zombie.x < x) {
                best = Some((zombie.x, id));
            }
        }
        best.map(|(_, id)| id)
    }

    /// `Projectile::FindCollisionTargetPlant`: the top plant on a square
    /// it's coming down on.
    fn find_collision_target_plant(&self, plants: &DataArray<Plant>) -> Option<sexy::data_array::Id<Plant>> {
        let rect = self.rect();
        plants
            .iter()
            .find(|(_, p)| p.row == self.row && !p.dead && rect_overlap(rect, p.rect()) > 8)
            .and_then(|(_, p)| top_plant_any(plants, p.col, p.row))
    }

    /// `Projectile::CheckForCollision` for a basketball: it hurts the plant,
    /// unless an umbrella leaf next to it is up (or goes up) in time.
    fn basketball_impact(
        &mut self,
        plants: &mut DataArray<Plant>,
        plant: Option<sexy::data_array::Id<Plant>>,
        zombies: &mut DataArray<Zombie>,
        env: &ZombieEnv,
        fx: &mut Fx,
    ) {
        // Behaviour per ruslan831 decomp, Projectile::CheckForCollision.
        let Some((col, row)) = plant.and_then(|id| plants.get(id)).map(|p| (p.col, p.row)) else {
            self.do_lob_impact(zombies, None, env, fx);
            return;
        };
        if let Some(umbrella) = find_umbrella_plant(plants, col, row).and_then(|id| plants.get_mut(id)) {
            match umbrella.state() {
                PlantState::UmbrellaReflecting => {
                    fx.sounds.play_foley(FoleyType::Splat);
                    let rng = fx.sounds.rng();
                    let order = make_render_order(render_layer::TOP, 0, 1);
                    fx.effects.add_particle(self.pos_x + 20.0, self.pos_y + 20.0, order, ParticleEffect::UmbrellaReflect, fx.res, fx.images, rng);
                    self.die(fx);
                }
                PlantState::UmbrellaTriggered => {}
                _ => {
                    fx.sounds.play_foley(FoleyType::Umbrella);
                    umbrella.open_umbrella(fx.effects);
                }
            }
            return;
        }
        if let Some(plant) = plant.and_then(|id| plants.get_mut(id)) {
            plant.health -= self.ty.damage();
            plant.eaten_flash_countdown = plant.eaten_flash_countdown.max(25);
        }
        fx.sounds.play_foley(FoleyType::Splat);
        self.die(fx);
    }

    /// `Projectile::CheckForCollision` for a zombotany head's pea: it hurts
    /// the plant, unless an umbrella leaf next to it is up (or goes up).
    fn zombie_pea_impact(&mut self, plants: &mut DataArray<Plant>, plant: sexy::data_array::Id<Plant>, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Projectile::CheckForCollision.
        let Some((col, row)) = plants.get(plant).map(|p| (p.col, p.row)) else {
            return;
        };
        if let Some(umbrella) = find_umbrella_plant(plants, col, row).and_then(|id| plants.get_mut(id)) {
            match umbrella.state() {
                PlantState::UmbrellaReflecting => {
                    fx.sounds.play_foley(FoleyType::Splat);
                    let rng = fx.sounds.rng();
                    let order = make_render_order(render_layer::TOP, 0, 1);
                    fx.effects.add_particle(self.pos_x + 20.0, self.pos_y + 20.0, order, ParticleEffect::UmbrellaReflect, fx.res, fx.images, rng);
                    self.die(fx);
                }
                PlantState::UmbrellaTriggered => {}
                _ => {
                    fx.sounds.play_foley(FoleyType::Umbrella);
                    umbrella.open_umbrella(fx.effects);
                }
            }
            return;
        }
        if let Some(plant) = plants.get_mut(plant) {
            plant.health -= self.ty.damage();
            plant.eaten_flash_countdown = plant.eaten_flash_countdown.max(25);
        }
        fx.sounds.play_foley(FoleyType::Splat);
        let rng = fx.sounds.rng();
        fx.effects.add_particle(self.pos_x - 3.0, self.pos_y + 17.0, self.render_order + 1, ParticleEffect::PeaSplat, fx.res, fx.images, rng);
        self.die(fx);
    }

    /// `Projectile::PlayImpactSound`.
    fn play_impact_sound(&self, zombie: &Zombie, fx: &mut Fx) {
        let mut splat = true;
        match zombie.helm_type() {
            HelmType::Pail => {
                fx.sounds.play_foley(FoleyType::ShieldHit);
                splat = false;
            }
            HelmType::TrafficCone | HelmType::Football => fx.sounds.play_foley(FoleyType::PlasticHit),
            HelmType::Wallnut | HelmType::Tallnut | HelmType::Digger => {}
            HelmType::None | HelmType::Bobsled => {}
        }
        if splat {
            fx.sounds.play_foley(FoleyType::Splat);
        }
    }

    /// `Projectile::DoImpact`.
    fn do_impact(&mut self, zombies: &mut DataArray<Zombie>, id: sexy::data_array::Id<Zombie>, env: &ZombieEnv, fx: &mut Fx) {
        if self.ty == ProjectileType::Fire {
            self.do_fire_impact(zombies, id, env, fx);
            return;
        }
        let Some(zombie) = zombies.get_mut(id) else { return };
        self.play_impact_sound(zombie, fx);
        let mut flags = if self.ty == ProjectileType::Snowpea { damage::FREEZE } else { 0 };
        // Behaviour per ruslan831 decomp, Projectile::GetDamageFlags: shots that
        // come from behind, or fly left, get past a shield.
        if self.motion == ProjectileMotion::Backwards || (matches!(self.motion, ProjectileMotion::Star { .. }) && self.vel_x < 0.0) {
            flags |= damage::BYPASSES_SHIELD;
        }
        zombie.take_damage(self.ty.damage(), flags, env, fx);

        // The splat sits on the zombie and moves with it.
        let (splat_offset, effect) = match self.ty {
            ProjectileType::Pea | ProjectileType::ZombiePea => (-15.0, ParticleEffect::PeaSplat),
            ProjectileType::Snowpea => (-15.0, ParticleEffect::SnowpeaSplat),
            ProjectileType::Puff => (-20.0, ParticleEffect::PuffSplat),
            ProjectileType::Spike | ProjectileType::Star => {
                self.die(fx);
                return;
            }
            ProjectileType::Fire
            | ProjectileType::Cabbage
            | ProjectileType::Kernel
            | ProjectileType::Butter
            | ProjectileType::Melon
            | ProjectileType::Wintermelon
            | ProjectileType::Cob
            | ProjectileType::Basketball => return,
        };
        let (splat_x, splat_y) = (self.pos_x + 12.0 + splat_offset, self.pos_y + 12.0);
        let mut x = splat_x + 52.0 - zombie.x as f32;
        let y = (splat_y - zombie.y as f32).clamp(20.0, 100.0);
        if self.motion == ProjectileMotion::Backwards {
            x -= 80.0;
        } else if self.pos_x > (zombie.x + 40) as f32 {
            x -= 60.0;
        }
        zombie.add_attached_particle(x, y, effect, fx);
        self.die(fx);
    }

    /// Whether a melon's splash reaches `zombie` (`IsZombieHitBySplash`).
    fn is_hit_by_splash(&self, zombie: &Zombie) -> bool {
        (zombie.row - self.row).abs() <= 1
            && zombie.effected_by_damage(self.damage_range_flags)
            && rect_overlap(self.rect(), zombie.zombie_rect()) >= 0
    }

    /// `Projectile::DoImpact` for a lobbed shot, which may come down on a
    /// zombie or only on the ground.
    fn do_lob_impact(&mut self, zombies: &mut DataArray<Zombie>, target: Option<sexy::data_array::Id<Zombie>>, env: &ZombieEnv, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Projectile::DoImpact and DoSplashDamage.
        match self.ty {
            ProjectileType::Kernel => fx.sounds.play_foley(FoleyType::KernelSplat),
            ProjectileType::Butter => fx.sounds.play_foley(FoleyType::Butter),
            _ => {
                if let Some(zombie) = target.and_then(|id| zombies.get(id)) {
                    self.play_impact_sound(zombie, fx);
                } else {
                    fx.sounds.play_foley(FoleyType::Splat);
                }
            }
        }
        if matches!(self.ty, ProjectileType::Melon | ProjectileType::Wintermelon) {
            let full = self.ty.damage();
            let splashed: Vec<_> = zombies.iter().filter(|(id, z)| Some(*id) != target && self.is_hit_by_splash(z)).map(|(id, _)| id).collect();
            let mut splash = full / 3;
            let total = splash * splashed.len() as i32;
            let cap = full * 7;
            if total > cap {
                splash = (full * cap / (total * 3)).max(1);
            }
            // Behaviour per ruslan831 decomp, Projectile::GetDamageFlags: the winter melon chills.
            let chill = if self.ty == ProjectileType::Wintermelon { damage::FREEZE } else { 0 };
            let flags = damage::HITS_SHIELD_AND_BODY | chill;
            if let Some(zombie) = target.and_then(|id| zombies.get_mut(id)) {
                zombie.take_damage(full, flags, env, fx);
            }
            for id in splashed {
                if let Some(zombie) = zombies.get_mut(id) {
                    zombie.take_damage(splash, flags, env, fx);
                }
            }
        } else if let Some(zombie) = target.and_then(|id| zombies.get_mut(id)) {
            zombie.take_damage(self.ty.damage(), damage::BYPASSES_SHIELD, env, fx);
            if self.ty == ProjectileType::Butter {
                zombie.apply_butter(fx);
            }
        }

        let last_x = self.pos_x - self.vel_x;
        let last_y = self.pos_y + self.pos_z - self.vel_z;
        let order = self.render_order + 1;
        let (splat_x, splat_y, effect) = match self.ty {
            ProjectileType::Cabbage => (last_x - 38.0, last_y + 23.0, Some(ParticleEffect::CabbageSplat)),
            ProjectileType::Butter => (last_x - 20.0, last_y + 63.0, Some(ParticleEffect::ButterSplat)),
            _ => (self.pos_x + 12.0, self.pos_y + 12.0, None),
        };
        let splash_effect = match self.ty {
            ProjectileType::Melon => Some(ParticleEffect::Melonsplash),
            ProjectileType::Wintermelon => Some(ParticleEffect::Wintermelon),
            _ => None,
        };
        if let Some(splash_effect) = splash_effect {
            let rng = fx.sounds.rng();
            fx.effects.add_particle(last_x + 30.0, last_y + 30.0, order, splash_effect, fx.res, fx.images, rng);
        }
        if let Some(effect) = effect {
            if let Some(zombie) = target.and_then(|id| zombies.get_mut(id)) {
                let x = splat_x + 52.0 - zombie.x as f32;
                let mut y = splat_y - zombie.y as f32;
                if matches!(zombie.phase, super::zombie::ZombiePhase::SnorkelWalkingInPool | super::zombie::ZombiePhase::DolphinWalkingInPool) {
                    y += 60.0;
                }
                zombie.add_attached_particle(x, y.clamp(20.0, 100.0), effect, fx);
            } else {
                let rng = fx.sounds.rng();
                fx.effects.add_particle(splat_x, splat_y, order, effect, fx.res, fx.images, rng);
            }
        }
        self.die(fx);
    }

    /// `Projectile::DoImpact` for a cob: everything within 115 px burns, in
    /// its row and the ones beside it, and the lawn is left scorched.
    fn do_cob_impact(&mut self, zombies: &mut DataArray<Zombie>, env: &ZombieEnv, fx: &mut Fx) {
        let (cx, cy) = (self.pos_x as i32 + 80, self.pos_y as i32 + 40);
        for (_, zombie) in zombies.iter_mut() {
            let row_dist = if zombie.is_boss() { 0 } else { zombie.row - self.row };
            if zombie.effected_by_damage(self.damage_range_flags)
                && row_dist.abs() <= 1
                && super::circle_rect_overlap(cx, cy, 115, zombie.zombie_rect())
            {
                zombie.apply_burn(env, fx);
            }
        }
        let ground = make_render_order(render_layer::GROUND, self.row, 2);
        let order = self.render_order + 1;
        let rng = fx.sounds.rng();
        fx.effects.add_particle(cx as f32, cy as f32, ground, ParticleEffect::Blastmark, fx.res, fx.images, rng);
        let rng = fx.sounds.rng();
        fx.effects.add_particle(cx as f32, cy as f32, order, ParticleEffect::Popcornsplash, fx.res, fx.images, rng);
        fx.sounds.play_sample("SOUND_DOOMSHROOM");
        self.landed = true;
        self.die(fx);
    }

    /// `Projectile::DoImpact` and `DoSplashDamage` for a fire pea: the zombie
    /// it hits takes the full damage and thaws; the rest of the row within
    /// reach take a third each, shared out so the total stays within one hit.
    fn do_fire_impact(&mut self, zombies: &mut DataArray<Zombie>, hit: sexy::data_array::Id<Zombie>, env: &ZombieEnv, fx: &mut Fx) {
        fx.sounds.play_foley(FoleyType::Ignite);
        if let Some(zombie) = zombies.get_mut(hit) {
            zombie.remove_cold_effects(fx.effects);
        }
        let mut reach = self.rect();
        reach.w = 100;
        let splashed: Vec<_> = zombies
            .iter()
            .filter(|(zid, z)| {
                z.row == self.row && z.effected_by_damage(self.damage_range_flags) && rect_overlap(reach, z.zombie_rect()) >= 0 && *zid != hit
            })
            .map(|(zid, _)| zid)
            .collect();
        let full = self.ty.damage();
        let mut splash = full / 3;
        let total = splash * splashed.len() as i32;
        if total > full {
            splash = (full * full / (total * 3)).max(1);
        }
        let flags = damage::HITS_SHIELD_AND_BODY;
        if let Some(zombie) = zombies.get_mut(hit) {
            zombie.take_damage(full, flags, env, fx);
        }
        for zid in splashed {
            if let Some(zombie) = zombies.get_mut(zid) {
                zombie.take_damage(splash, flags, env, fx);
            }
        }
        // The flash of fire where it hit.
        let order = self.render_order + 1;
        if let Ok((_, flame)) =
            fx.effects.add_reanimation(self.pos_x + 38.0, self.pos_y - 20.0, ReanimationType::JalapenoFire, fx.res, fx.images)
        {
            flame.anim_time = 0.25;
            flame.anim_rate = 24.0;
            flame.override_scale(0.7, 0.4);
            flame.render_order = order;
        }
        self.die(fx);
    }

    /// `Projectile::Draw`: in the projectile's own coordinates.
    pub fn draw(&self, g: &mut Graphics, res: &GameResources) {
        if self.lobbed {
            self.draw_lobbed(g, res);
            return;
        }
        let id = match self.ty {
            ProjectileType::Pea | ProjectileType::ZombiePea => "IMAGE_PROJECTILEPEA",
            ProjectileType::Snowpea => "IMAGE_PROJECTILESNOWPEA",
            ProjectileType::Spike => "IMAGE_PROJECTILECACTUS",
            ProjectileType::Star => "IMAGE_PROJECTILE_STAR",
            ProjectileType::Puff => "IMAGE_PUFFSHROOM_PUFF1",
            // The flames are a reanimation of their own.
            ProjectileType::Fire => return,
            ProjectileType::Cabbage
            | ProjectileType::Kernel
            | ProjectileType::Butter
            | ProjectileType::Melon
            | ProjectileType::Wintermelon
            | ProjectileType::Cob
            | ProjectileType::Basketball => return,
        };
        let Some(image) = res.image(id) else { return };
        g.push_state();
        g.translate(self.x as f32, self.y as f32);
        if matches!(self.ty, ProjectileType::Star | ProjectileType::Spike) && self.rotation != 0.0 {
            // Stars spin; a homing spike points where it's going.
            let (w, h) = (image.width as f32, image.height as f32);
            let (dx, dy) = (self.pos_x - self.x as f32 + w * 0.5, self.pos_y + self.pos_z - self.y as f32 + h * 0.5);
            let transform = Affine2::from_angle_translation(self.rotation, Vec2::new(dx, dy)) * Affine2::from_translation(Vec2::new(-w * 0.5, -h * 0.5));
            g.draw_image_matrix(image, transform);
        } else if self.ty == ProjectileType::Puff {
            // Puffs grow as they leave the mushroom.
            let scale = animate_curve_float(0, 30, self.age, 0.3, 1.0, TodCurve::Linear);
            let (dx, dy) = (self.pos_x - self.x as f32, self.pos_y + self.pos_z - self.y as f32);
            g.draw_image_cel_center_scaled(image, dx, dy, 0, scale, scale);
        } else {
            g.draw_image(image, 0.0, 0.0);
        }
        g.pop_state();
    }

    /// `Projectile::Draw` for the lobbed shots: spinning as they fly.
    fn draw_lobbed(&self, g: &mut Graphics, res: &GameResources) {
        let (id, scale) = match self.ty {
            ProjectileType::Cabbage => ("IMAGE_REANIM_CABBAGEPULT_CABBAGE", 1.0),
            ProjectileType::Kernel => ("IMAGE_REANIM_CORNPULT_KERNAL", 0.95),
            ProjectileType::Butter => ("IMAGE_REANIM_CORNPULT_BUTTER", 0.8),
            ProjectileType::Basketball => ("IMAGE_REANIM_ZOMBIE_CATAPULT_BASKETBALL", 1.1),
            ProjectileType::Cob => ("IMAGE_REANIM_COBCANNON_COB", 0.9),
            ProjectileType::Wintermelon => ("IMAGE_REANIM_WINTERMELON_PROJECTILE", 1.0),
            _ => ("IMAGE_REANIM_MELONPULT_MELON", 1.0),
        };
        let Some(image) = res.image(id) else { return };
        let (w, h) = (image.cel_width() as f32, image.cel_height() as f32);
        let center = Vec2::new(self.pos_x + w * 0.5, self.pos_y + self.pos_z + h * 0.5);
        // The caller translated to (x, y); the shot is placed by its own centre.
        let t = g.translation();
        let transform = Affine2::from_scale_angle_translation(Vec2::splat(scale), self.rotation, center - t)
            * Affine2::from_translation(Vec2::new(-w * 0.5, -h * 0.5));
        g.draw_image_matrix(image, transform);
    }

    /// `Projectile::DrawShadow`.
    pub fn draw_shadow(&self, g: &mut Graphics, night: bool, res: &GameResources) {
        if self.ty == ProjectileType::Puff {
            return;
        }
        let Some(image) = res.image("IMAGE_PEA_SHADOWS") else { return };
        let (offset_x, offset_y) = (self.pos_x - self.x as f32 + 3.0, self.pos_y - self.y as f32);
        g.push_state();
        g.translate(self.x as f32, self.y as f32);
        let y = self.shadow_y - self.pos_y + offset_y;
        if self.lobbed {
            // The shadow shrinks as the shot climbs (the original's 1.6 scale, less with height).
            let height = (-self.pos_z).clamp(0.0, 200.0);
            let scale = 1.6 * 200.0 / (height + 200.0);
            g.draw_image_cel_center_scaled(image, offset_x, y + 10.0, night as u32, scale, scale);
        } else if self.ty == ProjectileType::Snowpea {
            g.draw_image_cel_center_scaled(image, offset_x - 4.0, y, night as u32, 1.3, 1.3);
        } else {
            g.draw_image_cel(image, offset_x, y, night as u32, 0);
        }
        g.pop_state();
    }
}
