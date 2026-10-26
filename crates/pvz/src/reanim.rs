//! Playing and drawing reanimations, plus the effect system that owns them.
//!
//! The animation model lives in `sexy::reanim`; this module maps the game's
//! reanimation types to files and draws tracks through [`Graphics`].
//! Behaviour per ruslan831 decomp, `Reanimation::DrawTrack`/`DrawRenderGroup`,
//! `ReanimatorEnsureDefinitionLoaded` and `LawnApp::AddReanimation`.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::math::Affine2;
use bevy::prelude::*;
use sexy::data_array::{DataArray, Id};
use sexy::particle::ParticleSystem;
use sexy::rand::MtRand;
use sexy::matrix::Matrix3;
use sexy::reanim::{RENDER_GROUP_HIDDEN, RENDER_GROUP_NORMAL, ReanimDefinition};
use sexy::tod::float_round_to_int;

use crate::particle::ParticleEffect;
use crate::graphics::{Graphics, SexyColor, SexyRect};
use crate::resources::GameResources;

pub type Reanimation = sexy::reanim::Reanimation<Arc<ReanimDefinition>>;
pub type ReanimationId = Id<Reanimation>;

/// Reanimation types (`ReanimationType`), in the original's order, with
/// their files from `gLawnReanimationArray`.
#[allow(dead_code, reason = "the full table; screens use more of it as they're ported")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReanimationType {
    LoadbarSprout,
    LoadbarZombieHead,
    SodRoll,
    FinalWave,
    Peashooter,
    Wallnut,
    Lilypad,
    Sunflower,
    LawnMower,
    ReadySetPlant,
    Cherrybomb,
    Squash,
    Doomshroom,
    Snowpea,
    Repeater,
    Sunshroom,
    Tallnut,
    Fumeshroom,
    Puffshroom,
    Hypnoshroom,
    Chomper,
    Zombie,
    Sun,
    Potatomine,
    Spikeweed,
    Spikerock,
    Threepeater,
    Marigold,
    Iceshroom,
    ZombieFootball,
    ZombieNewspaper,
    ZombieZamboni,
    Splash,
    Jalapeno,
    JalapenoFire,
    CoinSilver,
    ZombieCharred,
    ZombieCharredImp,
    ZombieCharredDigger,
    ZombieCharredZamboni,
    ZombieCharredCatapult,
    ZombieCharredGargantuar,
    Scrareyshroom,
    Pumpkin,
    Plantern,
    Torchwood,
    Splitpea,
    Seashroom,
    Blover,
    FlowerPot,
    Cactus,
    Dancer,
    Tanglekelp,
    Starfruit,
    Polevaulter,
    Balloon,
    Gargantuar,
    Imp,
    Digger,
    DiggerDirt,
    ZombieDolphinrider,
    Pogo,
    BackupDancer,
    Bobsled,
    Jackinthebox,
    Snorkel,
    Bungee,
    Catapult,
    Ladder,
    Puff,
    Sleeping,
    GraveBuster,
    ZombiesWon,
    Magnetshroom,
    Boss,
    Cabbagepult,
    Kernelpult,
    Melonpult,
    Coffeebean,
    Umbrellaleaf,
    Gatlingpea,
    Cattail,
    Gloomshroom,
    BossIceball,
    BossFireball,
    Cobcannon,
    Garlic,
    GoldMagnet,
    WinterMelon,
    TwinSunflower,
    PoolCleaner,
    RoofCleaner,
    FirePea,
    Imitater,
    Yeti,
    BossDriver,
    LawnMoweredZombie,
    CrazyDave,
    TextFadeOn,
    Hammer,
    SlotMachineHandle,
    CreditsFootball,
    CreditsJackbox,
    SelectorScreen,
    PortalCircle,
    PortalSquare,
    ZengardenSprout,
    ZengardenWateringcan,
    ZengardenFertilizer,
    ZengardenBugspray,
    ZengardenPhonograph,
    Diamond,
    ZombieHand,
    Stinky,
    Rake,
    RainCircle,
    RainSplash,
    ZombieSurprise,
    CoinGold,
    Treeofwisdom,
    TreeofwisdomClouds,
    TreeofwisdomTreefood,
    CreditsMain,
    CreditsMain2,
    CreditsMain3,
    ZombieCreditsDance,
    CreditsStage,
    CreditsBigbrain,
    CreditsFlowerPetals,
    CreditsInfantry,
    CreditsThroat,
    CreditsCrazydave,
    CreditsBossdance,
    ZombieCreditsScreenDoor,
    ZombieCreditsConehead,
    CreditsZombiearmy1,
    CreditsZombiearmy2,
    CreditsTombstones,
    CreditsSolarpower,
    CreditsAnyhour,
    CreditsWearetheundead,
    CreditsDiscolights,
    Flag,
}

impl ReanimationType {
    /// File from `gLawnReanimationArray`.
    pub fn file(self) -> &'static str {
        match self {
            Self::LoadbarSprout => "reanim/LoadBar_sprout.reanim",
            Self::LoadbarZombieHead => "reanim/LoadBar_Zombiehead.reanim",
            Self::SodRoll => "reanim/SodRoll.reanim",
            Self::FinalWave => "reanim/FinalWave.reanim",
            Self::Peashooter => "reanim/PeaShooterSingle.reanim",
            Self::Wallnut => "reanim/Wallnut.reanim",
            Self::Lilypad => "reanim/Lilypad.reanim",
            Self::Sunflower => "reanim/SunFlower.reanim",
            Self::LawnMower => "reanim/LawnMower.reanim",
            Self::ReadySetPlant => "reanim/StartReadySetPlant.reanim",
            Self::Cherrybomb => "reanim/CherryBomb.reanim",
            Self::Squash => "reanim/Squash.reanim",
            Self::Doomshroom => "reanim/DoomShroom.reanim",
            Self::Snowpea => "reanim/SnowPea.reanim",
            Self::Repeater => "reanim/PeaShooter.reanim",
            Self::Sunshroom => "reanim/SunShroom.reanim",
            Self::Tallnut => "reanim/Tallnut.reanim",
            Self::Fumeshroom => "reanim/Fumeshroom.reanim",
            Self::Puffshroom => "reanim/Puffshroom.reanim",
            Self::Hypnoshroom => "reanim/Hypnoshroom.reanim",
            Self::Chomper => "reanim/Chomper.reanim",
            Self::Zombie => "reanim/Zombie.reanim",
            Self::Sun => "reanim/Sun.reanim",
            Self::Potatomine => "reanim/PotatoMine.reanim",
            Self::Spikeweed => "reanim/Caltrop.reanim",
            Self::Spikerock => "reanim/SpikeRock.reanim",
            Self::Threepeater => "reanim/ThreePeater.reanim",
            Self::Marigold => "reanim/Marigold.reanim",
            Self::Iceshroom => "reanim/IceShroom.reanim",
            Self::ZombieFootball => "reanim/Zombie_football.reanim",
            Self::ZombieNewspaper => "reanim/Zombie_paper.reanim",
            Self::ZombieZamboni => "reanim/Zombie_zamboni.reanim",
            Self::Splash => "reanim/splash.reanim",
            Self::Jalapeno => "reanim/Jalapeno.reanim",
            Self::JalapenoFire => "reanim/fire.reanim",
            Self::CoinSilver => "reanim/Coin_silver.reanim",
            Self::ZombieCharred => "reanim/Zombie_charred.reanim",
            Self::ZombieCharredImp => "reanim/Zombie_charred_imp.reanim",
            Self::ZombieCharredDigger => "reanim/Zombie_charred_digger.reanim",
            Self::ZombieCharredZamboni => "reanim/Zombie_charred_zamboni.reanim",
            Self::ZombieCharredCatapult => "reanim/Zombie_charred_catapult.reanim",
            Self::ZombieCharredGargantuar => "reanim/Zombie_charred_gargantuar.reanim",
            Self::Scrareyshroom => "reanim/ScaredyShroom.reanim",
            Self::Pumpkin => "reanim/Pumpkin.reanim",
            Self::Plantern => "reanim/Plantern.reanim",
            Self::Torchwood => "reanim/Torchwood.reanim",
            Self::Splitpea => "reanim/SplitPea.reanim",
            Self::Seashroom => "reanim/SeaShroom.reanim",
            Self::Blover => "reanim/Blover.reanim",
            Self::FlowerPot => "reanim/Pot.reanim",
            Self::Cactus => "reanim/Cactus.reanim",
            Self::Dancer => "reanim/Zombie_Jackson.reanim",
            Self::Tanglekelp => "reanim/Tanglekelp.reanim",
            Self::Starfruit => "reanim/Starfruit.reanim",
            Self::Polevaulter => "reanim/Zombie_polevaulter.reanim",
            Self::Balloon => "reanim/Zombie_balloon.reanim",
            Self::Gargantuar => "reanim/Zombie_gargantuar.reanim",
            Self::Imp => "reanim/Zombie_imp.reanim",
            Self::Digger => "reanim/Zombie_digger.reanim",
            Self::DiggerDirt => "reanim/Digger_rising_dirt.reanim",
            Self::ZombieDolphinrider => "reanim/Zombie_dolphinrider.reanim",
            Self::Pogo => "reanim/Zombie_pogo.reanim",
            Self::BackupDancer => "reanim/Zombie_dancer.reanim",
            Self::Bobsled => "reanim/Zombie_bobsled.reanim",
            Self::Jackinthebox => "reanim/Zombie_jackbox.reanim",
            Self::Snorkel => "reanim/Zombie_snorkle.reanim",
            Self::Bungee => "reanim/Zombie_bungi.reanim",
            Self::Catapult => "reanim/Zombie_catapult.reanim",
            Self::Ladder => "reanim/Zombie_ladder.reanim",
            Self::Puff => "reanim/Puff.reanim",
            Self::Sleeping => "reanim/Z.reanim",
            Self::GraveBuster => "reanim/Gravebuster.reanim",
            Self::ZombiesWon => "reanim/ZombiesWon.reanim",
            Self::Magnetshroom => "reanim/Magnetshroom.reanim",
            Self::Boss => "reanim/Zombie_boss.reanim",
            Self::Cabbagepult => "reanim/Cabbagepult.reanim",
            Self::Kernelpult => "reanim/Cornpult.reanim",
            Self::Melonpult => "reanim/Melonpult.reanim",
            Self::Coffeebean => "reanim/Coffeebean.reanim",
            Self::Umbrellaleaf => "reanim/Umbrellaleaf.reanim",
            Self::Gatlingpea => "reanim/GatlingPea.reanim",
            Self::Cattail => "reanim/Cattail.reanim",
            Self::Gloomshroom => "reanim/GloomShroom.reanim",
            Self::BossIceball => "reanim/Zombie_boss_iceball.reanim",
            Self::BossFireball => "reanim/Zombie_boss_fireball.reanim",
            Self::Cobcannon => "reanim/CobCannon.reanim",
            Self::Garlic => "reanim/Garlic.reanim",
            Self::GoldMagnet => "reanim/GoldMagnet.reanim",
            Self::WinterMelon => "reanim/WinterMelon.reanim",
            Self::TwinSunflower => "reanim/TwinSunflower.reanim",
            Self::PoolCleaner => "reanim/PoolCleaner.reanim",
            Self::RoofCleaner => "reanim/RoofCleaner.reanim",
            Self::FirePea => "reanim/FirePea.reanim",
            Self::Imitater => "reanim/Imitater.reanim",
            Self::Yeti => "reanim/Zombie_yeti.reanim",
            Self::BossDriver => "reanim/Zombie_Boss_driver.reanim",
            Self::LawnMoweredZombie => "reanim/LawnMoweredZombie.reanim",
            Self::CrazyDave => "reanim/CrazyDave.reanim",
            Self::TextFadeOn => "reanim/TextFadeOn.reanim",
            Self::Hammer => "reanim/Hammer.reanim",
            Self::SlotMachineHandle => "reanim/SlotMachine.reanim",
            Self::CreditsFootball => "reanim/Credits_Football.reanim",
            Self::CreditsJackbox => "reanim/Credits_Jackbox.reanim",
            Self::SelectorScreen => "reanim/SelectorScreen.reanim",
            Self::PortalCircle => "reanim/Portal_Circle.reanim",
            Self::PortalSquare => "reanim/Portal_Square.reanim",
            Self::ZengardenSprout => "reanim/ZenGarden_sprout.reanim",
            Self::ZengardenWateringcan => "reanim/ZenGarden_wateringcan.reanim",
            Self::ZengardenFertilizer => "reanim/ZenGarden_fertilizer.reanim",
            Self::ZengardenBugspray => "reanim/ZenGarden_bugspray.reanim",
            Self::ZengardenPhonograph => "reanim/ZenGarden_phonograph.reanim",
            Self::Diamond => "reanim/Diamond.reanim",
            Self::ZombieHand => "reanim/Zombie_hand.reanim",
            Self::Stinky => "reanim/Stinky.reanim",
            Self::Rake => "reanim/Rake.reanim",
            Self::RainCircle => "reanim/Rain_circle.reanim",
            Self::RainSplash => "reanim/Rain_splash.reanim",
            Self::ZombieSurprise => "reanim/Zombie_surprise.reanim",
            Self::CoinGold => "reanim/Coin_gold.reanim",
            Self::Treeofwisdom => "reanim/TreeOfWisdom.reanim",
            Self::TreeofwisdomClouds => "reanim/TreeOfWisdomClouds.reanim",
            Self::TreeofwisdomTreefood => "reanim/TreeFood.reanim",
            Self::CreditsMain => "reanim/Credits_Main.reanim",
            Self::CreditsMain2 => "reanim/Credits_Main2.reanim",
            Self::CreditsMain3 => "reanim/Credits_Main3.reanim",
            Self::ZombieCreditsDance => "reanim/Zombie_credits_dance.reanim",
            Self::CreditsStage => "reanim/Credits_stage.reanim",
            Self::CreditsBigbrain => "reanim/Credits_BigBrain.reanim",
            Self::CreditsFlowerPetals => "reanim/Credits_Flower_petals.reanim",
            Self::CreditsInfantry => "reanim/Credits_Infantry.reanim",
            Self::CreditsThroat => "reanim/Credits_Throat.reanim",
            Self::CreditsCrazydave => "reanim/Credits_CrazyDave.reanim",
            Self::CreditsBossdance => "reanim/Credits_Bossdance.reanim",
            Self::ZombieCreditsScreenDoor => "reanim/Zombie_Credits_Screendoor.reanim",
            Self::ZombieCreditsConehead => "reanim/Zombie_Credits_Conehead.reanim",
            Self::CreditsZombiearmy1 => "reanim/Credits_ZombieArmy1.reanim",
            Self::CreditsZombiearmy2 => "reanim/Credits_ZombieArmy2.reanim",
            Self::CreditsTombstones => "reanim/Credits_Tombstones.reanim",
            Self::CreditsSolarpower => "reanim/Credits_SolarPower.reanim",
            Self::CreditsAnyhour => "reanim/Credits_Anyhour.reanim",
            Self::CreditsWearetheundead => "reanim/Credits_WeAreTheUndead.reanim",
            Self::CreditsDiscolights => "reanim/Credits_DiscoLights.reanim",
            Self::Flag => "reanim/Zombie_FlagPole.reanim",
        }
    }
}

/// Running effects. Mirrors `EffectSystem`'s reanimation holder, plus the
/// part of its attachment holder that ties reanimations to other
/// reanimations' tracks (a plant's head on its stem, a blink on its face).
/// Particles and trails will join it as they're ported.
#[derive(Resource, Default)]
pub struct EffectSystem {
    reanimations: DataArray<Reanimation>,
    /// Reanimations attached to each reanimation's tracks: (track, attached).
    attachments: HashMap<ReanimationId, Vec<(usize, ReanimationId)>>,
    particles: DataArray<ParticleSystem>,
}

pub type ParticleSystemId = Id<ParticleSystem>;

impl EffectSystem {
    /// `LawnApp::AddReanimation`. Loads the definition on first use.
    pub fn add_reanimation(
        &mut self,
        x: f32,
        y: f32,
        ty: ReanimationType,
        resources: &mut GameResources,
        images: &mut Assets<Image>,
    ) -> Result<(ReanimationId, &mut Reanimation), String> {
        let definition = resources.reanim_definition(ty.file(), images)?;
        let id = self.reanimations.alloc(Reanimation::new(x, y, definition));
        Ok((id, self.reanimations.get_mut(id).unwrap()))
    }

    pub fn get(&self, id: ReanimationId) -> Option<&Reanimation> {
        self.reanimations.get(id)
    }

    pub fn get_mut(&mut self, id: ReanimationId) -> Option<&mut Reanimation> {
        self.reanimations.get_mut(id)
    }

    /// `Reanimation::AttachToAnotherReanimation`: `child` follows `track` of
    /// `parent` from now on, and is updated and drawn along with it.
    pub fn attach_to_another(&mut self, child: ReanimationId, parent: ReanimationId, track: &str) {
        let Some(p) = self.reanimations.get_mut(parent) else { return };
        let Some(track) = p.definition.find_track(track) else { return };
        if p.frame_base_pose == -1 {
            p.frame_base_pose = p.frame_start as i32;
        }
        if let Some(c) = self.reanimations.get_mut(child) {
            c.is_attachment = true;
        }
        self.attachments.entry(parent).or_default().push((track, child));
    }

    /// Updates one reanimation, then everything attached to it, moved to
    /// follow its track (`Reanimation::Update` + `AttachmentUpdateAndSetMatrix`).
    pub fn update_reanimation(&mut self, id: ReanimationId) {
        let Some(reanim) = self.reanimations.get_mut(id) else { return };
        reanim.update();
        let Some(attached) = self.attachments.get(&id).cloned() else { return };
        for (track, child) in attached {
            self.update_reanimation(child);
            let Some(matrix) = self.reanimations.get(id).map(|r| r.attachment_overlay_matrix(track)) else { break };
            let (color, extra) = self.reanimations.get(id).map_or((sexy::reanim::WHITE, None), |r| (r.color_override, r.extra_additive));
            if let Some(c) = self.reanimations.get_mut(child) {
                c.overlay = matrix * c.attach_offset;
                // PropogateColorToAttachments.
                c.color_override = color;
                c.extra_additive = extra;
            }
        }
    }

    /// `LawnApp::AddTodParticle`: starts a particle effect at (x, y).
    #[allow(clippy::too_many_arguments)]
    pub fn add_particle(
        &mut self,
        x: f32,
        y: f32,
        render_order: i32,
        effect: ParticleEffect,
        resources: &mut GameResources,
        images: &mut Assets<Image>,
        rng: &mut MtRand,
    ) -> Option<ParticleSystemId> {
        match resources.particle_definition(effect.file(), images) {
            Ok(def) => Some(self.particles.alloc(ParticleSystem::new(x, y, render_order, def, rng))),
            Err(e) => {
                error!("{e}");
                None
            }
        }
    }

    pub fn particle(&self, id: ParticleSystemId) -> Option<&ParticleSystem> {
        self.particles.get(id)
    }

    pub fn particle_mut(&mut self, id: ParticleSystemId) -> Option<&mut ParticleSystem> {
        self.particles.get_mut(id)
    }

    /// `LawnApp::RemoveParticle`.
    pub fn remove_particle(&mut self, id: ParticleSystemId) {
        self.particles.free(id);
    }

    /// Particle systems the board draws itself (not attached to anything).
    pub fn particle_ids(&self) -> impl Iterator<Item = (ParticleSystemId, &ParticleSystem)> {
        self.particles.iter()
    }

    /// `EffectSystem::Update`: one 100 Hz tick for everything not owned by
    /// something else; finished effects are removed.
    pub fn update(&mut self, rng: &mut MtRand) {
        for (_, system) in self.particles.iter_mut() {
            if !system.is_attachment {
                system.update(rng);
            }
        }
        self.particles.retain(|p| !p.dead);
        let free: Vec<_> = self.reanimations.iter().filter(|(_, r)| !r.is_attachment).map(|(id, _)| id).collect();
        for id in free {
            self.update_reanimation(id);
        }
        self.remove_dead();
    }

    /// `ReanimationDie` on a reanimation and everything attached to it.
    pub fn kill(&mut self, id: ReanimationId) {
        if let Some(r) = self.reanimations.get_mut(id) {
            r.dead = true;
        }
        if let Some(attached) = self.attachments.remove(&id) {
            for (_, child) in attached {
                self.kill(child);
            }
        }
    }

    /// Frees dead reanimations (and their attachment lists).
    pub fn remove_dead(&mut self) {
        let dead: Vec<_> = self.reanimations.iter().filter(|(_, r)| r.dead).map(|(id, _)| id).collect();
        for id in dead {
            self.kill(id);
            self.reanimations.free(id);
        }
        for list in self.attachments.values_mut() {
            list.retain(|(_, child)| self.reanimations.get(*child).is_some());
        }
    }

    pub fn reanimations(&self) -> impl Iterator<Item = &Reanimation> {
        self.reanimations.iter().map(|(_, r)| r)
    }

    pub fn reanimation_ids(&self) -> impl Iterator<Item = (ReanimationId, &Reanimation)> {
        self.reanimations.iter()
    }

    pub fn clear(&mut self) {
        self.reanimations.clear();
        self.attachments.clear();
        self.particles.clear();
    }

    /// `Reanimation::Draw` with its attachments, each drawn right after the
    /// track it's attached to (`DrawRenderGroup` + `AttachmentDraw`).
    pub fn draw(&self, g: &mut Graphics, id: ReanimationId, resources: &GameResources) {
        self.draw_group(g, id, RENDER_GROUP_NORMAL, resources);
    }

    /// `Reanimation::DrawRenderGroup` with its attachments: the tracks of one
    /// render group, each followed by what's attached to it.
    pub fn draw_group(&self, g: &mut Graphics, id: ReanimationId, group: i32, resources: &GameResources) {
        let Some(reanim) = self.reanimations.get(id) else { return };
        let attached = self.attachments.get(&id);
        if reanim.dead {
            return;
        }
        for (i, inst) in reanim.tracks.iter().enumerate() {
            if inst.render_group != group {
                continue;
            }
            draw_track(g, reanim, i, resources);
            for &(_, child) in attached.into_iter().flatten().filter(|(t, _)| *t == i) {
                if self.reanimations.get(child).is_some_and(|c| c.dont_draw_if_parent_hidden) && reanim.current_transform(i).frame() < 0 {
                    continue;
                }
                self.draw(g, child, resources);
            }
        }
    }
}

/// `ReanimatorCache::DrawReanimatorFrame`: draws the first frame of a
/// reanimation (or of one of its `anim_*` ranges) at (x, y), scaled. The
/// original renders these into cached images; here they're drawn directly,
/// which looks the same. The definition must already be loaded (see
/// [`GameResources::reanim_definition`]); if it isn't, nothing is drawn.
pub fn draw_reanimator_frame(
    g: &mut Graphics,
    resources: &GameResources,
    x: f32,
    y: f32,
    ty: ReanimationType,
    track: Option<&str>,
    scale: f32,
) {
    let Some(definition) = resources.loaded_reanim_definition(ty.file()) else { return };
    let mut reanim = Reanimation::new(x, y, definition);
    if let Some(track) = track
        && reanim.definition.find_track(track).is_some()
    {
        reanim.set_frames_for_layer(track);
    }
    match ty {
        ReanimationType::Kernelpult => reanim.assign_render_group_to_track("Cornpult_butter", RENDER_GROUP_HIDDEN),
        ReanimationType::Sunflower => reanim.anim_time = 0.15,
        _ => {}
    }
    reanim.assign_render_group_to_track("anim_waterline", RENDER_GROUP_HIDDEN);
    reanim.override_scale(scale, scale);
    draw_reanimation(g, &reanim, resources);
}

fn to_affine(m: Matrix3) -> Affine2 {
    Affine2::from_cols_array(&[m.m00, m.m10, m.m01, m.m11, m.m02, m.m12])
}

/// `Reanimation::Draw`: every track in the normal render group.
pub fn draw_reanimation(g: &mut Graphics, reanim: &Reanimation, resources: &GameResources) {
    draw_render_group(g, reanim, RENDER_GROUP_NORMAL, resources);
}

/// `Reanimation::DrawRenderGroup`.
pub fn draw_render_group(g: &mut Graphics, reanim: &Reanimation, group: i32, resources: &GameResources) {
    draw_render_group_colored(g, reanim, group, resources, reanim.color_override, reanim.extra_additive);
}

/// `Reanimation::DrawRenderGroup` with other colors than the reanimation's
/// own (`mColorOverride`, `mExtraAdditiveColor`), as a zombie's shield
/// flashes on its own when hit.
pub fn draw_render_group_colored(
    g: &mut Graphics,
    reanim: &Reanimation,
    group: i32,
    resources: &GameResources,
    color_override: [u8; 4],
    extra_additive: Option<[u8; 4]>,
) {
    if reanim.dead {
        return;
    }
    for (i, inst) in reanim.tracks.iter().enumerate() {
        if inst.render_group == group {
            draw_track_colored(g, reanim, i, resources, color_override, extra_additive);
        }
    }
}

/// `Reanimation::DrawTrack` for image tracks.
fn draw_track(g: &mut Graphics, reanim: &Reanimation, track: usize, resources: &GameResources) {
    draw_track_colored(g, reanim, track, resources, reanim.color_override, reanim.extra_additive);
}

fn draw_track_colored(
    g: &mut Graphics,
    reanim: &Reanimation,
    track: usize,
    resources: &GameResources,
    color_override: [u8; 4],
    extra_additive: Option<[u8; 4]>,
) {
    let transform = reanim.current_transform(track);
    let mut frame = transform.frame();
    if frame < 0 {
        return;
    }

    let inst = &reanim.tracks[track];
    let [r, gr, b, a] = inst.track_color;
    let mut color = SexyColor::rgba(r, gr, b, a);
    if !inst.ignore_color_override {
        let [r, gr, b, a] = color_override;
        color = color.multiply(SexyColor::rgba(r, gr, b, a));
    }
    if g.colorize_images() {
        color = color.multiply(g.color());
    }
    let alpha = float_round_to_int(transform.a * color.a as f32).clamp(0, 255);
    if alpha <= 0 {
        return;
    }
    color.a = alpha as u8;

    // TODO: text tracks (`font`/`text`).
    let Some(image) = transform.image.as_deref().and_then(|id| resources.image(id)) else {
        // The "fullscreen" track fills the whole screen with its color.
        if transform.font.is_none() && reanim.definition.tracks[track].name.eq_ignore_ascii_case("fullscreen") {
            let t = g.translation();
            g.push_state();
            g.set_color(color);
            g.fill_rect(-t.x, -t.y, 800.0, 600.0);
            g.pop_state();
        }
        return;
    };

    // The keyframe's image positions the track; an override only changes what's drawn.
    let (cel_w, cel_h) = (image.cel_width() as f32, image.cel_height() as f32);
    let matrix = (reanim.overlay * transform.matrix() * Matrix3::translation(cel_w * 0.5, cel_h * 0.5))
        .translated(-0.5, -0.5);
    let image = inst.image_override.as_deref().and_then(|id| resources.image(id)).unwrap_or(image);
    while frame >= image.cols as i32 {
        frame -= image.cols as i32;
    }
    let src = SexyRect::new(frame as f32 * cel_w, 0.0, cel_w, cel_h);

    g.push_state();
    if inst.ignore_clip_rect {
        g.reset_clip();
    }
    g.set_colorize_images(true);
    g.set_color(color);
    g.draw_image_matrix_src(image, to_affine(matrix), src);
    if let Some([r, gr, b, extra_alpha]) = extra_additive {
        g.set_additive(true);
        let alpha = (extra_alpha as u32 * color.a as u32 / 255) as u8;
        g.set_color(SexyColor::rgba(r, gr, b, alpha));
        g.draw_image_matrix_src(image, to_affine(matrix), src);
    }
    g.pop_state();
}
