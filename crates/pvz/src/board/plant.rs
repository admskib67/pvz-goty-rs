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
use sexy::tod::{TodCurve, animate_curve, animate_curve_float};

use crate::audio::FoleyType;
use crate::graphics::Graphics;
use crate::particle::ParticleEffect;
use crate::plant::{PlantSubClass, SeedType};
use crate::reanim::{EffectSystem, ParticleSystemId, ReanimationId, ReanimationType};
use crate::resources::GameResources;

use super::projectile::ProjectileType;
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
    Shoot { x: i32, y: i32, row: i32, render_order: i32, projectile: ProjectileType, short_range: bool, drift: f32, lob: Option<Lob> },
    /// `Plant::DoRowAreaDamage`: hurts every zombie in `row` touching `rect`
    /// (the fume-shroom's fumes).
    RowAreaDamage { row: i32, rect: IRect, damage: i32, flags: u32 },
    /// `Plant::IceZombies`: every zombie on the lawn freezes; the frost
    /// bursts from (x, y).
    IceShroom { x: i32, y: i32 },
    /// `Plant::DoSpecial` for the doom-shroom: a huge explosion at (x, y)
    /// that leaves a crater on its square.
    DoomShroom { x: i32, y: i32, col: i32, row: i32 },
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
    BowlingHit(sexy::data_array::Id<Zombie>),
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
    /// An instant plant counting down to going off.
    DoingSpecial,
}

/// Plants that shoot straight peas.
fn is_pea_shooter(seed: SeedType) -> bool {
    matches!(seed, SeedType::Peashooter | SeedType::Snowpea | SeedType::Repeater)
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
        self.update_blink(effects, res, images, rng);
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
        self.on_bungee == OnBungee::Rising
            || self.seed == SeedType::Squash
                && matches!(self.state, PlantState::SquashRising | PlantState::SquashFalling | PlantState::SquashDoneFalling)
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
            if zombie.row != self.row || !zombie.effected_by_damage(9) || !zombie.has_head {
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
            if z.row != self.row || !z.effected_by_damage(77) || !z.has_head || z.is_vaulting() {
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
            SeedType::Pumpkinshell => IRect::new(self.x, self.y, PLANT_SIZE - 20, PLANT_SIZE),
            _ => IRect::new(self.x + 10, self.y, PLANT_SIZE - 20, PLANT_SIZE),
        }
    }

    /// `Plant::GetPlantAttackRect`, for the plants ported so far.
    pub(super) fn attack_rect(&self) -> IRect {
        if self.bowling {
            return IRect::new(self.x, self.y, PLANT_SIZE - 20, PLANT_SIZE);
        }
        match self.seed {
            SeedType::Leftpeater => IRect::new(0, self.y, self.x, PLANT_SIZE),
            SeedType::Puffshroom | SeedType::Seashroom => IRect::new(self.x + 60, self.y, 230, PLANT_SIZE),
            SeedType::Fumeshroom => IRect::new(self.x + 60, self.y, 340, PLANT_SIZE),
            SeedType::Squash => IRect::new(self.x + 20, self.y, PLANT_SIZE - 35, PLANT_SIZE),
            SeedType::Spikeweed => IRect::new(self.x + 20, self.y, PLANT_SIZE - 50, PLANT_SIZE),
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
        if self.dead || self.is_asleep || self.on_bungee != OnBungee::No {
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
        } else if self.seed.makes_sun() {
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
        if self.seed == SeedType::Spikeweed {
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
            if self.state_countdown == 0 {
                self.state = PlantState::NotReady;
            } else if self.state_countdown == 75 {
                // 33: `DAMAGE_SPIKE` through the shield.
                let flags = damage::BYPASSES_SHIELD | damage::SPIKE;
                actions.push(PlantAction::RowAreaDamage { row: self.row, rect: self.attack_rect(), damage: 20, flags });
            }
            if self.body.and_then(|b| effects.get(b)).is_some_and(|b| b.loop_count > 0) {
                let rate = rng.range_f32(12.0, 15.0);
                self.play_body_reanim(effects, "anim_idle", LoopType::Loop, 20, rate);
            }
        } else if self.find_target_zombie(zombies).is_some() {
            self.play_body_reanim(effects, "anim_attack", LoopType::PlayOnceAndHold, 20, 18.0);
            actions.push(PlantAction::Foley(FoleyType::Throw));
            self.state = PlantState::SpikeweedAttacking;
            self.state_countdown = 100;
        }
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
            if zombie.row != self.row || !zombie.has_head || !zombie.effected_by_damage(Self::SQUASH_DAMAGE_RANGE) {
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
            actions.push(PlantAction::BowlingHit(target));
            // TODO: giant wall-nuts (which flatten everything and keep going straight).
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
        // TODO: money from nuts that hit many zombies (after level 10).
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
        self.launch_counter -= 1;
        if self.launch_counter <= 100 {
            let flash = animate_curve(100, 0, self.launch_counter, 0, 100, TodCurve::Linear);
            self.eaten_flash_countdown = self.eaten_flash_countdown.max(flash);
        }
        if self.launch_counter <= 0 {
            self.launch_counter = rng.range_int(self.launch_rate - 150, self.launch_rate);
            let small = self.state == PlantState::SunshroomSmall;
            actions.push(PlantAction::MakeSun { x: self.x, y: self.y, small });
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
            (z.row - self.row).abs() <= 1
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
            // TODO: the other shooters (cactus, split pea, ...).
            if self.seed == SeedType::Threepeater {
                self.launch_threepeater(zombies, effects, open_rows);
            } else if is_pea_shooter(self.seed)
                || matches!(
                    self.seed,
                    SeedType::Puffshroom
                        | SeedType::Seashroom
                        | SeedType::Fumeshroom
                        | SeedType::Scaredyshroom
                        | SeedType::Cabbagepult
                        | SeedType::Kernelpult
                        | SeedType::Melonpult
                )
            {
                self.find_target_and_fire(zombies, actions, effects, rng);
            }
        }
        if self.launch_counter == 25 && self.seed == SeedType::Repeater {
            self.find_target_and_fire(zombies, actions, effects, rng);
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
        self.find_target_zombie_in_row(zombies, self.row)
    }

    /// What this plant's shots can reach (`GetDamageRangeFlags`): the catapult
    /// plants' lobs come down on swimmers too.
    fn target_flags(&self) -> u32 {
        match self.seed {
            SeedType::Cabbagepult | SeedType::Kernelpult | SeedType::Melonpult => damage_range::GROUND | damage_range::SUBMERGED,
            _ => damage_range::GROUND,
        }
    }

    /// The zombie a catapult plant throws at, if it is one.
    fn lob_target(&self, zombies: &DataArray<Zombie>) -> Option<LobTarget> {
        if !matches!(self.seed, SeedType::Cabbagepult | SeedType::Kernelpult | SeedType::Melonpult) {
            return None;
        }
        let zombie = self.find_target_zombie(zombies).and_then(|id| zombies.get(id))?;
        Some(LobTarget {
            lead_x: zombie.target_lead_x(50.0),
            top: zombie.zombie_rect().y,
            riding_dolphin: zombie.phase == super::zombie::ZombiePhase::DolphinRiding,
            swimming_snorkel: zombie.phase == super::zombie::ZombiePhase::SnorkelWalkingInPool,
        })
    }

    /// `Plant::FindTargetZombie` for any `row` (the threepeater looks at three).
    fn find_target_zombie_in_row(&self, zombies: &DataArray<Zombie>, row: i32) -> Option<sexy::data_array::Id<Zombie>> {
        let attack = self.attack_rect();
        let mut best: Option<(i32, sexy::data_array::Id<Zombie>)> = None;
        for (id, zombie) in zombies.iter() {
            if zombie.row != row || !zombie.effected_by_damage(self.target_flags()) {
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
            }
        } else if self.body.and_then(|b| effects.get(b)).is_some_and(|b| b.definition.find_track("anim_shooting").is_some()) {
            // Mushrooms shoot with their whole body.
            self.play_body_reanim(effects, "anim_shooting", LoopType::PlayOnceAndHold, 20, 35.0);
            // TODO: the other body shooters' timings (cactus 35, ...).
            self.shooting_counter = match self.seed {
                SeedType::Fumeshroom => 50,
                SeedType::Scaredyshroom => 25,
                SeedType::Cabbagepult => 32,
                SeedType::Melonpult => 36,
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
        if self.shooting_counter == 1 {
            if self.seed == SeedType::Threepeater {
                self.fire_threepeater(actions, effects);
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
        if self.seed == SeedType::Fumeshroom {
            let rect = self.attack_rect();
            actions.push(PlantAction::RowAreaDamage { row: self.row, rect, damage: 20, flags: damage::HITS_SHIELD_AND_BODY });
            actions.push(PlantAction::Foley(FoleyType::Fume));
            return;
        }
        let (x, mut y) = match self.seed {
            SeedType::Puffshroom | SeedType::Seashroom => (self.x + 40, self.y + 40),
            SeedType::Scaredyshroom => (self.x + 29, self.y + 21),
            SeedType::Threepeater => (self.x + 45, self.y + 10),
            SeedType::Cabbagepult => (self.x + 5, self.y - 12),
            SeedType::Melonpult => (self.x + 25, self.y - 46),
            SeedType::Kernelpult if self.state == PlantState::KernelButter => (self.x + 12, self.y - 56),
            SeedType::Kernelpult => (self.x + 19, self.y - 37),
            _ => {
                let (offset_x, offset_y) = self.pea_head_offset(effects);
                (self.x + offset_x + 24, self.y + offset_y - 33)
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
            SeedType::Cabbagepult => ProjectileType::Cabbage,
            SeedType::Melonpult => ProjectileType::Melon,
            SeedType::Kernelpult if self.state == PlantState::KernelButter => ProjectileType::Butter,
            SeedType::Kernelpult => ProjectileType::Kernel,
            _ => ProjectileType::Pea,
        };
        // A catapult's shot is thrown at where the zombie will be (`Plant::Fire`).
        let lob = matches!(self.seed, SeedType::Cabbagepult | SeedType::Kernelpult | SeedType::Melonpult).then(|| {
            let (mut range_x, range_y) = match target {
                Some(t) => {
                    let mut range_x = t.lead_x - x as f32 - 30.0;
                    if t.riding_dolphin {
                        range_x -= 60.0;
                    }
                    if t.swimming_snorkel {
                        range_x -= 40.0;
                    }
                    (range_x, (t.top - y) as f32)
                }
                None => (700.0 - x as f32, 0.0),
            };
            range_x = range_x.max(40.0);
            Lob { range_x, range_y, range_flags: self.target_flags() | 8 }
        });
        // Only the puff-shroom's puffs fade out after a short way (`MOTION_PUFF`).
        let short_range = matches!(self.seed, SeedType::Puffshroom | SeedType::Seashroom);
        // The threepeater's side peas change lanes (`MOTION_THREEPEATER`).
        let drift = match row.cmp(&self.row) {
            _ if self.seed != SeedType::Threepeater => 0.0,
            std::cmp::Ordering::Less => -3.0,
            std::cmp::Ordering::Greater => 3.0,
            std::cmp::Ordering::Equal => 0.0,
        };
        actions.push(PlantAction::Shoot { x, y, row, render_order: self.render_order - 1, projectile, short_range, drift, lob });
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
            Peashooter | Snowpea | Repeater | Leftpeater | Gatlingpea => self.head,
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
        self.draw_shadow(g, shadow_x, shadow_y + offset_y, stage_is_night, res);
        if let Some(body) = self.body {
            effects.draw(g, body, res);
        }
        g.pop_state();
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
