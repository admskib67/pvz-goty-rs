//! Particle effects: which file each `ParticleEffect` comes from, and
//! drawing running particle systems through [`Graphics`].
//!
//! The simulation lives in `sexy::particle`. Behaviour per ruslan831
//! decomp, `Sexy.TodLib/TodParticle.cpp` (`gLawnParticleArray`,
//! `RenderParticle`).

use sexy::particle::ParticleSystem;
use sexy::tod::float_round_to_int;

use crate::graphics::{Graphics, SexyColor, SexyRect, scale_rotate_transform};
use crate::resources::GameResources;

/// `ParticleEffect`, in the original's order, with files from `gLawnParticleArray`.
#[allow(dead_code, reason = "the full table; the game uses more of it as it's ported")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParticleEffect {
    Melonsplash,
    Wintermelon,
    Fumecloud,
    Popcornsplash,
    Powie,
    Jackexplode,
    ZombieHead,
    ZombieArm,
    ZombieTrafficCone,
    ZombiePail,
    ZombieHelmet,
    ZombieFlag,
    ZombieDoor,
    ZombieNewspaper,
    ZombieHeadlight,
    Pow,
    ZombiePogo,
    ZombieNewspaperHead,
    ZombieBalloonHead,
    SodRoll,
    GraveStoneRise,
    Planting,
    PlantingPool,
    ZombieRise,
    GraveBuster,
    GraveBusterDie,
    PoolSplash,
    IceSparkle,
    SeedPacket,
    TallNutBlock,
    Doom,
    DiggerRise,
    DiggerTunnel,
    DancerRise,
    PoolSparkly,
    WallnutEatSmall,
    WallnutEatLarge,
    PeaSplat,
    ButterSplat,
    CabbageSplat,
    PuffSplat,
    StarSplat,
    IceTrap,
    SnowpeaSplat,
    SnowpeaPuff,
    SnowpeaTrail,
    LanternShine,
    SeedPacketPickup,
    PotatoMine,
    PotatoMineRise,
    PuffshroomTrail,
    PuffshroomMuzzle,
    SeedPacketFlash,
    WhackAZombieRise,
    ZombieLadder,
    UmbrellaReflect,
    SeedPacketPick,
    IceTrapZombie,
    IceTrapRelease,
    ZamboniSmoke,
    Gloomcloud,
    ZombiePogoHead,
    ZamboniTire,
    ZamboniExplosion,
    ZamboniExplosion2,
    CatapultExplosion,
    MowerCloud,
    BossIceBall,
    Blastmark,
    CoinPickupArrow,
    PresentPickup,
    ImitaterMorph,
    MoweredZombieHead,
    MoweredZombieArm,
    ZombieHeadPool,
    ZombieBossFireball,
    FireballDeath,
    IceballDeath,
    IceballTrail,
    FireballTrail,
    BossExplosion,
    ScreenFlash,
    TrophySparkle,
    PortalCircle,
    PortalSquare,
    PottedPlantGlow,
    PottedWaterPlantGlow,
    PottedZenGlow,
    MindControl,
    VaseShatter,
    VaseShatterLeaf,
    VaseShatterZombie,
    AwardPickupArrow,
    ZombieSeaweed,
    ZombieMustache,
    ZombieSunglass,
    ZombiePinata,
    DustSquash,
    DustFoot,
    ZombieDaisies,
    CreditStrobe,
    CreditsRayswipe,
    CreditsZombieheadwipe,
    Starburst,
    CreditsFog,
    PersentPickUpArrow,
}

impl ParticleEffect {
    pub fn file(self) -> &'static str {
        match self {
            Self::Melonsplash => "particles/MelonImpact.xml",
            Self::Wintermelon => "particles/WinterMelonImpact.xml",
            Self::Fumecloud => "particles/FumeCloud.xml",
            Self::Popcornsplash => "particles/PopcornSplash.xml",
            Self::Powie => "particles/Powie.xml",
            Self::Jackexplode => "particles/JackExplode.xml",
            Self::ZombieHead => "particles/ZombieHead.xml",
            Self::ZombieArm => "particles/ZombieArm.xml",
            Self::ZombieTrafficCone => "particles/ZombieTrafficCone.xml",
            Self::ZombiePail => "particles/ZombiePail.xml",
            Self::ZombieHelmet => "particles/ZombieHelmet.xml",
            Self::ZombieFlag => "particles/ZombieFlag.xml",
            Self::ZombieDoor => "particles/ZombieDoor.xml",
            Self::ZombieNewspaper => "particles/ZombieNewspaper.xml",
            Self::ZombieHeadlight => "particles/ZombieHeadLight.xml",
            Self::Pow => "particles/Pow.xml",
            Self::ZombiePogo => "particles/ZombiePogo.xml",
            Self::ZombieNewspaperHead => "particles/ZombieNewspaperHead.xml",
            Self::ZombieBalloonHead => "particles/ZombieBalloonHead.xml",
            Self::SodRoll => "particles/SodRoll.xml",
            Self::GraveStoneRise => "particles/GraveStoneRise.xml",
            Self::Planting => "particles/Planting.xml",
            Self::PlantingPool => "particles/PlantingPool.xml",
            Self::ZombieRise => "particles/ZombieRise.xml",
            Self::GraveBuster => "particles/GraveBuster.xml",
            Self::GraveBusterDie => "particles/GraveBusterDie.xml",
            Self::PoolSplash => "particles/PoolSplash.xml",
            Self::IceSparkle => "particles/IceSparkle.xml",
            Self::SeedPacket => "particles/SeedPacket.xml",
            Self::TallNutBlock => "particles/TallNutBlock.xml",
            Self::Doom => "particles/Doom.xml",
            Self::DiggerRise => "particles/DiggerRise.xml",
            Self::DiggerTunnel => "particles/DiggerTunnel.xml",
            Self::DancerRise => "particles/DancerRise.xml",
            Self::PoolSparkly => "particles/PoolSparkly.xml",
            Self::WallnutEatSmall => "particles/WallnutEatSmall.xml",
            Self::WallnutEatLarge => "particles/WallnutEatLarge.xml",
            Self::PeaSplat => "particles/PeaSplat.xml",
            Self::ButterSplat => "particles/ButterSplat.xml",
            Self::CabbageSplat => "particles/CabbageSplat.xml",
            Self::PuffSplat => "particles/PuffSplat.xml",
            Self::StarSplat => "particles/StarSplat.xml",
            Self::IceTrap => "particles/IceTrap.xml",
            Self::SnowpeaSplat => "particles/SnowPeaSplat.xml",
            Self::SnowpeaPuff => "particles/SnowPeaPuff.xml",
            Self::SnowpeaTrail => "particles/SnowPeaTrail.xml",
            Self::LanternShine => "particles/LanternShine.xml",
            Self::SeedPacketPickup => "particles/Award.xml",
            Self::PotatoMine => "particles/PotatoMine.xml",
            Self::PotatoMineRise => "particles/PotatoMineRise.xml",
            Self::PuffshroomTrail => "particles/PuffShroomTrail.xml",
            Self::PuffshroomMuzzle => "particles/PuffShroomMuzzle.xml",
            Self::SeedPacketFlash => "particles/SeedPacketFlash.xml",
            Self::WhackAZombieRise => "particles/WhackAZombieRise.xml",
            Self::ZombieLadder => "particles/ZombieLadder.xml",
            Self::UmbrellaReflect => "particles/UmbrellaReflect.xml",
            Self::SeedPacketPick => "particles/SeedPacketPick.xml",
            Self::IceTrapZombie => "particles/IceTrapZombie.xml",
            Self::IceTrapRelease => "particles/IceTrapRelease.xml",
            Self::ZamboniSmoke => "particles/ZamboniSmoke.xml",
            Self::Gloomcloud => "particles/GloomCloud.xml",
            Self::ZombiePogoHead => "particles/ZombiePogoHead.xml",
            Self::ZamboniTire => "particles/ZamboniTire.xml",
            Self::ZamboniExplosion => "particles/ZamboniExplosion.xml",
            Self::ZamboniExplosion2 => "particles/ZamboniExplosion2.xml",
            Self::CatapultExplosion => "particles/CatapultExplosion.xml",
            Self::MowerCloud => "particles/MowerCloud.xml",
            Self::BossIceBall => "particles/BossIceBallTrail.xml",
            Self::Blastmark => "particles/BlastMark.xml",
            Self::CoinPickupArrow => "particles/CoinPickupArrow.xml",
            Self::PresentPickup => "particles/PresentPickup.xml",
            Self::ImitaterMorph => "particles/ImitaterMorph.xml",
            Self::MoweredZombieHead => "particles/MoweredZombieHead.xml",
            Self::MoweredZombieArm => "particles/MoweredZombieArm.xml",
            Self::ZombieHeadPool => "particles/ZombieHeadPool.xml",
            Self::ZombieBossFireball => "particles/Zombie_boss_fireball.xml",
            Self::FireballDeath => "particles/FireballDeath.xml",
            Self::IceballDeath => "particles/IceballDeath.xml",
            Self::IceballTrail => "particles/Iceball_Trail.xml",
            Self::FireballTrail => "particles/Fireball_Trail.xml",
            Self::BossExplosion => "particles/BossExplosion.xml",
            Self::ScreenFlash => "particles/ScreenFlash.xml",
            Self::TrophySparkle => "particles/TrophySparkle.xml",
            Self::PortalCircle => "particles/PortalCircle.xml",
            Self::PortalSquare => "particles/PortalSquare.xml",
            Self::PottedPlantGlow => "particles/PottedPlantGlow.xml",
            Self::PottedWaterPlantGlow => "particles/PottedWaterPlantGlow.xml",
            Self::PottedZenGlow => "particles/PottedZenGlow.xml",
            Self::MindControl => "particles/MindControl.xml",
            Self::VaseShatter => "particles/VaseShatter.xml",
            Self::VaseShatterLeaf => "particles/VaseShatterLeaf.xml",
            Self::VaseShatterZombie => "particles/VaseShatterZombie.xml",
            Self::AwardPickupArrow => "particles/AwardPickupArrow.xml",
            Self::ZombieSeaweed => "particles/Zombie_seaweed.xml",
            Self::ZombieMustache => "particles/ZombieMustache.xml",
            Self::ZombieSunglass => "particles/ZombieFutureGlasses.xml",
            Self::ZombiePinata => "particles/Pinata.xml",
            Self::DustSquash => "particles/Dust_Squash.xml",
            Self::DustFoot => "particles/Dust_Foot.xml",
            Self::ZombieDaisies => "particles/Daisy.xml",
            Self::CreditStrobe => "particles/Credits_Strobe.xml",
            Self::CreditsRayswipe => "particles/Credits_RaysWipe.xml",
            Self::CreditsZombieheadwipe => "particles/Credits_ZombieHeadWipe.xml",
            Self::Starburst => "particles/Starburst.xml",
            Self::CreditsFog => "particles/Credits_fog.xml",
            Self::PersentPickUpArrow => "particles/UpsellArrow.xml",
        }
    }
}

/// `TodParticleSystem::Draw`: every particle as a transformed quad.
/// TODO: additive particles (`PARTICLE_ADDITIVE`) draw normally for now.
pub fn draw_particle_system(g: &mut Graphics, system: &ParticleSystem, resources: &GameResources) {
    if system.dead {
        return;
    }
    for p in system.draw_list() {
        let [r, gr, b, a] = p.color;
        let color = SexyColor::rgba(r, gr, b, a);
        if p.fullscreen {
            g.push_state();
            g.set_color(color);
            g.fill_rect(-10000.0, -10000.0, 20000.0, 20000.0);
            g.pop_state();
            continue;
        }
        let Some(image) = resources.image(&p.image) else { continue };
        // RenderParticle.
        let (cel_w, cel_h) = (image.cel_width() as f32, image.cel_height() as f32);
        let col = p.col.min(image.cols as i32 - 1).max(0);
        let row = p.row.min(image.rows as i32 - 1).max(0);
        let [top, bottom, left, right] = p.clip;
        let mut x = p.x + left * cel_w;
        let mut y = p.y + top * cel_h;
        let src = SexyRect::new(
            col as f32 * cel_w + float_round_to_int(left * cel_w) as f32,
            row as f32 * cel_h + float_round_to_int(top * cel_h) as f32,
            cel_w - float_round_to_int(cel_w * (left + right)) as f32,
            cel_h - float_round_to_int(cel_h * (top + bottom)) as f32,
        );
        if p.align_to_pixels {
            x = float_round_to_int(x) as f32;
            y = float_round_to_int(y) as f32;
        }
        let transform = scale_rotate_transform(x, y, p.spin, p.scale, p.stretch * p.scale);
        g.push_state();
        g.set_colorize_images(true);
        g.set_color(color);
        g.draw_image_matrix_src(image, transform, src);
        g.pop_state();
    }
}
