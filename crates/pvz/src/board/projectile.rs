//! Things plants shoot (`Projectile`). So far the pea and the snow pea.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Projectile.cpp`:
//! `ProjectileInitialize`, `Update`, `UpdateMotion`, `UpdateNormalMotion`,
//! `CheckForCollision`, `FindCollisionTarget`, `PlayImpactSound`,
//! `DoImpact`, `Draw`, `DrawShadow`, `Die` and `GetProjectileRect`.

use sexy::data_array::DataArray;

use crate::audio::FoleyType;
use crate::graphics::Graphics;
use crate::particle::ParticleEffect;
use crate::resources::GameResources;

use super::zombie::{Fx, HelmType, IRect, Zombie, ZombieEnv, damage, damage_range, rect_overlap};
use super::{make_render_order, render_layer};

/// `ProjectileType`, for what's ported so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectileType {
    Pea,
    Snowpea,
}

impl ProjectileType {
    /// `ProjectileDefinition::mDamage`.
    fn damage(self) -> i32 {
        match self {
            Self::Pea | Self::Snowpea => 20,
        }
    }
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
            dead: false,
        }
    }

    /// `Projectile::GetProjectileRect`.
    fn rect(&self) -> IRect {
        match self.ty {
            ProjectileType::Pea | ProjectileType::Snowpea => IRect::new(self.x - 15, self.y, self.width + 15, self.height),
        }
    }

    /// `Projectile::Update`.
    pub fn update(&mut self, zombies: &mut DataArray<Zombie>, env: &ZombieEnv, fx: &mut Fx) {
        self.age += 1;
        // Peas start under the plants' heads and come out on top after a tick.
        if self.age > 0 {
            self.render_order = make_render_order(render_layer::PROJECTILE, self.row, 0);
        }
        self.update_motion(zombies, env, fx);
    }

    /// `Projectile::UpdateMotion` + `UpdateNormalMotion`.
    fn update_motion(&mut self, zombies: &mut DataArray<Zombie>, env: &ZombieEnv, fx: &mut Fx) {
        self.pos_x += 3.33;
        self.check_for_collision(zombies, env, fx);
        self.x = self.pos_x as i32;
        self.y = (self.pos_y + self.pos_z) as i32;
    }

    /// `Projectile::CheckForCollision`.
    fn check_for_collision(&mut self, zombies: &mut DataArray<Zombie>, env: &ZombieEnv, fx: &mut Fx) {
        if self.pos_x > super::BOARD_WIDTH as f32 || self.pos_x + (self.width as f32) < 0.0 {
            self.dead = true;
            return;
        }
        // Peas lifted well off the ground go over everyone.
        if self.shadow_y - self.pos_y > 90.0 {
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
            if zombie.row != self.row || !zombie.effected_by_damage(self.damage_range_flags) {
                continue;
            }
            if rect_overlap(rect, zombie.zombie_rect()) >= 0 && best.is_none_or(|(x, _)| zombie.x < x) {
                best = Some((zombie.x, id));
            }
        }
        best.map(|(_, id)| id)
    }

    /// `Projectile::PlayImpactSound`.
    fn play_impact_sound(&self, zombie: &Zombie, fx: &mut Fx) {
        let mut splat = true;
        match zombie.helm_type() {
            HelmType::Pail => {
                fx.sounds.play_foley(FoleyType::ShieldHit);
                splat = false;
            }
            HelmType::TrafficCone => fx.sounds.play_foley(FoleyType::PlasticHit),
            HelmType::None => {}
        }
        if splat {
            fx.sounds.play_foley(FoleyType::Splat);
        }
    }

    /// `Projectile::DoImpact`.
    fn do_impact(&mut self, zombies: &mut DataArray<Zombie>, id: sexy::data_array::Id<Zombie>, env: &ZombieEnv, fx: &mut Fx) {
        let Some(zombie) = zombies.get_mut(id) else { return };
        self.play_impact_sound(zombie, fx);
        let flags = if self.ty == ProjectileType::Snowpea { damage::FREEZE } else { 0 };
        zombie.take_damage(self.ty.damage(), flags, env, fx);

        // The splat sits on the zombie and moves with it.
        let (splat_x, splat_y) = (self.pos_x + 12.0 - 15.0, self.pos_y + 12.0);
        let mut x = splat_x + 52.0 - zombie.x as f32;
        let y = (splat_y - zombie.y as f32).clamp(20.0, 100.0);
        if self.pos_x > (zombie.x + 40) as f32 {
            x -= 60.0;
        }
        let effect = if self.ty == ProjectileType::Snowpea { ParticleEffect::SnowpeaSplat } else { ParticleEffect::PeaSplat };
        zombie.add_attached_particle(x, y, effect, fx);
        self.dead = true;
    }

    /// `Projectile::Draw`: in the projectile's own coordinates.
    pub fn draw(&self, g: &mut Graphics, res: &GameResources) {
        let id = if self.ty == ProjectileType::Snowpea { "IMAGE_PROJECTILESNOWPEA" } else { "IMAGE_PROJECTILEPEA" };
        let Some(image) = res.image(id) else { return };
        g.push_state();
        g.translate(self.x as f32, self.y as f32);
        g.draw_image(image, 0.0, 0.0);
        g.pop_state();
    }

    /// `Projectile::DrawShadow`.
    pub fn draw_shadow(&self, g: &mut Graphics, night: bool, res: &GameResources) {
        let Some(image) = res.image("IMAGE_PEA_SHADOWS") else { return };
        let (offset_x, offset_y) = (self.pos_x - self.x as f32 + 3.0, self.pos_y - self.y as f32);
        g.push_state();
        g.translate(self.x as f32, self.y as f32);
        let y = self.shadow_y - self.pos_y + offset_y;
        if self.ty == ProjectileType::Snowpea {
            g.draw_image_cel_center_scaled(image, offset_x - 4.0, y, night as u32, 1.3, 1.3);
        } else {
            g.draw_image_cel(image, offset_x, y, night as u32, 0);
        }
        g.pop_state();
    }
}
