//! Plants on the lawn (`Plant`). So far a plant is planted, idles and
//! blinks; peashooters, snow peas, repeaters, puff-shrooms and
//! scaredy-shrooms (which hide from zombies close by) shoot at zombies in
//! their row, fume-shrooms hit every zombie in reach, sunflowers and
//! sun-shrooms make sun, wall-nuts crack as they're eaten, cherry bombs and
//! doom-shrooms explode, potato mines arm and blow up, ice-shrooms freeze
//! every zombie, hypno-shrooms turn the zombie that eats them, and grave
//! busters eat graves. Mushrooms sleep in the day. The other
//! plants' abilities come as they're needed.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Plant.cpp`: `PlantInitialize`,
//! `CalcRenderOrder`, `Update`, `Animate`, `UpdateReanim`, `UpdateBlink`,
//! `DoBlink`, `AttachBlinkAnim`, `EndBlink`, `Draw`, `DrawShadow`, `Die`,
//! `PlantDrawHeightOffset`, `UpdateAbilities`, `UpdateShooter`,
//! `UpdateShooting`, `FindTargetAndFire`, `FindTargetZombie`, `Fire`,
//! `UpdateProductionPlant`, `GetPlantRect`, `GetPlantAttackRect`,
//! `AnimateNuts`, `UpdatePotato`, `DoSpecial`, `PlayBodyReanim`,
//! `PlayIdleAnim`, `SetSleeping`, `UpdateSunShroom`, `UpdateGraveBuster`,
//! `UpdateScaredyShroom`, `UpdateIceShroom`, `IceZombies` and `DoRowAreaDamage`.

use std::f32::consts::PI;

use bevy::prelude::*;
use sexy::data_array::DataArray;
use sexy::rand::MtRand;
use sexy::reanim::{LoopType, RENDER_GROUP_HIDDEN, RENDER_GROUP_NORMAL};
use sexy::tod::{TodCurve, animate_curve, animate_curve_float, animate_curve_float_time};

use crate::audio::FoleyType;
use crate::graphics::Graphics;
use crate::particle::ParticleEffect;
use crate::plant::{PlantSubClass, SeedType};
use crate::reanim::{EffectSystem, ParticleSystemId, ReanimationId, ReanimationType};
use crate::resources::GameResources;

use super::projectile::{ProjectileMotion, ProjectileType};
use super::zombie::{IRect, Zombie, damage, damage_range, rect_overlap};
use super::{make_render_order, render_layer};

/// Something a plant does that the board carries out (it owns the
/// projectiles and coins).
#[derive(Debug, Clone, Copy)]
pub enum PlantAction {
    /// `Plant::Fire`: a projectile leaves the plant at this spot.
    ///
    /// A threepeater's side peas start in its own row and drift towards `row`
    /// (`drift` is their vertical speed, `MOTION_THREEPEATER`).
    Shoot { x: i32, y: i32, row: i32, render_order: i32, projectile: ProjectileType, motion: ProjectileMotion, range_flags: u32, short_range: bool, drift: f32, lob: Option<Lob> },
    /// `Plant::DoRowAreaDamage`: hurts every zombie in `row` touching `rect`
    /// (the fume-shroom's fumes).
    RowAreaDamage { col: i32, row: i32, spread: i32, rect: IRect, damage: i32, flags: u32 },
    /// `Plant::IceZombies`: every zombie on the lawn freezes; the frost
    /// bursts from (x, y).
    IceShroom { x: i32, y: i32 },
    /// `Plant::DoSpecial` for the doom-shroom: a huge explosion at (x, y)
    /// that leaves a crater on its square.
    DoomShroom { x: i32, y: i32, col: i32, row: i32 },
    /// The cob cannon's cob, launched from (x, y) at a spot `target_x`, `target_y`.
    Cob { x: i32, y: i32, render_order: i32, target_x: i32, target_y: i32 },
    /// A magnet-shroom reaches for a zombie's metal.
    MagnetTake { col: i32, row: i32, zombie: sexy::data_array::Id<Zombie> },
    /// The coin bank slides out.
    ShowCoinBank,
    /// A gold magnet pulls in the money lying about.
    GoldMagnetCollect { col: i32, row: i32 },
    /// A gold magnet's coin has arrived: the player gets `amount`.
    AddMoney { amount: i32 },
    /// An imitater has finished morphing into `seed`.
    ImitaterMorph { col: i32, row: i32, seed: SeedType },
    /// A coffee bean's wake-up call to the sleeper on its square.
    WakeUp { col: i32, row: i32 },
    /// A marigold's coin: gold or silver.
    MakeCoin { x: i32, y: i32, gold: bool },
    /// A sunflower or sun-shroom makes sun at its square; a small sun-shroom's is small.
    MakeSun { x: i32, y: i32, small: bool },
    /// `Plant::DoSpecial` for the cherry bomb: burns zombies near it.
    CherryBomb { x: i32, y: i32, row: i32 },
    /// `Plant::DoSpecial` for the potato mine: kills zombies on top of it.
    PotatoMine { x: i32, y: i32, row: i32 },
    /// A chomper swallows this zombie whole (`DieWithLoot`).
    EatZombie(sexy::data_array::Id<Zombie>),
    /// A particle effect at a spot on the board.
    Particle { x: i32, y: i32, render_order: i32, effect: ParticleEffect },
    /// A sound.
    Foley(FoleyType),
    /// A rolling wall-nut hits this zombie (`Plant::UpdateBowling`).
    BowlingHit { zombie: sexy::data_array::Id<Zombie>, giant: bool, bouncing: bool },
    /// An explode-o-nut blows up where it hit.
    ExplodeONut { x: i32, y: i32, row: i32 },
    /// `Plant::DoSpecial` for the jalapeno: a wall of fire burns the whole `row`.
    Jalapeno { row: i32 },
    /// `Plant::DoSquashDamage`: the squash lands on `rect` in `row`.
    Squash { row: i32, rect: IRect },
    /// `Plant::UpdateTanglekelp`: the kelp drags this zombie under the water.
    TangleKelp(sexy::data_array::Id<Zombie>),
    /// `Plant::BlowAwayFliers`: a blover blows the fog away for a while.
    BlowFog,
    /// The board jolts (`Board::ShakeBoard`).
    ShakeBoard { x: i32, y: i32 },
    /// A grave buster has eaten the grave on its square (`x`, `y` is the plant's spot).
    GraveBusterDone { x: i32, y: i32, col: i32, row: i32, render_order: i32 },
}

/// `PlantWeapon`: the plants with two ways to attack (the split pea's heads,
/// the cactus's high and low spikes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Weapon {
    Primary,
    Secondary,
}

/// `MagnetItem`: what a magnet-shroom is holding, drifting towards it.
#[derive(Debug, Clone)]
pub struct MagnetItem {
    pub image: String,
    pub x: f32,
    pub y: f32,
    /// Where it comes to rest, from the plant's corner.
    pub dest_x: f32,
    pub dest_y: f32,
    /// Money, for a gold magnet's coin (`GetCoinValue`); 0 for metal.
    pub value: i32,
}

/// `PlantState`, for the plants ported so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlantState {
    NotReady,
    Ready,
    PotatoRising,
    PotatoArmed,
    ChomperBiting,
    ChomperBitingGotOne,
    ChomperBitingMissed,
    ChomperDigesting,
    ChomperSwallowing,
    BowlingUp,
    BowlingDown,
    SunshroomSmall,
    SunshroomGrowing,
    SunshroomBig,
    GravebusterLanding,
    GravebusterEating,
    ScaredyshroomLowering,
    ScaredyshroomScared,
    ScaredyshroomRaising,
    /// The squash has spotted a zombie and looks at it, then crouches,
    /// rises, falls on it and stays put for a while.
    SquashLook,
    SquashPreLaunch,
    SquashRising,
    SquashFalling,
    SquashDoneFalling,
    SpikeweedAttacking,
    /// The kernel-pult has a pat of butter ready to throw.
    KernelButter,
    /// The tangle kelp has hold of a zombie and is pulling it under.
    TanglekelpGrabbing,
    /// The umbrella leaf opening over its neighbours...
    UmbrellaTriggered,
    /// ...and held open, bouncing what drops on it.
    UmbrellaReflecting,
    /// The cob cannon is being armed after planting or a shot...
    CobcannonArming,
    /// ...loads its cob...
    CobcannonLoading,
    /// ...waits to be clicked...
    CobcannonReady,
    /// ...and fires its cob.
    CobcannonFiring,
    /// The marigold's last 60 seconds, after the final wave has come.
    MarigoldEnding,
    /// The magnet-shroom has pulled something off a zombie and is holding it...
    MagnetSucking,
    /// ...then waits a while before it can pull again.
    MagnetCharging,
    /// The cactus waits low (shooting walkers), rises for a flyer, shoots high...
    CactusLow,
    CactusRising,
    CactusHigh,
    /// ...and lowers again.
    CactusLowering,
    /// The imitater is turning into the plant it copies.
    ImitaterMorphing,
    /// An instant plant counting down to going off.
    DoingSpecial,
}

/// Whether a spikeweed or spikerock hurts the zombies on it at this point
/// of its attack (the spikerock hits twice, per the 1.0.0.1051 binary).
fn rock_or_weed_hits(seed: SeedType, countdown: i32) -> bool {
    if seed == SeedType::Spikerock { matches!(countdown, 70 | 32) } else { countdown == 75 }
}

/// Plants that shoot straight peas.
fn is_pea_shooter(seed: SeedType) -> bool {
    matches!(seed, SeedType::Peashooter | SeedType::Snowpea | SeedType::Repeater | SeedType::Gatlingpea)
}

/// `PlantOnBungeeState`: a bungee zombie has hold of the plant, or is
/// carrying it off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnBungee {
    No,
    Grabbed,
    Rising,
}

/// `Board::GetTopPlantAt` with `TOPPLANT_BUNGEE_ORDER` (also `TOPPLANT_ANY`):
/// a coffee bean, then the plant, then a pumpkin, then a lily pad or pot.
pub fn top_plant_any(plants: &DataArray<Plant>, col: i32, row: i32) -> Option<sexy::data_array::Id<Plant>> {
    let rank = |p: &Plant| match p.seed {
        SeedType::InstantCoffee => 0,
        SeedType::Pumpkinshell => 2,
        SeedType::Lilypad | SeedType::Flowerpot => 3,
        _ => 1,
    };
    plants
        .iter()
        .filter(|(_, p)| p.col == col && p.row == row && !p.dead)
        .min_by_key(|(_, p)| rank(p))
        .map(|(id, _)| id)
}

/// `Board::FindUmbrellaPlant`: an umbrella leaf on or next to the square.
pub fn find_umbrella_plant(plants: &DataArray<Plant>, col: i32, row: i32) -> Option<sexy::data_array::Id<Plant>> {
    plants
        .iter()
        .find(|(_, p)| {
            p.seed == SeedType::Umbrella && !p.dead && !p.not_on_ground() && (p.col - col).abs() <= 1 && (p.row - row).abs() <= 1
        })
        .map(|(id, _)| id)
}

/// `Plant::mWidth` and `mHeight`.
const PLANT_SIZE: i32 = 80;

/// The render group of a pumpkin's back half.
const PUMPKIN_BACK_GROUP: i32 = 3;

/// `PLANT_ORDER`: how plants in the same square stack.
#[allow(dead_code, reason = "the full set; plants use more of it as they're ported")]
#[derive(Debug, Clone, Copy)]
enum PlantOrder {
    Lilypad,
    Normal,
    Pumpkin,
    Flyer,
    Cherrybomb,
}

#[derive(Debug, Clone)]
pub struct Plant {
    pub seed: SeedType,
    /// Grid column (`mPlantCol`) and row.
    pub col: i32,
    pub row: i32,
    /// Top-left of its square, in board pixels.
    pub x: i32,
    pub y: i32,
    pub render_order: i32,
    pub health: i32,
    max_health: i32,
    pub dead: bool,
    /// `mSquished`: flattened by a gargantuar or a vehicle; it goes after `disappear_countdown`.
    pub squished: bool,
    disappear_countdown: i32,
    shake_offset_x: f32,
    state: PlantState,
    state_countdown: i32,
    /// Counts down to the plant's special move (`mDoSpecialCountdown`).
    do_special_countdown: i32,
    blink_countdown: i32,
    pub launch_rate: i32,
    pub launch_counter: i32,
    pub shooting_counter: i32,
    /// Counts down after a zombie bites it (`mRecentlyEatenCountdown`).
    pub recently_eaten_countdown: i32,
    /// Flashes white for this many ticks after a bite (`mEatenFlashCountdown`).
    pub eaten_flash_countdown: i32,
    /// Planted on the wall-nut bowling level: it rolls instead of standing.
    bowling: bool,
    /// A mushroom in the day (`mIsAsleep`).
    is_asleep: bool,
    /// The Zs over a sleeping mushroom.
    sleeping: Option<ReanimationId>,
    body: Option<ReanimationId>,
    head: Option<ReanimationId>,
    /// The threepeater's other two heads (`mHeadReanimID2` and `3`); `head` is the first.
    extra_heads: [Option<ReanimationId>; 2],
    blink: Option<ReanimationId>,
    /// Stands in a flower pot (or is one) on the roof: drawn and shooting 5 px higher.
    pub in_pot: bool,
    /// On a square of water: floats, bobbing with `bob`.
    pub on_water: bool,
    /// This tick's bob on the water (`pool_bob_offset`), 0 off the water.
    pub bob: f32,
    /// Grabbed or being carried off by a bungee zombie (`mOnBungeeState`).
    pub on_bungee: OnBungee,
    /// The plantern's glow over the fog.
    light: Option<ParticleSystemId>,
    /// The squash's square, where it jumps from (`mStartRow` and its column).
    home: (i32, i32),
    /// The zombie it's jumping on (`mTargetZombieID`) and the x it jumps to.
    target_zombie: Option<sexy::data_array::Id<Zombie>>,
    target_x: i32,
    target_y: i32,
    /// Whether the level has reached its last wave (the board sets it each tick).
    pub last_wave: bool,
    /// Bowling nuts pay money for the zombies they hit (set by the board).
    pub bowling_pays: bool,
    /// A coffee bean has woken it: it gets up when this runs out (`mWakeUpCounter`).
    wake_up_counter: i32,
    magnet_items: Vec<MagnetItem>,
    /// What an imitater turns into (`mImitaterType`).
    imitater_type: Option<SeedType>,
    /// The zombie a cattail's spike is homing in on.
    homing_target: Option<sexy::data_array::Id<Zombie>>,
    /// Whether there's money lying about for a gold magnet to take (the board sets it each tick).
    pub coin_available: bool,
}

/// Where a catapult plant's shot is thrown: how far along and down it lands,
/// and what it may hit.
#[derive(Debug, Clone, Copy)]
pub struct Lob {
    pub range_x: f32,
    pub range_y: f32,
    pub range_flags: u32,
}

/// What a catapult plant works out about the zombie it's throwing at.
#[derive(Debug, Clone, Copy)]
struct LobTarget {
    lead_x: f32,
    top: i32,
    boss: bool,
    riding_dolphin: bool,
    swimming_snorkel: bool,
}

/// The bobbing of a plant floating on a pool square: up to 2 px either way,
/// each square's wave out of step with its neighbours, one cycle per 200
/// ticks. Behaviour per ruslan831 decomp, `Lawn/Plant.cpp`
/// (`PlantDrawHeightOffset`, the `doFloating` part).
pub fn pool_bob_offset(main_counter: i32, col: i32, row: i32) -> f32 {
    let phase = row as f32 * PI + col as f32 * 0.25 * PI;
    let time = main_counter as f32 * 2.0 * PI / 200.0;
    (phase + time).sin() * 2.0
}

/// `PlantDrawHeightOffset`, without the flower pots' offset.
pub fn plant_draw_height_offset(seed: SeedType) -> f32 {
    use SeedType::*;
    // TODO: spikeweed's height depends on its row and the stage.
    match seed {
        Flowerpot => 26.0,
        Lilypad => 25.0,
        Starfruit => 10.0,
        Tanglekelp => 24.0,
        Seashroom => 28.0,
        InstantCoffee => -20.0,
        Pumpkinshell => 15.0,
        Puffshroom => 5.0,
        Scaredyshroom => -14.0,
        Gravebuster => -40.0,
        Spikeweed => 15.0,
        Spikerock => 21.0,
        _ => 0.0,
    }
}

impl Plant {
    /// `Plant::PlantInitialize`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        col: i32,
        row: i32,
        x: i32,
        y: i32,
        seed: SeedType,
        bowling: bool,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        rng: &mut MtRand,
    ) -> Self {
        let def = seed.definition();
        let mut plant = Self {
            seed,
            col,
            row,
            x,
            y,
            render_order: 0,
            health: 300,
            max_health: 300,
            dead: false,
            squished: false,
            disappear_countdown: 0,
            shake_offset_x: 0.0,
            state: PlantState::NotReady,
            state_countdown: 0,
            do_special_countdown: 0,
            in_pot: false,
            on_water: false,
            bob: 0.0,
            light: None,
            blink_countdown: 0,
            launch_rate: def.launch_rate,
            launch_counter: 0,
            shooting_counter: 0,
            recently_eaten_countdown: 0,
            eaten_flash_countdown: 0,
            bowling,
            is_asleep: false,
            sleeping: None,
            body: None,
            head: None,
            extra_heads: [None; 2],
            blink: None,
            home: (x, y),
            target_zombie: None,
            on_bungee: OnBungee::No,
            target_x: x,
            target_y: y,
            last_wave: false,
            bowling_pays: false,
            wake_up_counter: 0,
            magnet_items: Vec::new(),
            imitater_type: None,
            homing_target: None,
            coin_available: false,
        };
        plant.render_order = plant.calc_render_order();

        let offset_y = plant_draw_height_offset(seed);
        match effects.add_reanimation(0.0, offset_y, def.reanim, res, images) {
            Ok((id, body)) => {
                body.render_order = plant.render_order + 1;
                body.loop_type = LoopType::Loop;
                body.anim_rate = rng.range_f32(10.0, 15.0);
                if body.definition.find_track("anim_idle").is_some() {
                    body.set_frames_for_layer("anim_idle");
                }
                if bowling && body.definition.find_track("_ground").is_some() {
                    body.set_frames_for_layer("_ground");
                    if matches!(seed, SeedType::Wallnut | SeedType::ExplodeONut) {
                        body.anim_rate = rng.range_f32(12.0, 18.0);
                    } else if seed == SeedType::GiantWallnut {
                        body.anim_rate = rng.range_f32(6.0, 10.0);
                    }
                }
                if seed == SeedType::ExplodeONut {
                    body.color_override = [255, 64, 64, 255];
                }
                body.is_attachment = true;
                plant.body = Some(id);
                plant.blink_countdown = 400 + rng.next_range(400) as i32;
            }
            Err(e) => error!("{e}"),
        }

        if plant.launch_rate > 0 {
            plant.launch_counter = if seed.makes_sun() {
                rng.range_int(300, plant.launch_rate / 2)
            } else {
                rng.range_int(0, plant.launch_rate)
            };
        }

        use SeedType::*;
        match seed {
            Peashooter | Snowpea | Repeater | Leftpeater | Gatlingpea => {
                if let Some(body_id) = plant.body {
                    let rate = rng.range_f32(15.0, 20.0);
                    if let Some(body) = effects.get_mut(body_id) {
                        body.anim_rate = rate;
                    }
                    if let Ok((head_id, head)) = effects.add_reanimation(0.0, 0.0, def.reanim, res, images) {
                        head.render_order = plant.render_order + 2;
                        head.loop_type = LoopType::Loop;
                        head.anim_rate = rate;
                        head.set_frames_for_layer("anim_head_idle");
                        plant.head = Some(head_id);
                        let stem = match effects.get(body_id) {
                            Some(b) if b.definition.find_track("anim_stem").is_some() => "anim_stem",
                            _ => "anim_idle",
                        };
                        effects.attach_to_another(head_id, body_id, stem);
                    }
                }
            }
            Threepeater => {
                // Three heads, each its own animation riding on the body.
                if let Some(body_id) = plant.body {
                    let rate = rng.range_f32(15.0, 20.0);
                    if let Some(body) = effects.get_mut(body_id) {
                        body.anim_rate = rate;
                    }
                    for i in 1..=3 {
                        if let Ok((head_id, head)) = effects.add_reanimation(0.0, 0.0, def.reanim, res, images) {
                            head.render_order = plant.render_order + 2;
                            head.loop_type = LoopType::Loop;
                            head.anim_rate = rate;
                            head.set_frames_for_layer(&format!("anim_head_idle{i}"));
                            effects.attach_to_another(head_id, body_id, &format!("anim_head{i}"));
                            match i {
                                1 => plant.head = Some(head_id),
                                _ => plant.extra_heads[i - 2] = Some(head_id),
                            }
                        }
                    }
                }
            }
            Splitpea => {
                // Two heads, one looking each way, riding on the body.
                if let Some(body_id) = plant.body {
                    let rate = rng.range_f32(15.0, 20.0);
                    if let Some(body) = effects.get_mut(body_id) {
                        body.anim_rate = rate;
                    }
                    for (layer, back) in [("anim_head_idle", false), ("anim_splitpea_idle", true)] {
                        if let Ok((head_id, head)) = effects.add_reanimation(0.0, 0.0, def.reanim, res, images) {
                            head.render_order = plant.render_order + 2;
                            head.loop_type = LoopType::Loop;
                            head.anim_rate = rate;
                            head.set_frames_for_layer(layer);
                            effects.attach_to_another(head_id, body_id, "anim_idle");
                            if back {
                                plant.extra_heads[0] = Some(head_id);
                            } else {
                                plant.head = Some(head_id);
                            }
                        }
                    }
                }
            }
            Cactus => plant.state = PlantState::CactusLow,
            Chomper | Scaredyshroom => plant.state = PlantState::Ready,
            Kernelpult => {
                if let Some(body) = plant.body.and_then(|b| effects.get_mut(b)) {
                    body.assign_render_group_to_prefix("Cornpult_butter", RENDER_GROUP_HIDDEN);
                }
            }
            Plantern => {
                // Behaviour per ruslan831 decomp, Plant::PlantInitialize: its glow shows above the fog.
                plant.state_countdown = 50;
                let order = make_render_order(render_layer::FOG, 0, 1);
                plant.light = effects.add_particle((x + 40) as f32, (y + 40) as f32, order, ParticleEffect::LanternShine, res, images, rng);
            }
            Blover => {
                plant.do_special_countdown = 50;
                if let Some(body) = plant.body.and_then(|b| effects.get_mut(b)) {
                    body.set_frames_for_layer("anim_blow");
                    body.loop_type = LoopType::PlayOnceAndHold;
                    body.anim_rate = 20.0;
                }
            }
            Cherrybomb | Jalapeno => {
                plant.do_special_countdown = 100;
                if let Some(body) = plant.body.and_then(|b| effects.get_mut(b)) {
                    body.set_frames_for_layer("anim_explode");
                    body.loop_type = LoopType::PlayOnceAndHold;
                }
            }
            Potatomine => {
                plant.state_countdown = 1500;
                if let Some(body) = plant.body.and_then(|b| effects.get_mut(b)) {
                    body.anim_rate = 12.0;
                    body.assign_render_group_to_track("anim_glow", RENDER_GROUP_HIDDEN);
                }
            }
            Wallnut | ExplodeONut | GiantWallnut => {
                plant.health = 4000;
                plant.blink_countdown = 1000 + rng.next_range(1000) as i32;
            }
            Spikerock => plant.health = 450,
            Imitater => {
                let rate = rng.range_f32(25.0, 30.0);
                if let Some(body) = plant.body.and_then(|b| effects.get_mut(b)) {
                    body.anim_rate = rate;
                }
                plant.state_countdown = 200;
            }
            Pumpkinshell => {
                plant.health = 4000;
                // The back half is drawn on its own, behind whatever stands inside.
                if let Some(body) = plant.body.and_then(|b| effects.get_mut(b)) {
                    body.assign_render_group_to_track("Pumpkin_back", PUMPKIN_BACK_GROUP);
                }
            }
            InstantCoffee => plant.do_special_countdown = 100,
            Marigold => {
                let rate = rng.range_f32(15.0, 20.0);
                if let Some(body) = plant.body.and_then(|b| effects.get_mut(b)) {
                    body.anim_rate = rate;
                }
            }
            Tallnut => {
                plant.health = 8000;
                plant.blink_countdown = 1000 + rng.next_range(1000) as i32;
            }
            Gravebuster => {
                if let Some(body) = plant.body.and_then(|b| effects.get_mut(b)) {
                    body.set_frames_for_layer("anim_land");
                    body.loop_type = LoopType::PlayOnceAndHold;
                }
                plant.state = PlantState::GravebusterLanding;
            }
            Sunshroom => {
                if let Some(body) = plant.body.and_then(|b| effects.get_mut(b)) {
                    body.frame_base_pose = 6;
                }
                plant.x += rng.next_range(10) as i32 - 5;
                plant.y += rng.next_range(10) as i32 - 5;
                plant.state = PlantState::SunshroomSmall;
                plant.state_countdown = 12000;
            }
            Puffshroom | Seashroom => {
                plant.x += rng.next_range(10) as i32 - 5;
                plant.y += rng.next_range(6) as i32 - 3;
            }
            // TODO: the rest of PlantInitialize's per-plant setup as plants are ported.
            _ => {}
        }
        plant.max_health = plant.health;
        plant
    }

    /// `Plant::CalcRenderOrder`: plants further right draw first, so the ones
    /// on their left overlap them.
    fn calc_render_order(&self) -> i32 {
        use SeedType::*;
        let order = match self.seed {
            Pumpkinshell => PlantOrder::Pumpkin,
            InstantCoffee => PlantOrder::Flyer,
            Flowerpot | Lilypad => PlantOrder::Lilypad,
            _ => PlantOrder::Normal,
        };
        let layer = if self.bowling { render_layer::PROJECTILE } else { render_layer::PLANT };
        make_render_order(layer, self.row, order as i32 * 5 - self.x + 800)
    }

    /// `PlantDrawHeightOffset` for this plant: potted plants sit 5 px higher.
    fn height_offset(&self) -> f32 {
        plant_draw_height_offset(self.seed) + if self.in_pot { 5.0 } else { 0.0 } + self.bob
    }

    /// `Plant::Update`, while playing.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        zombies: &DataArray<Zombie>,
        actions: &mut Vec<PlantAction>,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        row_y: &[i32; super::MAX_GRID_SIZE_Y],
        open_rows: &[bool; super::MAX_GRID_SIZE_Y],
        main_counter: i32,
        rng: &mut MtRand,
    ) {
        // Floating plants bob while the board runs (`PlantDrawHeightOffset`).
        self.bob = if self.on_water { pool_bob_offset(main_counter, self.col, self.row) } else { 0.0 };
        if self.squished {
            self.disappear_countdown -= 1;
            if self.disappear_countdown < 0 {
                self.die(effects);
                return;
            }
        }
        // Behaviour per ruslan831 decomp, Plant::Update: the woken sleeper stirs, then gets up.
        if self.wake_up_counter > 0 {
            self.wake_up_counter -= 1;
            if self.wake_up_counter == 60 {
                actions.push(PlantAction::Foley(FoleyType::WakeUp));
            }
            if self.wake_up_counter == 0 {
                self.set_sleeping(false, effects, res, images, rng);
            }
        }
        self.update_abilities(zombies, actions, effects, row_y, open_rows, rng);
        self.animate(actions, effects, res, images, rng);
        if self.health < 0 {
            self.die(effects);
        }
        self.update_reanim(effects);
    }

    /// `Plant::Animate`.
    fn animate(
        &mut self,
        actions: &mut Vec<PlantAction>,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        rng: &mut MtRand,
    ) {
        if self.recently_eaten_countdown > 0 {
            self.recently_eaten_countdown -= 1;
        }
        if self.eaten_flash_countdown > 0 {
            self.eaten_flash_countdown -= 1;
        }
        if matches!(self.seed, SeedType::Wallnut | SeedType::Tallnut) {
            self.animate_nuts(actions, effects, rng);
        }
        if self.seed == SeedType::Garlic {
            self.animate_garlic(effects);
        }
        if self.seed == SeedType::Pumpkinshell {
            self.animate_pumpkin(effects);
        }
        self.update_blink(effects, res, images, rng);
    }

    /// `Plant::AnimatePumpkin`: its front shows the dents.
    fn animate_pumpkin(&mut self, effects: &mut EffectSystem) {
        let Some(body) = self.body.and_then(|b| effects.get_mut(b)) else { return };
        let look = if self.health < self.max_health / 3 {
            Some("IMAGE_REANIM_PUMPKIN_DAMAGE3")
        } else if self.health < self.max_health * 2 / 3 {
            Some("IMAGE_REANIM_PUMPKIN_DAMAGE1")
        } else {
            None
        };
        body.set_image_override("Pumpkin_front", look);
    }

    /// `Plant::AnimateGarlic`: its face gets more battered as it's eaten.
    fn animate_garlic(&mut self, effects: &mut EffectSystem) {
        let Some(body) = self.body.and_then(|b| effects.get_mut(b)) else { return };
        if self.health < self.max_health / 3 {
            body.set_image_override("anim_face", Some("IMAGE_REANIM_GARLIC_BODY3"));
            body.assign_render_group_to_prefix("Garlic_stem", RENDER_GROUP_HIDDEN);
        } else if self.health < self.max_health * 2 / 3 {
            body.set_image_override("anim_face", Some("IMAGE_REANIM_GARLIC_BODY2"));
        } else {
            body.set_image_override("anim_face", None);
        }
    }

    /// `Plant::AnimateNuts`: nuts crack as they're eaten, and stop moving
    /// while a zombie chews on them.
    fn animate_nuts(&mut self, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem, rng: &mut MtRand) {
        let Some(body_id) = self.body else { return };
        let (cracked1, cracked2, track) = if self.seed == SeedType::Wallnut {
            ("IMAGE_REANIM_WALLNUT_CRACKED1", "IMAGE_REANIM_WALLNUT_CRACKED2", "anim_face")
        } else {
            ("IMAGE_REANIM_TALLNUT_CRACKED1", "IMAGE_REANIM_TALLNUT_CRACKED2", "anim_idle")
        };
        let (x, mut y) = (self.x + 40, self.y + 10);
        if self.seed == SeedType::Tallnut {
            y -= 32;
        }
        let order = self.render_order + 4;
        let Some(body) = effects.get_mut(body_id) else { return };
        let current = body.tracks[body.find_track(track)].image_override.clone();
        let crack = |body: &mut crate::reanim::Reanimation, image: &str, actions: &mut Vec<PlantAction>| {
            if current.as_deref() != Some(image) {
                body.set_image_override(track, Some(image));
                actions.push(PlantAction::Particle { x, y, render_order: order, effect: ParticleEffect::WallnutEatLarge });
            }
        };
        if self.health < self.max_health / 3 {
            crack(body, cracked2, actions);
        } else if self.health < self.max_health * 2 / 3 {
            crack(body, cracked1, actions);
        } else {
            body.set_image_override(track, None);
        }
        if self.recently_eaten_countdown > 0 {
            body.anim_rate = 0.1;
        } else if body.anim_rate < 1.0 && self.on_bungee != OnBungee::Rising {
            body.anim_rate = rng.range_f32(10.0, 15.0);
        }
    }

    /// `Plant::SetSleeping`: a sleeping mushroom snores (a cloud of Zs) and
    /// does nothing until a coffee bean wakes it.
    pub fn set_sleeping(
        &mut self,
        asleep: bool,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        rng: &mut MtRand,
    ) {
        if self.is_asleep == asleep {
            return;
        }
        self.is_asleep = asleep;
        if asleep {
            let (mut x, mut y) = (self.x as f32 + 50.0, self.y as f32 + 40.0);
            match self.seed {
                SeedType::Fumeshroom => x += 12.0,
                SeedType::Scaredyshroom => y -= 20.0,
                SeedType::Gloomshroom => y -= 12.0,
                _ => {}
            }
            match effects.add_reanimation(x, y, ReanimationType::Sleeping, res, images) {
                Ok((id, zs)) => {
                    zs.render_order = self.render_order + 2;
                    zs.loop_type = LoopType::Loop;
                    zs.anim_rate = rng.range_f32(6.0, 8.0);
                    zs.anim_time = rng.range_f32(0.0, 0.9);
                    self.sleeping = Some(id);
                }
                Err(e) => error!("{e}"),
            }
        } else if let Some(id) = self.sleeping.take() {
            effects.kill(id);
        }
        let Some(body) = self.body.and_then(|b| effects.get_mut(b)) else { return };
        let track = if asleep { "anim_sleep" } else { "anim_idle" };
        if body.definition.find_track(track).is_some() {
            let time = body.anim_time;
            body.start_blend(20);
            body.set_frames_for_layer(track);
            body.anim_time = time;
        } else if asleep {
            body.anim_rate = 1.0;
        }
        if !asleep && body.anim_rate < 2.0 {
            body.anim_rate = rng.range_f32(10.0, 15.0);
        }
        if asleep {
            self.end_blink(effects);
        }
    }

    /// What the plant is doing (`mState`).
    pub(super) fn state(&self) -> PlantState {
        self.state
    }

    /// A potato mine past `STATE_NOTREADY` (rising or armed): zombies
    /// can't eat it then.
    /// `Plant::NotOnGround`: a jumping or landed squash, or a plant flying
    /// off with a bungee zombie, can't be reached.
    pub fn not_on_ground(&self) -> bool {
        self.squished
            || self.dead
            || self.on_bungee == OnBungee::Rising
            || self.seed == SeedType::Squash
                && matches!(self.state, PlantState::SquashRising | PlantState::SquashFalling | PlantState::SquashDoneFalling)
    }

    /// `Plant::Squish`: stamped flat by a gargantuar or a vehicle. The instant
    /// plants go off instead (and a mine that's armed), a squash that's ready
    /// shrugs it off, and everything else stays flat for 5 s.
    pub fn squish(&mut self, effects: &mut EffectSystem) -> bool {
        if self.not_on_ground() {
            return false;
        }
        if !self.is_asleep {
            let goes_off = match self.seed {
                SeedType::Cherrybomb | SeedType::Jalapeno | SeedType::Doomshroom | SeedType::Iceshroom => true,
                SeedType::Potatomine => self.state != PlantState::NotReady,
                _ => false,
            };
            if goes_off {
                // It goes off on the plant's next update.
                self.do_special_countdown = 1;
                return false;
            }
        }
        if self.seed == SeedType::Squash && self.state != PlantState::NotReady {
            return false;
        }
        self.render_order = make_render_order(render_layer::GRAVE_STONE, self.row, 8);
        self.squished = true;
        self.disappear_countdown = 500;
        // RemoveEffects: the plant stops moving, with nothing glowing on it.
        if let Some(light) = self.light.take() {
            effects.remove_particle(light);
        }
        if let Some(body) = self.body.and_then(|b| effects.get_mut(b)) {
            body.anim_rate = 0.0;
        }
        true
    }

    /// A squash that is still looking or crouching can't be eaten (`Zombie::EatPlant`).
    pub fn is_squash_getting_ready(&self) -> bool {
        self.seed == SeedType::Squash && matches!(self.state, PlantState::SquashLook | PlantState::SquashPreLaunch)
    }

    pub fn is_armed_or_rising(&self) -> bool {
        self.state != PlantState::NotReady
    }

    /// A mushroom asleep in the day (`mIsAsleep`).
    pub fn is_asleep(&self) -> bool {
        self.is_asleep
    }

    /// `Plant::PlayBodyReanim`.
    fn play_body_reanim(&self, effects: &mut EffectSystem, track: &str, loop_type: LoopType, blend: i32, rate: f32) {
        if let Some(body) = self.body.and_then(|b| effects.get_mut(b)) {
            body.play(track, loop_type, blend, rate);
        }
    }

    /// `Plant::UpdatePotato`.
    fn update_potato(&mut self, zombies: &DataArray<Zombie>, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem, rng: &mut MtRand) {
        match self.state {
            PlantState::NotReady => {
                if self.state_countdown == 0 {
                    let (x, y) = (self.x + PLANT_SIZE / 2, self.y + PLANT_SIZE / 2);
                    actions.push(PlantAction::Particle { x, y, render_order: self.render_order, effect: ParticleEffect::PotatoMineRise });
                    self.play_body_reanim(effects, "anim_rise", LoopType::PlayOnceAndHold, 20, 18.0);
                    self.state = PlantState::PotatoRising;
                    actions.push(PlantAction::Foley(FoleyType::DirtRise));
                }
            }
            PlantState::PotatoRising => {
                if self.body.and_then(|b| effects.get(b)).is_some_and(|b| b.loop_count > 0) {
                    let rate = rng.range_f32(12.0, 15.0);
                    self.play_body_reanim(effects, "anim_armed", LoopType::Loop, 0, rate);
                    // TODO: the glow that grows as a zombie nears (anim_glow).
                    self.state = PlantState::PotatoArmed;
                    self.blink_countdown = 400 + rng.next_range(4000) as i32;
                }
            }
            PlantState::PotatoArmed if self.find_potato_target(zombies) => self.do_special(actions, effects),
            _ => {}
        }
    }

    /// `Plant::FindTargetZombie` for the chomper: a zombie with a head right
    /// in front of it (further if it's busy biting something).
    fn find_chomper_target(&self, zombies: &DataArray<Zombie>) -> Option<sexy::data_array::Id<Zombie>> {
        let attack = IRect::new(self.x + 80, self.y, 40, PLANT_SIZE);
        let mut best: Option<(i32, sexy::data_array::Id<Zombie>)> = None;
        for (id, zombie) in zombies.iter() {
            if !zombie.in_row(self.row) || !zombie.effected_by_damage(9) || !zombie.has_head {
                continue;
            }
            let extra = if zombie.is_eating || self.state == PlantState::ChomperBiting { 60 } else { 0 };
            let rect = zombie.zombie_rect();
            if rect_overlap(attack, rect) < -extra {
                continue;
            }
            if best.is_none_or(|(w, _)| -rect.x > w) {
                best = Some((-rect.x, id));
            }
        }
        best.map(|(_, id)| id)
    }

    /// `Plant::UpdateChomper`.
    fn update_chomper(&mut self, zombies: &DataArray<Zombie>, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem) {
        let loops = self.body.and_then(|b| effects.get(b)).is_some_and(|b| b.loop_count > 0);
        match self.state {
            PlantState::Ready => {
                if self.find_chomper_target(zombies).is_some() {
                    self.play_body_reanim(effects, "anim_bite", LoopType::PlayOnceAndHold, 20, 24.0);
                    self.state = PlantState::ChomperBiting;
                    self.state_countdown = 70;
                }
            }
            PlantState::ChomperBiting if self.state_countdown == 0 => {
                actions.push(PlantAction::Foley(FoleyType::BigChomp));
                // TODO: gargantuars and bosses are only hurt.
                match self.find_chomper_target(zombies) {
                    // Pole vaulters running or jumping are missed.
                    Some(id) if zombies.get(id).is_some_and(Zombie::is_vaulting) => self.state = PlantState::ChomperBitingMissed,
                    Some(id) => {
                        actions.push(PlantAction::EatZombie(id));
                        self.state = PlantState::ChomperBitingGotOne;
                    }
                    None => self.state = PlantState::ChomperBitingMissed,
                }
            }
            PlantState::ChomperBitingGotOne if loops => {
                self.play_body_reanim(effects, "anim_chew", LoopType::Loop, 0, 15.0);
                self.state = PlantState::ChomperDigesting;
                self.state_countdown = 4000;
            }
            PlantState::ChomperDigesting if self.state_countdown == 0 => {
                self.play_body_reanim(effects, "anim_swallow", LoopType::PlayOnceAndHold, 20, 12.0);
                self.state = PlantState::ChomperSwallowing;
            }
            PlantState::ChomperSwallowing | PlantState::ChomperBitingMissed if loops => {
                let fps = self.body.and_then(|b| effects.get(b)).map_or(12.0, |b| b.definition.fps);
                self.play_body_reanim(effects, "anim_idle", LoopType::Loop, 20, fps);
                self.state = PlantState::Ready;
            }
            _ => {}
        }
    }

    /// `Plant::DoSpecial` for the umbrella leaf: it opens over its neighbours.
    pub fn open_umbrella(&mut self, effects: &mut EffectSystem) {
        if !matches!(self.state, PlantState::UmbrellaTriggered | PlantState::UmbrellaReflecting) {
            self.state = PlantState::UmbrellaTriggered;
            self.state_countdown = 5;
            self.play_body_reanim(effects, "anim_block", LoopType::PlayOnceAndHold, 0, 22.0);
        }
    }

    /// `Plant::UpdateUmbrella`: once open it's drawn over the row below,
    /// and it folds back when the block animation is done.
    fn update_umbrella(&mut self, effects: &mut EffectSystem) {
        // Behaviour per ruslan831 decomp, Plant::UpdateUmbrella.
        match self.state {
            PlantState::UmbrellaTriggered if self.state_countdown == 0 => {
                self.render_order = make_render_order(render_layer::PROJECTILE, self.row + 1, 0);
                self.state = PlantState::UmbrellaReflecting;
            }
            PlantState::UmbrellaReflecting if self.body.and_then(|b| effects.get(b)).is_some_and(|b| b.loop_count > 0) => {
                self.play_body_reanim(effects, "anim_idle", LoopType::Loop, 20, 0.0);
                self.state = PlantState::NotReady;
                self.render_order = self.calc_render_order();
            }
            _ => {}
        }
    }

    /// `Plant::FindTargetZombie` for the potato mine: a zombie on its square
    /// (or biting something next to it).
    fn find_potato_target(&self, zombies: &DataArray<Zombie>) -> bool {
        let attack = IRect::new(self.x, self.y, PLANT_SIZE - 25, PLANT_SIZE);
        zombies.iter().any(|(_, z)| {
            if !z.in_row(self.row) || !z.effected_by_damage(77) || !z.has_head || z.is_vaulting() {
                return false;
            }
            let extra = if z.is_eating { 30 } else { 0 };
            rect_overlap(attack, z.zombie_rect()) >= -extra
        })
    }

    /// `Plant::DoSpecial`, for the instant and mine plants.
    fn do_special(&mut self, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem) {
        let (x, y) = (self.x + PLANT_SIZE / 2, self.y + PLANT_SIZE / 2);
        match self.seed {
            SeedType::Cherrybomb => {
                actions.push(PlantAction::Foley(FoleyType::Cherrybomb));
                actions.push(PlantAction::Foley(FoleyType::Juicy));
                actions.push(PlantAction::CherryBomb { x, y, row: self.row });
                self.die(effects);
            }
            SeedType::Jalapeno => {
                actions.push(PlantAction::Foley(FoleyType::JalapenoIgnite));
                actions.push(PlantAction::Foley(FoleyType::Juicy));
                actions.push(PlantAction::Jalapeno { row: self.row });
                self.die(effects);
            }
            SeedType::Potatomine => {
                actions.push(PlantAction::PotatoMine { x: x - 20, y, row: self.row });
                self.die(effects);
            }
            SeedType::Blover => {
                if self.state != PlantState::DoingSpecial {
                    self.state = PlantState::DoingSpecial;
                    self.state_countdown = 200;
                    actions.push(PlantAction::BlowFog);
                }
            }
            SeedType::Doomshroom => {
                actions.push(PlantAction::DoomShroom { x, y, col: self.col, row: self.row });
                self.die(effects);
            }
            SeedType::InstantCoffee => {
                actions.push(PlantAction::WakeUp { col: self.col, row: self.row });
                self.state = PlantState::DoingSpecial;
                self.play_body_reanim(effects, "anim_crumble", LoopType::PlayOnceAndHold, 20, 22.0);
                actions.push(PlantAction::Foley(FoleyType::Coffee));
            }
            SeedType::Iceshroom => {
                actions.push(PlantAction::Foley(FoleyType::Frozen));
                actions.push(PlantAction::IceShroom { x, y });
                self.die(effects);
            }
            _ => {}
        }
    }

    /// `Plant::GetPlantRect`.
    pub fn rect(&self) -> IRect {
        match self.seed {
            SeedType::Tallnut => IRect::new(self.x + 10, self.y, PLANT_SIZE, PLANT_SIZE),
            SeedType::Pumpkinshell => IRect::new(self.x, self.y, 100, PLANT_SIZE),
            SeedType::Cobcannon => IRect::new(self.x, self.y, 140, 80),
            _ => IRect::new(self.x + 10, self.y, PLANT_SIZE - 20, PLANT_SIZE),
        }
    }

    /// `Plant::GetPlantAttackRect`, for the plants ported so far.
    pub(super) fn attack_rect(&self) -> IRect {
        self.attack_rect_for(Weapon::Primary)
    }

    /// `Plant::GetPlantAttackRect`: a split pea's back head looks left.
    fn attack_rect_for(&self, weapon: Weapon) -> IRect {
        if weapon == Weapon::Secondary && self.seed == SeedType::Splitpea {
            return IRect::new(0, self.y, self.x + 16, PLANT_SIZE);
        }
        if self.bowling {
            return IRect::new(self.x, self.y, PLANT_SIZE - 20, PLANT_SIZE);
        }
        match self.seed {
            SeedType::Cattail => IRect::new(-super::BOARD_WIDTH, -600, super::BOARD_WIDTH * 2, 1200),
            SeedType::Leftpeater => IRect::new(0, self.y, self.x, PLANT_SIZE),
            SeedType::Puffshroom | SeedType::Seashroom => IRect::new(self.x + 60, self.y, 230, PLANT_SIZE),
            SeedType::Fumeshroom => IRect::new(self.x + 60, self.y, 340, PLANT_SIZE),
            SeedType::Gloomshroom => IRect::new(self.x - 80, self.y - 80, 240, 240),
            SeedType::Squash => IRect::new(self.x + 20, self.y, PLANT_SIZE - 35, PLANT_SIZE),
            SeedType::Spikeweed | SeedType::Spikerock => IRect::new(self.x + 20, self.y, PLANT_SIZE - 50, PLANT_SIZE),
            SeedType::Tanglekelp => IRect::new(self.x + 20, self.y, PLANT_SIZE - 50, PLANT_SIZE),
            SeedType::Torchwood => IRect::new(self.x + 50, self.y, 30, PLANT_SIZE),
            _ => IRect::new(self.x + 60, self.y, super::BOARD_WIDTH, PLANT_SIZE),
        }
    }

    /// `Plant::UpdateAbilities`.
    fn update_abilities(
        &mut self,
        zombies: &DataArray<Zombie>,
        actions: &mut Vec<PlantAction>,
        effects: &mut EffectSystem,
        row_y: &[i32; super::MAX_GRID_SIZE_Y],
        open_rows: &[bool; super::MAX_GRID_SIZE_Y],
        rng: &mut MtRand,
    ) {
        if self.dead || self.squished || self.is_asleep || self.on_bungee != OnBungee::No {
            return;
        }
        self.update_shooting(zombies, actions, effects);
        if self.state_countdown > 0 {
            self.state_countdown -= 1;
        }
        if self.bowling {
            self.update_bowling(zombies, actions, effects, row_y, rng);
            return;
        }
        if self.seed == SeedType::Potatomine {
            self.update_potato(zombies, actions, effects, rng);
        }
        if self.seed == SeedType::Chomper {
            self.update_chomper(zombies, actions, effects);
        }
        if self.seed == SeedType::Sunshroom {
            self.update_sun_shroom(actions, effects, rng);
        } else if self.seed.makes_sun() || self.seed == SeedType::Marigold {
            self.update_production_plant(actions, rng);
        }
        if self.seed == SeedType::Gravebuster {
            self.update_grave_buster(actions, effects);
        }
        if self.seed == SeedType::Scaredyshroom {
            self.update_scaredy_shroom(zombies, effects, rng);
        }
        if self.seed == SeedType::Squash {
            self.update_squash(zombies, actions, effects);
        }
        if matches!(self.seed, SeedType::Spikeweed | SeedType::Spikerock) {
            self.update_spikeweed(zombies, actions, effects, rng);
        }
        if self.seed == SeedType::Tanglekelp {
            self.update_tangle_kelp(zombies, actions, effects);
        }
        if self.seed == SeedType::Blover {
            self.update_blover(effects);
        }
        if self.seed == SeedType::Umbrella {
            self.update_umbrella(effects);
        }
        if self.seed == SeedType::Cobcannon {
            self.update_cob_cannon(actions, effects);
        }
        if self.seed == SeedType::Cactus {
            self.update_cactus(zombies, actions, effects);
        }
        if self.seed == SeedType::Imitater {
            self.update_imitater(actions, effects);
        }
        if self.seed == SeedType::Magnetshroom {
            self.update_magnet_shroom(zombies, actions, effects, rng);
        }
        if self.seed == SeedType::GoldMagnet {
            self.update_gold_magnet(actions, effects, rng);
        }
        // UpdateCoffeeBean: it's gone once it has crumbled.
        if self.seed == SeedType::InstantCoffee
            && self.state == PlantState::DoingSpecial
            && self.body.and_then(|b| effects.get(b)).is_some_and(|b| b.loop_count > 0)
        {
            self.die(effects);
        }
        // UpdateIceShroom: it goes off as soon as it's awake.
        if self.seed == SeedType::Iceshroom && self.state != PlantState::DoingSpecial {
            self.state = PlantState::DoingSpecial;
            self.do_special_countdown = 100;
        }
        // UpdateDoomShroom: it swells up first.
        if self.seed == SeedType::Doomshroom && self.state != PlantState::DoingSpecial {
            self.state = PlantState::DoingSpecial;
            self.do_special_countdown = 100;
            if let Some(body) = self.body.and_then(|b| effects.get_mut(b)) {
                body.set_frames_for_layer("anim_explode");
                body.anim_rate = 23.0;
                body.loop_type = LoopType::PlayOnceAndHold;
                // TODO: its heads shake (SetShakeOverride).
            }
            actions.push(PlantAction::Foley(FoleyType::ReverseExplosion));
        }
        if self.seed.definition().sub_class == PlantSubClass::Shooter {
            self.update_shooter(zombies, actions, effects, open_rows, rng);
        }
        if self.do_special_countdown > 0 {
            self.do_special_countdown -= 1;
            if self.do_special_countdown == 0 {
                self.do_special(actions, effects);
            }
        }
        // TODO: the other plants' abilities.
    }

    /// `Plant::UpdateSpikeweed` and `SpikeweedAttack`: the spikes come up
    /// under any zombie standing on it, and hurt it once per attack.
    fn update_spikeweed(
        &mut self,
        zombies: &DataArray<Zombie>,
        actions: &mut Vec<PlantAction>,
        effects: &mut EffectSystem,
        rng: &mut MtRand,
    ) {
        if self.state == PlantState::SpikeweedAttacking {
            let spikes = rock_or_weed_hits(self.seed, self.state_countdown);
            if self.state_countdown == 0 {
                self.state = PlantState::NotReady;
            } else if spikes {
                // 33: `DAMAGE_SPIKE` through the shield.
                let flags = damage::BYPASSES_SHIELD | damage::SPIKE;
                actions.push(PlantAction::RowAreaDamage { col: self.col, row: self.row, spread: 0, rect: self.attack_rect(), damage: 20, flags });
            }
            if self.body.and_then(|b| effects.get(b)).is_some_and(|b| b.loop_count > 0) {
                let rate = rng.range_f32(12.0, 15.0);
                self.play_body_reanim(effects, "anim_idle", LoopType::Loop, 20, rate);
            }
        } else if self.find_target_zombie(zombies).is_some() {
            self.spikeweed_attack(actions, effects);
        }
    }

    /// `Plant::SpikeweedAttack`: the spikes come up.
    fn spikeweed_attack(&mut self, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem) {
        if self.state != PlantState::SpikeweedAttacking {
            self.play_body_reanim(effects, "anim_attack", LoopType::PlayOnceAndHold, 20, 18.0);
            actions.push(PlantAction::Foley(FoleyType::Throw));
            self.state = PlantState::SpikeweedAttacking;
            self.state_countdown = 100;
        }
    }

    /// `Plant::SpikeRockTakeDamage`: whatever hits a spikerock gets spiked
    /// back, and each hit knocks a spike off. Returns whether it's gone.
    pub fn spike_rock_take_damage(&mut self, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem) -> bool {
        self.spikeweed_attack(actions, effects);
        self.health -= 50;
        if let Some(body) = self.body.and_then(|b| effects.get_mut(b)) {
            if self.health <= 300 {
                body.assign_render_group_to_track("bigspike3", RENDER_GROUP_HIDDEN);
            }
            if self.health <= 150 {
                body.assign_render_group_to_track("bigspike2", RENDER_GROUP_HIDDEN);
            }
        }
        if self.health <= 0 {
            actions.push(PlantAction::Foley(FoleyType::Squish));
            self.die(effects);
            return true;
        }
        false
    }

    /// `Plant::UpdateMagnetShroom`: the held item drifts in; an idle magnet
    /// goes for the nearest zombie with metal on it, then needs 15 s to recharge.
    fn update_magnet_shroom(&mut self, zombies: &DataArray<Zombie>, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem, rng: &mut MtRand) {
        for item in &mut self.magnet_items {
            let (dx, dy) = (self.x as f32 + item.dest_x - item.x, self.y as f32 + item.dest_y - item.y);
            if dx.hypot(dy) > 20.0 {
                item.x += dx * 0.05;
                item.y += dy * 0.05;
            }
        }
        match self.state {
            PlantState::MagnetCharging => {
                if self.state_countdown == 0 {
                    self.state = PlantState::Ready;
                    let rate = rng.range_f32(10.0, 15.0);
                    self.play_body_reanim(effects, "anim_idle", LoopType::Loop, 30, rate);
                    self.magnet_items.clear();
                }
            }
            PlantState::MagnetSucking => {
                if self.body.and_then(|b| effects.get(b)).is_some_and(|b| b.loop_count > 0) {
                    self.play_body_reanim(effects, "anim_nonactive_idle2", LoopType::Loop, 20, 2.0);
                    self.state = PlantState::MagnetCharging;
                }
            }
            _ => {
                let mut closest: Option<(f32, sexy::data_array::Id<Zombie>)> = None;
                for (id, zombie) in zombies.iter() {
                    let row_diff = zombie.row - self.row;
                    let rect = zombie.zombie_rect();
                    if !zombie.is_magnet_target() || rect.x > super::BOARD_WIDTH || row_diff.abs() > 2 {
                        continue;
                    }
                    let radius = if zombie.is_eating { 320 } else { 270 };
                    if super::circle_rect_overlap(self.x, self.y + 20, radius, rect) {
                        let distance = ((self.x - rect.x) as f32).hypot((self.y - rect.y) as f32) + row_diff.abs() as f32 * 80.0;
                        if closest.is_none_or(|(d, _)| distance < d) {
                            closest = Some((distance, id));
                        }
                    }
                }
                // TODO: ladders propped against plants are pulled too.
                if let Some((_, zombie)) = closest {
                    self.state = PlantState::MagnetSucking;
                    self.state_countdown = 1500;
                    self.play_body_reanim(effects, "anim_shooting", LoopType::PlayOnceAndHold, 20, 12.0);
                    actions.push(PlantAction::Foley(FoleyType::Magnetshroom));
                    actions.push(PlantAction::MagnetTake { col: self.col, row: self.row, zombie });
                }
            }
        }
    }

    /// `Plant::UpdateGoldMagnetShroom`: coins drift in and are cashed; an idle
    /// magnet now and then reaches for any money lying about, anywhere on the lawn.
    fn update_gold_magnet(&mut self, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem, rng: &mut MtRand) {
        let mut sucking = false;
        let (px, py) = (self.x as f32, self.y as f32);
        self.magnet_items.retain_mut(|item| {
            let (dx, dy) = (px + item.dest_x - item.x, py + item.dest_y - item.y);
            let distance = dx.hypot(dy);
            if distance < 20.0 {
                actions.push(PlantAction::AddMoney { amount: item.value });
                return false;
            }
            let speed = animate_curve_float_time(30.0, 0.0, distance, 0.02, 0.05, TodCurve::Linear);
            item.x += dx * speed;
            item.y += dy * speed;
            sucking = true;
            true
        });
        match self.state {
            PlantState::MagnetCharging => {
                if self.state_countdown == 0 {
                    self.state = PlantState::Ready;
                }
            }
            PlantState::MagnetSucking => {
                let Some(body) = self.body.and_then(|b| effects.get(b)) else { return };
                let (trigger, done) = (body.should_trigger_timed_event(0.4), body.loop_count > 0);
                if trigger {
                    actions.push(PlantAction::Foley(FoleyType::Magnetshroom));
                    actions.push(PlantAction::GoldMagnetCollect { col: self.col, row: self.row });
                }
                if done && !sucking {
                    self.play_body_reanim(effects, "anim_idle", LoopType::Loop, 20, 14.0);
                    self.state = PlantState::MagnetCharging;
                    self.state_countdown = rng.range_int(200, 300);
                }
            }
            _ => {
                // TODO: it waits while another gold magnet is about to suck (`IsAGoldMagnetAboutToSuck`).
                if rng.next_range(50) == 0 && self.coin_available {
                    actions.push(PlantAction::ShowCoinBank);
                    self.state = PlantState::MagnetSucking;
                    self.play_body_reanim(effects, "anim_attract", LoopType::PlayOnceAndHold, 20, 12.0);
                }
            }
        }
    }

    /// Takes what the magnet-shroom pulled (`GetFreeMagnetItem`: it holds one).
    pub fn hold_magnet_item(&mut self, item: MagnetItem) {
        self.magnet_items.clear();
        self.magnet_items.push(item);
    }

    /// A gold magnet holds up to five coins at once (`MAX_MAGNET_ITEMS`).
    pub fn has_room_for_coin(&self) -> bool {
        self.magnet_items.len() < 5
    }

    pub fn add_magnet_coin(&mut self, item: MagnetItem) {
        self.magnet_items.push(item);
    }

    /// `Plant::PlantInitialize` for a cob cannon in play: it starts unarmed.
    pub fn start_cob_cannon(&mut self, effects: &mut EffectSystem) {
        self.state = PlantState::CobcannonArming;
        self.state_countdown = 500;
        if let Some(body) = self.body.and_then(|b| effects.get_mut(b)) {
            body.set_frames_for_layer("anim_unarmed_idle");
        }
    }

    /// `mPlantMaxHealth`.
    pub fn max_health(&self) -> i32 {
        self.max_health
    }

    /// Whether a coffee bean is already waking this plant.
    pub fn is_waking(&self) -> bool {
        self.wake_up_counter > 0
    }

    /// `Plant::DoSpecial` for a coffee bean: the sleeper gets up in a second.
    pub fn start_waking(&mut self) {
        self.wake_up_counter = 100;
    }

    /// Whether the cob cannon is loaded and can be clicked (`STATE_COBCANNON_READY`).
    pub fn is_cob_ready(&self) -> bool {
        self.seed == SeedType::Cobcannon && self.state == PlantState::CobcannonReady && !self.dead
    }

    /// `Plant::UpdateCobCannon`.
    fn update_cob_cannon(&mut self, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem) {
        match self.state {
            PlantState::CobcannonArming if self.state_countdown == 0 => {
                self.state = PlantState::CobcannonLoading;
                self.play_body_reanim(effects, "anim_charge", LoopType::PlayOnceAndHold, 20, 12.0);
            }
            PlantState::CobcannonLoading => {
                let Some(body) = self.body.and_then(|b| effects.get(b)) else { return };
                let (shoop, done) = (body.should_trigger_timed_event(0.5), body.loop_count > 0);
                if shoop {
                    actions.push(PlantAction::Foley(FoleyType::Shoop));
                }
                if done {
                    self.state = PlantState::CobcannonReady;
                    self.play_body_reanim(effects, "anim_idle", LoopType::Loop, 20, 12.0);
                }
            }
            // TODO: the cob flashes while it waits (`GetFlashingColor` on its track).
            PlantState::CobcannonFiring if self.body.and_then(|b| effects.get(b)).is_some_and(|b| b.should_trigger_timed_event(0.48)) => {
                actions.push(PlantAction::Foley(FoleyType::CobLaunch));
            }
            _ => {}
        }
    }

    /// `Plant::CobCannonFire`: the cannon aims at a point on the lawn and
    /// launches its cob a few seconds later.
    pub fn cob_cannon_fire(&mut self, target_x: i32, target_y: i32, effects: &mut EffectSystem) {
        if self.state != PlantState::CobcannonReady {
            return;
        }
        self.state = PlantState::CobcannonFiring;
        self.shooting_counter = 206;
        self.play_body_reanim(effects, "anim_shooting", LoopType::PlayOnceAndHold, 20, 12.0);
        self.target_x = target_x - 47;
        self.target_y = target_y;
    }

    /// `Plant::UpdateBlover`: it blows once, then spins on its stalk until it fades.
    fn update_blover(&mut self, effects: &mut EffectSystem) {
        if let Some(body) = self.body.and_then(|b| effects.get_mut(b))
            && body.loop_count > 0
            && body.loop_type != LoopType::Loop
        {
            body.set_frames_for_layer("anim_loop");
            body.loop_type = LoopType::Loop;
        }
        if self.state == PlantState::DoingSpecial && self.state_countdown == 0 {
            self.die(effects);
        }
    }

    /// `Plant::UpdateTanglekelp`: a zombie stepping onto its square is grabbed,
    /// dragged under, and the kelp goes with it.
    fn update_tangle_kelp(&mut self, zombies: &DataArray<Zombie>, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem) {
        if self.state == PlantState::TanglekelpGrabbing {
            if self.state_countdown == 50
                && let Some(id) = self.target_zombie
            {
                actions.push(PlantAction::TangleKelp(id));
            }
            if self.state_countdown == 0 {
                self.die(effects);
            }
        } else if let Some(id) = self.find_target_zombie(zombies) {
            self.target_zombie = Some(id);
            self.state = PlantState::TanglekelpGrabbing;
            self.state_countdown = 100;
            self.play_body_reanim(effects, "anim_grab", LoopType::PlayOnceAndHold, 0, 24.0);
        }
    }

    /// `Plant::GetDamageRangeFlags` for the squash.
    const SQUASH_DAMAGE_RANGE: u32 = 13;

    /// `Plant::FindSquashTarget`: the closest zombie in its row that it can
    /// jump on, sticking with the one it's already after.
    fn find_squash_target(&self, zombies: &DataArray<Zombie>) -> Option<sexy::data_array::Id<Zombie>> {
        use super::zombie::ZombiePhase;
        use crate::zombie::ZombieType;
        let attack = self.attack_rect();
        let mut closest: Option<(i32, sexy::data_array::Id<Zombie>)> = None;
        for (id, zombie) in zombies.iter() {
            if !zombie.in_row(self.row) || !zombie.has_head || !zombie.effected_by_damage(Self::SQUASH_DAMAGE_RANGE) {
                continue;
            }
            let rect = zombie.zombie_rect();
            let pre_vault = zombie.phase == ZombiePhase::PolePreVault;
            if (pre_vault && rect.x >= self.x + 20) || zombie.phase == ZombiePhase::PoleInVault {
                continue;
            }
            let range = -rect_overlap(attack, rect);
            if range > if zombie.is_eating { 110 } else { 70 } {
                continue;
            }
            // Zombies that run or lunge are met earlier.
            let ahead = pre_vault || zombie.zombie_type == ZombieType::Polevaulter || zombie.zombie_type == ZombieType::Football;
            let plant_x = if ahead { attack.x - 60 } else { attack.x };
            if !(zombie.is_walking_backwards() || rect.x + rect.w >= plant_x) {
                continue;
            }
            if self.target_zombie == Some(id) {
                return Some(id);
            }
            if closest.is_none_or(|(r, _)| range < r) {
                closest = Some((range, id));
            }
        }
        closest.map(|(_, id)| id)
    }

    /// `Plant::UpdateSquash`.
    fn update_squash(&mut self, zombies: &DataArray<Zombie>, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem) {
        let lead_x = |zombies: &DataArray<Zombie>, id: Option<sexy::data_array::Id<Zombie>>, time: f32| {
            id.and_then(|id| zombies.get(id)).map(|z| z.target_lead_x(time) as i32 - PLANT_SIZE / 2)
        };
        let (home_x, home_y) = self.home;
        // The square it lands in, and where on the lawn its feet go.
        let dest_y = home_y + 8;
        match self.state {
            PlantState::NotReady => {
                if let Some(id) = self.find_squash_target(zombies) {
                    self.target_zombie = Some(id);
                    self.target_x = lead_x(zombies, Some(id), 0.0).unwrap_or(self.x);
                    self.state = PlantState::SquashLook;
                    self.state_countdown = 80;
                    let track = if self.target_x < self.x { "anim_lookleft" } else { "anim_lookright" };
                    self.play_body_reanim(effects, track, LoopType::PlayOnceAndHold, 10, 24.0);
                    actions.push(PlantAction::Foley(FoleyType::SquashHmm));
                }
            }
            PlantState::SquashLook if self.state_countdown <= 0 => {
                self.play_body_reanim(effects, "anim_jumpup", LoopType::PlayOnceAndHold, 20, 24.0);
                self.state = PlantState::SquashPreLaunch;
                self.state_countdown = 45;
            }
            PlantState::SquashPreLaunch if self.state_countdown <= 0 => {
                // Aim a little ahead of a moving zombie.
                let target = self.find_squash_target(zombies);
                if let Some(x) = lead_x(zombies, target, 30.0) {
                    self.target_x = x;
                }
                self.state = PlantState::SquashRising;
                self.state_countdown = 50;
                self.render_order = make_render_order(render_layer::PARTICLE, self.row, 0);
            }
            PlantState::SquashRising => {
                self.x = animate_curve(50, 20, self.state_countdown, home_x, self.target_x, TodCurve::EaseInOut);
                self.y = animate_curve(50, 20, self.state_countdown, home_y, dest_y - 120, TodCurve::EaseInOut);
                if self.state_countdown == 0 {
                    self.play_body_reanim(effects, "anim_jumpdown", LoopType::PlayOnceAndHold, 0, 60.0);
                    self.state = PlantState::SquashFalling;
                    self.state_countdown = 10;
                }
            }
            PlantState::SquashFalling => {
                self.y = animate_curve(10, 0, self.state_countdown, dest_y - 120, dest_y, TodCurve::Linear);
                if self.state_countdown == 5 {
                    actions.push(PlantAction::Squash { row: self.row, rect: self.attack_rect() });
                }
                if self.state_countdown == 0 {
                    self.state = PlantState::SquashDoneFalling;
                    self.state_countdown = 100;
                    actions.push(PlantAction::ShakeBoard { x: 1, y: 4 });
                    actions.push(PlantAction::Foley(FoleyType::Thump));
                    let render_order = self.render_order + 4;
                    actions.push(PlantAction::Particle { x: self.x + 40, y: self.y + 80, render_order, effect: ParticleEffect::DustSquash });
                }
            }
            PlantState::SquashDoneFalling if self.state_countdown == 0 => self.die(effects),
            _ => {}
        }
    }

    /// `Plant::UpdateBowling`: a nut rolls right along its row, flattening
    /// what it hits and bouncing off up or down the lawn after the first hit.
    fn update_bowling(
        &mut self,
        zombies: &DataArray<Zombie>,
        actions: &mut Vec<PlantAction>,
        effects: &mut EffectSystem,
        row_y: &[i32; super::MAX_GRID_SIZE_Y],
        rng: &mut MtRand,
    ) {
        if let Some(body) = self.body.and_then(|b| effects.get(b))
            && body.definition.find_track("_ground").is_some()
        {
            let mut speed = body.track_velocity("_ground");
            if self.seed == SeedType::GiantWallnut {
                speed *= 2.0;
            }
            self.x = (self.x as f32 - speed) as i32;
            if self.x > 800 {
                self.die(effects);
                return;
            }
        }
        match self.state {
            PlantState::BowlingUp => self.y -= 2,
            PlantState::BowlingDown => self.y += 2,
            _ => {}
        }
        if (row_y[self.row as usize] - self.y).abs() > 2 {
            return;
        }

        let mut new_state = self.state;
        if self.state == PlantState::BowlingUp && self.row <= 0 {
            new_state = PlantState::BowlingDown;
        } else if self.state == PlantState::BowlingDown && self.row >= 4 {
            new_state = PlantState::BowlingUp;
        }
        if let Some(target) = self.find_target_zombie(zombies) {
            if self.seed == SeedType::ExplodeONut {
                actions.push(PlantAction::ExplodeONut { x: self.x + PLANT_SIZE / 2, y: self.y + PLANT_SIZE / 2, row: self.row });
                self.die(effects);
                return;
            }
            let giant = self.seed == SeedType::GiantWallnut;
            actions.push(PlantAction::BowlingHit { zombie: target, giant, bouncing: self.state != PlantState::NotReady });
            // Behaviour per ruslan831 decomp, Plant::UpdateBowling: past level
            // 10 a wall-nut pays out more the more zombies it flattens.
            if self.bowling_pays && self.seed == SeedType::Wallnut {
                self.launch_counter += 1;
                let (x, y) = (self.x + PLANT_SIZE / 2, self.y + PLANT_SIZE / 2);
                let coins: &[(i32, bool)] = match self.launch_counter {
                    2 => &[(0, false)],
                    3 => &[(-5, false), (5, false)],
                    4 => &[(-10, false), (0, false), (10, false)],
                    n if n >= 5 => &[(0, true)],
                    _ => &[],
                };
                for &(dx, gold) in coins {
                    actions.push(PlantAction::MakeCoin { x: x + dx, y, gold });
                }
            }
            // A giant wall-nut rolls straight on; the others bounce to another lane.
            if !giant {
                new_state = if self.row == 4 || self.state == PlantState::BowlingDown {
                    PlantState::BowlingUp
                } else if self.row == 0 || self.state == PlantState::BowlingUp {
                    PlantState::BowlingDown
                } else if rng.next_range(2) != 0 {
                    PlantState::BowlingUp
                } else {
                    PlantState::BowlingDown
                };
            }
        }
        match new_state {
            PlantState::BowlingUp => {
                self.row -= 1;
                self.state = new_state;
                self.render_order = self.calc_render_order();
            }
            PlantState::BowlingDown => {
                self.state = new_state;
                self.render_order = self.calc_render_order();
                self.row += 1;
            }
            _ => {}
        }
    }

    /// `Plant::UpdateProductionPlant`, for sunflowers and sun-shrooms: the
    /// plant glows brighter for a second, then makes sun.
    fn update_production_plant(&mut self, actions: &mut Vec<PlantAction>, rng: &mut MtRand) {
        // Behaviour per ruslan831 decomp, Plant::UpdateProductionPlant: a
        // marigold only keeps going for a minute once the last wave is out.
        if self.seed == SeedType::Marigold && self.last_wave {
            if self.state != PlantState::MarigoldEnding {
                self.state = PlantState::MarigoldEnding;
                self.state_countdown = 6000;
            } else if self.state_countdown <= 0 {
                return;
            }
        }
        self.launch_counter -= 1;
        if self.launch_counter <= 100 {
            let flash = animate_curve(100, 0, self.launch_counter, 0, 100, TodCurve::Linear);
            self.eaten_flash_countdown = self.eaten_flash_countdown.max(flash);
        }
        if self.launch_counter <= 0 {
            self.launch_counter = rng.range_int(self.launch_rate - 150, self.launch_rate);
            if self.seed == SeedType::Marigold {
                actions.push(PlantAction::MakeCoin { x: self.x, y: self.y, gold: rng.next_range(100) < 10 });
                return;
            }
            let small = self.state == PlantState::SunshroomSmall;
            actions.push(PlantAction::MakeSun { x: self.x, y: self.y, small });
            // A twin sunflower makes two.
            if self.seed == SeedType::Twinsunflower {
                actions.push(PlantAction::MakeSun { x: self.x, y: self.y, small });
            }
        }
    }

    /// `Plant::UpdateSunShroom`: small sun-shrooms make small sun, and grow
    /// up after two minutes.
    fn update_sun_shroom(&mut self, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem, rng: &mut MtRand) {
        match self.state {
            PlantState::SunshroomSmall => {
                if self.state_countdown == 0 {
                    self.play_body_reanim(effects, "anim_grow", LoopType::PlayOnceAndHold, 10, 12.0);
                    self.state = PlantState::SunshroomGrowing;
                    actions.push(PlantAction::Foley(FoleyType::PlantGrow));
                }
                self.update_production_plant(actions, rng);
            }
            PlantState::SunshroomGrowing => {
                if self.body.and_then(|b| effects.get(b)).is_some_and(|b| b.loop_count > 0) {
                    let rate = rng.range_f32(12.0, 15.0);
                    self.play_body_reanim(effects, "anim_bigidle", LoopType::Loop, 10, rate);
                    self.state = PlantState::SunshroomBig;
                }
            }
            _ => self.update_production_plant(actions, rng),
        }
    }

    /// `Plant::UpdateScaredyShroom`: it ducks while a zombie is close by
    /// (and doesn't shoot), and comes back up once they've gone.
    fn update_scaredy_shroom(&mut self, zombies: &DataArray<Zombie>, effects: &mut EffectSystem, rng: &mut MtRand) {
        if self.shooting_counter > 0 {
            return;
        }
        let nearby = zombies.iter().any(|(_, z)| {
            (z.is_boss() || (z.row - self.row).abs() <= 1)
                && !z.is_dead_or_dying()
                && !z.mind_controlled
                && super::circle_rect_overlap(self.x, self.y + 20, 120, z.zombie_rect())
        });
        let loops = self.body.and_then(|b| effects.get(b)).is_some_and(|b| b.loop_count > 0);
        match self.state {
            PlantState::Ready if nearby => {
                self.state = PlantState::ScaredyshroomLowering;
                self.play_body_reanim(effects, "anim_scared", LoopType::PlayOnceAndHold, 10, 10.0);
            }
            PlantState::ScaredyshroomLowering if loops => {
                self.state = PlantState::ScaredyshroomScared;
                self.play_body_reanim(effects, "anim_scaredidle", LoopType::Loop, 10, 0.0);
            }
            PlantState::ScaredyshroomScared if !nearby => {
                self.state = PlantState::ScaredyshroomRaising;
                let rate = rng.range_f32(7.0, 12.0);
                self.play_body_reanim(effects, "anim_grow", LoopType::PlayOnceAndHold, 10, rate);
            }
            PlantState::ScaredyshroomRaising if loops => {
                self.state = PlantState::Ready;
                let rate = rng.range_f32(10.0, 15.0);
                self.play_body_reanim(effects, "anim_idle", LoopType::Loop, 20, rate);
            }
            _ => {}
        }
        if self.state != PlantState::Ready {
            self.launch_counter = self.launch_rate;
        }
    }

    /// `Plant::UpdateGraveBuster`: it lands on the grave, chews it for four
    /// seconds, then the grave and the grave buster are gone.
    fn update_grave_buster(&mut self, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem) {
        match self.state {
            PlantState::GravebusterLanding => {
                if self.body.and_then(|b| effects.get(b)).is_some_and(|b| b.loop_count > 0) {
                    self.play_body_reanim(effects, "anim_idle", LoopType::Loop, 10, 12.0);
                    self.state_countdown = 400;
                    self.state = PlantState::GravebusterEating;
                    // TODO: the original attaches this to the plant (AddAttachedParticle).
                    actions.push(PlantAction::Particle {
                        x: self.x + 40,
                        y: self.y + 40,
                        render_order: self.render_order + 4,
                        effect: ParticleEffect::GraveBuster,
                    });
                }
            }
            PlantState::GravebusterEating if self.state_countdown == 0 => {
                actions.push(PlantAction::GraveBusterDone {
                    x: self.x,
                    y: self.y,
                    col: self.col,
                    row: self.row,
                    render_order: self.render_order + 4,
                });
                self.die(effects);
            }
            _ => {}
        }
    }

    /// While a grave buster chews its grave, how long it has left; the
    /// grave sinks as it goes (`GridItem::DrawGraveStone`).
    pub(super) fn grave_buster_eaten_countdown(&self) -> Option<i32> {
        (self.state == PlantState::GravebusterEating).then_some(self.state_countdown)
    }

    /// `Plant::UpdateShooter`.
    fn update_shooter(
        &mut self,
        zombies: &DataArray<Zombie>,
        actions: &mut Vec<PlantAction>,
        effects: &mut EffectSystem,
        open_rows: &[bool; super::MAX_GRID_SIZE_Y],
        rng: &mut MtRand,
    ) {
        self.launch_counter -= 1;
        if self.launch_counter <= 0 {
            self.launch_counter = self.launch_rate - rng.next_range(15) as i32;
            if self.seed == SeedType::Threepeater {
                self.launch_threepeater(zombies, effects, open_rows);
            } else if self.seed == SeedType::Starfruit {
                self.launch_starfruit(zombies, effects);
            } else if self.seed == SeedType::Cattail {
                self.find_target_and_fire_weapon(zombies, Weapon::Primary, effects);
            } else if self.seed == SeedType::Splitpea {
                self.find_target_and_fire_weapon(zombies, Weapon::Secondary, effects);
            } else if self.seed == SeedType::Cactus {
                // A raised cactus shoots flyers, a lowered one walkers.
                match self.state {
                    PlantState::CactusHigh => self.find_target_and_fire_weapon(zombies, Weapon::Primary, effects),
                    PlantState::CactusLow => self.find_target_and_fire_weapon(zombies, Weapon::Secondary, effects),
                    _ => {}
                }
            } else if is_pea_shooter(self.seed)
                || matches!(
                    self.seed,
                    SeedType::Puffshroom
                        | SeedType::Seashroom
                        | SeedType::Fumeshroom
                        | SeedType::Gloomshroom
                        | SeedType::Scaredyshroom
                        | SeedType::Cabbagepult
                        | SeedType::Kernelpult
                        | SeedType::Melonpult
                        | SeedType::Wintermelon
                )
            {
                self.find_target_and_fire(zombies, actions, effects, rng);
            }
        }
        if self.launch_counter == 50 && self.seed == SeedType::Cattail {
            self.find_target_and_fire_weapon(zombies, Weapon::Primary, effects);
        }
        if self.launch_counter == 25 && self.seed == SeedType::Repeater {
            self.find_target_and_fire(zombies, actions, effects, rng);
        }
        if self.launch_counter == 25 && self.seed == SeedType::Splitpea {
            self.find_target_and_fire_weapon(zombies, Weapon::Primary, effects);
            self.find_target_and_fire_weapon(zombies, Weapon::Secondary, effects);
        }
    }

    /// `Plant::FindTargetZombie`, for peashooters: the leftmost zombie in
    /// the row that's in range.
    /// The zombie this tangle kelp is dragging under, if it has one
    /// (`IsTangleKelpTarget` looks at `mTargetZombieID` of the kelp plants).
    pub fn tangle_kelp_target(&self) -> Option<sexy::data_array::Id<Zombie>> {
        if self.seed == SeedType::Tanglekelp && !self.dead { self.target_zombie } else { None }
    }

    fn find_target_zombie(&self, zombies: &DataArray<Zombie>) -> Option<sexy::data_array::Id<Zombie>> {
        // A gloom-shroom's cloud reaches the rows beside its own (`FindTargetZombie`).
        if self.seed == SeedType::Gloomshroom {
            return (self.row - 1..=self.row + 1).find_map(|row| self.find_target_zombie_in_row(zombies, row));
        }
        self.find_target_zombie_in_row(zombies, self.row)
    }

    /// What this plant's shots can reach (`GetDamageRangeFlags`): the catapult
    /// plants' lobs come down on swimmers too.
    fn target_flags(&self) -> u32 {
        self.target_flags_for(Weapon::Primary)
    }

    /// `GetDamageRangeFlags`: a raised cactus shoots only flyers, a lowered one only walkers.
    fn target_flags_for(&self, weapon: Weapon) -> u32 {
        match self.seed {
            // Cattails reach walkers and flyers alike (`GetDamageRangeFlags`: 11).
            SeedType::Cattail => damage_range::GROUND | damage_range::FLYING | 8,
            SeedType::Cactus => if weapon == Weapon::Secondary { damage_range::GROUND } else { damage_range::FLYING },
            SeedType::Cabbagepult | SeedType::Kernelpult | SeedType::Melonpult | SeedType::Wintermelon => damage_range::GROUND | damage_range::SUBMERGED,
            _ => damage_range::GROUND,
        }
    }

    /// The zombie a catapult plant throws at, if it is one.
    fn lob_target(&self, zombies: &DataArray<Zombie>) -> Option<LobTarget> {
        if !matches!(self.seed, SeedType::Cabbagepult | SeedType::Kernelpult | SeedType::Melonpult | SeedType::Wintermelon) {
            return None;
        }
        let zombie = self.find_target_zombie(zombies).and_then(|id| zombies.get(id))?;
        Some(LobTarget {
            lead_x: zombie.target_lead_x(50.0),
            top: zombie.zombie_rect().y,
            boss: zombie.is_boss(),
            riding_dolphin: zombie.phase == super::zombie::ZombiePhase::DolphinRiding,
            swimming_snorkel: zombie.phase == super::zombie::ZombiePhase::SnorkelWalkingInPool,
        })
    }

    /// `Plant::FindTargetZombie` for any `row` (the threepeater looks at three).
    fn find_target_zombie_in_row(&self, zombies: &DataArray<Zombie>, row: i32) -> Option<sexy::data_array::Id<Zombie>> {
        self.find_target_zombie_for(zombies, row, Weapon::Primary)
    }

    /// `Plant::FindTargetZombie` for one of its weapons.
    fn find_target_zombie_for(&self, zombies: &DataArray<Zombie>, row: i32, weapon: Weapon) -> Option<sexy::data_array::Id<Zombie>> {
        // Behaviour per ruslan831 decomp, Plant::FindTargetZombie: a cattail
        // picks the nearest zombie anywhere, flyers first.
        if self.seed == SeedType::Cattail {
            let flags = self.target_flags();
            let (cx, cy) = ((self.x + 40) as f32, (self.y + 40) as f32);
            return zombies
                .iter()
                .filter(|(_, z)| z.effected_by_damage(flags) && z.has_head)
                .map(|(id, z)| {
                    let rect = z.zombie_rect();
                    let distance = (cx - (rect.x + rect.w / 2) as f32).hypot(cy - (rect.y + rect.h / 2) as f32);
                    (id, -distance + if z.is_flying() { 10000.0 } else { 0.0 })
                })
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(id, _)| id);
        }
        let attack = self.attack_rect_for(weapon);
        let mut best: Option<(i32, sexy::data_array::Id<Zombie>)> = None;
        for (id, zombie) in zombies.iter() {
            if !zombie.in_row(row) || !zombie.effected_by_damage(self.target_flags_for(weapon)) {
                continue;
            }
            if self.seed == SeedType::ExplodeONut && zombie.phase == super::zombie::ZombiePhase::PoleInVault {
                continue;
            }
            let rect = zombie.zombie_rect();
            if rect_overlap(attack, rect) < 0 {
                continue;
            }
            let weight = -rect.x;
            if best.is_none_or(|(w, _)| weight > w) {
                best = Some((weight, id));
            }
        }
        best.map(|(_, id)| id)
    }

    /// `Plant::LaunchThreepeater`: the heads that have a row to shoot into
    /// lean forward when a zombie is in any of the three rows.
    fn launch_threepeater(&mut self, zombies: &DataArray<Zombie>, effects: &mut EffectSystem, open_rows: &[bool; super::MAX_GRID_SIZE_Y]) {
        let open = |row: i32| (0..super::MAX_GRID_SIZE_Y as i32).contains(&row) && open_rows[row as usize];
        let (above, below) = (self.row - 1, self.row + 1);
        let sees = |row: i32| self.find_target_zombie_in_row(zombies, row).is_some();
        if !(sees(self.row) || (open(above) && sees(above)) || (open(below) && sees(below))) {
            return;
        }
        // Head 1 shoots the row below, head 2 its own, head 3 the row above.
        let heads = [(self.head, open(below), "anim_shooting1"), (self.extra_heads[0], true, "anim_shooting2"), (self.extra_heads[1], open(above), "anim_shooting3")];
        for (head, shoots, track) in heads {
            if shoots && let Some(head) = head.and_then(|h| effects.get_mut(h)) {
                head.start_blend(10);
                head.loop_type = LoopType::PlayOnceAndHold;
                head.anim_rate = 20.0;
                head.set_frames_for_layer(track);
            }
        }
        self.shooting_counter = 35;
    }

    /// The threepeater's shot, from each head that leaned forward.
    fn fire_threepeater(&mut self, actions: &mut Vec<PlantAction>, effects: &EffectSystem) {
        let shooting = |head: Option<ReanimationId>| {
            head.and_then(|h| effects.get(h)).is_some_and(|h| h.loop_type == LoopType::PlayOnceAndHold)
        };
        let rows = [(self.head, self.row + 1), (self.extra_heads[0], self.row), (self.extra_heads[1], self.row - 1)];
        for (head, row) in rows {
            if shooting(head) {
                self.fire(row, None, actions, effects);
            }
        }
    }

    /// `Plant::FindTargetAndFire` for the plants with two weapons.
    fn find_target_and_fire_weapon(&mut self, zombies: &DataArray<Zombie>, weapon: Weapon, effects: &mut EffectSystem) {
        if self.find_target_zombie_for(zombies, self.row, weapon).is_none() {
            return;
        }
        self.end_blink(effects);
        match (self.seed, weapon) {
            (SeedType::Splitpea, Weapon::Secondary) => {
                if let Some(head) = self.extra_heads[0].and_then(|h| effects.get_mut(h)) {
                    head.start_blend(20);
                    head.loop_type = LoopType::PlayOnceAndHold;
                    head.anim_rate = 35.0;
                    head.set_frames_for_layer("anim_splitpea_shooting");
                }
                self.shooting_counter = 26;
            }
            (SeedType::Splitpea, Weapon::Primary) => {
                if let Some(head) = self.head.and_then(|h| effects.get_mut(h)) {
                    head.start_blend(20);
                    head.loop_type = LoopType::PlayOnceAndHold;
                    head.anim_rate = 45.0;
                    head.set_frames_for_layer("anim_shooting");
                }
                self.shooting_counter = 26;
            }
            (SeedType::Cattail, _) => {
                self.play_body_reanim(effects, "anim_shooting", LoopType::PlayOnceAndHold, 20, 30.0);
                self.shooting_counter = 50;
            }
            (SeedType::Cactus, Weapon::Primary) => {
                self.play_body_reanim(effects, "anim_shootinghigh", LoopType::PlayOnceAndHold, 20, 35.0);
                self.shooting_counter = 23;
            }
            _ => {
                self.play_body_reanim(effects, "anim_shooting", LoopType::PlayOnceAndHold, 20, 35.0);
                self.shooting_counter = 35;
            }
        }
    }

    /// `Plant::FindStarFruitTarget`: whether any zombie is in line with one
    /// of the five directions the stars fly in.
    fn find_starfruit_target(&self, zombies: &DataArray<Zombie>) -> bool {
        if self.recently_eaten_countdown > 0 {
            return true;
        }
        let (center_x, center_y) = (self.x + 40, self.y + 40);
        zombies.iter().any(|(_, zombie)| {
            if !zombie.effected_by_damage(self.target_flags()) {
                return false;
            }
            let mut rect = zombie.zombie_rect();
            if zombie.is_boss() && self.col >= 5 {
                return true;
            }
            if zombie.row == self.row {
                return rect.x + rect.w < center_x;
            }
            if zombie.zombie_type == crate::zombie::ZombieType::Digger {
                rect.w += 10;
            }
            let time = ((center_x - (rect.x + rect.w / 2)) as f32).hypot((center_y - (rect.y + rect.h / 2)) as f32) / 3.33;
            let hit_x = zombie.target_lead_x(time) as i32 - rect.w / 2;
            if hit_x + rect.w > center_x && hit_x < center_x {
                return true;
            }
            let (to_x, to_y) = ((hit_x + rect.w / 2 - center_x) as f32, (rect.y + rect.h / 2 - center_y) as f32);
            let angle = to_y.atan2(to_x).to_degrees();
            if (zombie.row - self.row).abs() < 2 {
                (angle > 20.0 && angle < 40.0) || (angle < -25.0 && angle > -45.0)
            } else {
                (angle > 25.0 && angle < 35.0) || (angle < -28.0 && angle > -38.0)
            }
        })
    }

    /// `Plant::LaunchStarFruit`.
    fn launch_starfruit(&mut self, zombies: &DataArray<Zombie>, effects: &mut EffectSystem) {
        if self.find_starfruit_target(zombies) {
            self.play_body_reanim(effects, "anim_shoot", LoopType::PlayOnceAndHold, 20, 28.0);
            self.shooting_counter = 40;
        }
    }

    /// The plant this imitater will turn into.
    pub fn set_imitating(&mut self, seed: SeedType) {
        self.imitater_type = Some(seed);
        // The imitater stands where its copy will (`CalcRenderOrder`).
        self.render_order = self.calc_render_order();
    }

    /// `Plant::UpdateImitater`: a moment after it's planted it puffs into a
    /// copy of the plant it imitates.
    fn update_imitater(&mut self, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem) {
        if self.state != PlantState::ImitaterMorphing {
            if self.state_countdown == 0 {
                self.state = PlantState::ImitaterMorphing;
                self.play_body_reanim(effects, "anim_explode", LoopType::PlayOnceAndHold, 0, 26.0);
            }
            return;
        }
        let Some(body) = self.body.and_then(|b| effects.get(b)) else { return };
        if body.should_trigger_timed_event(0.8) {
            actions.push(PlantAction::Particle { x: self.x + 40, y: self.y + 40, render_order: render_layer::TOP, effect: ParticleEffect::ImitaterMorph });
        }
        if body.loop_count > 0
            && let Some(seed) = self.imitater_type
        {
            actions.push(PlantAction::ImitaterMorph { col: self.col, row: self.row, seed });
            self.die(effects);
        }
    }

    /// `Plant::ImitaterMorph`'s filter: the copy looks washed out.
    pub fn wash_out(&mut self, effects: &mut EffectSystem) {
        // Behaviour per ruslan831 decomp, Plant::ImitaterMorph: a few plants are washed out less.
        let tint = if matches!(self.seed, SeedType::Hypnoshroom | SeedType::Squash | SeedType::Potatomine | SeedType::Garlic | SeedType::Lilypad) {
            [225, 225, 225, 255]
        } else {
            [185, 185, 185, 255]
        };
        for id in [self.body, self.head, self.extra_heads[0], self.extra_heads[1]].into_iter().flatten() {
            if let Some(reanim) = effects.get_mut(id) {
                reanim.color_override = tint;
            }
        }
    }

    /// `Plant::UpdateCactus`: it rises when a flyer's in its row and lowers
    /// again when the flyer's gone.
    fn update_cactus(&mut self, zombies: &DataArray<Zombie>, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem) {
        if self.shooting_counter > 0 {
            return;
        }
        let Some(body) = self.body.and_then(|b| effects.get(b)) else { return };
        let (done, fps) = (body.loop_count > 0, body.definition.fps);
        let flyer = self.find_target_zombie_for(zombies, self.row, Weapon::Primary).is_some();
        match self.state {
            PlantState::CactusRising => {
                if done {
                    self.state = PlantState::CactusHigh;
                    self.play_body_reanim(effects, "anim_idlehigh", LoopType::Loop, 20, 0.0);
                }
            }
            PlantState::CactusHigh => {
                if !flyer {
                    self.state = PlantState::CactusLowering;
                    self.play_body_reanim(effects, "anim_lower", LoopType::PlayOnceAndHold, 20, fps);
                }
            }
            PlantState::CactusLowering => {
                if done {
                    self.state = PlantState::CactusLow;
                    self.play_body_reanim(effects, "anim_idle", LoopType::Loop, 20, 0.0);
                }
            }
            _ => {
                if flyer {
                    self.state = PlantState::CactusRising;
                    self.play_body_reanim(effects, "anim_rise", LoopType::PlayOnceAndHold, 20, fps);
                    actions.push(PlantAction::Foley(FoleyType::PlantGrow));
                }
            }
        }
    }

    /// `Plant::FindTargetAndFire`: starts the shooting animation if there's
    /// something to shoot at.
    fn find_target_and_fire(&mut self, zombies: &DataArray<Zombie>, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem, rng: &mut MtRand) {
        if self.find_target_zombie(zombies).is_none() {
            return;
        }
        self.end_blink(effects);
        let head = self.head.and_then(|h| effects.get_mut(h)).filter(|h| h.definition.find_track("anim_shooting").is_some());
        if let Some(head) = head {
            head.start_blend(20);
            head.loop_type = LoopType::PlayOnceAndHold;
            head.anim_rate = 35.0;
            head.set_frames_for_layer("anim_shooting");
            self.shooting_counter = 35;
            if self.seed == SeedType::Repeater {
                head.anim_rate = 45.0;
                self.shooting_counter = 26;
            } else if self.seed == SeedType::Gatlingpea {
                head.anim_rate = 38.0;
                self.shooting_counter = 100;
            }
        } else if self.body.and_then(|b| effects.get(b)).is_some_and(|b| b.definition.find_track("anim_shooting").is_some()) {
            // Mushrooms shoot with their whole body.
            let rate = if self.seed == SeedType::Gloomshroom { 14.0 } else { 35.0 };
            self.play_body_reanim(effects, "anim_shooting", LoopType::PlayOnceAndHold, 20, rate);
            // TODO: the other body shooters' timings (cactus 35, ...).
            self.shooting_counter = match self.seed {
                SeedType::Fumeshroom => 50,
                SeedType::Gloomshroom => 200,
                SeedType::Scaredyshroom => 25,
                SeedType::Cabbagepult => 32,
                SeedType::Melonpult | SeedType::Wintermelon => 36,
                SeedType::Kernelpult => {
                    // One throw in four is a pat of butter.
                    if rng.next_range(4) == 0 {
                        self.state = PlantState::KernelButter;
                        if let Some(body) = self.body.and_then(|b| effects.get_mut(b)) {
                            body.assign_render_group_to_prefix("Cornpult_butter", RENDER_GROUP_NORMAL);
                            body.assign_render_group_to_prefix("Cornpult_kernal", RENDER_GROUP_HIDDEN);
                        }
                    }
                    30
                }
                _ => 29,
            };
        } else {
            self.fire(self.row, None, actions, effects);
        }
    }

    /// `Plant::UpdateShooting`: fires when the animation reaches the shot,
    /// then goes back to idle once it's done.
    fn update_shooting(&mut self, zombies: &DataArray<Zombie>, actions: &mut Vec<PlantAction>, effects: &mut EffectSystem) {
        if self.shooting_counter == 0 {
            return;
        }
        self.shooting_counter -= 1;
        // The fumes billow out a moment before they hurt.
        if self.seed == SeedType::Fumeshroom && self.shooting_counter == 15 {
            let order = make_render_order(render_layer::PARTICLE, self.row, 0);
            actions.push(PlantAction::Particle { x: self.x + 85, y: self.y + 31, render_order: order, effect: ParticleEffect::Fumecloud });
        }
        // The gloom-shroom billows four clouds, each hurting a moment later.
        if self.seed == SeedType::Gloomshroom {
            if matches!(self.shooting_counter, 136 | 108 | 80 | 52) {
                let order = make_render_order(render_layer::PARTICLE, self.row, 0);
                actions.push(PlantAction::Particle { x: self.x + 40, y: self.y + 40, render_order: order, effect: ParticleEffect::Gloomcloud });
            }
            if matches!(self.shooting_counter, 126 | 98 | 70 | 42) {
                self.fire(self.row, None, actions, effects);
            }
        } else if self.seed == SeedType::Cattail {
            // The spike leaves a moment into the animation, at whoever's nearest then.
            if self.shooting_counter == 19
                && let Some(target) = self.find_target_zombie(zombies)
            {
                self.homing_target = Some(target);
                self.fire(self.row, None, actions, effects);
            }
        } else if self.seed == SeedType::Gatlingpea {
            // Behaviour per ruslan831 decomp, Plant::UpdateShooting: the
            // gatling pea fires four times in one long volley.
            if matches!(self.shooting_counter, 18 | 35 | 51 | 68) {
                self.fire(self.row, None, actions, effects);
            }
        } else if self.shooting_counter == 1 {
            if self.seed == SeedType::Threepeater {
                self.fire_threepeater(actions, effects);
            } else if self.seed == SeedType::Splitpea {
                // Whichever heads leaned in fire.
                let shooting = |head: Option<ReanimationId>| head.and_then(|h| effects.get(h)).is_some_and(|h| h.loop_type == LoopType::PlayOnceAndHold);
                let (front, back) = (shooting(self.head), shooting(self.extra_heads[0]));
                if front {
                    self.fire_weapon(self.row, None, Weapon::Primary, actions, effects);
                }
                if back {
                    self.fire_weapon(self.row, None, Weapon::Secondary, actions, effects);
                }
            } else if self.seed == SeedType::Cactus && self.state == PlantState::CactusLow {
                self.fire_weapon(self.row, None, Weapon::Secondary, actions, effects);
            } else {
                let target = self.lob_target(zombies);
                self.fire(self.row, target, actions, effects);
                // The butter's thrown: back to kernels.
                if self.seed == SeedType::Kernelpult && self.state == PlantState::KernelButter {
                    self.state = PlantState::NotReady;
                    if let Some(body) = self.body.and_then(|b| effects.get_mut(b)) {
                        body.assign_render_group_to_prefix("Cornpult_butter", RENDER_GROUP_HIDDEN);
                        body.assign_render_group_to_prefix("Cornpult_kernal", RENDER_GROUP_NORMAL);
                    }
                }
            }
            return;
        }
        if self.shooting_counter != 0 {
            return;
        }
        let (rate, time) = self.body.and_then(|b| effects.get(b)).map_or((0.0, 0.0), |b| (b.anim_rate, b.anim_time));
        if self.seed == SeedType::Threepeater {
            let done = self.extra_heads[0].and_then(|h| effects.get(h)).is_some_and(|h| h.loop_count > 0);
            if done {
                let heads = [self.head, self.extra_heads[0], self.extra_heads[1]];
                for (i, head) in heads.into_iter().enumerate() {
                    if let Some(head) = head.and_then(|h| effects.get_mut(h))
                        && (i == 1 || head.loop_type == LoopType::PlayOnceAndHold)
                    {
                        head.start_blend(20);
                        head.loop_type = LoopType::Loop;
                        head.set_frames_for_layer(&format!("anim_head_idle{}", i + 1));
                        head.anim_rate = rate;
                        head.anim_time = time;
                    }
                }
                return;
            }
        } else if let Some(head) = self.head.and_then(|h| effects.get_mut(h))
            && head.loop_count > 0
        {
            head.start_blend(20);
            head.loop_type = LoopType::Loop;
            head.set_frames_for_layer("anim_head_idle");
            head.anim_rate = rate;
            head.anim_time = time;
            return;
        }
        // The split pea's back head settles, too (`UpdateShooting`).
        if self.seed == SeedType::Splitpea
            && let Some(head) = self.extra_heads[0].and_then(|h| effects.get_mut(h))
            && head.loop_count > 0
        {
            head.start_blend(20);
            head.loop_type = LoopType::Loop;
            head.set_frames_for_layer("anim_splitpea_idle");
            head.anim_rate = rate;
            head.anim_time = time;
        }
        // A raised cactus goes back to waiting up there.
        if self.seed == SeedType::Cactus
            && self.state == PlantState::CactusHigh
            && let Some(body) = self.body.and_then(|b| effects.get(b))
            && body.loop_count > 0
        {
            let fps = body.definition.fps;
            self.play_body_reanim(effects, "anim_idlehigh", LoopType::Loop, 20, 0.0);
            if let Some(body) = self.body.and_then(|b| effects.get_mut(b)) {
                body.anim_rate = fps;
            }
            return;
        }
        // The cob cannon starts loading its next cob.
        if self.seed == SeedType::Cobcannon
            && let Some(body) = self.body.and_then(|b| effects.get(b))
            && body.loop_count > 0
        {
            let fps = body.definition.fps;
            self.state = PlantState::CobcannonArming;
            self.state_countdown = 3000;
            self.play_body_reanim(effects, "anim_unarmed_idle", LoopType::Loop, 20, fps);
            return;
        }
        // PlayIdleAnim, once a body shooter's shot is done.
        if self.head.is_none()
            && let Some(body) = self.body.and_then(|b| effects.get(b))
            && body.loop_count > 0
        {
            let fps = body.definition.fps;
            self.play_body_reanim(effects, "anim_idle", LoopType::Loop, 20, fps);
            return;
        }
        self.shooting_counter = 1;
    }

    /// `Plant::GetPeaHeadOffset`: where the stem is right now.
    fn pea_head_offset(&self, effects: &EffectSystem) -> (i32, i32) {
        let Some(body) = self.body.and_then(|b| effects.get(b)) else { return (0, 0) };
        let track = body.definition.find_track("anim_stem").or_else(|| body.definition.find_track("anim_idle")).unwrap_or(0);
        let t = body.current_transform(track);
        (t.x as i32, t.y as i32)
    }

    /// `Plant::Fire`, for peashooters and the mushrooms.
    fn fire(&mut self, row: i32, target: Option<LobTarget>, actions: &mut Vec<PlantAction>, effects: &EffectSystem) {
        self.fire_weapon(row, target, Weapon::Primary, actions, effects);
    }

    /// `Plant::Fire` with the weapon that fires.
    fn fire_weapon(&mut self, row: i32, target: Option<LobTarget>, weapon: Weapon, actions: &mut Vec<PlantAction>, effects: &EffectSystem) {
        if self.seed == SeedType::Fumeshroom {
            let rect = self.attack_rect();
            actions.push(PlantAction::RowAreaDamage { col: self.col, row: self.row, spread: 0, rect, damage: 20, flags: damage::HITS_SHIELD_AND_BODY });
            actions.push(PlantAction::Foley(FoleyType::Fume));
            return;
        }
        // Behaviour per ruslan831 decomp, Plant::StarFruitFire: five stars,
        // one left, one up, one down and two on diagonals to the right.
        if self.seed == SeedType::Starfruit {
            let (dx, dy) = (30f32.to_radians().cos() * 3.33, 30f32.to_radians().sin() * 3.33);
            for (vel_x, vel_y) in [(-3.33, 0.0), (0.0, 3.33), (0.0, -3.33), (dx, dy), (dx, -dy)] {
                actions.push(PlantAction::Shoot {
                    x: self.x + 25,
                    y: self.y + 25,
                    row: self.row,
                    render_order: self.render_order - 1,
                    projectile: ProjectileType::Star,
                    motion: ProjectileMotion::Star { vel_x, vel_y },
                    range_flags: self.target_flags(),
                    short_range: false,
                    drift: 0.0,
                    lob: None,
                });
            }
            return;
        }
        // Behaviour per ruslan831 decomp, Plant::Fire: the cob rises out of the
        // cannon and comes down where it was aimed.
        if self.seed == SeedType::Cobcannon {
            let pot = if self.in_pot { 5 } else { 0 };
            actions.push(PlantAction::Cob {
                x: self.x - 44,
                y: self.y - 184 - pot,
                render_order: self.render_order - 1,
                target_x: self.target_x,
                target_y: self.target_y,
            });
            return;
        }
        // Behaviour per ruslan831 decomp, Plant::Fire: the gloom-shroom's
        // cloud hurts the rows beside its own too.
        if self.seed == SeedType::Gloomshroom {
            let rect = self.attack_rect();
            actions.push(PlantAction::RowAreaDamage { col: self.col, row: self.row, spread: 1, rect, damage: 20, flags: damage::HITS_SHIELD_AND_BODY });
            return;
        }
        let (x, mut y) = match self.seed {
            SeedType::Puffshroom | SeedType::Seashroom => (self.x + 40, self.y + 40),
            SeedType::Scaredyshroom => (self.x + 29, self.y + 21),
            SeedType::Splitpea => {
                let (offset_x, offset_y) = self.pea_head_offset(effects);
                (self.x + offset_x + if weapon == Weapon::Secondary { -64 } else { 24 }, self.y + offset_y - 33)
            }
            SeedType::Cattail => (self.x + 20, self.y - 3),
            SeedType::Cactus if weapon == Weapon::Primary => (self.x + 93, self.y - 50),
            SeedType::Cactus => (self.x + 70, self.y + 23),
            SeedType::Threepeater => (self.x + 45, self.y + 10),
            SeedType::Cabbagepult => (self.x + 5, self.y - 12),
            SeedType::Melonpult | SeedType::Wintermelon => (self.x + 25, self.y - 46),
            SeedType::Kernelpult if self.state == PlantState::KernelButter => (self.x + 12, self.y - 56),
            SeedType::Kernelpult => (self.x + 19, self.y - 37),
            _ => {
                let (offset_x, offset_y) = self.pea_head_offset(effects);
                let muzzle = if self.seed == SeedType::Gatlingpea { 34 } else { 24 };
                (self.x + offset_x + muzzle, self.y + offset_y - 33)
            }
        };
        // Shots leave 5 pixels higher from a flower pot.
        if self.in_pot {
            y -= 5;
        }
        let muzzle = make_render_order(render_layer::LAWN_MOWER, self.row, 1);
        let projectile = match self.seed {
            SeedType::Snowpea => {
                // The frosty puff at the plant's mouth.
                actions.push(PlantAction::Particle { x: x + 8, y: y + 13, render_order: muzzle, effect: ParticleEffect::SnowpeaPuff });
                actions.push(PlantAction::Foley(FoleyType::SnowPeaSparkles));
                ProjectileType::Snowpea
            }
            SeedType::Puffshroom | SeedType::Seashroom | SeedType::Scaredyshroom => {
                let effect = ParticleEffect::PuffshroomMuzzle;
                let dx = if self.seed == SeedType::Scaredyshroom { 27 } else { 18 };
                actions.push(PlantAction::Particle { x: x + dx, y: y + 13, render_order: muzzle, effect });
                actions.push(PlantAction::Foley(FoleyType::Puff));
                ProjectileType::Puff
            }
            SeedType::Cactus | SeedType::Cattail => ProjectileType::Spike,
            SeedType::Cabbagepult => ProjectileType::Cabbage,
            SeedType::Melonpult => ProjectileType::Melon,
            SeedType::Wintermelon => {
                actions.push(PlantAction::Foley(FoleyType::SnowPeaSparkles));
                ProjectileType::Wintermelon
            }
            SeedType::Kernelpult if self.state == PlantState::KernelButter => ProjectileType::Butter,
            SeedType::Kernelpult => ProjectileType::Kernel,
            _ => ProjectileType::Pea,
        };
        // A catapult's shot is thrown at where the zombie will be (`Plant::Fire`).
        let lob = matches!(self.seed, SeedType::Cabbagepult | SeedType::Kernelpult | SeedType::Melonpult | SeedType::Wintermelon).then(|| {
            let (mut range_x, range_y) = match target {
                Some(t) => {
                    let mut range_x = t.lead_x - x as f32 - 30.0;
                    if t.riding_dolphin {
                        range_x -= 60.0;
                    }
                    if t.swimming_snorkel {
                        range_x -= 40.0;
                    }
                    // At the boss, the shot flies to the level of the lawn's right end (`Plant::Fire`).
                    let range_y = if t.boss { (self.y - (5 - self.col).max(0) * 20 - y) as f32 } else { (t.top - y) as f32 };
                    (range_x, range_y)
                }
                None => (700.0 - x as f32, 0.0),
            };
            range_x = range_x.max(40.0);
            Lob { range_x, range_y, range_flags: self.target_flags() | 8 }
        });
        // Only the puff-shroom's puffs fade out after a short way (`MOTION_PUFF`).
        let short_range = matches!(self.seed, SeedType::Puffshroom | SeedType::Seashroom);
        let motion = match (self.seed, self.homing_target) {
            (SeedType::Splitpea, _) if weapon == Weapon::Secondary => ProjectileMotion::Backwards,
            (SeedType::Cattail, Some(target)) => ProjectileMotion::Homing(target),
            _ => ProjectileMotion::Straight,
        };
        // The threepeater's side peas change lanes (`MOTION_THREEPEATER`).
        let drift = match row.cmp(&self.row) {
            _ if self.seed != SeedType::Threepeater => 0.0,
            std::cmp::Ordering::Less => -3.0,
            std::cmp::Ordering::Greater => 3.0,
            std::cmp::Ordering::Equal => 0.0,
        };
        actions.push(PlantAction::Shoot { x, y, row, render_order: self.render_order - 1, projectile, motion, range_flags: self.target_flags_for(weapon), short_range, drift, lob });
    }

    /// `Plant::UpdateReanim`.
    fn update_reanim(&mut self, effects: &mut EffectSystem) {
        let Some(body) = self.body else { return };
        let (mut offset_x, mut offset_y) = (self.shake_offset_x, self.height_offset());
        let (mut scale_x, mut scale_y) = (1.0, 1.0);
        match self.seed {
            SeedType::GiantWallnut => {
                (scale_x, scale_y) = (2.0, 2.0);
                offset_x -= 76.0;
                offset_y -= 64.0;
            }
            SeedType::InstantCoffee => {
                (scale_x, scale_y) = (0.8, 0.8);
                offset_x += 12.0;
                offset_y += 10.0;
            }
            SeedType::Potatomine => {
                (scale_x, scale_y) = (0.8, 0.8);
                offset_x += 12.0;
                offset_y += 12.0;
            }
            _ => {}
        }
        // Behaviour per ruslan831 decomp, Plant::Draw: a squished plant is drawn
        // at a quarter of its height, lying on the ground.
        if self.squished {
            scale_y *= 0.25;
            offset_y += 70.0 + if self.seed == SeedType::Flowerpot { -15.0 } else { 0.0 } + if self.seed == SeedType::InstantCoffee { -20.0 } else { 0.0 };
        }
        // A grave buster sinks into the grave it's eating.
        if self.state == PlantState::GravebusterEating {
            offset_y += animate_curve_float(400, 0, self.state_countdown, 0.0, 30.0, TodCurve::Linear);
        }
        effects.update_reanimation(body);
        if self.seed == SeedType::Leftpeater {
            offset_x += 80.0 * scale_x;
            scale_x = -scale_x;
        }
        if let Some(body) = effects.get_mut(body) {
            body.set_position(offset_x, offset_y);
            body.override_scale(scale_x, scale_y);
            // UpdateReanimColor: a bitten plant flashes.
            body.extra_additive = (self.eaten_flash_countdown > 0).then(|| {
                let gray = (self.eaten_flash_countdown * 3).clamp(0, 255) as u8;
                [gray, gray, gray, 255]
            });
        }
    }

    /// `Plant::UpdateBlink`.
    fn update_blink(&mut self, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>, rng: &mut MtRand) {
        if let Some(blink) = self.blink
            && effects.get(blink).is_none_or(|b| b.loop_count > 0)
        {
            self.end_blink(effects);
        }
        if self.blink_countdown > 0 {
            self.blink_countdown -= 1;
            if self.blink_countdown == 0 {
                self.do_blink(effects, res, images, rng);
            }
        }
    }

    /// `Plant::DoBlink` + `AttachBlinkAnim`.
    fn do_blink(&mut self, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>, rng: &mut MtRand) {
        use SeedType::*;
        self.blink_countdown = 400 + rng.next_range(400) as i32;
        if self.shooting_counter != 0 || self.is_asleep {
            return;
        }
        self.end_blink(effects);
        let Some(body_id) = self.body else { return };
        let nut = matches!(self.seed, Wallnut | Tallnut | ExplodeONut | GiantWallnut);
        if nut {
            self.blink_countdown = 1000 + rng.next_range(1000) as i32;
        }

        let Some(body) = effects.get(body_id) else { return };
        let has_track = |name: &str| body.definition.find_track(name).is_some();
        let mut track_to_play = "anim_blink";
        if nut {
            let hit = rng.next_range(10);
            track_to_play = if hit < 1 && has_track("anim_blink_twitch") {
                "anim_blink_twitch"
            } else if hit < 7 {
                "anim_blink_twice"
            } else {
                "anim_blink_thrice"
            };
        }
        // Peashooters blink on their head; most plants on their body.
        // TODO: the threepeater's, split pea's and twin sunflower's several faces.
        let attach_to = match self.seed {
            Peashooter | Snowpea | Repeater | Leftpeater | Gatlingpea | Splitpea => self.head,
            _ => Some(body_id),
        };
        let Some(attach_to) = attach_to else { return };
        if !has_track(track_to_play) {
            return;
        }
        let color = body.color_override;
        let ty = self.seed.definition().reanim;
        let Ok((blink_id, blink)) = effects.add_reanimation(0.0, 0.0, ty, res, images) else { return };
        blink.set_frames_for_layer(track_to_play);
        blink.loop_type = LoopType::PlayOnceFullLastFrameAndHold;
        blink.anim_rate = 15.0;
        blink.color_override = color;
        let face = effects.get(attach_to).map(|r| {
            if r.definition.find_track("anim_face").is_some() { "anim_face" } else { "anim_idle" }
        });
        if let Some(face) = face {
            effects.attach_to_another(blink_id, attach_to, face);
        }
        self.blink = Some(blink_id);
        if let Some(body) = effects.get_mut(body_id) {
            body.assign_render_group_to_prefix("anim_eye", RENDER_GROUP_HIDDEN);
        }
    }

    /// `Plant::EndBlink`.
    fn end_blink(&mut self, effects: &mut EffectSystem) {
        if let Some(blink) = self.blink.take() {
            effects.kill(blink);
            if let Some(body) = self.body.and_then(|b| effects.get_mut(b)) {
                body.assign_render_group_to_prefix("anim_eye", RENDER_GROUP_NORMAL);
            }
        }
    }

    /// Slows (or speeds) the plant's body animation: a plant carried off by
    /// a bungee zombie barely moves.
    pub fn set_body_anim_rate(&self, effects: &mut EffectSystem, rate: f32) {
        if let Some(body) = self.body.and_then(|b| effects.get_mut(b)) {
            body.anim_rate = rate;
        }
    }

    /// `Plant::Die`.
    pub fn die(&mut self, effects: &mut EffectSystem) {
        self.dead = true;
        let extra_heads = std::mem::take(&mut self.extra_heads);
        let owned = [self.body.take(), self.head.take(), self.blink.take(), self.sleeping.take(), extra_heads[0], extra_heads[1]];
        for id in owned.into_iter().flatten() {
            effects.kill(id);
        }
        if let Some(light) = self.light.take() {
            effects.remove_particle(light);
        }
    }

    /// `Plant::Draw`: its shadow, then its body (with head and blink attached).
    /// Drawn in the plant's own coordinates (its square's top-left).
    pub fn draw(&self, g: &mut Graphics, stage_is_night: bool, effects: &EffectSystem, res: &GameResources) {
        g.push_state();
        g.translate(self.x as f32, self.y as f32);
        let offset_y = self.height_offset();
        // The squash's shadow stays on the ground as it jumps.
        let airborne = self.seed == SeedType::Squash
            && matches!(self.state, PlantState::SquashRising | PlantState::SquashFalling | PlantState::SquashDoneFalling);
        let (shadow_x, shadow_y) = if airborne { ((self.home.0 - self.x) as f32, (self.home.1 - self.y) as f32) } else { (0.0, 0.0) };
        if !self.squished {
            self.draw_shadow(g, shadow_x, shadow_y + offset_y, stage_is_night, res);
        }
        if let Some(body) = self.body {
            if self.seed == SeedType::Pumpkinshell {
                effects.draw_group(g, body, RENDER_GROUP_NORMAL, res);
            } else {
                effects.draw(g, body, res);
            }
        }
        // `Plant::DrawMagnetItems`: what the magnet-shroom has pulled in.
        for item in &self.magnet_items {
            if let Some(image) = res.image(&item.image) {
                if item.value > 0 {
                    g.draw_image_cel_center_scaled(image, item.x - self.x as f32, item.y - self.y as f32, 0, 1.0, 1.0);
                } else {
                    g.draw_image_scaled(image, item.x - self.x as f32, item.y - self.y as f32, 0.8, 0.8);
                }
            }
        }
        g.pop_state();
    }

    /// The pumpkin's back half, drawn behind the plant inside it.
    pub fn draw_pumpkin_back(&self, g: &mut Graphics, effects: &EffectSystem, res: &GameResources) {
        let Some(body) = self.body else { return };
        g.push_state();
        g.translate(self.x as f32, self.y as f32);
        effects.draw_group(g, body, PUMPKIN_BACK_GROUP, res);
        g.pop_state();
    }

    /// Where the pumpkin's back half sorts: just behind the plants of its square.
    pub fn pumpkin_back_order(&self) -> i32 {
        make_render_order(render_layer::PLANT, self.row, PlantOrder::Normal as i32 * 5 - self.x + 800 - 1)
    }

    /// `Plant::DrawShadow`.
    fn draw_shadow(&self, g: &mut Graphics, x: f32, y: f32, night: bool, res: &GameResources) {
        use SeedType::*;
        if matches!(self.seed, Lilypad | Starfruit | Tanglekelp | Seashroom | Cobcannon | Spikeweed | Spikerock | Gravebuster | Cattail)
            || self.on_bungee == OnBungee::Rising
        {
            return;
        }
        let (dx, dy, scale) = match self.seed {
            Puffshroom => (-3.0, 42.0, 0.5),
            Sunshroom => (-3.0, 42.0, 1.0),
            Umbrella => (-7.0, 52.0, 0.5),
            Fumeshroom | Gloomshroom => (-3.0, 47.0, 1.3),
            Cabbagepult | Melonpult | Wintermelon => (-3.0, 47.0, 1.0),
            Kernelpult => (0.0, 47.0, 1.0),
            Scaredyshroom => (-9.0, 55.0, 1.0),
            Chomper => (-21.0, 57.0, 1.0),
            Flowerpot => (-4.0, 46.0, 1.0),
            Tallnut => (-3.0, 54.0, 1.3),
            Pumpkinshell => (-3.0, 46.0, 1.4),
            Cactus => (-8.0, 50.0, 1.0),
            Plantern => (-3.0, 57.0, 1.0),
            InstantCoffee => (-3.0, 81.0, 1.0),
            GiantWallnut => (-33.0, 56.0, 1.7),
            _ => (-3.0, 51.0, 1.0),
        };
        let id = if night { "IMAGE_PLANTSHADOW2" } else { "IMAGE_PLANTSHADOW" };
        if let Some(shadow) = res.image(id) {
            g.draw_image_cel_center_scaled(shadow, x + dx, y + dy, 0, scale, scale);
        }
    }
}
