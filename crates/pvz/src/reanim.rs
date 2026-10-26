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
use sexy::reanim::{LoopType, RENDER_GROUP_HIDDEN, RENDER_GROUP_NORMAL, ReanimDefinition};
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
    /// What each reanimation made by an attacher track is (`mReanimationType`), by file name.
    attacher_names: HashMap<ReanimationId, String>,
    /// Attacher names with no file, so they're only looked for once.
    missing_attachers: std::collections::HashSet<String>,
}

/// What an `attacher__` track's text asks for (`AttacherInfo`).
struct AttacherInfo {
    reanim: String,
    track: String,
    anim_rate: f32,
    loop_type: LoopType,
}

/// `Reanimation::ParseAttacherTrack`: `attacher__REANIM__TRACK[tag][tag]`.
fn parse_attacher(text: &str) -> Option<AttacherInfo> {
    let mut info = AttacherInfo { reanim: String::new(), track: String::new(), anim_rate: 12.0, loop_type: LoopType::Loop };
    let after_name = text.find("__")? + 2;
    let rest = &text[after_name..];
    let tags = rest.find('[');
    let track_at = rest.find("__");
    if let (Some(t), Some(k)) = (tags, track_at)
        && t < k
    {
        return None;
    }
    match (track_at, tags) {
        (Some(k), _) => {
            info.reanim = rest[..k].to_owned();
            let track = &rest[k + 2..];
            info.track = track[..track.find('[').unwrap_or(track.len())].to_owned();
        }
        (None, Some(t)) => info.reanim = rest[..t].to_owned(),
        (None, None) => info.reanim = rest.to_owned(),
    }
    let mut cursor = tags.map(|t| &rest[t..]);
    while let Some(tag) = cursor {
        let Some(end) = tag.find(']') else { break };
        let code = &tag[1..end];
        if let Some(rate) = code.trim().parse::<f32>().ok().or_else(|| parse_leading_float(code)) {
            info.anim_rate = rate;
        } else if code == "hold" {
            info.loop_type = LoopType::PlayOnceAndHold;
        } else if code == "once" {
            info.loop_type = LoopType::PlayOnce;
        }
        cursor = tag[end + 1..].find('[').map(|n| &tag[end + 1 + n..]);
    }
    Some(info)
}

/// The number `sscanf("%f")` reads from the start of `text`, if any.
fn parse_leading_float(text: &str) -> Option<f32> {
    let end = text.find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))).unwrap_or(text.len());
    text[..end].parse().ok()
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

    /// `AddReanimation` for a file named by an attacher track (`reanim/NAME.reanim`).
    pub fn add_reanimation_file(
        &mut self,
        x: f32,
        y: f32,
        path: &str,
        resources: &mut GameResources,
        images: &mut Assets<Image>,
    ) -> Result<(ReanimationId, &mut Reanimation), String> {
        let definition = resources.reanim_definition(path, images)?;
        let id = self.reanimations.alloc(Reanimation::new(x, y, definition));
        Ok((id, self.reanimations.get_mut(id).unwrap()))
    }

    /// `Reanimation::UpdateAttacherTrack` for `id` and everything attached to
    /// it: each `attacher__` track holds a reanimation named by its text
    /// (which changes from keyframe to keyframe), playing the track it names.
    pub fn update_attachers(&mut self, id: ReanimationId, resources: &mut GameResources, images: &mut Assets<Image>) {
        let Some(reanim) = self.reanimations.get(id) else { return };
        let attachers: Vec<(usize, Option<AttacherInfo>)> = reanim
            .definition
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.name.len() >= 10 && t.name[..10].eq_ignore_ascii_case("attacher__"))
            .map(|(i, _)| {
                let transform = reanim.current_transform(i);
                (i, if transform.frame() < 0 { None } else { parse_attacher(&transform.text) })
            })
            .collect();
        for (track, info) in attachers {
            self.update_attacher_track(id, track, info, resources, images);
        }
        // Whatever it holds may hold attachers too.
        let children: Vec<_> = self.attachments.get(&id).map(|l| l.iter().map(|(_, c)| *c).collect()).unwrap_or_default();
        for child in children {
            self.update_attachers(child, resources, images);
        }
    }

    fn update_attacher_track(
        &mut self,
        id: ReanimationId,
        track: usize,
        info: Option<AttacherInfo>,
        resources: &mut GameResources,
        images: &mut Assets<Image>,
    ) {
        let existing = self.attachments.get(&id).and_then(|l| l.iter().find(|(t, _)| *t == track).map(|(_, c)| *c));
        let wanted = info.as_ref().map(|i| i.reanim.to_lowercase()).filter(|n| !n.is_empty() && !self.missing_attachers.contains(n));
        let Some(info) = info.filter(|_| wanted.is_some()) else {
            if let Some(old) = existing {
                self.detach_attached(id, old);
            }
            return;
        };
        let wanted = wanted.expect("checked above");
        let mut child = existing.filter(|c| self.attacher_names.get(c).is_some_and(|n| *n == wanted));
        if child.is_none() {
            if let Some(old) = existing {
                self.detach_attached(id, old);
            }
            let path = format!("reanim/{}.reanim", info.reanim);
            match self.add_reanimation_file(0.0, 0.0, &path, resources, images) {
                Ok((new, reanim)) => {
                    reanim.loop_type = info.loop_type;
                    reanim.anim_rate = info.anim_rate;
                    reanim.is_attachment = true;
                    self.attacher_names.insert(new, wanted.clone());
                    self.attachments.entry(id).or_default().push((track, new));
                    if let Some(parent) = self.reanimations.get_mut(id) {
                        // The parent has no base pose of its own once something is attached.
                        parent.frame_base_pose = sexy::reanim::NO_BASE_POSE;
                    }
                    child = Some(new);
                }
                Err(_) => {
                    self.missing_attachers.insert(wanted);
                    return;
                }
            }
        }
        let Some(child) = child.and_then(|c| self.reanimations.get_mut(c)) else { return };
        if !info.track.is_empty() {
            let range = child.definition.frames_for_layer(&info.track);
            if (child.frame_start, child.frame_count) != range {
                child.start_blend(20);
                child.set_frames_for_layer(&info.track);
            }
            child.anim_rate = info.anim_rate;
            child.loop_type = info.loop_type;
        }
    }

    /// Kills what an attacher track held, and forgets it.
    fn detach_attached(&mut self, parent: ReanimationId, child: ReanimationId) {
        if let Some(list) = self.attachments.get_mut(&parent) {
            list.retain(|(_, c)| *c != child);
        }
        self.kill(child);
    }

    /// `FindSubReanim`: the reanimation made from `file_stem` (a name like
    /// `Sunflower`, case aside) among `root`'s attachments, at any depth.
    pub fn find_attached(&self, root: ReanimationId, file_stem: &str) -> Option<ReanimationId> {
        let wanted = file_stem.to_lowercase();
        for &(_, child) in self.attachments.get(&root).into_iter().flatten() {
            if self.attacher_names.get(&child).is_some_and(|n| *n == wanted) {
                return Some(child);
            }
            if let Some(found) = self.find_attached(child, file_stem) {
                return Some(found);
            }
        }
        None
    }

    /// What's attached to `id`'s tracks: (track, attached).
    pub fn attachments_of(&self, id: ReanimationId) -> Vec<(usize, ReanimationId)> {
        self.attachments.get(&id).cloned().unwrap_or_default()
    }

    /// Whether `id` was made by an attacher track for the reanimation called `file_stem`.
    pub fn is_attached_as(&self, id: ReanimationId, file_stem: &str) -> bool {
        self.attacher_names.get(&id).is_some_and(|n| *n == file_stem.to_lowercase())
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
            let (mut color, extra) = self.reanimations.get(id).map_or((sexy::reanim::WHITE, None), |r| (r.color_override, r.extra_additive));
            // An attacher track's own alpha fades what it holds (`UpdateAttacherTrack`).
            if let Some(r) = self.reanimations.get(id)
                && r.definition.tracks[track].name.len() >= 10
                && r.definition.tracks[track].name[..10].eq_ignore_ascii_case("attacher__")
            {
                let alpha = r.current_transform(track).a;
                color[3] = (alpha * color[3] as f32).round().clamp(0.0, 255.0) as u8;
            }
            if let Some(c) = self.reanimations.get_mut(child) {
                c.overlay = matrix * c.attach_offset;
                // PropogateColorToAttachments.
                c.color_override = color;
                c.extra_additive = extra;
            }
        }
    }

    /// Puts everything attached to `id` (at any depth) where its track is now,
    /// without advancing anything, so a sub-animation made this tick isn't drawn
    /// once at the origin.
    pub fn place_attachments(&mut self, id: ReanimationId) {
        let Some(attached) = self.attachments.get(&id).cloned() else { return };
        for (track, child) in attached {
            let Some(parent) = self.reanimations.get(id) else { return };
            let matrix = parent.attachment_overlay_matrix(track);
            if let Some(c) = self.reanimations.get_mut(child) {
                c.overlay = matrix * c.attach_offset;
            }
            self.place_attachments(child);
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
        self.attacher_names.clear();
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

    let Some(image) = transform.image.as_deref().and_then(|id| resources.image(id)) else {
        // A track of text: its center is its position, its top the baseline less the ascent.
        if let (Some(font), false) = (transform.font.as_deref().and_then(|id| resources.font(id)), transform.text.is_empty()) {
            let width = font.font.string_width(&transform.text) as f32;
            let matrix = (reanim.overlay * transform.matrix() * Matrix3::translation(-width * 0.5, font.font.ascent as f32))
                .translated(-0.5, -0.5);
            g.push_state();
            g.set_color(color);
            g.draw_string_matrix(font, to_affine(matrix), &transform.text);
            g.pop_state();
            return;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attacher_text_names_the_reanimation_track_and_tags() {
        let a = parse_attacher("attacher__Credits_stage[12.25]").unwrap();
        assert_eq!((a.reanim.as_str(), a.track.as_str(), a.anim_rate), ("Credits_stage", "", 12.25));
        let a = parse_attacher("attacher__Zombie_credits_dance__anim_dance2").unwrap();
        assert_eq!((a.reanim.as_str(), a.track.as_str(), a.anim_rate), ("Zombie_credits_dance", "anim_dance2", 12.0));
        assert_eq!(a.loop_type, LoopType::Loop);
        let a = parse_attacher("attacher__Sun__Sun1[8][hold]").unwrap();
        assert_eq!((a.reanim.as_str(), a.track.as_str(), a.anim_rate), ("Sun", "Sun1", 8.0));
        assert_eq!(a.loop_type, LoopType::PlayOnceAndHold);
        assert!(parse_attacher("no underscores").is_none());
    }
}
