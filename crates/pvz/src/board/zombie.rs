//! Zombies on the lawn (`Zombie`). So far the plain family (normal, flag,
//! conehead, buckethead), the pole vaulter, and the night's newspaper,
//! screen door, football and dancing zombies with their backup dancers.
//! They walk in from the right, eat plants that are in their way, lose
//! their shield, helmet, arm and head as they're hurt, and fall over when
//! they die. On the last wave of a night level some climb out of the graves.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Zombie.cpp`: `ZombieInitialize`,
//! `LoadReanim`, `LoadPlainZombieReanim`, `SetupReanimLayers`,
//! `PickRandomSpeed`, `UpdateAnimSpeed`, `Update`, `UpdatePlaying`,
//! `UpdateZombiePosition`, `UpdateZombieWalking`, `CheckIfPreyCaught`,
//! `EatPlant`, `StartEating`/`StopEating`/`StartWalkAnim`, `Animate`,
//! `UpdateReanim`, `GetDrawPos`, `GetZombieRect`/`GetZombieAttackRect`,
//! `TakeDamage`, `TakeHelmDamage`, `TakeBodyDamage`, `UpdateDamageStates`,
//! `DropHead`/`DropArm`/`DropHelm`/`DropFlag`, `PlayDeathAnim`,
//! `UpdateDeath`, `DieNoLoot`, `EffectedByDamage`, `DrawShadow`,
//! `RiseFromGrave`, `UpdateZombieRiseFromGrave`, `DrawReanim`'s clipping
//! and `HasShadow`; the shields (`AttachShield`, `DetachShield`,
//! `TakeShieldDamage`, `DropShield`, `ShowDoorArms`), `UpdateZombieNewspaper`,
//! the dancers (`UpdateZombieDancer`, `UpdateZombieBackupDancer`,
//! `SummonBackupDancer(s)`, `NeedsMoreBackupDancers`, `GetDancerFrame`,
//! `GetDancerPhase`, `ZombieNotWalking`, `IsMovingAtChilledSpeed`,
//! `DrawDancerReanim`) and `SetupReanimForLostArm`.
//!
//! Not yet: the dance's `anim_dance` loop, the loot a zombie drops by value,
//! and rising from the pool's and roof's graves.

use bevy::prelude::*;
use sexy::data_array::{DataArray, Id};
use sexy::reanim::{LoopType, RENDER_GROUP_HIDDEN, RENDER_GROUP_NORMAL};
use sexy::tod::{TodCurve, animate_curve, animate_curve_float};

use crate::audio::{FoleyType, Sounds};
use crate::graphics::{Graphics, SexyColor};
use crate::particle::ParticleEffect;
use crate::reanim::{
    EffectSystem, ParticleSystemId, ReanimationId, ReanimationType, draw_reanimation, draw_render_group,
    draw_render_group_colored,
};
use crate::resources::{GameResources, ImageRes};
use crate::zombie::ZombieType;

use super::plant::Plant;
use super::{BackgroundType, make_render_order, render_layer};

mod boss;
pub use boss::{BossPart, BossRequest, BossState, NUM_BOSS_BUNGEES};

/// `ZOMBIE_START_RANDOM_OFFSET`.
const START_RANDOM_OFFSET: u32 = 40;
/// `TICKS_BETWEEN_EATS` and `DAMAGE_PER_EAT`.
const TICKS_BETWEEN_EATS: i32 = 4;
pub const DAMAGE_PER_EAT: i32 = 4;
/// `BOARD_EDGE`: zombies whose x gets this far left have reached the house.
const BOARD_EDGE: i32 = -100;
const WIDE_BOARD_WIDTH: f32 = 800.0;
/// `Zombie::ZOMBIE_WAVE_WINNER`: the zombie that walks into the house.
pub const ZOMBIE_WAVE_WINNER: i32 = -4;
/// `Zombie::ZOMBIE_WAVE_CUTSCENE`: a zombie waiting on the street during the
/// level intro.
pub const ZOMBIE_WAVE_CUTSCENE: i32 = -2;
/// `Zombie::ZOMBIE_WAVE_UI`: a zombie shown off the board (the almanac's).
pub const ZOMBIE_WAVE_UI: i32 = -3;

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
    /// What cherry bombs and doom-shrooms reach (`127`): everything but
    /// hypnotized zombies.
    pub const EXPLOSION: u32 = ONLY_MINDCONTROLLED - 1;
    /// What a potato mine reaches (`77`).
    pub const POTATO_MINE: u32 = 77;
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
    Football,
    /// The bobsled team's sled.
    Bobsled,
    /// The wall-nut a WallnutHead wears.
    Wallnut,
    /// The tall-nut a TallnutHead wears.
    Tallnut,
    /// The digger's hard hat (its pickaxe is `has_object`).
    Digger,
}

/// `ShieldType`, for what's ported so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShieldType {
    None,
    Door,
    Newspaper,
    /// The ladder zombie's ladder, until it props it against a plant.
    Ladder,
}

/// `ZombieAttackType`: what a zombie is looking for a plant for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttackType {
    Chew,
    Vault,
    /// A ladder zombie looking for a nut to prop its ladder against.
    Ladder,
}

/// Which squares have a ladder propped against their plant (`GetLadderAt`).
pub type LadderGrid = [[bool; super::MAX_GRID_SIZE_Y]; super::MAX_GRID_SIZE_X];

/// A zombie's body draws its shield (`RENDER_GROUP_SHIELD`) and the hands
/// holding it (`RENDER_GROUP_OVER_SHIELD`) separately.
const RENDER_GROUP_SHIELD: i32 = 1;
const RENDER_GROUP_OVER_SHIELD: i32 = 3;
/// `NUM_BACKUP_DANCERS`: above, below, behind and in front of the dancer.
pub const NUM_BACKUP_DANCERS: usize = 4;
/// `ZOMBIE_BACKUP_DANCER_RISE_HEIGHT`.
const BACKUP_DANCER_RISE_HEIGHT: i32 = -200;
/// `POGO_BOUNCE_TIME`: ticks per bounce of the pogo stick.
const POGO_BOUNCE_TIME: i32 = 80;
/// `ZOMBIE_LIMP_SPEED_FACTOR`: the jack and jalapeno head fuses run twice
/// as long as walking the distance would take.
const ZOMBIE_LIMP_SPEED_FACTOR: i32 = 2;
/// The bungee zombie's lower arms, drawn over the plant it carries.
const RENDER_GROUP_ARMS: i32 = 2;
/// `BUNGEE_ZOMBIE_HEIGHT`: how far up a bungee zombie starts its dive.
const BUNGEE_ZOMBIE_HEIGHT: f32 = 3000.0;
/// `THOWN_ZOMBIE_GRAVITY`: what pulls a thrown imp down.
const THROWN_ZOMBIE_GRAVITY: f32 = 0.05;

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
    /// Climbing out of a grave on the last wave of a night level.
    RisingFromGrave,
    /// The jack-in-the-box running at the house, counting down...
    JackRunning,
    /// ...then popping its lid and blowing up.
    JackPopping,
    /// The digger underground, under the plants...
    DiggerTunneling,
    /// ...bursting out at the back of the lawn...
    DiggerRising,
    /// ...waiting out the loss of its pickaxe...
    DiggerTunnelingPauseWithoutAxe,
    DiggerRiseWithoutAxe,
    /// ...dazed, then walking back through the defences.
    DiggerStunned,
    DiggerWalking,
    DiggerWalkingWithoutAxe,
    /// Bobbing over the street during the level intro.
    DiggerCutscene,
    /// The pogo zombie bouncing along...
    PogoBouncing,
    /// ...bouncing high to clear a plant...
    PogoHighBounce,
    /// ...then over it.
    PogoForwardBounce,
    /// The yeti fleeing the lawn.
    YetiRunning,
    /// The squash head's squash, in the air...
    SquashRising,
    SquashFalling,
    /// ...and landed, squashing its square.
    SquashDoneFalling,
    /// The squash head's squash waiting for something to bite.
    SquashPreLaunch,
    /// The newspaper zombie reading as it walks.
    NewspaperReading,
    /// Its paper's gone: it gasps...
    NewspaperMaddening,
    /// ...and storms on, fast.
    NewspaperMad,
    /// The dancer moonwalking in.
    DancerDancingIn,
    /// Snapping its fingers to call up backup dancers.
    DancerSnappingFingers,
    DancerSnappingFingersWithLight,
    DancerSnappingFingersHold,
    /// The dance steps every dancer takes in time (`GetDancerPhase`).
    DancerDancingLeft,
    DancerWalkToRaise,
    DancerRaiseLeft1,
    DancerRaiseRight1,
    DancerRaiseLeft2,
    DancerRaiseRight2,
    /// A backup dancer coming up out of the ground.
    DancerRising,
    /// The snorkel zombie walking on land, or diving in.
    SnorkelWalking,
    SnorkelIntoPool,
    /// Submerged and swimming; surfaces to bite.
    SnorkelWalkingInPool,
    SnorkelUpToEat,
    SnorkelEatingInPool,
    SnorkelDownFromEat,
    /// The dolphin rider running along the grass towards the pool.
    DolphinWalking,
    DolphinIntoPool,
    /// Riding its dolphin through the water.
    DolphinRiding,
    /// Leaping over the first plant in its way.
    DolphinInJump,
    /// Landed with no dolphin under it: wades on foot.
    DolphinWalkingInPool,
    DolphinWalkingWithoutDolphin,
    /// A bobsled team sliding in on the ice.
    BobsledSliding,
    BobsledBoarding,
    /// The sled broke up: the team tumbles off and gets up to walk.
    BobsledCrashing,
    /// A bungee zombie dropping out of the sky onto its target square...
    BungeeDiving,
    /// ...screaming for the last part of the fall.
    BungeeDivingScreaming,
    /// Landed: it waits a moment before grabbing.
    BungeeAtBottom,
    /// Taking hold of the plant.
    BungeeGrabbing,
    /// Springing back up, with the plant if it got one.
    BungeeRising,
    /// Bobbing over the street during the level intro.
    BungeeCutscene,
    /// The ladder zombie running along with its ladder...
    LadderCarrying,
    /// ...and propping it against a nut.
    LadderPlacing,
    /// The catapult throwing a basketball...
    CatapultLaunching,
    /// ...and loading the next.
    CatapultReloading,
    /// The gargantuar flinging its imp...
    GargantuarThrowing,
    /// ...and bringing its pole down on a plant.
    GargantuarSmashing,
    /// An imp flying through the air...
    ImpGettingThrown,
    /// ...and landing on its feet.
    ImpLanding,
    /// The balloon zombie floating over the plants...
    BalloonFlying,
    /// ...its balloon popped, it drops...
    BalloonPopping,
    /// ...and walks on foot.
    BalloonWalking,
    /// Dr. Zomboss coming in at the start of the level...
    BossEnter,
    /// ...standing about between attacks...
    BossIdle,
    /// ...dropping a zombie in a row...
    BossSpawning,
    /// ...stomping the back rows...
    BossStomping,
    /// ...lowering three bungee zombies...
    BossBungeesEnter,
    BossBungeesDrop,
    BossBungeesLeave,
    /// ...dropping the vehicle on a block of plants...
    BossDropRv,
    /// ...and lowering his head, spitting a fireball or an iceball, and
    /// raising it again. Only then can he be hurt.
    BossHeadEnter,
    BossHeadIdleBeforeSpit,
    BossHeadSpit,
    BossHeadIdleAfterSpit,
    BossHeadLeave,
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
    /// `IsLittleTroubleLevel`: zombies are half size with a quarter of the health
    /// (`Zombie::ZombieInitialize`), and groan at a higher pitch.
    pub tiny_zombies: bool,
    /// `mMainCounter`: dancers step in time with it (`GetDancerFrame`).
    pub main_counter: i32,
    /// `HasLevelAwardDropped`: the level's won.
    pub award_dropped: bool,
    /// Which rows are water (`mPlantRow == PLANTROW_POOL`).
    pub pool_rows: [bool; super::MAX_GRID_SIZE_Y],
    /// Where each row's ice trail starts (`mIceMinX`).
    pub ice_min_x: [i32; super::MAX_GRID_SIZE_Y],
    /// The ladders on the lawn.
    pub ladders: LadderGrid,
    /// The level intro is playing (the boss stamps in while it does).
    pub scene_intro: bool,
    /// Which rows zombies can walk down (`RowCanHaveZombieType`).
    pub open_rows: [bool; super::MAX_GRID_SIZE_Y],
    /// `BossAreBungeesDone`: the bungees the boss dropped have come and gone.
    pub boss_bungees_done: bool,
    /// `IsFinalBossLevel`.
    pub final_boss: bool,
}

impl ZombieEnv {
    /// The setting for a zombie off the board: a plain day lawn.
    pub fn ui() -> Self {
        Self {
            background: BackgroundType::Day,
            level: 1,
            finished_adventure: false,
            zombie_count: 0,
            zombies_on_screen: 0,
            bowling: false,
            tiny_zombies: false,
            main_counter: 0,
            award_dropped: false,
            pool_rows: [false; super::MAX_GRID_SIZE_Y],
            ice_min_x: [super::BOARD_ICE_START; super::MAX_GRID_SIZE_Y],
            ladders: [[false; super::MAX_GRID_SIZE_Y]; super::MAX_GRID_SIZE_X],
            scene_intro: false,
            open_rows: [true; super::MAX_GRID_SIZE_Y],
            boss_bungees_done: false,
            final_boss: false,
        }
    }

    /// `Board::IsPoolSquare` for the square under pixel column `x` in `row`.
    pub fn is_pool_square(&self, x: i32, row: i32) -> bool {
        x >= super::cursor::LAWN_XMIN && self.pool_rows.get(row as usize).copied().unwrap_or(false)
    }
}

/// `ZombieHeight`, for the parts a pool lawn uses: a swimmer sinks in and
/// climbs out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZombieHeight {
    Normal,
    IntoPool,
    OutOfPool,
    /// Climbing a ladder over a plant...
    UpLadder,
    /// ...and dropping down the other side.
    Falling,
    /// The zombie that won on the roof, dropping down the chimney.
    InToChimney,
}

/// What a magnet-shroom pulled off a zombie (`MagnetShroomAttactItem`): the
/// picture, where it was, and where it's headed (an offset from the plant).
pub struct MagnetLoot {
    pub image: String,
    pub x: f32,
    pub y: f32,
    /// `x`, `y` are the item's middle rather than its top-left.
    pub centre: bool,
    pub dest: (f32, f32),
}

/// The things a zombie touches besides itself and the plants.
pub struct Fx<'a, 'w, 's> {
    pub effects: &'a mut EffectSystem,
    pub res: &'a mut GameResources,
    pub images: &'a mut Assets<Image>,
    pub sounds: &'a mut Sounds<'w, 's>,
}

/// `ReanimatorCache::DrawCachedZombie`: a still picture of a zombie type
/// (the almanac's thumbnails), its frame's top-left at (x, y), drawn at the
/// current scale. `with_pole` is `ZOMBIE_CACHED_POLEVAULTER_WITH_POLE`.
/// Behaviour per ruslan831 decomp, `ReanimatorCache::MakeCachedZombieFrame`.
/// The reanimations must be loaded beforehand.
pub fn draw_cached_zombie(g: &mut Graphics, res: &GameResources, x: f32, y: f32, ty: ZombieType, with_pole: bool) {
    use sexy::reanim::Reanimation;
    let Some(reanim_type) = ty.reanim() else { return };
    let load = |ty: ReanimationType, x: f32, y: f32| res.loaded_reanim_definition(ty.file()).map(|d| Reanimation::new(x, y, d));
    let (pos_x, pos_y) = (x + 40.0, y + 40.0);
    match reanim_type {
        ReanimationType::Zombie => {
            let Some(mut reanim) = load(reanim_type, pos_x, pos_y) else { return };
            reanim.set_frames_for_layer("anim_idle");
            setup_reanim_layers(&mut reanim, ty);
            if ty == ZombieType::Door {
                reanim.assign_render_group_to_track("anim_screendoor", RENDER_GROUP_NORMAL);
            } else if ty == ZombieType::Flag
                && let Some(mut flag) = load(ReanimationType::Flag, pos_x, pos_y)
            {
                flag.set_frames_for_layer("Zombie_flag");
                draw_reanimation(g, &flag, res);
            }
            draw_reanimation(g, &reanim, res);
        }
        ReanimationType::Boss => {
            let (Some(mut boss), Some(mut driver)) =
                (load(reanim_type, x - 524.0, y - 88.0), load(ReanimationType::BossDriver, x + 46.0, y + 22.0))
            else {
                return;
            };
            boss.set_frames_for_layer("anim_head_idle");
            driver.set_frames_for_layer("anim_idle");
            draw_reanimation(g, &boss, res);
            draw_reanimation(g, &driver, res);
            for track in ["boss_body1", "boss_neck", "boss_head2"] {
                boss.assign_render_group_to_track(track, RENDER_GROUP_HIDDEN);
            }
            draw_reanimation(g, &boss, res);
        }
        _ => {
            let track = match ty {
                ZombieType::Pogo => "anim_pogo",
                ZombieType::Polevaulter if !with_pole => "anim_walk",
                _ => "anim_idle",
            };
            let pos_y = if ty == ZombieType::Gargantuar { y + 60.0 } else { pos_y };
            crate::reanim::draw_reanimator_frame(g, res, pos_x, pos_y, reanim_type, Some(track), 1.0);
        }
    }
}

/// `Zombie::SetupReanimLayers`: which parts of the plain zombie's
/// reanimation each of its types wears.
fn setup_reanim_layers(reanim: &mut sexy::reanim::Reanimation, ty: ZombieType) {
    for prefix in ["anim_cone", "anim_bucket", "anim_screendoor", "Zombie_flaghand", "Zombie_duckytube", "anim_tongue", "Zombie_mustache"] {
        reanim.assign_render_group_to_prefix(prefix, RENDER_GROUP_HIDDEN);
    }
    // SetupDoorArms(false).
    for prefix in ["Zombie_outerarm_hand", "Zombie_outerarm_lower", "Zombie_outerarm_upper", "anim_innerarm"] {
        reanim.assign_render_group_to_prefix(prefix, RENDER_GROUP_NORMAL);
    }
    for prefix in ["Zombie_outerarm_screendoor", "Zombie_innerarm_screendoor", "Zombie_innerarm_screendoor_hand"] {
        reanim.assign_render_group_to_prefix(prefix, RENDER_GROUP_HIDDEN);
    }
    match ty {
        ZombieType::TrafficCone | ZombieType::Pail => {
            let helmet = if ty == ZombieType::TrafficCone { "anim_cone" } else { "anim_bucket" };
            reanim.assign_render_group_to_prefix(helmet, RENDER_GROUP_NORMAL);
            reanim.assign_render_group_to_prefix("anim_hair", RENDER_GROUP_HIDDEN);
        }
        ZombieType::Door => {
            for prefix in ["Zombie_outerarm_hand", "Zombie_outerarm_lower", "Zombie_outerarm_upper", "anim_innerarm"] {
                reanim.assign_render_group_to_prefix(prefix, RENDER_GROUP_HIDDEN);
            }
            for prefix in ["Zombie_outerarm_screendoor", "Zombie_innerarm_screendoor", "Zombie_innerarm_screendoor_hand"] {
                reanim.assign_render_group_to_prefix(prefix, RENDER_GROUP_NORMAL);
            }
        }
        ZombieType::Newspaper => reanim.assign_render_group_to_prefix("Zombie_paper_paper", RENDER_GROUP_HIDDEN),
        ZombieType::Flag => {
            reanim.assign_render_group_to_prefix("anim_innerarm", RENDER_GROUP_HIDDEN);
            reanim.assign_render_group_to_track("Zombie_flaghand", RENDER_GROUP_NORMAL);
            reanim.assign_render_group_to_track("Zombie_innerarm_screendoor", RENDER_GROUP_NORMAL);
        }
        ZombieType::DuckyTube => reanim.assign_render_group_to_prefix("Zombie_duckytube", RENDER_GROUP_NORMAL),
        _ => {}
    }
}

/// `ZombieDrawPosition`.
#[derive(Debug, Clone, Copy)]
struct DrawPos {
    head_x: i32,
    head_y: i32,
    body_y: f32,
    image_offset_x: f32,
    image_offset_y: f32,
    /// How much of the bottom is hidden (`mClipHeight`); `CLIP_HEIGHT_OFF` when nothing is.
    clip_height: f32,
}

/// `BOBSLED_CRASH_TIME`: how long the broken sled's team takes to get up.
const BOBSLED_CRASH_TIME: i32 = 150;
/// `NUM_BOBSLED_FOLLOWERS`.
pub const NUM_BOBSLED_FOLLOWERS: usize = 3;

/// What a bobsled follower needs to know about its leader to draw the sled.
#[derive(Debug, Clone, Copy, Default)]
pub struct SledInfo {
    pos_x: f32,
    damage_index: i32,
}

/// `DOLPHIN_JUMP_TIME`: how long the dolphin rider's leap lasts.
const DOLPHIN_JUMP_TIME: i32 = 120;

/// `CLIP_HEIGHT_LIMIT` and `CLIP_HEIGHT_OFF`: a draw position clips the
/// zombie only when its clip height is above the limit.
const CLIP_HEIGHT_LIMIT: f32 = -100.0;
const CLIP_HEIGHT_OFF: f32 = -200.0;

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
    /// Ticks left frozen solid by an ice-shroom (`mIceTrapCounter`).
    ice_trap_counter: i32,
    /// Ticks left stuck in butter (`mButteredCounter`).
    buttered_counter: i32,
    /// Bitten a garlic and grossed out (`mYuckyFace`, `mYuckyFaceCounter`): it
    /// stops eating, pulls a face and changes lanes.
    yucky_face: bool,
    yucky_face_counter: i32,
    phase_counter: i32,
    /// How far above the ground it is (`mAltitude`); below it while rising from a grave.
    altitude: f32,
    /// Sinking into or climbing out of the water (`mZombieHeight`).
    height: ZombieHeight,
    /// Swimming (`mInPool`).
    in_pool: bool,
    /// The Zamboni's tires are popped (`mFlatTires`).
    flat_tires: bool,
    /// 0 for a bobsled team's leader, 1 to 3 for the zombies behind it, -1 when
    /// it isn't riding a sled (`GetBobsledPosition`).
    pub bobsled_position: i32,
    /// The sled's helmet broke: crash on the next update.
    bobsled_crash_pending: bool,
    /// The leader crashed this tick: the board tumbles the others off.
    pub bobsled_crashed: bool,
    /// The sled is on the ice: the board keeps the trail from melting.
    pub ice_keep: bool,
    /// A follower's view of its leader's sled, kept by the board.
    pub sled: SledInfo,
    /// Where the Zamboni's ice trail reaches this tick; the board lays it down.
    pub ice_trail: Option<i32>,
    /// The zombie's row is water (`mPlantRow == PLANTROW_POOL`).
    water_row: bool,
    /// Where the body animation is (`mAnimTime`), for the swimmers' waterline.
    body_anim_time: f32,
    pub has_head: bool,
    has_arm: bool,
    /// The flag zombie still has its flag.
    has_object: bool,
    helm_type: HelmType,
    helm_health: i32,
    helm_max_health: i32,
    shield_type: ShieldType,
    shield_health: i32,
    shield_max_health: i32,
    /// The shield flashes (`mShieldJustGotShotCounter`) and jolts back
    /// (`mShieldRecoilCounter`) when hit.
    shield_just_got_shot_counter: i32,
    shield_recoil_counter: i32,
    /// A dancer's backup dancers (`mFollowerZombieID`).
    pub followers: [Option<Id<Zombie>>; NUM_BACKUP_DANCERS],
    /// A backup dancer's dancer (`mRelatedZombieID`).
    pub leader: Option<Id<Zombie>>,
    /// Counts down to the dancer calling up more backup (`mSummonCounter`).
    summon_counter: i32,
    /// What the board works out about a dancer's troupe each tick: one of
    /// them is eating, so none of them move (`ZombieNotWalking`); one is
    /// chilled, so all move slowly (`IsMovingAtChilledSpeed`); the dancer
    /// has room for more backup (`NeedsMoreBackupDancers`).
    pub troupe_stalled: bool,
    pub troupe_chilled: bool,
    pub needs_more_backup: bool,
    /// The dancer snapped its fingers: the board brings up its backup
    /// dancers (`SummonBackupDancers`).
    pub summon_requested: bool,
    /// The "!" over a newspaper zombie that's lost its paper.
    surprise: Option<ReanimationId>,
    /// Hypnotized by a hypno-shroom (`mMindControlled`): it turns round,
    /// walks back and eats the other zombies.
    pub mind_controlled: bool,
    /// It was hypnotized this tick: the board unhooks it from its dancer
    /// troupe and checks whether that was the last zombie.
    pub just_hypnotized: bool,
    /// The zombie in front of it on the other side (`FindZombieTarget`),
    /// worked out by the board before zombies move.
    pub zombie_target: Option<Id<Zombie>>,
    /// The zombie it bit this tick (`EatZombie`); the board hurts it.
    pub bitten_zombie: Option<Id<Zombie>>,
    /// The board should see whether this zombie leaving the fight ends the
    /// level (`TrySpawnLevelAward`), without it dropping anything.
    pub award_check: bool,
    /// The plant it finished eating this tick (its type and column), for
    /// the board's count and advice.
    pub ate_plant: Option<(crate::plant::SeedType, i32)>,
    /// `mScaleZombie`: 1, or 0.5 for Little Trouble's tiny zombies. Only the
    /// body is drawn at this size; the zombie's rectangle stays the same.
    pub scale: f32,
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
    /// The balloon zombie's propeller hat.
    propeller: Option<ReanimationId>,
    /// Dr. Zomboss's state, and what he's asked the board for.
    boss: BossState,
    boss_requests: Vec<BossRequest>,
    /// A bungee zombie's cord is cut off above this y while the boss holds it (`DrawBungeeCord`).
    cord_clip: Option<f32>,
    /// Where the zombie that won vanishes into the house: the left edge and
    /// the height it's cut off at (`SetupDrawZombieWon`).
    win_clip: (f32, f32),
    /// `mFlyingHealth`: the balloon's, which must go before the zombie is hurt.
    flying_health: i32,
    /// `mBlowingAway`: a blover is blowing the zombie off the right of the lawn.
    blowing_away: bool,
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
    /// The column a bungee zombie drops on (`mTargetCol`), or -1.
    pub target_col: i32,
    /// The plant a bungee zombie has hold of (`mTargetPlantID`).
    pub target_plant: Option<Id<Plant>>,
    /// A bungee zombie bounced off an umbrella leaf (`mHitUmbrella`).
    hit_umbrella: bool,
    /// The column of the ladder it last climbed (`mUseLadderCol`), so it
    /// doesn't climb the same one twice.
    use_ladder_col: i32,
    /// The ladders on the lawn as of this tick.
    ladders: LadderGrid,
    /// A ladder it just propped against a plant; the board puts it there.
    pub placed_ladder: Option<(i32, i32)>,
    /// A basketball the catapult just threw, for the board to launch.
    pub thrown_basketball: Option<Basketball>,
    /// The imp a gargantuar just flung, for the board to add.
    pub thrown_imp: Option<ThrownImp>,
    /// A pea a zombotany head just fired, for the board to spawn.
    pub fired_pea: Option<FiredPea>,
    /// A jack-in-the-box that just blew up, for the board to resolve.
    pub jack_blast: Option<JackBlast>,
    /// A jalapeno head that just went off, for the board to burn the row.
    pub jalapeno_blast: Option<JalapenoBlast>,
    vel_z: f32,
    /// The board should shake (`ShakeBoard`), as a gargantuar lands a blow
    /// or falls.
    pub shake: Option<(i32, i32)>,
}

/// The imp a gargantuar throws: where from, how high, and whether it's as
/// chilled as its thrower.
#[derive(Debug, Clone, Copy)]
pub struct ThrownImp {
    pub x: f32,
    pub row: i32,
    pub vel_z: f32,
    pub chilled_counter: i32,
    pub render_order: i32,
}

/// `Zombie::ZombieCatapultFire`'s shot: where it starts and how it's thrown.
#[derive(Debug, Clone, Copy)]
pub struct Basketball {
    pub x: f32,
    pub y: f32,
    pub vel_x: f32,
    pub vel_z: f32,
}

/// A pea a PeaHead or GatlingHead just fired: where it starts, which row it
/// flies down, and whether it flies at the house or back at the zombies.
#[derive(Debug, Clone, Copy)]
pub struct FiredPea {
    pub x: f32,
    pub y: f32,
    pub row: i32,
    /// Hypnotized heads shoot peas at the zombies (`PROJECTILE_PEA`);
    /// otherwise the pea flies backwards at the plants (`PROJECTILE_ZOMBIE_PEA`).
    pub mind_controlled: bool,
}

/// A jack-in-the-box that just blew up: the board kills everything in range.
#[derive(Debug, Clone, Copy)]
pub struct JackBlast {
    pub row: i32,
    pub x: i32,
    pub y: i32,
}

/// A jalapeno head that just went off: the board burns its row.
#[derive(Debug, Clone, Copy)]
pub struct JalapenoBlast {
    pub row: i32,
    pub mind_controlled: bool,
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
            ice_trap_counter: 0,
            buttered_counter: 0,
            yucky_face: false,
            yucky_face_counter: 0,
            phase_counter: 0,
            altitude: 0.0,
            height: ZombieHeight::Normal,
            in_pool: false,
            flat_tires: false,
            bobsled_position: -1,
            bobsled_crash_pending: false,
            bobsled_crashed: false,
            ice_keep: false,
            sled: SledInfo::default(),
            ice_trail: None,
            water_row: false,
            body_anim_time: 0.0,
            has_head: true,
            has_arm: true,
            has_object: false,
            helm_type: HelmType::None,
            helm_health: 0,
            helm_max_health: 0,
            shield_type: ShieldType::None,
            shield_health: 0,
            shield_max_health: 0,
            shield_just_got_shot_counter: 0,
            shield_recoil_counter: 0,
            followers: [None; NUM_BACKUP_DANCERS],
            leader: None,
            summon_counter: 0,
            troupe_stalled: false,
            troupe_chilled: false,
            needs_more_backup: false,
            summon_requested: false,
            surprise: None,
            mind_controlled: false,
            just_hypnotized: false,
            zombie_target: None,
            bitten_zombie: None,
            award_check: false,
            ate_plant: None,
            scale: 1.0,
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
            propeller: None,
            boss: BossState::default(),
            boss_requests: Vec::new(),
            cord_clip: None,
            win_clip: (-123.0, 600.0),
            flying_health: 0,
            blowing_away: false,
            zombie_fade: -1,
            original_anim_rate: 0.0,
            mowered_reanim: None,
            dropped_loot: false,
            loot_pending: false,
            dead: false,
            render_order: 0,
            attached: Vec::new(),
            target_col: -1,
            target_plant: None,
            hit_umbrella: false,
            use_ladder_col: -1,
            ladders: env.ladders,
            placed_ladder: None,
            thrown_basketball: None,
            thrown_imp: None,
            fired_pea: None,
            jack_blast: None,
            jalapeno_blast: None,
            vel_z: 0.0,
            shake: None,
        };
        z.pos_y = if z.is_on_board() { z.pos_y_based_on_row(row, env) } else { 0.0 };
        z.pick_random_speed(fx);

        if let Some(ty) = zombie_type.reanim() {
            z.load_reanim(ty, env, fx);
        }
        match zombie_type {
            ZombieType::Normal | ZombieType::DuckyTube => z.load_plain_zombie_reanim(fx),
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
            ZombieType::Door => {
                z.shield_type = ShieldType::Door;
                z.shield_health = 1100;
                z.load_plain_zombie_reanim(fx);
                z.attach_shield(fx);
            }
            ZombieType::Football => {
                z.zombie_rect = IRect::new(50, 0, 57, 115);
                z.reanim_show_prefix(fx, "anim_hair", RENDER_GROUP_HIDDEN);
                z.helm_type = HelmType::Football;
                z.helm_health = 1400;
                z.anim_ticks_per_frame = 6;
                z.variant = false;
            }
            ZombieType::Newspaper => {
                z.attack_rect = IRect::new(20, 0, 50, 115);
                z.phase = ZombiePhase::NewspaperReading;
                z.shield_type = ShieldType::Newspaper;
                z.shield_health = 150;
                z.variant = false;
                z.attach_shield(fx);
            }
            ZombieType::Bobsled => {
                z.zombie_rect = IRect::new(-50, 0, 275, 115);
                z.helm_type = HelmType::Bobsled;
                z.helm_health = 300;
                z.pos_x = WIDE_BOARD_WIDTH + 80.0;
                z.altitude = -10.0;
                z.vel_x = 0.6;
                z.phase = ZombiePhase::BobsledSliding;
                z.phase_counter = 500;
                z.variant = false;
                if from_wave == ZOMBIE_WAVE_CUTSCENE {
                    z.play_zombie_reanim(fx.effects, "anim_jump", LoopType::PlayOnceAndHold, 0, 20.0);
                    if let Some(body) = z.body.and_then(|b| fx.effects.get_mut(b)) {
                        body.anim_time = 1.0;
                    }
                    z.altitude = 18.0;
                } else if z.is_on_board() {
                    z.play_zombie_reanim(fx.effects, "anim_push", LoopType::Loop, 0, 30.0);
                }
            }
            ZombieType::Zamboni => {
                z.body_health = 1350;
                z.anim_frames = 2;
                z.anim_ticks_per_frame = 8;
                z.pos_x = WIDE_BOARD_WIDTH + fx.sounds.rng().next_range(10) as f32;
                z.play_zombie_reanim(fx.effects, "anim_drive", LoopType::Loop, 0, 12.0);
                z.zombie_rect = IRect::new(0, -13, 153, 140);
                z.attack_rect = IRect::new(10, -13, 133, 140);
                z.variant = false;
            }
            ZombieType::DolphinRider => {
                z.body_health = 500;
                z.anim_ticks_per_frame = 6;
                z.phase = ZombiePhase::DolphinWalking;
                z.variant = false;
                if z.is_on_board() {
                    z.play_zombie_reanim(fx.effects, "anim_walkdolphin", LoopType::Loop, 0, 0.0);
                    z.pick_random_speed(fx);
                }
                z.setup_water_track(fx, "zombie_dolphinrider_whitewater");
                z.setup_water_track(fx, "zombie_dolphinrider_dolphininwater");
            }
            ZombieType::Snorkel => {
                z.zombie_rect = IRect::new(12, 0, 62, 115);
                z.attack_rect = IRect::new(-5, 0, 55, 115);
                z.setup_water_track(fx, "Zombie_snorkle_whitewater");
                z.setup_water_track(fx, "Zombie_snorkle_whitewater2");
                z.variant = false;
                z.phase = ZombiePhase::SnorkelWalking;
            }
            ZombieType::Dancer => {
                if z.is_on_board() {
                    z.phase = ZombiePhase::DancerDancingIn;
                    z.vel_x = 0.5;
                    z.phase_counter = 300 + fx.sounds.rng().next_range(12) as i32;
                    z.play_zombie_reanim(fx.effects, "anim_moonwalk", LoopType::Loop, 0, 24.0);
                } else {
                    z.play_zombie_reanim(fx.effects, "anim_moonwalk", LoopType::Loop, 0, 12.0);
                }
                z.body_health = 500;
                z.variant = false;
            }
            ZombieType::Gargantuar => {
                z.width = 180;
                z.body_health = 3000;
                z.anim_frames = 24;
                z.anim_ticks_per_frame = 8;
                z.pos_x = WIDE_BOARD_WIDTH + 45.0 + fx.sounds.rng().next_range(10) as f32;
                z.zombie_rect = IRect::new(-17, -38, 125, 154);
                z.attack_rect = IRect::new(-30, -38, 89, 154);
                z.variant = false;
                // It carries an imp on its back (mHasObject).
                z.has_object = true;
                let hit = fx.sounds.rng().next_range(100);
                let pole = if !z.is_on_board() || env.level == 48 {
                    None
                } else if hit < 10 {
                    Some("IMAGE_REANIM_ZOMBIE_GARGANTUAR_ZOMBIE")
                } else if hit < 35 {
                    Some("IMAGE_REANIM_ZOMBIE_GARGANTUAR_DUCKXING")
                } else {
                    None
                };
                if let Some(image) = pole
                    && let Some(body) = z.body.and_then(|b| fx.effects.get_mut(b))
                {
                    body.set_image_override("Zombie_gargantuar_telephonepole", Some(image));
                }
            }
            ZombieType::Imp => {
                if !z.is_on_board() {
                    z.play_zombie_reanim(fx.effects, "anim_walk", LoopType::Loop, 0, 12.0);
                }
            }
            ZombieType::Catapult => {
                z.body_health = 850;
                z.pos_x = WIDE_BOARD_WIDTH + 25.0 + fx.sounds.rng().next_range(10) as f32;
                // It carries twenty basketballs (mSummonCounter).
                z.summon_counter = 20;
                if z.is_on_board() {
                    z.play_zombie_reanim(fx.effects, "anim_walk", LoopType::Loop, 0, 5.5);
                } else {
                    z.play_zombie_reanim(fx.effects, "anim_idle", LoopType::Loop, 0, 8.0);
                }
                z.zombie_rect = IRect::new(0, -13, 153, 140);
                z.attack_rect = IRect::new(10, -13, 133, 140);
                z.variant = false;
            }
            ZombieType::Ladder => {
                z.body_health = 500;
                z.shield_health = 500;
                z.shield_type = ShieldType::Ladder;
                z.attack_rect = IRect::new(10, 0, 50, 115);
                if z.is_on_board() {
                    z.phase = ZombiePhase::LadderCarrying;
                    z.start_walk_anim(0, env, fx);
                }
                z.attach_shield(fx);
            }
            ZombieType::Bungee => {
                // The board picks where it drops (PickBungeeZombieTarget).
                z.body_health = 450;
                z.anim_frames = 4;
                z.altitude = BUNGEE_ZOMBIE_HEIGHT + fx.sounds.rng().range_int(0, 150) as f32;
                z.vel_x = 0.0;
                z.variant = false;
                if z.is_on_board() {
                    z.phase = ZombiePhase::BungeeDiving;
                } else {
                    z.phase = ZombiePhase::BungeeCutscene;
                    z.phase_counter = fx.sounds.rng().range_int(0, 200);
                }
                z.play_zombie_reanim(fx.effects, "anim_drop", LoopType::Loop, 0, 24.0);
                for prefix in [
                    "Zombie_bungi_rightarm_lower2",
                    "Zombie_bungi_rightarm_hand2",
                    "Zombie_bungi_leftarm_lower2",
                    "Zombie_bungi_leftarm_hand2",
                ] {
                    z.reanim_show_prefix(fx, prefix, RENDER_GROUP_ARMS);
                }
                if let Some(body) = z.body.and_then(|b| fx.effects.get_mut(b)) {
                    for track in &mut body.tracks {
                        track.truncate_disappearing_frames = false;
                    }
                }
                z.zombie_rect = IRect::new(-20, 22, 110, 94);
                z.attack_rect = IRect::new(0, 0, 0, 0);
            }
            ZombieType::Balloon => {
                // Behaviour per ruslan831 decomp, Zombie::ZombieInitialize (ZOMBIE_BALLOON).
                if let Some(body) = z.body.and_then(|b| fx.effects.get_mut(b)) {
                    for track in &mut body.tracks {
                        track.truncate_disappearing_frames = false;
                    }
                }
                if z.is_on_board() {
                    z.altitude = 25.0;
                    z.phase = ZombiePhase::BalloonFlying;
                    let rate = z.body_anim_rate(fx.effects);
                    z.play_zombie_reanim(fx.effects, "anim_idle", LoopType::Loop, 0, rate);
                } else {
                    let rate = fx.sounds.rng().range_f32(8.0, 10.0);
                    z.original_anim_rate = rate;
                    z.apply_anim_rate(fx.effects, rate);
                }
                if let Ok((propeller, reanim)) = fx.effects.add_reanimation(0.0, 0.0, ReanimationType::Balloon, fx.res, fx.images) {
                    reanim.set_frames_for_layer("Propeller");
                    reanim.loop_type = LoopType::LoopFullLastFrame;
                    z.propeller = Some(propeller);
                    if let Some(body) = z.body {
                        fx.effects.attach_to_another(propeller, body, "hat");
                    }
                }
                z.flying_health = 20;
                z.zombie_rect = IRect::new(36, 30, 42, 115);
                z.attack_rect = IRect::new(20, 30, 50, 115);
                z.variant = false;
            }
            ZombieType::Boss => {
                // Behaviour per ruslan831 decomp, Zombie::ZombieInitialize (ZOMBIE_BOSS).
                z.pos_x = 0.0;
                z.pos_y = 0.0;
                z.zombie_rect = IRect::new(700, 80, 90, 430);
                z.attack_rect = IRect::new(0, 0, 0, 0);
                z.body_health = 40000;
                z.variant = false;
                if z.is_on_board() {
                    z.play_zombie_reanim(fx.effects, "anim_enter", LoopType::PlayOnceAndHold, 0, 12.0);
                    z.summon_counter = 500;
                    z.boss.head_counter = 5000;
                    z.phase = ZombiePhase::BossEnter;
                } else {
                    z.play_zombie_reanim(fx.effects, "anim_head_idle", LoopType::Loop, 0, 12.0);
                }
                z.boss_setup_reanim(fx);
            }
            ZombieType::BackupDancer => {
                if !z.is_on_board() {
                    z.play_zombie_reanim(fx.effects, "anim_armraise", LoopType::Loop, 0, 12.0);
                }
                z.phase = ZombiePhase::DancerDancingLeft;
                z.variant = false;
            }
            ZombieType::JackInTheBox => {
                // Behaviour per ruslan831 decomp, Zombie::ZombieInitialize.
                z.body_health = 500;
                z.anim_ticks_per_frame = 6;
                let mut distance = 450.0 + fx.sounds.rng().next_range(300) as f32;
                if fx.sounds.rng().next_range(20) == 0 {
                    distance /= 3.0;
                }
                z.phase_counter = (distance / z.vel_x * ZOMBIE_LIMP_SPEED_FACTOR as f32) as i32;
                z.attack_rect = IRect::new(20, 0, 50, 115);
                if z.is_on_board() {
                    z.phase = ZombiePhase::JackRunning;
                }
                z.variant = false;
            }
            ZombieType::Digger => {
                // Behaviour per ruslan831 decomp, Zombie::ZombieInitialize.
                z.helm_type = HelmType::Digger;
                z.helm_health = 100;
                z.variant = false;
                z.has_object = true;
                z.zombie_rect = IRect::new(50, 0, 28, 115);
                if let Some(body) = z.body.and_then(|b| fx.effects.get_mut(b)) {
                    for track in &mut body.tracks {
                        track.truncate_disappearing_frames = false;
                    }
                }
                if !z.is_on_board() {
                    z.phase = ZombiePhase::DiggerCutscene;
                } else {
                    z.phase = ZombiePhase::DiggerTunneling;
                    z.add_attached_particle(60.0, 100.0, ParticleEffect::DiggerTunnel, fx);
                    z.play_zombie_reanim(fx.effects, "anim_dig", LoopType::LoopFullLastFrame, 0, 12.0);
                    z.pick_random_speed(fx);
                }
            }
            ZombieType::Pogo => {
                // Behaviour per ruslan831 decomp, Zombie::ZombieInitialize.
                z.variant = false;
                z.phase = ZombiePhase::PogoBouncing;
                z.pick_random_speed(fx);
                z.phase_counter = fx.sounds.rng().next_range(POGO_BOUNCE_TIME as u32) as i32 + 1;
                z.has_object = true;
                z.body_health = 500;
                z.attack_rect = IRect::new(10, 0, 30, 115);
                z.play_zombie_reanim(fx.effects, "anim_pogo", LoopType::PlayOnceAndHold, 0, 40.0);
                if let Some(body) = z.body.and_then(|b| fx.effects.get_mut(b)) {
                    body.anim_time = 1.0;
                }
            }
            ZombieType::Yeti => {
                // Behaviour per ruslan831 decomp, Zombie::ZombieInitialize.
                z.body_health = 1350;
                z.phase_counter = fx.sounds.rng().range_int(1500, 2000);
                z.has_object = true;
                z.attack_rect = IRect::new(20, 0, 50, 115);
                z.variant = false;
            }
            ZombieType::RedeyeGargantuar => {
                // Behaviour per ruslan831 decomp, Zombie::ZombieInitialize: a
                // gargantuar with red eyes and twice the health.
                z.width = 180;
                z.body_health = 6000;
                z.anim_frames = 24;
                z.anim_ticks_per_frame = 8;
                z.pos_x = WIDE_BOARD_WIDTH + 45.0 + fx.sounds.rng().next_range(10) as f32;
                z.zombie_rect = IRect::new(-17, -38, 125, 154);
                z.attack_rect = IRect::new(-30, -38, 89, 154);
                z.variant = false;
                z.has_object = true;
                if let Some(body) = z.body.and_then(|b| fx.effects.get_mut(b)) {
                    body.set_image_override("anim_head1", Some("IMAGE_REANIM_ZOMBIE_GARGANTUAR_HEAD_REDEYE"));
                }
            }
            ZombieType::PeaHead => {
                z.attach_plant_head(&["anim_head2"], ReanimationType::Peashooter, "anim_head_idle", "anim_head1", true, (65.0, -5.0, 0.2, -1.0, 1.0), true, fx);
                z.phase_counter = 150;
                z.variant = false;
            }
            ZombieType::WallnutHead => {
                z.attach_plant_head(&["anim_head", "Zombie_tie"], ReanimationType::Wallnut, "anim_idle", "Zombie_body", false, (50.0, 0.0, 0.2, -0.8, 0.8), false, fx);
                z.helm_type = HelmType::Wallnut;
                z.helm_health = 1100;
                z.variant = false;
            }
            ZombieType::TallnutHead => {
                z.attach_plant_head(&["anim_head", "Zombie_tie"], ReanimationType::Tallnut, "anim_idle", "Zombie_body", false, (37.0, 0.0, 0.2, -0.8, 0.8), false, fx);
                z.helm_type = HelmType::Tallnut;
                z.helm_health = 2200;
                z.variant = false;
                z.pos_x += 30.0;
            }
            ZombieType::JalapenoHead => {
                z.attach_plant_head(&["anim_head", "Zombie_tie"], ReanimationType::Jalapeno, "anim_idle", "Zombie_body", false, (55.0, -5.0, 0.2, -1.0, 1.0), false, fx);
                z.variant = false;
                z.body_health = 500;
                let distance = 275.0 + fx.sounds.rng().next_range(175) as f32;
                z.phase_counter = (distance / z.vel_x * ZOMBIE_LIMP_SPEED_FACTOR as f32) as i32;
            }
            ZombieType::GatlingHead => {
                z.attach_plant_head(&["anim_head2"], ReanimationType::Gatlingpea, "anim_head_idle", "anim_head1", true, (65.0, -5.0, 0.2, -1.0, 1.0), true, fx);
                z.phase_counter = 150;
                z.variant = false;
            }
            ZombieType::SquashHead => {
                z.attach_plant_head(&["anim_head2"], ReanimationType::Squash, "anim_idle", "anim_head1", true, (55.0, -15.0, 0.2, -0.75, 0.75), true, fx);
                z.phase = ZombiePhase::SquashPreLaunch;
                z.variant = false;
            }
        }

        // Behaviour per ruslan831 decomp, Zombie::LoadPlainZombieReanim: a zombie
        // in a pool row, or a ducky tube zombie, wears the tube.
        if z.zombie_type.reanim() == Some(ReanimationType::Zombie)
            && (zombie_type == ZombieType::DuckyTube || env.pool_rows.get(row as usize).copied().unwrap_or(false))
        {
            z.reanim_show_prefix(fx, "zombie_duckytube", RENDER_GROUP_NORMAL);
            for track in ["Zombie_duckytube", "Zombie_outerarm_hand", "Zombie_innerarm3"] {
                z.reanim_ignore_clip_rect(fx, track, true);
            }
            for track in ["Zombie_whitewater", "Zombie_whitewater2"] {
                z.setup_water_track(fx, track);
            }
        }
        // Behaviour per ruslan831 decomp, Zombie::ZombieInitialize: Little
        // Trouble's zombies are half size with a quarter of the health. The
        // almanac's zombies keep theirs. The scale comes first: the walk's
        // animation rate divides by it (`update_anim_speed`).
        if env.tiny_zombies && from_wave != ZOMBIE_WAVE_UI {
            z.scale = 0.5;
            z.body_health /= 4;
            z.helm_health /= 4;
            z.shield_health /= 4;
        }
        z.update_anim_speed(fx.effects);
        if z.variant {
            z.reanim_show_prefix(fx, "anim_tongue", RENDER_GROUP_NORMAL);
        }
        z.body_max_health = z.body_health;
        z.helm_max_health = z.helm_health;
        z.shield_max_health = z.shield_health;
        z.x = z.pos_x as i32;
        z.y = z.pos_y as i32;
        z.render_order = z.initial_render_order();
        z.update_reanim(fx.effects);
        z
    }

    /// Where `ZombieInitialize` puts the zombie among the board's objects.
    fn initial_render_order(&self) -> i32 {
        let (layer, offset) = match self.zombie_type {
            ZombieType::Zamboni | ZombieType::Gargantuar => (render_layer::ZOMBIE, 8),
            ZombieType::Bobsled => (render_layer::ZOMBIE, 3),
            ZombieType::Bungee => (render_layer::GRAVE_STONE, 7),
            ZombieType::Boss => (render_layer::TOP, 0),
            _ => (render_layer::ZOMBIE, 4),
        };
        make_render_order(layer, self.row, offset)
    }

    /// `PickBungeeZombieTarget`'s end: the board picked the square a bungee
    /// zombie drops on.
    pub fn set_bungee_target(&mut self, col: i32, row: i32, x: f32, env: &ZombieEnv) {
        self.target_col = col;
        self.row = row;
        self.pos_x = x;
        self.pos_y = self.pos_y_based_on_row(row, env);
        self.x = self.pos_x as i32;
        self.y = self.pos_y as i32;
        self.render_order = self.initial_render_order();
    }

    /// The leader of a bobsled team has its three followers (`ZombieInitialize`
    /// with a parent zombie): they sit behind it on the sled.
    pub fn lead_bobsled(&mut self, followers: [Id<Zombie>; NUM_BOBSLED_FOLLOWERS]) {
        self.bobsled_position = 0;
        for (slot, id) in self.followers.iter_mut().zip(followers) {
            *slot = Some(id);
        }
    }

    /// A follower takes its place on the sled behind `leader`.
    pub fn join_bobsled(&mut self, leader: Id<Zombie>, leader_pos_x: f32, position: usize) {
        self.helm_type = HelmType::None;
        self.helm_health = 0;
        self.helm_max_health = 0;
        self.zombie_rect = IRect::new(36, 0, 42, 115);
        self.leader = Some(leader);
        self.bobsled_position = position as i32 + 1;
        self.pos_x = leader_pos_x + (position as f32 + 1.0) * 50.0;
        self.x = self.pos_x as i32;
        let (offset, altitude) = match position {
            0 => (1, 9.0),
            1 => (2, -7.0),
            _ => (0, 9.0),
        };
        self.altitude = altitude;
        self.render_order = make_render_order(render_layer::ZOMBIE, self.row, offset);
    }

    /// `Zombie::ReanimIgnoreClipRect`.
    fn reanim_ignore_clip_rect(&self, fx: &mut Fx, track: &str, ignore: bool) {
        if let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b))
            && let Some(i) = body.definition.find_track(track)
        {
            body.tracks[i].ignore_clip_rect = ignore;
        }
    }

    /// `Zombie::SetupWaterTrack`: the foam isn't tinted or clipped.
    fn setup_water_track(&self, fx: &mut Fx, track: &str) {
        if let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b))
            && let Some(i) = body.definition.find_track(track)
        {
            body.tracks[i].ignore_color_override = true;
            body.tracks[i].ignore_clip_rect = true;
        }
    }

    /// How fast it moves (`mVelX`).
    pub fn velocity(&self) -> f32 {
        self.vel_x
    }

    /// The body animation's rate (`mAnimRate`).
    pub fn body_anim_rate(&self, effects: &EffectSystem) -> f32 {
        self.body.and_then(|b| effects.get(b)).map_or(0.0, |b| b.anim_rate)
    }

    /// Swimming, or on the way in or out of the water.
    #[cfg(test)]
    pub(super) fn in_pool(&self) -> bool {
        self.in_pool
    }

    #[cfg(test)]
    pub(super) fn flat_tires(&self) -> bool {
        self.flat_tires
    }

    #[cfg(test)]
    pub(super) fn altitude(&self) -> f32 {
        self.altitude
    }

    /// `Zombie::PoolSplash`: a splash and a sound where the zombie enters or leaves the water.
    fn pool_splash(&self, into_pool: bool, fx: &mut Fx) {
        let (x, y) = (self.pos_x + 23.0, self.pos_y + 78.0);
        if let Ok((_, splash)) = fx.effects.add_reanimation(x, y, ReanimationType::Splash, fx.res, fx.images) {
            splash.loop_type = LoopType::PlayOnce;
            splash.render_order = self.render_order + 1;
            splash.override_scale(0.8, 0.8);
        }
        let rng = fx.sounds.rng();
        fx.effects.add_particle(x + 37.0, y + 42.0, self.render_order + 1, ParticleEffect::PlantingPool, fx.res, fx.images, rng);
        fx.sounds.play_foley(if into_pool { FoleyType::ZombieSplash } else { FoleyType::PlantWater });
    }

    /// `Zombie::CheckForPool`: wading into water sinks the zombie, wading out lifts it.
    fn check_for_pool(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::CheckForPool.
        // The snorkel and dolphin rider dive in under their own control.
        if !self.zombie_type.can_go_in_pool() || self.zombie_type.is_pool_only() || self.height != ZombieHeight::Normal {
            return;
        }
        let on_water = env.is_pool_square(self.x + 75, self.row) && env.is_pool_square(self.x + 45, self.row) && self.x < 680;
        if !self.in_pool && on_water {
            self.height = ZombieHeight::IntoPool;
            self.in_pool = true;
            self.pool_splash(true, fx);
        } else if self.in_pool && !on_water {
            self.height = ZombieHeight::OutOfPool;
            self.start_walk_anim(0, env, fx);
            self.pool_splash(false, fx);
        }
    }

    /// `Zombie::UpdateZombiePool`: the sinking and the climbing out.
    fn update_zombie_pool(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        match self.height {
            ZombieHeight::OutOfPool => {
                self.altitude += if self.zombie_type == ZombieType::Snorkel { 2.0 } else { 1.0 };
                if self.altitude >= 0.0 {
                    self.altitude = 0.0;
                    self.height = ZombieHeight::Normal;
                    self.in_pool = false;
                }
            }
            ZombieHeight::IntoPool => {
                self.altitude -= 1.0;
                let depth = -40.0 * self.scale;
                if self.altitude <= depth {
                    self.altitude = depth;
                    self.height = ZombieHeight::Normal;
                    self.start_walk_anim(0, env, fx);
                }
            }
            ZombieHeight::Normal | ZombieHeight::UpLadder | ZombieHeight::Falling | ZombieHeight::InToChimney => {}
        }
    }

    /// `Zombie::GetPosYBasedOnRow`.
    fn pos_y_based_on_row(&self, row: i32, env: &ZombieEnv) -> f32 {
        let y = super::pos_y_based_on_row(env.background, self.pos_x + 40.0, row) - 30.0;
        if self.zombie_type == ZombieType::Balloon { y - 30.0 } else { y }
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
        self.from_wave != ZOMBIE_WAVE_CUTSCENE && self.from_wave != ZOMBIE_WAVE_UI
    }

    /// `Zombie::Update` for a zombie waiting on the street: it only
    /// animates.
    pub fn update_street(&mut self, effects: &mut EffectSystem) {
        self.age += 1;
        if self.phase == ZombiePhase::BungeeCutscene {
            // UpdateZombieBungee: it bobs on its cord.
            if self.phase_counter > 0 {
                self.phase_counter -= 1;
            }
            self.altitude = animate_curve(200, 0, self.phase_counter, 40, 0, TodCurve::SinWave) as f32;
            if self.phase_counter <= 0 {
                self.phase_counter = 200;
            }
        }
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
            ZombieType::Door => self.setup_door_arms(fx, true),
            _ => {}
        }
    }

    /// Zombotany setup shared by the six plant-headed zombies
    /// (`Zombie::ZombieInitialize`): a plain body with its head hidden, and
    /// the plant head attached to `body_track` at `offset` (x, y, angle,
    /// scale-x, scale-y, like `TodScaleRotateTransformMatrix`).
    #[allow(clippy::too_many_arguments)]
    fn attach_plant_head(
        &mut self,
        hide: &[&str],
        head: ReanimationType,
        head_anim: &str,
        body_track: &str,
        blank_body_head: bool,
        offset: (f32, f32, f32, f32, f32),
        walk2: bool,
        fx: &mut Fx,
    ) {
        self.load_plain_zombie_reanim(fx);
        self.reanim_show_prefix(fx, "anim_hair", RENDER_GROUP_HIDDEN);
        for prefix in hide {
            self.reanim_show_prefix(fx, prefix, RENDER_GROUP_HIDDEN);
        }
        if walk2
            && self.is_on_board()
            && let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b))
        {
            body.set_frames_for_layer("anim_walk2");
        }
        if blank_body_head
            && let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b))
        {
            body.set_image_override("anim_head1", Some("IMAGE_BLANK"));
        }
        let Ok((head_id, head_reanim)) = fx.effects.add_reanimation(0.0, 0.0, head, fx.res, fx.images) else { return };
        head_reanim.play(head_anim, LoopType::Loop, 0, 15.0);
        head_reanim.render_order = self.render_order + 1;
        self.special_head = Some(head_id);
        if let Some(body) = self.body {
            fx.effects.attach_to_another(head_id, body, body_track);
        }
        let (x, y, angle, sx, sy) = offset;
        let (cos, sin) = (angle.cos(), angle.sin());
        if let Some(head) = fx.effects.get_mut(head_id) {
            head.attach_offset = sexy::matrix::Matrix3 {
                m00: sx * cos,
                m01: -sy * sin,
                m02: x,
                m10: sx * sin,
                m11: sy * cos,
                m12: y,
            };
        }
        if let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b)) {
            body.frame_base_pose = 0;
        }
    }

    /// `Zombie::ShowDoorArms`: the arms holding the door, or the plain ones
    /// while eating or once the door's gone.
    fn show_door_arms(&self, fx: &mut Fx, show: bool) {
        self.setup_door_arms(fx, show);
        if !self.has_arm {
            self.reanim_show_prefix(fx, "Zombie_outerarm_lower", RENDER_GROUP_HIDDEN);
            self.reanim_show_prefix(fx, "Zombie_outerarm_hand", RENDER_GROUP_HIDDEN);
        }
    }

    /// `Zombie::AttachShield`: the shield and the hands in front of it draw
    /// apart from the body.
    fn attach_shield(&self, fx: &mut Fx) {
        let track = match self.shield_type {
            ShieldType::Door => {
                self.show_door_arms(fx, true);
                self.reanim_show_prefix(fx, "Zombie_outerarm_screendoor", RENDER_GROUP_OVER_SHIELD);
                "anim_screendoor"
            }
            ShieldType::Newspaper => {
                self.reanim_show_prefix(fx, "Zombie_paper_hands", RENDER_GROUP_OVER_SHIELD);
                "Zombie_paper_paper"
            }
            ShieldType::Ladder => {
                self.reanim_show_prefix(fx, "Zombie_outerarm", RENDER_GROUP_OVER_SHIELD);
                "Zombie_ladder_1"
            }
            ShieldType::None => return,
        };
        self.reanim_show_track(fx, track, RENDER_GROUP_SHIELD);
    }

    /// `Zombie::DetachShield`.
    fn detach_shield(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        match self.shield_type {
            ShieldType::Door => self.show_door_arms(fx, false),
            ShieldType::Newspaper => self.reanim_show_prefix(fx, "Zombie_paper_hands", RENDER_GROUP_NORMAL),
            ShieldType::Ladder => {
                // It walks and eats like any zombie from now on.
                self.reanim_show_prefix(fx, "Zombie_outerarm", RENDER_GROUP_NORMAL);
                self.phase = ZombiePhase::Normal;
                if self.is_eating {
                    self.play_zombie_reanim(fx.effects, "anim_eat", LoopType::Loop, 20, 0.0);
                } else {
                    self.start_walk_anim(0, env, fx);
                }
            }
            ShieldType::None => {}
        }
        self.shield_type = ShieldType::None;
        self.shield_health = 0;
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
        if matches!(self.phase, ZombiePhase::DiggerWalking | ZombiePhase::DiggerWalkingWithoutAxe) {
            self.vel_x = 0.12;
        } else if self.phase == ZombiePhase::YetiRunning {
            self.vel_x = 0.8;
        } else if self.zombie_type == ZombieType::Yeti {
            self.vel_x = 0.4;
        } else if matches!(self.zombie_type, ZombieType::Flag | ZombieType::Dancer | ZombieType::BackupDancer | ZombieType::Pogo) {
            self.vel_x = 0.45;
        } else if self.phase == ZombiePhase::PolePreVault || self.phase == ZombiePhase::DiggerTunneling || matches!(self.zombie_type, ZombieType::Football | ZombieType::Snorkel | ZombieType::JackInTheBox) {
            self.vel_x = rng.range_f32(0.66, 0.68);
        } else if self.phase == ZombiePhase::LadderCarrying || self.zombie_type == ZombieType::SquashHead {
            self.vel_x = rng.range_f32(0.79, 0.81);
        } else if matches!(self.phase, ZombiePhase::NewspaperMad | ZombiePhase::DolphinWalking | ZombiePhase::DolphinWalkingWithoutDolphin) {
            self.vel_x = rng.range_f32(0.89, 0.91);
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
        let has_swim = body.definition.find_track("anim_swim").is_some();
        self.pick_random_speed(fx);
        if self.phase == ZombiePhase::LadderCarrying {
            self.play_zombie_reanim(fx.effects, "anim_ladderwalk", LoopType::Loop, blend, 0.0);
            return;
        }
        if self.phase == ZombiePhase::NewspaperMad {
            self.play_zombie_reanim(fx.effects, "anim_walk_nopaper", LoopType::Loop, blend, 0.0);
            return;
        }
        if self.in_pool && self.height == ZombieHeight::Normal && has_swim {
            self.play_zombie_reanim(fx.effects, "anim_swim", LoopType::Loop, blend, 0.0);
            return;
        }
        // TODO: the dance mode's "anim_dance".
        let mut variant = fx.sounds.rng().next_range(2);
        if self.zombie_type == ZombieType::Flag || self.zombie_type == ZombieType::PeaHead {
            variant = 0;
        }
        if variant == 0 && has_walk2 {
            self.play_zombie_reanim(fx.effects, "anim_walk2", LoopType::Loop, blend, 0.0);
        } else if has_walk {
            self.play_zombie_reanim(fx.effects, "anim_walk", LoopType::Loop, blend, 0.0);
        }
    }

    /// `Zombie::StartEating`.
    fn start_eating(&mut self, fx: &mut Fx) {
        if self.is_eating {
            return;
        }
        self.is_eating = true;
        if self.phase == ZombiePhase::LadderCarrying {
            self.play_zombie_reanim(fx.effects, "anim_laddereat", LoopType::Loop, 20, 0.0);
            return;
        }
        if self.phase == ZombiePhase::NewspaperMad {
            self.play_zombie_reanim(fx.effects, "anim_eat_nopaper", LoopType::Loop, 20, 0.0);
            return;
        }
        if self.zombie_type != ZombieType::Snorkel {
            self.play_zombie_reanim(fx.effects, "anim_eat", LoopType::Loop, 20, 0.0);
        }
        if self.shield_type == ShieldType::Door {
            self.show_door_arms(fx, false);
        }
    }

    /// Whether it's still carrying what a magnet-shroom would take (a pogo
    /// stick, a pickaxe, an imp).
    #[cfg(test)]
    pub(super) fn has_object(&self) -> bool {
        self.has_object
    }

    /// Whether a magnet-shroom would go for this zombie (the metal ones).
    pub fn is_magnet_target(&self) -> bool {
        if self.mind_controlled || !self.has_head || self.height != ZombieHeight::Normal || self.is_dead_or_dying() {
            return false;
        }
        // A digger only shows its pickaxe once it's walking; a pogo stick
        // only while it's bouncing along on it (`Plant::UpdateMagnetShroom`).
        if matches!(self.zombie_type, ZombieType::Digger | ZombieType::Pogo) && !self.has_object {
            return false;
        }
        if self.phase == ZombiePhase::RisingFromGrave {
            return false;
        }
        if matches!(self.phase, ZombiePhase::DiggerTunneling | ZombiePhase::DiggerStunned | ZombiePhase::DiggerWalking) {
            return self.has_object;
        }
        matches!(self.helm_type, HelmType::Pail | HelmType::Football)
            || matches!(self.shield_type, ShieldType::Door | ShieldType::Ladder)
            || self.zombie_type == ZombieType::Pogo
            || self.phase == ZombiePhase::JackRunning
    }

    /// `Plant::MagnetShroomAttactItem`, the zombie's part: it loses its bucket,
    /// helmet, door or ladder, and the plant gets the item.
    pub fn magnet_take(&mut self, env: &ZombieEnv, fx: &mut Fx) -> Option<MagnetLoot> {
        let damage = |index: i32| index.clamp(0, 2) as usize;
        if self.helm_type == HelmType::Pail {
            let index = self.helm_damage_index();
            self.helm_health = 0;
            self.helm_type = HelmType::None;
            let (x, y) = self.track_position(fx.effects, "anim_bucket");
            self.reanim_show_prefix(fx, "anim_bucket", RENDER_GROUP_HIDDEN);
            self.reanim_show_prefix(fx, "anim_hair", RENDER_GROUP_NORMAL);
            let image = format!("IMAGE_REANIM_ZOMBIE_BUCKET{}", damage(index) + 1);
            return Some(MagnetLoot { image, x, y, centre: true, dest: (25.0, 20.0) });
        }
        if self.helm_type == HelmType::Football {
            let index = self.helm_damage_index();
            self.helm_health = 0;
            self.helm_type = HelmType::None;
            let (x, y) = self.track_position(fx.effects, "zombie_football_helmet");
            self.reanim_show_prefix(fx, "zombie_football_helmet", RENDER_GROUP_HIDDEN);
            self.reanim_show_prefix(fx, "anim_hair", RENDER_GROUP_NORMAL);
            let image = ["IMAGE_REANIM_ZOMBIE_FOOTBALL_HELMET", "IMAGE_REANIM_ZOMBIE_FOOTBALL_HELMET2", "IMAGE_REANIM_ZOMBIE_FOOTBALL_HELMET3"][damage(index)];
            return Some(MagnetLoot { image: image.into(), x: x + 37.0, y: y - 60.0, centre: false, dest: (20.0, 20.0) });
        }
        // The magnet-shroom takes the pogo stick, the jack's box, and the
        // digger's pickaxe (`Plant::MagnetShroomAttactItem`).
        if self.zombie_type == ZombieType::Pogo && self.has_object {
            self.pogo_break(damage::DOESNT_LEAVE_BODY, env, fx);
            let (x, y) = self.track_position(fx.effects, "Zombie_pogo_stick");
            return Some(MagnetLoot { image: "IMAGE_REANIM_ZOMBIE_POGO_STICK".into(), x: x + 40.0, y: y + 84.0, centre: false, dest: (30.0, 0.0) });
        }
        if self.zombie_type == ZombieType::Digger && self.has_object {
            self.digger_lose_axe(fx);
            let (x, y) = self.track_position(fx.effects, "Zombie_digger_pickaxe");
            return Some(MagnetLoot { image: "IMAGE_REANIM_ZOMBIE_DIGGER_PICKAXE".into(), x, y, centre: false, dest: (45.0, 15.0) });
        }
        if self.phase == ZombiePhase::JackRunning {
            self.reanim_show_prefix(fx, "Zombie_jackbox_box", RENDER_GROUP_HIDDEN);
            self.reanim_show_prefix(fx, "Zombie_jackbox_handle", RENDER_GROUP_HIDDEN);
            let (x, y) = self.track_position(fx.effects, "Zombie_jackbox_box");
            self.phase = ZombiePhase::Normal;
            self.pick_random_speed(fx);
            return Some(MagnetLoot { image: "IMAGE_REANIM_ZOMBIE_JACKBOX_BOX".into(), x, y, centre: false, dest: (20.0, 15.0) });
        }
        match self.shield_type {
            ShieldType::Door => {
                let index = self.shield_damage_index();
                self.detach_shield(env, fx);
                self.phase = ZombiePhase::Normal;
                if !self.is_eating {
                    self.start_walk_anim(0, env, fx);
                }
                let (x, y) = self.track_position(fx.effects, "anim_screendoor");
                let image = format!("IMAGE_REANIM_ZOMBIE_SCREENDOOR{}", damage(index) + 1);
                Some(MagnetLoot { image, x, y, centre: true, dest: (30.0, 0.0) })
            }
            ShieldType::Ladder => {
                let index = self.shield_damage_index();
                self.detach_shield(env, fx);
                let image = ["IMAGE_REANIM_ZOMBIE_LADDER_1", "IMAGE_REANIM_ZOMBIE_LADDER_1_DAMAGE1", "IMAGE_REANIM_ZOMBIE_LADDER_1_DAMAGE2"][damage(index)];
                Some(MagnetLoot { image: image.into(), x: self.pos_x + 31.0, y: self.pos_y + 20.0, centre: true, dest: (30.0, 0.0) })
            }
            _ => None,
        }
    }

    /// `Zombie::HasYuckyFaceImage`: the zombies that have a grossed-out face.
    fn has_yucky_face_image(&self) -> bool {
        use ZombieType::*;
        matches!(self.zombie_type, Normal | TrafficCone | Pail | Flag | Door | DuckyTube | Dancer | BackupDancer | Newspaper | Polevaulter)
    }

    /// `Zombie::ShowYuckyFace`.
    fn show_yucky_face(&self, show: bool, fx: &mut Fx) {
        if !self.has_yucky_face_image() {
            return;
        }
        let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b)) else { return };
        if show {
            body.set_image_override("anim_head1", Some("IMAGE_REANIM_ZOMBIE_HEAD_GROSSOUT"));
            for track in ["anim_head2", "anim_head_jaw", "anim_tongue"] {
                body.assign_render_group_to_track(track, RENDER_GROUP_HIDDEN);
            }
        } else if self.has_head {
            body.set_image_override("anim_head1", None);
            body.assign_render_group_to_track("anim_head2", RENDER_GROUP_NORMAL);
            body.assign_render_group_to_track("anim_head_jaw", RENDER_GROUP_NORMAL);
            if self.variant {
                body.assign_render_group_to_track("anim_tongue", RENDER_GROUP_NORMAL);
            }
        }
    }

    /// `Zombie::UpdateYuckyFace`: it stops chewing, pulls a face for a few
    /// seconds, then turns into another lane.
    fn update_yucky_face(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        let yuck = |this: &Self, fx: &mut Fx, env: &ZombieEnv| {
            if env.zombies_on_screen <= 5 && this.has_head || env.zombies_on_screen <= 10 && this.has_head && fx.sounds.rng().next_range(2) == 0 {
                fx.sounds.play_foley(FoleyType::Yuck);
            }
        };
        self.yucky_face_counter += 1;
        if self.yucky_face_counter > 20 && self.yucky_face_counter < 170 && !self.has_yucky_face_image() {
            self.stop_eating(env, fx);
            self.yucky_face_counter = 170;
            yuck(self, fx, env);
        }
        if self.yucky_face_counter > 270 {
            self.show_yucky_face(false, fx);
            self.yucky_face = false;
            self.yucky_face_counter = 0;
            return;
        }
        if self.yucky_face_counter == 70 {
            self.stop_eating(env, fx);
            self.show_yucky_face(true, fx);
            yuck(self, fx, env);
        }
        if self.yucky_face_counter == 170 {
            self.start_walk_anim(20, env, fx);
            let pool = env.pool_rows.get(self.row as usize).copied().unwrap_or(false);
            let can_go = |row: i32| {
                (0..super::MAX_GRID_SIZE_Y as i32).contains(&row)
                    && env.open_rows[row as usize]
                    && env.pool_rows[row as usize] == pool
            };
            let new_row = match (can_go(self.row + 1), can_go(self.row - 1)) {
                (true, false) => self.row + 1,
                (false, true) => self.row - 1,
                (true, true) => if fx.sounds.rng().next_range(2) == 0 { self.row + 1 } else { self.row - 1 },
                (false, false) => self.row,
            };
            self.row = new_row;
            self.pos_y = self.pos_y_based_on_row(new_row, env);
            self.y = self.pos_y as i32;
            self.render_order = self.initial_render_order();
        }
    }

    /// `Zombie::StopEating`.
    fn stop_eating(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        if !self.is_eating {
            return;
        }
        self.is_eating = false;
        if self.body.is_some() && self.zombie_type != ZombieType::Snorkel {
            self.start_walk_anim(20, env, fx);
        }
        if self.shield_type == ShieldType::Door {
            self.show_door_arms(fx, true);
        }
        self.update_anim_speed(fx.effects);
    }

    /// `Zombie::IsMovingAtChilledSpeed`: a dancer's troupe slows down together.
    fn is_moving_at_chilled_speed(&self) -> bool {
        self.chilled_counter > 0 || self.troupe_chilled
    }

    /// `Zombie::ZombieNotWalking`.
    fn zombie_not_walking(&self) -> bool {
        use ZombiePhase::*;
        self.is_eating
            || self.is_immobilized()
            || self.troupe_stalled
            || matches!(
                self.phase,
                NewspaperMaddening
                    | DancerSnappingFingers
                    | DancerSnappingFingersWithLight
                    | DancerSnappingFingersHold
                    | DancerRising
                    | DancerWalkToRaise
                    | DancerRaiseLeft1
                    | DancerRaiseRight1
                    | DancerRaiseLeft2
                    | DancerRaiseRight2
                    | LadderPlacing
                    | CatapultLaunching
                    | CatapultReloading
                    | GargantuarThrowing
                    | GargantuarSmashing
                    | ImpGettingThrown
                    | ImpLanding
                    | JackPopping
                    | DiggerRising
                    | DiggerTunnelingPauseWithoutAxe
                    | DiggerRiseWithoutAxe
                    | DiggerStunned
            )
    }

    fn apply_anim_rate(&self, effects: &mut EffectSystem, rate: f32) {
        let rate = if self.is_moving_at_chilled_speed() { rate * 0.5 } else { rate };
        if let Some(body) = self.body.and_then(|b| effects.get_mut(b)) {
            body.anim_rate = rate;
        }
    }

    /// `Zombie::CanBeChilled`.
    fn can_be_chilled(&self) -> bool {
        self.zombie_type != ZombieType::Zamboni
            && !self.is_bobsled_team_with_sled()
            && !self.is_dead_or_dying()
            && !matches!(self.phase, ZombiePhase::RisingFromGrave | ZombiePhase::DancerRising | ZombiePhase::DiggerTunneling | ZombiePhase::DiggerRising | ZombiePhase::DiggerTunnelingPauseWithoutAxe | ZombiePhase::DiggerRiseWithoutAxe)
            && !self.mind_controlled
            // The boss only feels it when his head's down.
            && (!self.is_boss()
                || matches!(
                    self.phase,
                    ZombiePhase::BossHeadIdleBeforeSpit | ZombiePhase::BossHeadIdleAfterSpit | ZombiePhase::BossHeadSpit
                ))
    }

    /// `Zombie::CanBeFrozen`: frozen solid or buttered. Not in mid-leap or
    /// mid-dive, and a bungee zombie only while it waits at the bottom.
    fn can_be_frozen(&self) -> bool {
        use ZombiePhase::*;
        self.can_be_chilled()
            && !matches!(self.phase, PoleInVault | DolphinIntoPool | DolphinInJump | SnorkelIntoPool | BobsledCrashing | ImpGettingThrown | ImpLanding | JackPopping | SquashRising | SquashFalling | SquashDoneFalling | PogoBouncing | PogoHighBounce | PogoForwardBounce)
            && !self.is_flying()
            && (self.zombie_type != ZombieType::Bungee || self.phase == BungeeAtBottom)
    }

    /// `Zombie::IsImmobilizied`: frozen solid (or, later, buttered).
    pub fn is_immobilized(&self) -> bool {
        self.ice_trap_counter > 0 || self.buttered_counter > 0
    }

    /// `Zombie::ApplyButter`: a pat of butter holds it still for a while.
    pub fn apply_butter(&mut self, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::ApplyButter.
        if !self.has_head || !self.can_be_frozen() || matches!(self.zombie_type, ZombieType::Zamboni | ZombieType::Boss) || self.is_bobsled_team_with_sled() {
            return;
        }
        self.buttered_counter = 400;
        self.update_anim_speed(fx.effects);
    }

    /// Keeps the butter on for good, so `PVZ_DEV_SCENE=butter` can be
    /// screenshotted whenever.
    pub(super) fn dev_hold_butter(&mut self) {
        if self.buttered_counter > 0 {
            self.buttered_counter = i32::MAX;
        }
    }

    /// Whether it's stuck in butter.
    pub fn is_buttered(&self) -> bool {
        self.buttered_counter > 0
    }

    /// `Zombie::ApplyChill`: frozen zombies move and bite at half speed;
    /// an ice-shroom's cold lasts twice as long.
    fn apply_chill(&mut self, ice_trap: bool, fx: &mut Fx) {
        if !self.can_be_chilled() {
            return;
        }
        if self.chilled_counter == 0 {
            fx.sounds.play_foley(FoleyType::Frozen);
        }
        self.chilled_counter = self.chilled_counter.max(if ice_trap { 2000 } else { 1000 });
        self.update_anim_speed(fx.effects);
    }

    /// `Zombie::HitIceTrap`: an ice-shroom freezes the zombie in place for
    /// a few seconds (less if it was already cold), then leaves it chilled.
    pub fn hit_ice_trap(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        if !self.is_on_board() {
            return;
        }
        let cold = self.chilled_counter > 0 || self.ice_trap_counter != 0;
        self.apply_chill(true, fx);
        if !self.can_be_frozen() {
            return;
        }
        let rng = fx.sounds.rng();
        self.ice_trap_counter = if cold { rng.range_int(300, 400) } else { rng.range_int(400, 600) };
        self.take_damage(20, damage::BYPASSES_SHIELD, env, fx);
        self.update_anim_speed(fx.effects);
    }

    /// `Zombie::RemoveColdEffects`: fire thaws a zombie.
    pub fn remove_cold_effects(&mut self, effects: &mut EffectSystem) {
        self.chilled_counter = 0;
        self.ice_trap_counter = 0;
        self.update_anim_speed(effects);
    }

    /// `Zombie::ApplyBurn`: cherry bombs and jalapenos.
    pub fn apply_burn(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        if self.dead || self.phase == ZombiePhase::Burned {
            return;
        }
        if self.zombie_type == ZombieType::Zamboni {
            self.zamboni_explode(fx);
            return;
        }
        if self.is_bobsled_team_with_sled() {
            self.die_with_loot(fx);
            return;
        }
        if self.body_health >= 1800 || self.is_boss() {
            self.take_damage(1800, 18, env, fx);
            return;
        }
        // A digger caught underground burns up where it is (`Zombie::ApplyBurn`).
        if matches!(
            self.phase,
            ZombiePhase::Dying
                | ZombiePhase::Mowered
                | ZombiePhase::PoleInVault
                | ZombiePhase::DiggerTunneling
                | ZombiePhase::DiggerTunnelingPauseWithoutAxe
                | ZombiePhase::DiggerRising
                | ZombiePhase::DiggerRiseWithoutAxe
        ) {
            self.die_with_loot(fx);
        } else if !self.has_head
            || matches!(self.zombie_type, ZombieType::Bungee | ZombieType::Yeti)
            || self.zombie_type.is_zombotany()
            || self.is_flying()
        {
            // Headless zombies (and bungee, yeti and zombotany zombies) stay
            // put, charred, for a few moments.
            self.apply_anim_rate(fx.effects, 0.0);
            self.original_anim_rate = 0.0;
            self.phase = ZombiePhase::Burned;
            self.phase_counter = 300;
            self.just_got_shot_counter = 0;
            self.drop_loot();
        } else {
            let (ty, dx, dy) = match self.zombie_type {
                ZombieType::Digger => (ReanimationType::ZombieCharredDigger, 0.0, 0.0),
                ZombieType::Catapult => (ReanimationType::ZombieCharredCatapult, -36.0, -20.0),
                ZombieType::Gargantuar | ZombieType::RedeyeGargantuar => (ReanimationType::ZombieCharredGargantuar, -15.0, -10.0),
                ZombieType::Imp => (ReanimationType::ZombieCharredImp, -6.0, 0.0),
                _ => (ReanimationType::ZombieCharred, 0.0, 0.0),
            };
            if let Ok((_, charred)) = fx.effects.add_reanimation(self.pos_x + 22.0 + dx, self.pos_y - 10.0 + dy, ty, fx.res, fx.images) {
                // A gargantuar that threw its imp burns without one.
                if ty == ReanimationType::ZombieCharredGargantuar && !self.has_object {
                    charred.set_image_override("impblink", Some("IMAGE_BLANK"));
                    charred.set_image_override("imphead", Some("IMAGE_BLANK"));
                }
                charred.anim_rate *= fx.sounds.rng().range_f32(0.9, 1.1);
                charred.render_order = self.render_order;
                // Behaviour per ruslan831 decomp, Zombie::ApplyBurn: a tiny
                // zombie's charred remains are scaled and dropped to its size.
                let s = self.scale;
                if s != 1.0 {
                    charred.overlay.m00 = s;
                    charred.overlay.m11 = s;
                    charred.overlay.m02 += 20.0 - s * 20.0;
                    charred.overlay.m12 += 120.0 - s * 120.0;
                    charred.override_scale(s, s);
                }
                if self.is_walking_backwards() {
                    charred.override_scale(-s, s);
                    charred.overlay.m02 += 60.0 * s;
                }
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
        if self.is_immobilized() || (self.yucky_face && self.yucky_face_counter < 170) {
            self.apply_anim_rate(effects, 0.0);
        } else if self.is_dead_or_dying() {
            self.apply_anim_rate(effects, self.original_anim_rate);
        } else if self.is_eating {
            self.apply_anim_rate(effects, if matches!(self.zombie_type, ZombieType::Polevaulter | ZombieType::Snorkel | ZombieType::Balloon) { 20.0 } else { 36.0 });
        } else if self.zombie_not_walking() {
            self.apply_anim_rate(effects, self.original_anim_rate);
        } else if let Some(track) = body.definition.find_track("_ground") {
            let frames = &body.definition.tracks[track].transforms;
            let last = (body.frame_start + body.frame_count).saturating_sub(1);
            let distance = frames[last].x - frames[body.frame_start].x;
            if distance >= 1e-6 {
                let one_over_speed = body.frame_count as f32 / distance;
                let rate = self.vel_x * one_over_speed * 47.0 / self.scale;
                self.apply_anim_rate(effects, rate);
            }
        } else {
            self.apply_anim_rate(effects, self.original_anim_rate);
        }
    }

    pub(super) fn variant(&self) -> bool {
        self.variant
    }

    pub(super) fn helm_health(&self) -> i32 {
        self.helm_health
    }

    pub(super) fn shield_health(&self) -> i32 {
        self.shield_health
    }

    /// Restores worn protection after a load (`Board::LoadGame`): the type
    /// comes from a fresh `Zombie::new`, the damage from the save.
    pub(super) fn set_armor_health(&mut self, helm: i32, shield: i32) {
        self.helm_health = helm;
        self.shield_health = shield;
    }

     #[cfg(test)]
     pub(super) fn has_arm(&self) -> bool {
         self.has_arm
     }

    /// Slowed by cold (`mChilledCounter`).
    pub fn is_chilled(&self) -> bool {
        self.chilled_counter > 0
    }

    /// What `TotalZombiesHealthInWave` counts for this zombie.
    pub fn total_health(&self) -> i32 {
        self.body_health + self.helm_health + self.shield_health
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

    /// `Zombie::IsWalkingBackwards`: hypnotized zombies walk the other way,
    /// and so does a digger coming back through the defences it tunnelled under.
    pub(super) fn is_walking_backwards(&self) -> bool {
        self.mind_controlled
            || matches!(
                self.phase,
                ZombiePhase::DiggerRising | ZombiePhase::DiggerStunned | ZombiePhase::DiggerWalking
            )
    }

    /// `Zombie::StartMindControlled`, from `AnimateChewSound` biting a
    /// hypno-shroom: it slowly walks back the way it came.
    fn start_mind_controlled(&mut self, fx: &mut Fx) {
        fx.sounds.play_sample("SOUND_MINDCONTROLLED");
        self.mind_controlled = true;
        self.just_hypnotized = true;
        let rng = fx.sounds.rng();
        fx.effects.add_particle(self.pos_x + 60.0, self.pos_y + 40.0, self.render_order + 1, ParticleEffect::MindControl, fx.res, fx.images, rng);
        self.vel_x = 0.17;
        self.anim_ticks_per_frame = 18;
        self.update_anim_speed(fx.effects);
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
        match self.zombie_type {
            ZombieType::Polevaulter => {
                image_offset_x -= 6.0;
                image_offset_y -= 11.0;
            }
            ZombieType::Football => image_offset_y -= 16.0,
            ZombieType::Balloon => image_offset_y += 17.0,
            ZombieType::Bungee => image_offset_x -= 18.0,
            ZombieType::Gargantuar | ZombieType::RedeyeGargantuar => image_offset_y -= 8.0,
            ZombieType::Pogo => image_offset_y += 16.0,
            ZombieType::Yeti => image_offset_y -= 20.0,
            ZombieType::DolphinRider => match self.phase {
                ZombiePhase::DolphinRiding => image_offset_x += 70.0,
                ZombiePhase::DolphinInJump => image_offset_x += 70.0 + self.altitude,
                ZombiePhase::DolphinWalkingInPool | ZombiePhase::Dying => image_offset_y += 50.0,
                _ => {}
            },
            _ => {}
        }
        let body_y = -self.altitude;
        let t = self.body_anim_time;
        let out_of_pool = self.height == ZombieHeight::OutOfPool;
        let snorkel_diving = self.phase == ZombiePhase::SnorkelIntoPool;
        let clip_height = match self.phase {
            // The snorkel's cut off at the water once it has dived (anim time 0.8 on).
            _ if snorkel_diving => {
                if self.body_anim_time >= 0.8 { -10.0 } else { CLIP_HEIGHT_OFF }
            }
            // The dolphin rider is cut off at the water for each part of its dive and leap.
            ZombiePhase::DolphinIntoPool if (0.56..=0.65).contains(&t) => 0.0,
            ZombiePhase::DolphinIntoPool if t >= 0.75 => -self.altitude - 10.0,
            ZombiePhase::DolphinRiding => -self.altitude - 10.0,
            ZombiePhase::DolphinInJump if t <= 0.06 => -self.altitude - 10.0,
            ZombiePhase::DolphinInJump if (0.5..=0.76).contains(&t) => -13.0,
            ZombiePhase::Dying if self.zombie_type == ZombieType::DolphinRider => -self.altitude + 44.0,
            ZombiePhase::DolphinWalking | ZombiePhase::DolphinWalkingWithoutDolphin if out_of_pool => -self.altitude,
            _ if self.zombie_type == ZombieType::DolphinRider => CLIP_HEIGHT_OFF,
            _ if self.zombie_type == ZombieType::Snorkel && self.in_pool => -self.altitude - 5.0 + (20.0 - 20.0 * self.scale),
            ZombiePhase::RisingFromGrave => body_y + (self.phase_counter as f32).min(40.0),
            ZombiePhase::DancerRising => body_y,
            // Swimmers are cut off at the waterline.
            _ if self.in_pool => -self.altitude - 7.0 + (10.0 - 10.0 * self.scale) + if self.is_eating { 7.0 } else { 0.0 },
            _ => CLIP_HEIGHT_OFF,
        };
        DrawPos { head_x, head_y, body_y, image_offset_x, image_offset_y, clip_height }
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
    pub(super) fn attack_rect(&self) -> IRect {
        let mut rect = if self.phase == ZombiePhase::DolphinInJump { IRect::new(-40, 0, 100, 115) } else { self.attack_rect };
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
        // Plants only go for the zombies still against them.
        if (damage_range & damage_range::ONLY_MINDCONTROLLED != 0) != self.mind_controlled {
            return false;
        }
        if self.zombie_type == ZombieType::Bobsled && self.bobsled_position > 0 {
            return false;
        }
        // Behaviour per ruslan831 decomp, Zombie::EffectedByDamage: a bungee
        // zombie can only be hit while it's down on the lawn.
        if self.zombie_type == ZombieType::Bungee && !matches!(self.phase, ZombiePhase::BungeeAtBottom | ZombiePhase::BungeeGrabbing) {
            return false;
        }
        // Behaviour per ruslan831 decomp, Zombie::EffectedByDamage: the boss can
        // only be hurt while his head is down, and not while it's on its way.
        if self.is_boss() {
            let time = self.body_anim_time;
            if (self.phase == ZombiePhase::BossHeadEnter && time < 0.5) || (self.phase == ZombiePhase::BossHeadLeave && time > 0.5) {
                return false;
            }
            if !matches!(
                self.phase,
                ZombiePhase::BossHeadIdleBeforeSpit | ZombiePhase::BossHeadIdleAfterSpit | ZombiePhase::BossHeadSpit
            ) {
                return false;
            }
        }
        if self.zombie_type != ZombieType::Bobsled && self.zombie_rect().x > WIDE_BOARD_WIDTH as i32 {
            return false;
        }
        if matches!(
            self.phase,
            ZombiePhase::PoleInVault
                | ZombiePhase::ImpGettingThrown
                | ZombiePhase::RisingFromGrave
                | ZombiePhase::DancerRising
                | ZombiePhase::SnorkelIntoPool
                | ZombiePhase::DolphinIntoPool
                | ZombiePhase::DolphinInJump
                | ZombiePhase::BalloonPopping
                | ZombiePhase::DiggerRising
                | ZombiePhase::DiggerTunnelingPauseWithoutAxe
                | ZombiePhase::DiggerRiseWithoutAxe
        ) {
            return damage_range & damage_range::OFF_GROUND != 0;
        }
        // A snorkel under the water can only be reached by things that go under it.
        if self.zombie_type == ZombieType::Snorkel && self.in_pool && !self.is_eating {
            return damage_range & damage_range::SUBMERGED != 0;
        }
        // A tunnelling digger can only be reached by things that go under it.
        if self.phase == ZombiePhase::DiggerTunneling {
            return damage_range & damage_range::UNDERGROUND != 0;
        }
        if self.is_flying() {
            return damage_range & damage_range::FLYING != 0;
        }
        damage_range & damage_range::GROUND != 0
    }

    /// `Zombie::ZombieTargetLeadX`: where the middle of the zombie will be in
    /// `time` ticks, for lobbed shots and the squash's jump.
    pub fn target_lead_x(&self, time: f32) -> f32 {
        let mut speed = self.vel_x;
        if self.chilled_counter > 0 {
            speed *= 0.4;
        }
        if self.is_walking_backwards() {
            speed = -speed;
        }
        if self.zombie_not_walking() {
            speed = 0.0;
        }
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
                self.update_zombie_walking(fx);
            }
            ZombiePhase::Mowered => self.update_mowered(fx),
            ZombiePhase::Burned => self.update_burn(fx),
            _ => {
                if self.phase_counter > 0 && !self.is_immobilized() {
                    self.phase_counter -= 1;
                }
                self.update_playing(plants, env, fx);
                if self.yucky_face {
                    self.update_yucky_face(env, fx);
                }
                if self.zombie_type == ZombieType::Bungee {
                    self.update_zombie_bungee(plants, env, fx);
                }
                self.animate(plants, fx);
            }
        }
        self.just_got_shot_counter -= 1;
        self.shield_just_got_shot_counter = (self.shield_just_got_shot_counter - 1).max(0);
        self.shield_recoil_counter = (self.shield_recoil_counter - 1).max(0);
        if self.zombie_fade > 0 {
            self.zombie_fade -= 1;
            if self.zombie_fade == 0 {
                self.die_no_loot(fx.effects);
            }
        }
        self.x = self.pos_x as i32;
        self.y = self.pos_y as i32;
        if let Some(body) = self.body.and_then(|b| fx.effects.get(b)) {
            self.body_anim_time = body.anim_time;
        }
        // AttachmentUpdateAndMove.
        for &(id, dx, dy) in &self.attached {
            if let Some(system) = fx.effects.particle_mut(id) {
                system.system_move(self.pos_x + dx, self.pos_y + dy);
            }
        }
        if let Some(surprise) = self.surprise.and_then(|id| fx.effects.get_mut(id)) {
            surprise.set_position(self.pos_x - 11.0, self.pos_y);
        }
        self.update_reanim(fx.effects);
    }

    /// `Zombie::Update` for the zombie that's walking into the house after
    /// the level was lost: the others stand still.
    pub fn update_winner(&mut self, show_walking: bool, cutscene_time: i32, plants: &mut DataArray<Plant>, fx: &mut Fx) {
        self.age += 1;
        // UpdateZombieChimney: on the roof it drops down the chimney.
        if self.height == ZombieHeight::InToChimney {
            self.altitude = animate_curve_float(4000, 5000, cutscene_time, 200.0, 0.0, TodCurve::EaseIn);
        }
        if show_walking {
            self.update_zombie_walking(fx);
        }
        self.animate(plants, fx);
        self.just_got_shot_counter -= 1;
        self.x = self.pos_x as i32;
        self.y = self.pos_y as i32;
        self.update_reanim(fx.effects);
    }

    /// `Zombie::WalkIntoHouse`: the zombie that got in stands in the house's
    /// doorway. Day, night, pool and fog lawns share one height; the pool
    /// moves it 80 px left (10 for a football player) to the water's edge.
    pub fn walk_into_house(&mut self, background: BackgroundType, env: &ZombieEnv, fx: &mut Fx) {
        self.from_wave = ZOMBIE_WAVE_WINNER;
        // Behaviour per ruslan831 decomp, Zombie::WalkIntoHouse.
        if self.phase == ZombiePhase::PolePreVault {
            // A vaulter that hasn't jumped yet walks on, as after a vault
            // (PHASE_POLEVAULTER_POST_VAULT: the normal phase here).
            self.phase = ZombiePhase::Normal;
            self.attack_rect = IRect::new(50, 0, 20, 115);
            self.start_walk_anim(0, env, fx);
        }
        self.win_clip = match background {
            BackgroundType::Day | BackgroundType::Night => (-123.0, 600.0),
            BackgroundType::Pool | BackgroundType::Fog => (-172.0, 600.0),
            BackgroundType::Roof | BackgroundType::Boss => (-220.0, 187.0),
        };
        if matches!(background, BackgroundType::Roof | BackgroundType::Boss) {
            // The roof's house is a chimney the zombie drops into.
            self.pos_x = -180.0;
            self.pos_y = 250.0;
            self.height = ZombieHeight::InToChimney;
            self.render_order = make_render_order(render_layer::GRAVE_STONE, 0, 2);
            match self.zombie_type {
                ZombieType::Gargantuar | ZombieType::RedeyeGargantuar => self.pos_y += 5.0,
                ZombieType::Football => self.pos_x -= 14.0,
                ZombieType::Zamboni => self.pos_x -= 28.0,
                _ => {}
            }
            let has_idle = self.body.and_then(|b| fx.effects.get(b)).is_some_and(|b| b.definition.find_track("anim_idle").is_some());
            if has_idle && self.zombie_type != ZombieType::Polevaulter {
                self.play_zombie_reanim(fx.effects, "anim_idle", LoopType::Loop, 0, 15.0);
            }
            return;
        }
        self.pos_y = 290.0;
        self.render_order = make_render_order(render_layer::ZOMBIE, 2, 0);
        if matches!(background, BackgroundType::Day | BackgroundType::Night | BackgroundType::Pool | BackgroundType::Fog) {
            // The reference's "+35 x for a pole vaulter still running up" can't
            // happen: the vault ended above, so it's not in that phase any more.
            if matches!(self.zombie_type, ZombieType::Gargantuar | ZombieType::RedeyeGargantuar) {
                self.pos_y += 30.0;
            } else if self.zombie_type == ZombieType::Zamboni {
                self.pos_y += 15.0;
            }
            if background.has_pool() {
                self.pos_x -= if self.zombie_type == ZombieType::Football { 10.0 } else { 80.0 };
            }
        }
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
        if self.dead || self.phase == ZombiePhase::Mowered || self.is_boss() {
            return;
        }
        if self.zombie_type == ZombieType::Catapult {
            self.vehicle_explode(ParticleEffect::CatapultExplosion, fx);
            return;
        }
        if matches!(self.phase, ZombiePhase::Dying | ZombiePhase::RisingFromGrave | ZombiePhase::DancerRising) {
            // Falling over or still in the ground: just a puff of smoke.
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

    /// `Zombie::DropLoot`: asks the board whether this zombie ends the level
    /// or drops something worth its value.
    fn drop_loot(&mut self) {
        self.loot_pending = true;
    }

    /// `Zombie::DieWithLoot`.
    pub fn die_with_loot(&mut self, fx: &mut Fx) {
        self.die_no_loot(fx.effects);
        self.drop_loot();
    }

    /// `Zombie::UpdatePlaying`.
    fn update_playing(&mut self, plants: &mut DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        if self.ice_trap_counter > 0 {
            self.ice_trap_counter -= 1;
            if self.ice_trap_counter == 0 {
                // RemoveIceTrap.
                self.update_anim_speed(fx.effects);
                self.add_attached_particle(75.0, 106.0, ParticleEffect::IceTrapRelease, fx);
            }
        }
        if self.buttered_counter > 0 {
            self.buttered_counter -= 1;
            if self.buttered_counter == 0 {
                self.update_anim_speed(fx.effects);
            }
        }
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
            && !self.is_boss()
        {
            if self.zombie_type == ZombieType::Gargantuar {
                fx.sounds.play_foley(FoleyType::LowGroan);
            } else {
                // Little Trouble's zombies groan higher (ruslan831 decomp, Zombie::UpdatePlaying).
                let pitch = if env.tiny_zombies { fx.sounds.rng().range_f32(40.0, 50.0) } else { 0.0 };
                fx.sounds.play_foley_pitch(FoleyType::Groan, pitch);
            }
            self.groan_counter = fx.sounds.rng().next_range(1000) as i32 + 500;
        }
        if self.phase == ZombiePhase::RisingFromGrave {
            self.update_zombie_rise_from_grave();
            return;
        }

        self.water_row = env.pool_rows.get(self.row as usize).copied().unwrap_or(false);
        self.ladders = env.ladders;
        if self.height == ZombieHeight::UpLadder {
            self.update_climbing_ladder();
        }
        if matches!(self.height, ZombieHeight::IntoPool | ZombieHeight::OutOfPool) || self.in_pool {
            self.update_zombie_pool(env, fx);
        }
        if self.height == ZombieHeight::Falling {
            self.update_zombie_falling();
        }
        if !self.is_immobilized() {
            self.check_for_pool(env, fx);
            // UpdateActions.
            match self.zombie_type {
                ZombieType::Polevaulter => self.update_zombie_polevaulter(plants, env, fx),
                ZombieType::Snorkel => self.update_zombie_snorkel(env, fx),
                ZombieType::Zamboni => self.update_zamboni(),
                ZombieType::Bobsled => self.update_zombie_bobsled(env, fx),
                ZombieType::DolphinRider => self.update_zombie_dolphin_rider(plants, env, fx),
                ZombieType::Newspaper => self.update_zombie_newspaper(env, fx),
                ZombieType::Dancer => self.update_zombie_dancer(env, fx),
                ZombieType::BackupDancer => self.update_zombie_backup_dancer(env, fx),
                ZombieType::Ladder => self.update_ladder(plants, env, fx),
                ZombieType::Catapult => self.update_zombie_catapult(plants, fx),
                ZombieType::Gargantuar | ZombieType::RedeyeGargantuar => self.update_zombie_gargantuar(plants, env, fx),
                ZombieType::Imp => self.update_zombie_imp(env, fx),
                ZombieType::Balloon => self.update_zombie_flyer(env, fx),
                ZombieType::JackInTheBox => self.update_zombie_jack(fx),
                ZombieType::Digger => self.update_zombie_digger(env, fx),
                ZombieType::Pogo => self.update_zombie_pogo(plants, env, fx),
                ZombieType::Yeti => self.update_yeti(fx),
                ZombieType::PeaHead => self.update_zombie_pea_head(fx),
                ZombieType::GatlingHead => self.update_zombie_gatling_head(fx),
                ZombieType::JalapenoHead => self.update_zombie_jalapeno_head(fx),
                ZombieType::SquashHead => self.update_zombie_squash_head(plants, env, fx),
                _ => {}
            }
            self.update_zombie_position(fx, env);
            if self.is_vehicle() {
                if !self.flat_tires {
                    self.check_squish(plants, fx);
                }
            } else if !self.is_bobsled_team_with_sled() {
                self.check_if_prey_caught(plants, env, fx);
            }
        }

        if self.is_boss() {
            self.update_boss(plants, env, fx);
        }

        // A zombie that's lost its head dies slowly.
        let failing = !self.has_head || (self.is_vehicle() && self.body_health < 200);
        if !self.is_dead_or_dying() && failing && fx.sounds.rng().next_range(5) == 0 {
            let damage = if self.body_max_health >= 500 { 3 } else { 1 };
            self.take_damage(damage, damage::HEADLESS, env, fx);
        }
    }

    /// Whether the zombie is on a bobsled team that still has its sled (`IsBobsledTeamWithSled`).
    pub fn is_bobsled_team_with_sled(&self) -> bool {
        self.bobsled_position >= 0
    }

    /// `Zombie::BobsledCrash`, for the leader.
    fn bobsled_crash(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        self.altitude = 0.0;
        self.zombie_rect = IRect::new(36, 0, 42, 115);
        self.phase = ZombiePhase::BobsledCrashing;
        self.phase_counter = BOBSLED_CRASH_TIME;
        self.start_walk_anim(0, env, fx);
        self.bobsled_crashed = true;
    }

    /// `Zombie::BobsledCrash`, for the zombies behind the leader.
    pub fn bobsled_crash_follower(&mut self, leader_vel_x: f32, leader_anim_rate: f32, env: &ZombieEnv, fx: &mut Fx) {
        self.phase = ZombiePhase::BobsledCrashing;
        self.phase_counter = BOBSLED_CRASH_TIME;
        self.pos_y = self.pos_y_based_on_row(self.row, env);
        self.altitude = 0.0;
        self.start_walk_anim(0, env, fx);
        self.vel_x = leader_vel_x;
        let time = fx.sounds.rng().range_f32(0.0, 1.0);
        if let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b)) {
            body.anim_time = time;
            body.anim_rate = leader_anim_rate;
        }
    }

    /// `Zombie::UpdateZombieBobsled`: the team slides in, hops aboard, and
    /// rides the ice until the sled breaks or runs off the end of it.
    fn update_zombie_bobsled(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateZombieBobsled.
        if self.bobsled_crash_pending {
            self.bobsled_crash_pending = false;
            self.bobsled_crash(env, fx);
        }
        match self.phase {
            ZombiePhase::BobsledCrashing => {
                if self.phase_counter == 0 {
                    self.phase = ZombiePhase::Normal;
                    self.bobsled_position = -1;
                    self.leader = None;
                    self.followers = [None; NUM_BACKUP_DANCERS];
                    self.pick_random_speed(fx);
                }
                return;
            }
            ZombiePhase::BobsledSliding => {
                if self.phase_counter == 0 {
                    self.phase = ZombiePhase::BobsledBoarding;
                    self.play_zombie_reanim(fx.effects, "anim_jump", LoopType::PlayOnceAndHold, 0, 20.0);
                }
            }
            ZombiePhase::BobsledBoarding => {
                let time = self.body.and_then(|b| fx.effects.get(b)).map_or(1.0, |b| b.anim_time);
                let counter = (time * 50.0) as i32;
                let from = if matches!(self.bobsled_position, 1 | 3) { 8.0 } else { -9.0 };
                self.altitude = animate_curve_float(0, 50, counter, from, 18.0, TodCurve::Linear);
            }
            _ => return,
        }
        self.ice_keep = true;
        // Off the end of the ice, the sled falls apart.
        if self.bobsled_position == 0 && self.pos_x + 10.0 < env.ice_min_x[self.row as usize] as f32 {
            self.take_damage(6, damage::DOESNT_CAUSE_FLASH, env, fx);
        }
    }

    /// `Zombie::DrawBobsledReanim`: the sled itself, in two halves around the zombie.
    fn draw_bobsled(&self, g: &mut Graphics, res: &GameResources, before_zombie: bool) {
        let position = self.bobsled_position;
        let (mut front, mut back) = (false, false);
        let cutscene = self.from_wave == ZOMBIE_WAVE_CUTSCENE;
        if !cutscene && position == -1 {
            return;
        }
        if cutscene {
            back = before_zombie;
            front = !before_zombie;
        } else {
            let t = self.body_anim_time;
            match self.phase {
                ZombiePhase::BobsledCrashing => {
                    if position == 0 && !before_zombie {
                        front = true;
                        back = true;
                    }
                }
                ZombiePhase::BobsledSliding | ZombiePhase::Burned => {
                    if position == 2 && before_zombie {
                        front = true;
                        back = true;
                    }
                }
                ZombiePhase::BobsledBoarding => {
                    if t < 0.5 {
                        if position == 2 && before_zombie {
                            front = true;
                            back = true;
                        }
                    } else if position == 0 && !before_zombie {
                        front = true;
                    } else if position == 3 && before_zombie {
                        back = true;
                    }
                }
                _ => {}
            }
        }
        let (leader_x, damage_index) = if position <= 0 { (self.pos_x, self.helm_damage_index()) } else { (self.sled.pos_x, self.sled.damage_index) };
        let draw_pos = self.draw_pos();
        let mut x = leader_x + draw_pos.image_offset_x - self.pos_x - 76.0;
        let mut y = 15.0;
        let mut status = damage_index;
        let mut alpha = 255u8;
        if self.phase == ZombiePhase::BobsledCrashing {
            status = 3;
            let counter = self.phase_counter;
            alpha = animate_curve(30, 0, counter, 255, 0, TodCurve::Linear).clamp(0, 255) as u8;
            x += (BOBSLED_CRASH_TIME - counter) as f32 * self.vel_x / 0.4;
            x -= animate_curve_float(BOBSLED_CRASH_TIME, 0, counter, 0.0, 50.0, TodCurve::EaseOut);
            y += animate_curve_float(BOBSLED_CRASH_TIME, 75, counter, 5.0, 10.0, TodCurve::Linear);
        }
        let image = res.image(match status {
            0 => "IMAGE_ZOMBIE_BOBSLED1",
            1 => "IMAGE_ZOMBIE_BOBSLED2",
            2 => "IMAGE_ZOMBIE_BOBSLED3",
            _ => "IMAGE_ZOMBIE_BOBSLED4",
        });
        g.push_state();
        if alpha != 255 {
            g.set_colorize_images(true);
            g.set_color(SexyColor::rgba(255, 255, 255, alpha));
        }
        if self.phase == ZombiePhase::Burned {
            g.set_colorize_images(true);
            g.set_color(SexyColor::BLACK);
        }
        if back
            && status != 3
            && let Some(inside) = res.image("IMAGE_ZOMBIE_BOBSLED_INSIDE")
        {
            g.draw_image(inside, x, y);
        }
        if front && let Some(image) = image {
            g.draw_image(image, x, y);
        }
        g.pop_state();
    }

    /// What the board shows a follower of its leader's sled.
    pub fn sled_info(&self) -> SledInfo {
        SledInfo { pos_x: self.pos_x, damage_index: self.helm_damage_index() }
    }

    /// `Zombie::UpdateZamboni`: it speeds up on its way in, and its ice trail
    /// grows with it.
    fn update_zamboni(&mut self) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateZamboni.
        if self.pos_x > 400.0 && !self.flat_tires {
            self.vel_x = animate_curve_float(700, 300, self.pos_x as i32, 0.25, 0.05, TodCurve::Linear);
        } else if self.flat_tires && self.vel_x > 0.0005 {
            self.vel_x -= 0.0005;
        }
        let ice_x = (self.pos_x as i32 + 118).max(25);
        if ice_x < 800 {
            self.ice_trail = Some(ice_x);
        }
    }

    /// `Zombie::CheckSquish` for driving over: the first plant in reach is
    /// flattened, with everything else in its square.
    fn check_squish(&mut self, plants: &mut DataArray<Plant>, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::CheckSquish.
        use crate::plant::SeedType;
        let attack = self.attack_rect();
        let squishable = |p: &Plant| {
            !p.dead
                && !p.not_on_ground()
                && !matches!(p.seed, SeedType::Spikeweed | SeedType::Spikerock)
                && match p.seed {
                    SeedType::Cherrybomb | SeedType::Jalapeno | SeedType::Blover | SeedType::Squash => false,
                    SeedType::Doomshroom | SeedType::Iceshroom => p.is_asleep(),
                    _ => true,
                }
        };
        let target = plants
            .iter()
            .find(|(_, p)| p.row == self.row && rect_overlap(attack, p.rect()) >= 20 && squishable(p))
            .map(|(_, p)| (p.col, p.row));
        if let Some((col, row)) = target {
            self.squish_all_in_square(plants, col, row, fx);
        }
    }

    /// `Zombie::ZamboniDeath`: spikes pop its tires and leave it to coast
    /// and die; anything else blows it up.
    fn zamboni_death(&mut self, flags: u32, fx: &mut Fx) {
        if flags & damage::SPIKE != 0 {
            self.flat_tires = true;
            fx.sounds.play_sample("SOUND_BALLOON_POP");
            self.phase = ZombiePhase::Dying;
            let rng = fx.sounds.rng();
            let (x, y, order) = (self.pos_x + 29.0, self.pos_y + 114.0, self.render_order + 1);
            fx.effects.add_particle(x, y, order, ParticleEffect::ZamboniTire, fx.res, fx.images, rng);
            self.vel_x = 0.0;
            self.drop_loot();
            if fx.sounds.rng().next_range(4) == 0 && self.pos_x < 600.0 {
                self.play_zombie_reanim(fx.effects, "anim_wheelie2", LoopType::PlayOnceAndHold, 10, 10.0);
                self.phase_counter = 280;
            } else {
                self.add_attached_particle(35.0, 85.0, ParticleEffect::ZamboniSmoke, fx);
                self.phase_counter = 280;
                self.play_zombie_reanim(fx.effects, "anim_wheelie1", LoopType::PlayOnceAndHold, 10, 12.0);
            }
        } else {
            self.zamboni_explode(fx);
        }
    }

    fn zamboni_explode(&mut self, fx: &mut Fx) {
        self.vehicle_explode(ParticleEffect::ZamboniExplosion, fx);
    }

    /// `Zombie::UpdateZombieDolphinRider`: rides through the water and leaps
    /// the first plant in its way, then wades on foot.
    fn update_zombie_dolphin_rider(&mut self, plants: &DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateZombieDolphinRider.
        use crate::plant::SeedType;
        let backwards = self.is_walking_backwards();
        let anim = self.body.and_then(|b| fx.effects.get(b)).map(|b| (b.anim_time, b.loop_count > 0, b));
        let triggered = |t: f32| anim.as_ref().is_some_and(|(_, _, b)| b.should_trigger_timed_event(t));
        let (time, done) = anim.as_ref().map_or((1.0, true), |(t, d, _)| (*t, *d));
        let splash = |this: &Self, dx: f32, fx: &mut Fx| {
            if let Ok((_, splash)) = fx.effects.add_reanimation(this.pos_x + dx, this.pos_y + 73.0, ReanimationType::Splash, fx.res, fx.images) {
                splash.loop_type = LoopType::PlayOnce;
                splash.render_order = this.render_order + 1;
                splash.override_scale(1.2, 0.8);
            }
            let rng = fx.sounds.rng();
            let effect = ParticleEffect::PlantingPool;
            fx.effects.add_particle(this.pos_x + dx + 37.0, this.pos_y + 115.0, this.render_order + 1, effect, fx.res, fx.images, rng);
            fx.sounds.play_foley(FoleyType::ZombieEnteringWater);
        };
        let (into_splash, jump_splash) = (triggered(0.56), triggered(0.49));
        let block_check = triggered(0.3);
        match self.phase {
            ZombiePhase::DolphinWalking if !backwards => {
                if self.x > 700 && self.x <= 720 {
                    self.phase = ZombiePhase::DolphinIntoPool;
                    self.play_zombie_reanim(fx.effects, "anim_jumpinpool", LoopType::PlayOnceAndHold, 20, 16.0);
                }
            }
            ZombiePhase::DolphinIntoPool => {
                if into_splash {
                    splash(self, -83.0, fx);
                }
                if done {
                    self.pos_x -= 70.0;
                    self.phase = ZombiePhase::DolphinRiding;
                    self.in_pool = true;
                    self.attack_rect = IRect::new(-29, 0, 70, 115);
                    self.play_zombie_reanim(fx.effects, "anim_ride", LoopType::LoopFullLastFrame, 0, 12.0);
                }
            }
            ZombiePhase::DolphinRiding => {
                if self.x <= 10 {
                    self.altitude = -40.0;
                    self.height = ZombieHeight::OutOfPool;
                    self.phase = ZombiePhase::DolphinWalking;
                    self.pool_splash(false, fx);
                    self.play_zombie_reanim(fx.effects, "anim_walkdolphin", LoopType::Loop, 0, 0.0);
                    self.pick_random_speed(fx);
                    return;
                }
                if self.has_head && self.find_plant_target(plants).is_some() {
                    fx.sounds.play_foley(FoleyType::DolphinBeforeJumping);
                    fx.sounds.play_foley(FoleyType::PlantWater);
                    self.vel_x = 0.5;
                    self.phase = ZombiePhase::DolphinInJump;
                    self.phase_counter = DOLPHIN_JUMP_TIME;
                    self.play_zombie_reanim(fx.effects, "anim_dolphinjump", LoopType::PlayOnceAndHold, 0, 10.0);
                }
            }
            ZombiePhase::DolphinInJump => {
                self.altitude = animate_curve_float(DOLPHIN_JUMP_TIME, 0, self.phase_counter, 0.0, 10.0, TodCurve::Linear);
                let mut ended = false;
                if block_check {
                    // A tall-nut is too high to leap: the jump ends against it.
                    if let Some(plant) = self.find_plant_target(plants).and_then(|id| plants.get(id)).filter(|p| p.seed == SeedType::Tallnut) {
                        fx.sounds.play_foley(FoleyType::Bonk);
                        let order = self.render_order + 1;
                        let rng = fx.sounds.rng();
                        fx.effects.add_particle((plant.x + 60) as f32, (plant.y - 20) as f32, order, ParticleEffect::TallNutBlock, fx.res, fx.images, rng);
                        ended = true;
                        self.pos_x = plant.x as f32 + 25.0;
                        self.altitude = 0.0;
                    }
                } else if jump_splash {
                    splash(self, -63.0, fx);
                    self.vel_x = 0.0;
                } else if done {
                    ended = true;
                    self.pos_x -= 94.0;
                    self.altitude = 0.0;
                }
                if ended {
                    self.attack_rect = IRect::new(30, 0, 30, 115);
                    self.zombie_rect = IRect::new(20, 0, 42, 115);
                    self.phase = ZombiePhase::DolphinWalkingInPool;
                    self.start_walk_anim(0, env, fx);
                }
            }
            ZombiePhase::DolphinWalkingInPool if (self.x <= 10 && !backwards) || (self.x > 680 && backwards) => {
                self.altitude = -40.0;
                self.height = ZombieHeight::OutOfPool;
                self.phase = ZombiePhase::DolphinWalkingWithoutDolphin;
                self.pool_splash(false, fx);
                self.play_zombie_reanim(fx.effects, "anim_walk", LoopType::Loop, 0, 0.0);
                self.pick_random_speed(fx);
            }
            _ => {}
        }
        let _ = time;
    }

    /// `Zombie::UpdateZombieSnorkel`: walks to the water, dives in, swims
    /// submerged, and surfaces to eat whatever blocks the way.
    fn update_zombie_snorkel(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateZombieSnorkel.
        let backwards = self.is_walking_backwards();
        match self.phase {
            ZombiePhase::SnorkelWalking if !backwards => {
                if self.x > 700 && self.x <= 720 {
                    self.vel_x = 0.2;
                    self.phase = ZombiePhase::SnorkelIntoPool;
                    self.play_zombie_reanim(fx.effects, "anim_jumpinpool", LoopType::PlayOnceAndHold, 20, 16.0);
                }
            }
            ZombiePhase::SnorkelIntoPool => {
                let (time, done) = self.body.and_then(|b| fx.effects.get(b)).map_or((1.0, true), |b| (b.anim_time, b.loop_count > 0));
                self.altitude = time * 10.0;
                if self.body.and_then(|b| fx.effects.get(b)).is_some_and(|b| b.should_trigger_timed_event(0.83)) {
                    let (x, y) = (self.pos_x - 47.0, self.pos_y + 73.0);
                    if let Ok((_, splash)) = fx.effects.add_reanimation(x, y, ReanimationType::Splash, fx.res, fx.images) {
                        splash.loop_type = LoopType::PlayOnce;
                        splash.render_order = self.render_order + 1;
                        splash.override_scale(1.2, 0.8);
                    }
                    let rng = fx.sounds.rng();
                    let effect = ParticleEffect::PlantingPool;
                    fx.effects.add_particle(self.pos_x - 10.0, self.pos_y + 115.0, self.render_order + 1, effect, fx.res, fx.images, rng);
                    fx.sounds.play_foley(FoleyType::ZombieEnteringWater);
                }
                if done {
                    self.phase = ZombiePhase::SnorkelWalkingInPool;
                    self.in_pool = true;
                    self.play_zombie_reanim(fx.effects, "anim_swim", LoopType::LoopFullLastFrame, 0, 12.0);
                }
            }
            ZombiePhase::SnorkelWalkingInPool => {
                if !self.has_head {
                    self.take_damage(1800, damage::HEADLESS, env, fx);
                } else if (self.x <= 25 && !backwards) || (self.x > 640 && backwards) {
                    self.altitude = -90.0;
                    self.pos_x += if backwards { 15.0 } else { -15.0 };
                    self.phase = ZombiePhase::SnorkelWalking;
                    self.height = ZombieHeight::OutOfPool;
                    self.pool_splash(false, fx);
                    self.start_walk_anim(0, env, fx);
                } else if self.is_eating {
                    self.phase = ZombiePhase::SnorkelUpToEat;
                    self.play_zombie_reanim(fx.effects, "anim_uptoeat", LoopType::PlayOnceAndHold, 0, 24.0);
                }
            }
            ZombiePhase::SnorkelUpToEat => {
                let done = self.body.and_then(|b| fx.effects.get(b)).is_some_and(|b| b.loop_count > 0);
                if !self.is_eating {
                    self.phase = ZombiePhase::SnorkelDownFromEat;
                    self.play_zombie_reanim(fx.effects, "anim_uptoeat", LoopType::PlayOnceAndHold, 0, -24.0);
                } else if done {
                    self.phase = ZombiePhase::SnorkelEatingInPool;
                    self.play_zombie_reanim(fx.effects, "anim_eat", LoopType::Loop, 0, 0.0);
                }
            }
            ZombiePhase::SnorkelEatingInPool => {
                if !self.is_eating {
                    self.phase = ZombiePhase::SnorkelDownFromEat;
                    self.play_zombie_reanim(fx.effects, "anim_uptoeat", LoopType::PlayOnceAndHold, 0, -24.0);
                }
            }
            ZombiePhase::SnorkelDownFromEat if self.body.and_then(|b| fx.effects.get(b)).is_some_and(|b| b.loop_count > 0) => {
                self.phase = ZombiePhase::SnorkelWalkingInPool;
                self.play_zombie_reanim(fx.effects, "anim_swim", LoopType::LoopFullLastFrame, 0, 0.0);
                self.pick_random_speed(fx);
            }
            _ => {}
        }
    }

    /// `Zombie::UpdateZombiePolevaulter`: runs until a plant's in reach,
    /// then vaults over it and walks on.
    fn update_zombie_polevaulter(&mut self, plants: &DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        if self.phase == ZombiePhase::PolePreVault && self.has_head && self.height == ZombieHeight::Normal {
            if let Some(plant) = self.find_plant_target_for(plants, AttackType::Vault).and_then(|id| plants.get(id)) {
                // Behaviour per ruslan831 decomp, Zombie::UpdateZombiePolevaulter:
                // a ladder's quicker than a jump.
                if self.ladder_at(plant.col, plant.row) {
                    if (plant.x + 40) as f32 > self.pos_x && self.use_ladder_col != plant.col {
                        self.height = ZombieHeight::UpLadder;
                        self.use_ladder_col = plant.col;
                    }
                    return;
                }
                let plant_x = plant.x;
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
            let mut ended = body.loop_count > 0;
            let anim_time = body.anim_time;
            let (step, vault) = (body.should_trigger_timed_event(0.2), body.should_trigger_timed_event(0.4));
            // Behaviour per ruslan831 decomp, Zombie::UpdateZombiePolevaulter:
            // a tall-nut is too high, so the jump ends against it.
            let blocker = if anim_time > 0.6 && anim_time <= 0.7 {
                self.find_plant_target(plants).and_then(|id| plants.get(id)).filter(|p| p.seed == crate::plant::SeedType::Tallnut)
            } else {
                None
            };
            if let Some(plant) = blocker {
                fx.sounds.play_foley(FoleyType::Bonk);
                let order = self.render_order + 1;
                let rng = fx.sounds.rng();
                fx.effects.add_particle((plant.x + 60) as f32, (plant.y - 20) as f32, order, ParticleEffect::TallNutBlock, fx.res, fx.images, rng);
                self.pos_x = plant.x as f32;
                ended = true;
            } else if ended {
                self.pos_x -= 150.0;
            }
            if step {
                fx.sounds.play_sample("SOUND_GRASSSTEP");
            }
            if vault {
                fx.sounds.play_sample("SOUND_POLEVAULT");
            }
            if ended {
                self.x = self.pos_x as i32;
                self.phase = ZombiePhase::Normal;
                self.attack_rect = IRect::new(50, 0, 20, 115);
                self.start_walk_anim(0, env, fx);
            }
        }
    }

    /// `Zombie::RiseFromGrave`: the zombie starts under the grave whose
    /// column is at `grave_x` in its row, and climbs out. TODO: rising out
    /// of the pool.
    /// `Zombie::FindCatapultTarget`: the leftmost plant in its row at least
    /// 100 px ahead, the top one on its square. Spikes are left alone.
    fn find_catapult_target(&self, plants: &DataArray<Plant>) -> Option<Id<Plant>> {
        use crate::plant::SeedType;
        plants
            .iter()
            .filter(|(_, p)| {
                p.row == self.row && !p.dead && self.x >= p.x + 100 && !p.not_on_ground() && !matches!(p.seed, SeedType::Spikeweed | SeedType::Spikerock)
            })
            .min_by_key(|(_, p)| p.col)
            .and_then(|(_, p)| super::plant::top_plant_any(plants, p.col, p.row))
    }

    /// `Zombie::UpdateZombieCatapult`: from 650 px in it stops to lob its
    /// basketballs at the plants, one every three seconds, then drives on.
    fn update_zombie_catapult(&mut self, plants: &DataArray<Plant>, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateZombieCatapult.
        match self.phase {
            ZombiePhase::Normal => {
                if self.pos_x <= 650.0 && self.summon_counter > 0 && self.find_catapult_target(plants).is_some() {
                    self.phase = ZombiePhase::CatapultLaunching;
                    self.phase_counter = 300;
                    self.play_zombie_reanim(fx.effects, "anim_shoot", LoopType::PlayOnceAndHold, 0, 24.0);
                }
            }
            ZombiePhase::CatapultLaunching => {
                let Some(body) = self.body.and_then(|b| fx.effects.get(b)) else { return };
                let (fire, done) = (body.should_trigger_timed_event(0.545), body.loop_count > 0);
                if fire {
                    let target = self.find_catapult_target(plants).and_then(|id| plants.get(id)).map(|p| (p.x as f32, p.y as f32));
                    self.catapult_fire(target, fx);
                }
                if done {
                    self.summon_counter -= 1;
                    let ball = match self.summon_counter {
                        4 => Some("Zombie_catapult_basketball"),
                        3 => Some("Zombie_catapult_basketball2"),
                        2 => Some("Zombie_catapult_basketball3"),
                        1 => Some("Zombie_catapult_basketball4"),
                        _ => None,
                    };
                    if let Some(track) = ball {
                        self.reanim_show_track(fx, track, RENDER_GROUP_HIDDEN);
                    }
                    if self.summon_counter == 0 {
                        self.play_zombie_reanim(fx.effects, "anim_walk", LoopType::Loop, 20, 6.0);
                        self.phase = ZombiePhase::Normal;
                    } else {
                        self.play_zombie_reanim(fx.effects, "anim_idle", LoopType::Loop, 20, 12.0);
                        self.phase = ZombiePhase::CatapultReloading;
                    }
                }
            }
            ZombiePhase::CatapultReloading if self.phase_counter == 0 => {
                if self.find_catapult_target(plants).is_some() {
                    self.phase = ZombiePhase::CatapultLaunching;
                    self.phase_counter = 300;
                    self.play_zombie_reanim(fx.effects, "anim_shoot", LoopType::PlayOnceAndHold, 20, 24.0);
                } else {
                    self.play_zombie_reanim(fx.effects, "anim_walk", LoopType::Loop, 20, 6.0);
                    self.phase = ZombiePhase::Normal;
                }
            }
            _ => {}
        }
        // UpdateReanim: the pole's look follows the damage and the balls left.
        let damaged = self.body_damage_index() == 2;
        if let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b)) {
            let image = match (damaged, self.summon_counter) {
                (true, 0) => Some("IMAGE_REANIM_ZOMBIE_CATAPULT_POLE_DAMAGE"),
                (true, _) => Some("IMAGE_REANIM_ZOMBIE_CATAPULT_POLE_DAMAGE_WITHBALL"),
                (false, 0) => Some("IMAGE_REANIM_ZOMBIE_CATAPULT_POLE"),
                (false, _) => None,
            };
            if image.is_some() {
                body.set_image_override("Zombie_catapult_pole", image);
            }
        }
    }

    /// `Zombie::ZombieCatapultFire`: a basketball lobbed onto the plant at
    /// (x, y), or out in front if there's none.
    fn catapult_fire(&mut self, target: Option<(f32, f32)>, fx: &mut Fx) {
        let (x, y) = (self.pos_x + 113.0, self.pos_y - 44.0);
        let (target_x, target_y) = target.unwrap_or((self.pos_x - 300.0, 0.0));
        fx.sounds.play_foley(FoleyType::Basketball);
        let range_x = (x - target_x - 20.0).max(40.0);
        let range_y = target_y - y;
        self.thrown_basketball = Some(Basketball { x, y, vel_x: -range_x / 120.0, vel_z: range_y / 120.0 - 7.0 });
    }

    /// `Zombie::PogoBreak`: the pogo stick breaks and it walks the rest.
    fn pogo_break(&mut self, flags: u32, env: &ZombieEnv, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::PogoBreak.
        if !self.has_object {
            return;
        }
        if flags & damage::DOESNT_LEAVE_BODY == 0 {
            let (x, y) = self.track_position(fx.effects, "Zombie_pogo_stick");
            let rng = fx.sounds.rng();
            let particle = fx.effects.add_particle(x, y + 30.0, self.render_order + 1, ParticleEffect::ZombiePogo, fx.res, fx.images, rng);
            self.scale_particle(fx, particle);
        }
        self.height = ZombieHeight::Falling;
        self.phase = ZombiePhase::Normal;
        self.start_walk_anim(0, env, fx);
        self.zombie_rect = IRect::new(36, 17, 42, 115);
        self.attack_rect = IRect::new(20, 17, 50, 115);
        self.shield_health = 0;
        self.shield_type = ShieldType::None;
        self.has_object = false;
    }

    /// The headless half of `PogoBreak` for `DropHead`, which has no board
    /// environment for a fresh walk animation: it plays the walk and picks
    /// its speed directly.
    fn pogo_break_on_head_loss(&mut self, fx: &mut Fx) {
        if !self.has_object {
            return;
        }
        self.height = ZombieHeight::Falling;
        self.phase = ZombiePhase::Normal;
        self.play_zombie_reanim(fx.effects, "anim_walk", LoopType::Loop, 0, 0.0);
        self.pick_random_speed(fx);
        self.zombie_rect = IRect::new(36, 17, 42, 115);
        self.attack_rect = IRect::new(20, 17, 50, 115);
        self.shield_health = 0;
        self.shield_type = ShieldType::None;
    }

    /// `Zombie::DiggerLoseAxe`: the magnet-shroom takes its pickaxe, and
    /// without it the digger comes up on its own.
    pub fn digger_lose_axe(&mut self, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::DiggerLoseAxe.
        if self.phase == ZombiePhase::DiggerTunneling {
            self.phase = ZombiePhase::DiggerTunnelingPauseWithoutAxe;
            self.phase_counter = 200;
            self.apply_anim_rate(fx.effects, 0.0);
            self.update_anim_speed(fx.effects);
            let drop: Vec<_> = self.attached.iter().map(|(id, _, _)| *id).collect();
            for id in drop {
                fx.effects.remove_particle(id);
            }
            self.attached.clear();
        }
        self.has_object = false;
        self.reanim_show_track(fx, "Zombie_digger_pickaxe", RENDER_GROUP_HIDDEN);
        self.reanim_show_track(fx, "Zombie_digger_dirt", RENDER_GROUP_HIDDEN);
    }

    /// `Zombie::UpdateZombieJackInTheBox`: it runs at the house, then pops
    /// its lid and blows everything around it up.
    fn update_zombie_jack(&mut self, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateZombieJackInTheBox.
        if self.phase == ZombiePhase::JackRunning {
            if self.phase_counter <= 0 && self.has_head {
                self.phase_counter = 110;
                self.phase = ZombiePhase::JackPopping;
                self.play_zombie_reanim(fx.effects, "anim_pop", LoopType::PlayOnceAndHold, 20, 28.0);
                fx.sounds.play_sample("SOUND_BOING");
            }
        } else if self.phase == ZombiePhase::JackPopping {
            if self.phase_counter == 80 {
                fx.sounds.play_foley(FoleyType::JackSurprise);
            }
            if self.phase_counter <= 0 {
                let r = self.zombie_rect();
                let (x, y) = (r.x + r.w / 2, r.y + r.h / 2);
                self.jack_blast = Some(JackBlast { row: self.row, x, y });
                self.die_no_loot(fx.effects);
            }
        }
    }

    /// `Zombie::UpdateZombieDigger`: underground to the back of the lawn,
    /// then up and back through the defences.
    fn update_zombie_digger(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateZombieDigger.
        match self.phase {
            ZombiePhase::DiggerTunneling => {
                if self.pos_x < 10.0 {
                    self.altitude = -120.0;
                    self.phase = ZombiePhase::DiggerRising;
                    self.phase_counter = 130;
                    self.play_zombie_reanim(fx.effects, "anim_drill", LoopType::Loop, 0, 20.0);
                    fx.sounds.play_foley(FoleyType::DirtRise);
                    fx.sounds.play_foley(FoleyType::WakeUp);
                    let drop: Vec<_> = self.attached.iter().map(|(id, _, _)| *id).collect();
                    for id in drop {
                        fx.effects.remove_particle(id);
                    }
                    self.attached.clear();
                    let rng = fx.sounds.rng();
                    fx.effects.add_particle(self.pos_x + 60.0, self.pos_y + 118.0, self.render_order + 1, ParticleEffect::DiggerRise, fx.res, fx.images, rng);
                    if let Ok((_, dirt)) = fx.effects.add_reanimation(self.pos_x + 13.0, self.pos_y + 97.0, ReanimationType::DiggerDirt, fx.res, fx.images) {
                        dirt.anim_rate = 24.0;
                        dirt.render_order = self.render_order + 1;
                    }
                }
            }
            ZombiePhase::DiggerRising => {
                if self.phase_counter > 40 {
                    self.altitude = animate_curve_float(130, 40, self.phase_counter, -120.0, 20.0, TodCurve::EaseOut);
                } else {
                    self.altitude = animate_curve_float(30, 0, self.phase_counter, 20.0, 0.0, TodCurve::EaseIn);
                }
                if self.phase_counter == 30 {
                    self.play_zombie_reanim(fx.effects, "anim_landing", LoopType::PlayOnceAndHold, 0, 12.0);
                }
                if self.phase_counter == 0 {
                    self.altitude = 0.0;
                    self.phase = ZombiePhase::DiggerStunned;
                    self.play_zombie_reanim(fx.effects, "anim_dizzy", LoopType::Loop, 10, 12.0);
                }
            }
            ZombiePhase::DiggerTunnelingPauseWithoutAxe => {
                if self.phase_counter == 150 {
                    use ReanimationType::ZombieSurprise;
                    if let Ok((id, surprise)) = fx.effects.add_reanimation(23.0, 93.0, ZombieSurprise, fx.res, fx.images) {
                        surprise.render_order = self.render_order;
                        self.surprise = Some(id);
                    }
                }
                if self.phase_counter == 0 {
                    self.altitude = -120.0;
                    self.phase = ZombiePhase::DiggerRiseWithoutAxe;
                    self.phase_counter = 130;
                    self.play_zombie_reanim(fx.effects, "anim_landing", LoopType::PlayOnceAndHold, 0, 0.0);
                    fx.sounds.play_foley(FoleyType::DirtRise);
                    let rng = fx.sounds.rng();
                    fx.effects.add_particle(self.pos_x + 60.0, self.pos_y + 118.0, self.render_order + 1, ParticleEffect::DiggerRise, fx.res, fx.images, rng);
                    if let Ok((_, dirt)) = fx.effects.add_reanimation(self.pos_x + 13.0, self.pos_y + 97.0, ReanimationType::DiggerDirt, fx.res, fx.images) {
                        dirt.anim_rate = 24.0;
                        dirt.render_order = self.render_order + 1;
                    }
                }
            }
            ZombiePhase::DiggerRiseWithoutAxe => {
                if self.phase_counter > 40 {
                    self.altitude = animate_curve_float(130, 40, self.phase_counter, -120.0, 20.0, TodCurve::EaseOut);
                } else {
                    self.altitude = animate_curve_float(30, 0, self.phase_counter, 20.0, 0.0, TodCurve::EaseIn);
                }
                if self.phase_counter == 30 {
                    self.play_zombie_reanim(fx.effects, "anim_landing", LoopType::PlayOnceAndHold, 20, 12.0);
                }
                if self.phase_counter == 0 {
                    self.altitude = 0.0;
                    self.phase = ZombiePhase::DiggerWalkingWithoutAxe;
                    self.start_walk_anim(20, env, fx);
                }
            }
            ZombiePhase::DiggerStunned => {
                let dizzy = self.body.and_then(|b| fx.effects.get(b)).is_some_and(|b| b.loop_count > 1);
                if dizzy {
                    self.phase = ZombiePhase::DiggerWalking;
                    self.start_walk_anim(20, env, fx);
                }
            }
            _ => {}
        }
    }

    /// `Zombie::UpdateZombiePogo`: it bounces along, hops high over the
    /// first plant in its way, and lands past it (unless it's a tall-nut).
    fn update_zombie_pogo(&mut self, plants: &DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateZombiePogo.
        if self.is_dead_or_dying() || self.is_immobilized() {
            return;
        }
        if !matches!(self.phase, ZombiePhase::PogoBouncing | ZombiePhase::PogoHighBounce | ZombiePhase::PogoForwardBounce) {
            return;
        }
        if self.height == ZombieHeight::InToChimney {
            return;
        }
        let height = match self.phase {
            ZombiePhase::PogoHighBounce => 110.0,
            ZombiePhase::PogoForwardBounce => 170.0,
            _ => 40.0,
        };
        self.altitude = animate_curve_float(POGO_BOUNCE_TIME, 0, self.phase_counter, 9.0, height + 9.0, TodCurve::BounceSlowMiddle);
        if self.phase_counter == 7
            && let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b))
        {
            body.anim_time = 0.0;
            body.loop_type = LoopType::PlayOnceAndHold;
        }
        if self.is_on_board() && self.phase_counter == 5 {
            fx.sounds.play_foley(FoleyType::PogoZombie);
        }
        if self.phase == ZombiePhase::PogoForwardBounce && self.phase_counter == 70 {
            let tallnut = self
                .find_plant_target(plants)
                .and_then(|id| plants.get(id))
                .filter(|p| p.seed == crate::plant::SeedType::Tallnut);
            if let Some(plant) = tallnut {
                fx.sounds.play_foley(FoleyType::Bonk);
                let order = self.render_order + 1;
                let rng = fx.sounds.rng();
                fx.effects.add_particle((plant.x + 60) as f32, (plant.y - 20) as f32, order, ParticleEffect::TallNutBlock, fx.res, fx.images, rng);
                self.pogo_break(0, env, fx);
                return;
            }
        }
        if self.phase_counter != 0 {
            return;
        }
        let target = if self.is_on_board() { self.find_plant_target_for(plants, AttackType::Vault) } else { None };
        let Some(id) = target else {
            self.phase = ZombiePhase::PogoBouncing;
            self.pick_random_speed(fx);
            self.phase_counter = POGO_BOUNCE_TIME;
            return;
        };
        if self.phase == ZombiePhase::PogoHighBounce {
            let plant_x = plants.get(id).map(|p| p.x).unwrap_or(self.x);
            self.phase = ZombiePhase::PogoForwardBounce;
            self.vel_x = (self.x - plant_x + 60) as f32 / POGO_BOUNCE_TIME as f32;
            self.phase_counter = POGO_BOUNCE_TIME;
        } else {
            self.phase = ZombiePhase::PogoHighBounce;
            self.vel_x = 0.0;
            self.phase_counter = POGO_BOUNCE_TIME;
        }
    }

    /// `Zombie::UpdateYeti`: it lumbers at the house, then gives up and runs.
    fn update_yeti(&mut self, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateYeti.
        if self.mind_controlled || !self.has_head || self.is_dead_or_dying() {
            return;
        }
        if self.phase == ZombiePhase::Normal && self.phase_counter == 0 {
            self.phase = ZombiePhase::YetiRunning;
            self.has_object = false;
            self.pick_random_speed(fx);
        }
    }

    /// Where a zombotany head's pea leaves its plant head.
    fn head_pea_origin(&self, effects: &EffectSystem) -> (f32, f32) {
        let (mut dx, mut dy) = (-9.0, 6.0);
        if let Some(body) = self.body.and_then(|b| effects.get(b)) {
            let m = body.track_matrix(body.find_track("anim_head1"));
            (dx, dy) = (m.m02 - 9.0, m.m12 + 6.0);
        }
        (self.pos_x + dx, self.pos_y + dy - self.altitude)
    }

    /// Fires a pea backwards at the plants, or at the zombies when
    /// hypnotized (`Zombie::UpdateZombiePeaHead`'s shot).
    fn fire_zombie_pea(&mut self, fx: &mut Fx) {
        let (x, y) = self.head_pea_origin(fx.effects);
        fx.sounds.play_foley(FoleyType::Throw);
        self.fired_pea = Some(FiredPea { x, y, row: self.row, mind_controlled: self.mind_controlled });
    }

    /// `Zombie::UpdateZombiePeaHead`: a peashooter on a zombie's shoulders,
    /// firing backwards every 150 ticks.
    fn update_zombie_pea_head(&mut self, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateZombiePeaHead.
        if !self.has_head {
            return;
        }
        if self.phase_counter == 35 {
            if let Some(head) = self.special_head.and_then(|id| fx.effects.get_mut(id)) {
                head.play("anim_shooting", LoopType::PlayOnceAndHold, 20, 35.0);
            }
        } else if self.phase_counter == 0 {
            if let Some(head) = self.special_head.and_then(|id| fx.effects.get_mut(id)) {
                head.play("anim_head_idle", LoopType::PlayOnceAndHold, 20, 15.0);
            }
            self.fire_zombie_pea(fx);
            self.phase_counter = 150;
        }
    }

    /// `Zombie::UpdateZombieGatlingHead`: four peas per volley.
    fn update_zombie_gatling_head(&mut self, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateZombieGatlingHead.
        if !self.has_head {
            return;
        }
        if self.phase_counter == 100 {
            if let Some(head) = self.special_head.and_then(|id| fx.effects.get_mut(id)) {
                head.play("anim_shooting", LoopType::PlayOnceAndHold, 20, 38.0);
            }
        } else if matches!(self.phase_counter, 18 | 35 | 51 | 68) {
            self.fire_zombie_pea(fx);
        } else if self.phase_counter == 0 {
            if let Some(head) = self.special_head.and_then(|id| fx.effects.get_mut(id)) {
                head.play("anim_head_idle", LoopType::PlayOnceAndHold, 20, 15.0);
            }
            self.phase_counter = 150;
        }
    }

    /// `Zombie::UpdateZombieJalapenoHead`: it counts down, then burns its row.
    fn update_zombie_jalapeno_head(&mut self, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateZombieJalapenoHead.
        if !self.has_head || self.phase_counter != 0 {
            return;
        }
        fx.sounds.play_foley(FoleyType::JalapenoIgnite);
        fx.sounds.play_foley(FoleyType::Juicy);
        self.jalapeno_blast = Some(JalapenoBlast { row: self.row, mind_controlled: self.mind_controlled });
        self.die_no_loot(fx.effects);
    }

    /// `Zombie::UpdateZombieSquashHead`: its squash jumps on whatever it eats.
    fn update_zombie_squash_head(&mut self, plants: &mut DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateZombieSquashHead.
        if self.has_head && self.is_eating && self.phase == ZombiePhase::SquashPreLaunch {
            self.stop_eating(env, fx);
            self.play_zombie_reanim(fx.effects, "anim_idle", LoopType::Loop, 20, 12.0);
            self.has_head = false;
            let order = self.render_order + 1;
            if let Some(id) = self.special_head.take() {
                fx.effects.kill(id);
            }
            let (sx, sy) = (self.pos_x + 6.0, self.pos_y - 21.0);
            if let Ok((id, head)) = fx.effects.add_reanimation(sx, sy, ReanimationType::Squash, fx.res, fx.images) {
                head.play("anim_jumpup", LoopType::PlayOnceAndHold, 20, 24.0);
                head.render_order = order;
                head.override_scale(0.75, 0.75);
                self.special_head = Some(id);
            }
            self.phase = ZombiePhase::SquashRising;
            self.phase_counter = 95;
        }
        if self.phase == ZombiePhase::SquashRising {
            let col = ((self.x.max(80) - super::cursor::LAWN_XMIN) / 80).clamp(0, super::MAX_GRID_SIZE_X as i32 - 1);
            let dest = super::cursor::LAWN_XMIN + col * 80;
            let dx = animate_curve(95, 0, self.phase_counter, 0, dest - self.pos_x as i32, TodCurve::EaseInOut) as f32;
            let dy = animate_curve(50, 20, self.phase_counter, 0, -20, TodCurve::EaseInOut) as f32;
            if let Some(head) = self.special_head.and_then(|id| fx.effects.get_mut(id)) {
                head.overlay = sexy::matrix::Matrix3::translation(self.pos_x + dx + 6.0, self.pos_y + dy - 21.0);
            }
            if self.phase_counter == 0 {
                if let Some(head) = self.special_head.and_then(|id| fx.effects.get_mut(id)) {
                    head.play("anim_jumpdown", LoopType::PlayOnceAndHold, 0, 60.0);
                }
                self.phase = ZombiePhase::SquashFalling;
                self.phase_counter = 10;
            }
        }
        if self.phase == ZombiePhase::SquashFalling {
            let col = ((self.x.max(80) - super::cursor::LAWN_XMIN) / 80).clamp(0, super::MAX_GRID_SIZE_X as i32 - 1);
            let dest = super::cursor::LAWN_XMIN + col * 80;
            let dy = animate_curve(10, 0, self.phase_counter, -20, 74, TodCurve::Linear) as f32;
            if let Some(head) = self.special_head.and_then(|id| fx.effects.get_mut(id)) {
                head.overlay = sexy::matrix::Matrix3::translation(self.pos_x + 6.0 + dest as f32 - self.pos_x, self.pos_y - 21.0 + dy);
            }
            if self.phase_counter == 2 {
                self.squish_all_in_square(plants, col, self.row, fx);
            }
            if self.phase_counter == 0 {
                self.phase = ZombiePhase::SquashDoneFalling;
                self.phase_counter = 100;
                self.shake = Some((1, 4));
                fx.sounds.play_foley(FoleyType::Thump);
            }
        }
        if self.phase == ZombiePhase::SquashDoneFalling && self.phase_counter == 0 {
            if let Some(id) = self.special_head.take() {
                fx.effects.kill(id);
            }
            self.take_damage(1800, damage::HEADLESS, env, fx);
        }
    }

    /// `Zombie::UpdateZombieGargantuar`: it brings its pole down on the
    /// plant in front of it, and once it's down to half health it flings its
    /// imp over the plants.
    fn update_zombie_gargantuar(&mut self, plants: &mut DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateZombieGargantuar.
        let Some(body) = self.body.and_then(|b| fx.effects.get(b)) else { return };
        let (smash, throw, done) = (body.should_trigger_timed_event(0.64), body.should_trigger_timed_event(0.74), body.loop_count > 0);
        if self.phase == ZombiePhase::GargantuarSmashing {
            if smash {
                if let Some(id) = self.find_plant_target(plants)
                    && let Some((col, row, seed)) = plants.get(id).map(|p| (p.col, p.row, p.seed))
                {
                    if seed == crate::plant::SeedType::Spikerock {
                        // The spikes hurt it back, and the rock loses one.
                        self.take_damage(20, damage::SPIKE, env, fx);
                        let mut actions = Vec::new();
                        let gone = plants.get_mut(id).is_some_and(|p| p.spike_rock_take_damage(&mut actions, fx.effects));
                        for action in actions {
                            if let crate::board::plant::PlantAction::Foley(foley) = action {
                                fx.sounds.play_foley(foley);
                            }
                        }
                        if gone {
                            self.squish_all_in_square(plants, col, row, fx);
                        }
                    } else {
                        self.squish_all_in_square(plants, col, row, fx);
                    }
                }
                fx.sounds.play_foley(FoleyType::Thump);
                self.shake = Some((0, 3));
            }
            if done {
                self.phase = ZombiePhase::Normal;
                self.start_walk_anim(20, env, fx);
            }
            return;
        }
        let mut throw_distance = self.pos_x - 360.0;
        if self.phase == ZombiePhase::GargantuarThrowing {
            if throw {
                self.has_object = false;
                self.reanim_show_prefix(fx, "Zombie_imp", RENDER_GROUP_HIDDEN);
                self.reanim_show_track(fx, "Zombie_gargantuar_whiterope", RENDER_GROUP_HIDDEN);
                fx.sounds.play_foley(FoleyType::Swing);
                let mut min_distance = 40.0;
                if env.background.has_roof() {
                    throw_distance -= 180.0;
                    min_distance = -140.0;
                }
                if throw_distance < min_distance {
                    throw_distance = min_distance;
                } else if throw_distance > 140.0 {
                    throw_distance -= fx.sounds.rng().range_f32(0.0, 100.0);
                }
                // The imp flies at 3 px a tick, and lands this far along.
                let vel_z = 0.5 * (throw_distance / 3.0) * THROWN_ZOMBIE_GRAVITY;
                self.thrown_imp = Some(ThrownImp {
                    x: self.pos_x - 133.0,
                    row: self.row,
                    vel_z,
                    chilled_counter: self.chilled_counter,
                    render_order: self.render_order + 1,
                });
                fx.sounds.play_foley(FoleyType::Imp);
            }
            if done {
                self.phase = ZombiePhase::Normal;
                self.start_walk_anim(20, env, fx);
            }
            return;
        }
        if self.is_immobilized() || !self.has_head {
            return;
        }
        if self.has_object && self.body_health < self.body_max_health / 2 && throw_distance > 40.0 {
            self.phase = ZombiePhase::GargantuarThrowing;
            self.play_zombie_reanim(fx.effects, "anim_throw", LoopType::PlayOnceAndHold, 20, 24.0);
            return;
        }
        if self.find_plant_target(plants).is_some() {
            self.phase = ZombiePhase::GargantuarSmashing;
            fx.sounds.play_foley(FoleyType::LowGroan);
            self.play_zombie_reanim(fx.effects, "anim_smash", LoopType::PlayOnceAndHold, 20, 16.0);
        }
    }

    /// `Zombie::SquishAllInSquare`: everything on the square is flattened
    /// (but a spikerock takes it).
    fn squish_all_in_square(&mut self, plants: &mut DataArray<Plant>, col: i32, row: i32, fx: &mut Fx) {
        for (_, plant) in plants.iter_mut() {
            if plant.col == col && plant.row == row && !plant.dead && plant.seed != crate::plant::SeedType::Spikerock {
                self.ate_plant = Some((plant.seed, plant.col));
                if plant.squish(fx.effects) {
                    fx.sounds.play_foley(FoleyType::Squish);
                }
            }
        }
    }

    /// The board adds a gargantuar's imp; this sets it flying
    /// (`UpdateZombieGargantuar`'s throw).
    pub fn throw_as_imp(&mut self, throw: &ThrownImp, env: &ZombieEnv, fx: &mut Fx) {
        self.row = throw.row;
        self.pos_x = throw.x;
        self.pos_y = self.pos_y_based_on_row(throw.row, env);
        self.variant = false;
        self.altitude = 88.0;
        self.render_order = throw.render_order;
        self.phase = ZombiePhase::ImpGettingThrown;
        self.vel_x = 3.0;
        self.chilled_counter = throw.chilled_counter;
        self.vel_z = throw.vel_z;
        self.play_zombie_reanim(fx.effects, "anim_thrown", LoopType::PlayOnceAndHold, 0, 18.0);
        self.x = self.pos_x as i32;
        self.y = self.pos_y as i32;
        self.update_reanim(fx.effects);
    }

    /// `Zombie::UpdateZombieImp`: it flies in an arc, then lands and walks.
    fn update_zombie_imp(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateZombieImp.
        match self.phase {
            ZombiePhase::ImpGettingThrown => {
                self.vel_z -= THROWN_ZOMBIE_GRAVITY;
                self.altitude += self.vel_z;
                self.pos_x -= self.vel_x;
                // It follows the roof's slope as it flies.
                let diff_y = self.pos_y_based_on_row(self.row, env) - self.pos_y;
                self.pos_y += diff_y;
                self.altitude += diff_y;
                if self.altitude <= 0.0 {
                    self.altitude = 0.0;
                    self.phase = ZombiePhase::ImpLanding;
                    self.play_zombie_reanim(fx.effects, "anim_land", LoopType::PlayOnceAndHold, 0, 24.0);
                }
            }
            ZombiePhase::ImpLanding if self.body.and_then(|b| fx.effects.get(b)).is_some_and(|b| b.loop_count > 0) => {
                self.phase = ZombiePhase::Normal;
                self.start_walk_anim(0, env, fx);
            }
            _ => {}
        }
    }

    /// Whether a ladder leans on the plant at `(col, row)` (`GetLadderAt`).
    fn ladder_at(&self, col: i32, row: i32) -> bool {
        usize::try_from(col).ok().zip(usize::try_from(row).ok()).is_some_and(|(c, r)| self.ladders.get(c).and_then(|l| l.get(r)).copied().unwrap_or(false))
    }

    /// `Zombie::UpdateLadder`: the ladder zombie runs up to a nut and props
    /// its ladder against it, then climbs over.
    fn update_ladder(&mut self, plants: &DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateLadder.
        if self.mind_controlled || !self.has_head || self.is_dead_or_dying() {
            return;
        }
        if self.phase == ZombiePhase::LadderCarrying && self.height == ZombieHeight::Normal {
            if self.find_plant_target_for(plants, AttackType::Ladder).is_some() {
                self.stop_eating(env, fx);
                self.phase = ZombiePhase::LadderPlacing;
                self.play_zombie_reanim(fx.effects, "anim_placeladder", LoopType::PlayOnceAndHold, 10, 24.0);
            }
        } else if self.phase == ZombiePhase::LadderPlacing {
            if !self.body.and_then(|b| fx.effects.get(b)).is_some_and(|b| b.loop_count > 0) {
                return;
            }
            if let Some(plant) = self.find_plant_target_for(plants, AttackType::Ladder).and_then(|id| plants.get(id)) {
                self.placed_ladder = Some((plant.col, plant.row));
                fx.sounds.play_sample("SOUND_LADDER_ZOMBIE");
                self.height = ZombieHeight::UpLadder;
                self.use_ladder_col = plant.col;
                self.detach_shield(env, fx);
            } else {
                self.phase = ZombiePhase::LadderCarrying;
                self.start_walk_anim(0, env, fx);
            }
        }
    }

    /// `Zombie::UpdateClimbingLadder`: up 90 px while it keeps walking, then
    /// it drops down the far side. Without the ladder under it, it falls.
    fn update_climbing_ladder(&mut self) {
        let x = self.x as f32 + 5.0 + self.altitude * 0.5;
        // PixelToGridXKeepOnBoard.
        let col = ((x as i32 - super::cursor::LAWN_XMIN).div_euclid(80)).clamp(0, super::MAX_GRID_SIZE_X as i32 - 1);
        if !self.ladder_at(col, self.row) {
            self.height = ZombieHeight::Falling;
            return;
        }
        self.altitude += 0.8;
        if self.vel_x < 0.5 {
            self.pos_x -= 0.5;
        }
        if self.altitude >= 90.0 {
            self.height = ZombieHeight::Falling;
        }
    }

    /// `Zombie::UpdateZombieFalling`.
    /// `Zombie::IsFlying`: the balloon is up, or just popped.
    pub fn is_flying(&self) -> bool {
        matches!(self.phase, ZombiePhase::BalloonFlying | ZombiePhase::BalloonPopping)
    }

    /// `Zombie::TakeFlyingDamage`: the balloon takes the blow first; once it
    /// is gone the zombie lands.
    fn take_flying_damage(&mut self, damage: i32, flags: u32, env: &ZombieEnv, fx: &mut Fx) -> i32 {
        if flags & damage::DOESNT_CAUSE_FLASH == 0 {
            self.just_got_shot_counter = 25;
        }
        let actual = self.flying_health.min(damage);
        self.flying_health -= actual;
        if self.flying_health == 0 {
            self.land_flyer(flags, env, fx);
        }
        damage - actual
    }

    /// `Zombie::LandFlyer`: the balloon pops and the zombie drops (or drowns).
    fn land_flyer(&mut self, flags: u32, env: &ZombieEnv, fx: &mut Fx) {
        if flags & damage::DOESNT_LEAVE_BODY == 0 && self.phase == ZombiePhase::BalloonFlying {
            fx.sounds.play_sample("SOUND_BALLOON_POP");
            self.phase = ZombiePhase::BalloonPopping;
            self.play_zombie_reanim(fx.effects, "anim_pop", LoopType::PlayOnceAndHold, 20, 24.0);
        }
        if env.pool_rows.get(self.row as usize).copied().unwrap_or(false) {
            self.die_with_loot(fx);
        } else {
            self.height = ZombieHeight::Falling;
        }
    }

    /// `Zombie::UpdateZombieFlyer`: once the balloon's popped, it walks on.
    fn update_zombie_flyer(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        if self.phase == ZombiePhase::BalloonPopping && self.body.and_then(|b| fx.effects.get(b)).is_some_and(|b| b.loop_count > 0) {
            self.phase = ZombiePhase::BalloonWalking;
            self.start_walk_anim(0, env, fx);
        }
    }

    /// A blover blows the flyers away (`Plant::BlowAwayFliers`).
    pub fn blow_away_if_flying(&mut self) {
        if !self.is_dead_or_dying() && self.phase == ZombiePhase::BalloonFlying {
            self.blowing_away = true;
        }
    }

    fn update_zombie_falling(&mut self) {
        self.altitude -= 1.0;
        if self.phase == ZombiePhase::PolePreVault {
            self.altitude -= 1.0;
        }
        if self.altitude <= 0.0 {
            self.altitude = 0.0;
            self.height = ZombieHeight::Normal;
        }
    }

    /// `Zombie::UpdateZombieBungee`: dive, wait, grab, and spring back up.
    fn update_zombie_bungee(&mut self, plants: &mut DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        // Behaviour per ruslan831 decomp, Zombie::UpdateZombieBungee.
        if self.is_dead_or_dying() || self.is_immobilized() {
            return;
        }
        match self.phase {
            ZombiePhase::BungeeDiving | ZombiePhase::BungeeDivingScreaming => {
                let old_altitude = self.altitude;
                self.altitude -= 8.0;
                let thud = BUNGEE_ZOMBIE_HEIGHT - 404.0;
                if self.altitude <= thud && old_altitude > thud {
                    fx.sounds.play_foley(FoleyType::GrassStep);
                }
                self.bungee_landing(plants, env, fx);
            }
            ZombiePhase::BungeeAtBottom if self.phase_counter <= 0 => {
                self.bungee_steal_target(plants, fx);
                self.phase = ZombiePhase::BungeeGrabbing;
            }
            ZombiePhase::BungeeGrabbing => {
                if self.body.and_then(|b| fx.effects.get(b)).is_some_and(|b| b.loop_count > 0) {
                    self.bungee_lift_target(plants, fx);
                    self.phase = ZombiePhase::BungeeRising;
                }
            }
            ZombiePhase::BungeeRising => {
                self.altitude += 8.0;
                if self.altitude >= 600.0 {
                    // BungeeDie: the board finishes off the plant it carried.
                    self.die_no_loot(fx.effects);
                }
            }
            _ => {}
        }
    }

    /// `Zombie::BungeeLanding`: an umbrella leaf next to its square bounces
    /// it back up before it touches down.
    fn bungee_landing(&mut self, plants: &mut DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        // (The boss's bungees don't scream.)
        if self.phase == ZombiePhase::BungeeDiving && self.altitude < 1500.0 && !env.final_boss {
            fx.sounds.play_foley(FoleyType::BungeeScream);
            self.phase = ZombiePhase::BungeeDivingScreaming;
        }
        if self.altitude > 40.0 {
            return;
        }
        if let Some(umbrella) = super::plant::find_umbrella_plant(plants, self.target_col, self.row).and_then(|id| plants.get_mut(id)) {
            fx.sounds.play_sample("SOUND_BOING");
            fx.sounds.play_foley(FoleyType::Umbrella);
            umbrella.open_umbrella(fx.effects);
            self.phase = ZombiePhase::BungeeRising;
            self.render_order = make_render_order(render_layer::TOP, 0, 1);
            self.hit_umbrella = true;
            return;
        }
        if self.altitude > 0.0 {
            return;
        }
        self.altitude = 0.0;
        self.phase = ZombiePhase::BungeeAtBottom;
        self.phase_counter = 300;
        self.play_zombie_reanim(fx.effects, "anim_idle", LoopType::Loop, 5, 24.0);
        if let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b)) {
            body.anim_time = 0.5;
        }
    }

    /// `Zombie::BungeeStealTarget`: takes hold of the top plant on its square.
    fn bungee_steal_target(&mut self, plants: &mut DataArray<Plant>, fx: &mut Fx) {
        use crate::plant::SeedType;
        self.play_zombie_reanim(fx.effects, "anim_grab", LoopType::PlayOnceAndHold, 20, 24.0);
        let Some(id) = super::plant::top_plant_any(plants, self.target_col, self.row) else { return };
        let Some(plant) = plants.get_mut(id) else { return };
        if !plant.not_on_ground() && !matches!(plant.seed, SeedType::Cobcannon | SeedType::Gravebuster) {
            self.target_plant = Some(id);
            plant.on_bungee = super::plant::OnBungee::Grabbed;
            self.render_order = make_render_order(render_layer::PROJECTILE, self.row, 0);
        }
    }

    /// `Zombie::BungeeLiftTarget`: up it goes, plant and all.
    fn bungee_lift_target(&mut self, plants: &mut DataArray<Plant>, fx: &mut Fx) {
        self.play_zombie_reanim(fx.effects, "anim_raise", LoopType::PlayOnceAndHold, 0, 36.0);
        let Some(plant) = self.target_plant.and_then(|id| plants.get_mut(id)) else { return };
        plant.on_bungee = super::plant::OnBungee::Rising;
        fx.sounds.play_foley(FoleyType::Floop);
        plant.set_body_anim_rate(fx.effects, 0.1);
    }

    pub fn rise_from_grave(&mut self, grave_x: i32, env: &ZombieEnv, fx: &mut Fx) {
        self.start_rising(grave_x, env);
        fx.sounds.play_foley(FoleyType::GravestoneRumble);
        let order = make_render_order(render_layer::PARTICLE, self.row, 0);
        let rng = fx.sounds.rng();
        fx.effects.add_particle(self.pos_x + 60.0, self.pos_y + 110.0, order, ParticleEffect::ZombieRise, fx.res, fx.images, rng);
        self.update_reanim(fx.effects);
    }

    /// `RiseFromGrave` on whack-a-zombie: it pops up quickly, at `speed`.
    pub fn rise_out_of_whack_grave(&mut self, grave_x: i32, speed: f32, env: &ZombieEnv, fx: &mut Fx) {
        self.start_rising(grave_x, env);
        self.phase_counter = 50;
        fx.sounds.play_foley(FoleyType::DirtRise);
        let order = make_render_order(render_layer::PARTICLE, self.row, 0);
        let rng = fx.sounds.rng();
        let effect = ParticleEffect::WhackAZombieRise;
        fx.effects.add_particle(self.pos_x + 60.0, self.pos_y + 110.0, order, effect, fx.res, fx.images, rng);
        self.vel_x = speed;
        self.update_anim_speed(fx.effects);
        self.update_reanim(fx.effects);
    }

    /// The start of `Zombie::RiseFromGrave`, under the grave in its row.
    fn start_rising(&mut self, grave_x: i32, env: &ZombieEnv) {
        self.pos_x = (grave_x - 25) as f32;
        self.pos_y = self.pos_y_based_on_row(self.row, env);
        self.x = self.pos_x as i32;
        self.y = self.pos_y as i32;
        self.altitude = CLIP_HEIGHT_OFF;
        self.phase = ZombiePhase::RisingFromGrave;
        self.phase_counter = 150;
        self.render_order = make_render_order(render_layer::ZOMBIE, self.row, 4);
    }

    /// `MouseDownWhackAZombie` on this zombie: a helmet takes 900 damage;
    /// a bare head means it's done for. Returns whether it was knocked out.
    pub fn whack(&mut self, fx: &mut Fx) -> bool {
        match self.helm_type {
            HelmType::None => {
                fx.sounds.play_foley(FoleyType::Bonk);
                self.die_with_loot(fx);
                true
            }
            helm => {
                match helm {
                    HelmType::Pail => fx.sounds.play_foley(FoleyType::ShieldHit),
                    HelmType::TrafficCone => fx.sounds.play_foley(FoleyType::PlasticHit),
                    _ => {}
                }
                self.take_helm_damage(900, 0, fx);
                false
            }
        }
    }

    /// `Zombie::UpdateZombieRiseFromGrave`.
    fn update_zombie_rise_from_grave(&mut self) {
        self.altitude = animate_curve(50, 0, self.phase_counter, -200, 0, TodCurve::Linear) as f32;
        if self.phase_counter == 0 {
            self.phase = ZombiePhase::Normal;
        }
    }

    /// `Zombie::UpdateZombieNewspaper`: once it's got over losing its paper,
    /// it storms on.
    fn update_zombie_newspaper(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        if self.phase != ZombiePhase::NewspaperMaddening {
            return;
        }
        if !self.body.and_then(|b| fx.effects.get(b)).is_some_and(|b| b.loop_count > 0) {
            return;
        }
        self.phase = ZombiePhase::NewspaperMad;
        if env.zombies_on_screen <= 10 && self.has_head {
            fx.sounds.play_foley(FoleyType::NewspaperRarrgh);
        }
        self.start_walk_anim(20, env, fx);
        if let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b)) {
            body.set_image_override("anim_head1", Some("IMAGE_REANIM_ZOMBIE_PAPER_MADHEAD"));
        }
    }

    /// `Zombie::GetDancerFrame`: every dancer on the lawn steps in time.
    /// Counted from the board's clock (the original's fixed version).
    fn dancer_frame(&self, env: &ZombieEnv) -> i32 {
        if self.is_immobilized() {
            return 0;
        }
        let (frame_length, frames) = if self.phase == ZombiePhase::DancerDancingIn { (10, 11) } else { (20, 23) };
        env.main_counter.rem_euclid(frame_length * frames) / frame_length
    }

    /// `Zombie::GetDancerPhase`.
    fn dancer_phase(&self, env: &ZombieEnv) -> ZombiePhase {
        match self.dancer_frame(env) {
            0..=11 => ZombiePhase::DancerDancingLeft,
            12 => ZombiePhase::DancerWalkToRaise,
            13..=15 => ZombiePhase::DancerRaiseRight1,
            16..=18 => ZombiePhase::DancerRaiseLeft1,
            19..=21 => ZombiePhase::DancerRaiseRight2,
            _ => ZombiePhase::DancerRaiseLeft2,
        }
    }

    /// The dance steps every dancer follows (part of `UpdateZombieDancer`
    /// and `UpdateZombieBackupDancer`).
    fn follow_dance(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        let phase = self.dancer_phase(env);
        if phase == self.phase {
            return;
        }
        self.phase = phase;
        match phase {
            ZombiePhase::DancerDancingLeft => self.play_zombie_reanim(fx.effects, "anim_walk", LoopType::Loop, 10, 0.0),
            ZombiePhase::DancerWalkToRaise => {
                self.play_zombie_reanim(fx.effects, "anim_armraise", LoopType::Loop, 10, 18.0);
                if let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b)) {
                    body.anim_time = 0.6;
                }
            }
            _ => self.play_zombie_reanim(fx.effects, "anim_armraise", LoopType::Loop, 10, 18.0),
        }
    }

    /// `Zombie::UpdateZombieDancer`: moonwalks in, snaps its fingers for
    /// backup dancers, then dances on with them, calling up more as they fall.
    fn update_zombie_dancer(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        if self.is_eating {
            return;
        }
        if self.summon_counter > 0 {
            self.summon_counter -= 1;
            if self.summon_counter == 0 {
                if self.dancer_frame(env) == 12 && self.has_head && self.pos_x < 700.0 {
                    self.phase = ZombiePhase::DancerSnappingFingersWithLight;
                    self.play_zombie_reanim(fx.effects, "anim_point", LoopType::PlayOnceAndHold, 20, 24.0);
                } else {
                    self.summon_counter = 1;
                }
            }
        }
        match self.phase {
            ZombiePhase::DancerDancingIn => {
                if self.has_head && self.phase_counter == 0 {
                    self.phase = ZombiePhase::DancerSnappingFingers;
                    self.play_zombie_reanim(fx.effects, "anim_point", LoopType::PlayOnceAndHold, 20, 24.0);
                    self.pick_random_speed(fx);
                }
            }
            ZombiePhase::DancerSnappingFingers | ZombiePhase::DancerSnappingFingersWithLight => {
                if self.body.and_then(|b| fx.effects.get(b)).is_some_and(|b| b.loop_count > 0) {
                    if self.phase == ZombiePhase::DancerSnappingFingers && env.zombies_on_screen <= 15 {
                        fx.sounds.play_foley(FoleyType::Dancer);
                    }
                    if self.has_head {
                        self.summon_requested = true;
                    }
                    self.phase = ZombiePhase::DancerSnappingFingersHold;
                    self.phase_counter = 200;
                }
            }
            _ => {
                if self.phase == ZombiePhase::DancerSnappingFingersHold {
                    if self.phase_counter != 0 {
                        return;
                    }
                    self.phase = ZombiePhase::DancerDancingLeft;
                    self.play_zombie_reanim(fx.effects, "anim_walk", LoopType::Loop, 20, 0.0);
                }
                self.follow_dance(env, fx);
                if self.has_head && self.summon_counter == 0 && self.needs_more_backup {
                    self.summon_counter = 100;
                }
            }
        }
    }

    /// `Zombie::UpdateZombieBackupDancer`: rises out of the ground, then
    /// dances in step.
    fn update_zombie_backup_dancer(&mut self, env: &ZombieEnv, fx: &mut Fx) {
        if self.is_eating {
            return;
        }
        if self.phase == ZombiePhase::DancerRising {
            self.altitude = animate_curve(150, 0, self.phase_counter, BACKUP_DANCER_RISE_HEIGHT, 0, TodCurve::Linear) as f32;
            if self.phase_counter != 0 {
                return;
            }
        }
        self.follow_dance(env, fx);
    }

    /// `Zombie::SummonBackupDancer`, for the zombie just made to be one:
    /// it comes up out of the ground at `x` in `row`.
    pub fn rise_as_backup_dancer(&mut self, row: i32, x: f32, leader: Id<Zombie>, env: &ZombieEnv, fx: &mut Fx) {
        self.pos_x = x;
        self.row = row;
        self.pos_y = self.pos_y_based_on_row(row, env);
        self.render_order = make_render_order(render_layer::ZOMBIE, row, 4);
        self.x = self.pos_x as i32;
        self.y = self.pos_y as i32;
        self.altitude = BACKUP_DANCER_RISE_HEIGHT as f32;
        self.phase = ZombiePhase::DancerRising;
        self.phase_counter = 150;
        self.leader = Some(leader);
        // SetAnimRate(0): it stands still while it rises.
        self.original_anim_rate = 0.0;
        self.apply_anim_rate(fx.effects, 0.0);
        let order = make_render_order(render_layer::PARTICLE, row, 0);
        let rng = fx.sounds.rng();
        fx.effects.add_particle(self.pos_x + 60.0, self.pos_y + 110.0, order, ParticleEffect::DancerRise, fx.res, fx.images, rng);
        fx.sounds.play_foley(FoleyType::GravestoneRumble);
        self.update_reanim(fx.effects);
    }

    /// `Zombie::UpdateZombiePosition`.
    fn update_zombie_position(&mut self, fx: &mut Fx, env: &ZombieEnv) {
        if matches!(self.zombie_type, ZombieType::Bungee | ZombieType::Boss) {
            return;
        }
        self.update_zombie_walking(fx);
        // Behaviour per ruslan831 decomp, Zombie::UpdateZombiePosition: a
        // blover sweeps the balloon off the lawn.
        if self.blowing_away {
            self.pos_x += 10.0;
            if self.pos_x as i32 > 850 {
                self.die_with_loot(fx);
                return;
            }
        }
        let desired = self.pos_y_based_on_row(self.row, env);
        if self.pos_y < desired {
            self.pos_y += (desired - self.pos_y).min(1.0);
        } else if self.pos_y > desired {
            self.pos_y -= (self.pos_y - desired).min(1.0);
        }
    }

    /// `Zombie::UpdateZombieWalking`: the walk animation's `_ground` track
    /// says how far to move each tick.
    fn update_zombie_walking(&mut self, fx: &mut Fx) {
        if self.zombie_not_walking() || self.is_boss() {
            return;
        }
        let body = self.body.and_then(|b| fx.effects.get(b));
        let speed = match body {
            _ if self.is_vehicle() || self.is_bobsled_team_with_sled() => self.vel_x,
            // Behaviour per ruslan831 decomp, Zombie::UpdateZombieWalking: a
            // bouncing pogo stick and a tunnelling digger move at their own
            // speed, not the walk track's.
            _ if matches!(
                self.phase,
                ZombiePhase::PoleInVault
                    | ZombiePhase::SnorkelIntoPool
                    | ZombiePhase::DolphinInJump
                    | ZombiePhase::DiggerTunneling
            ) => self.vel_x,
            _ if matches!(
                self.phase,
                ZombiePhase::SnorkelWalkingInPool
                    | ZombiePhase::DolphinRiding
                    | ZombiePhase::BalloonFlying
                    | ZombiePhase::PogoBouncing
                    | ZombiePhase::PogoHighBounce
                    | ZombiePhase::PogoForwardBounce
            ) => {
                if self.is_moving_at_chilled_speed() { self.vel_x * 0.4 } else { self.vel_x }
            }
            // Behaviour per ruslan831 decomp, Zombie::UpdateZombieWalking: the walk's speed is scaled too.
            Some(body) if body.definition.find_track("_ground").is_some() => body.track_velocity("_ground") * self.scale,
            _ if self.is_moving_at_chilled_speed() => self.vel_x * 0.4,
            _ => self.vel_x,
        };
        // The dancer moonwalks in backwards.
        if self.is_walking_backwards() || self.phase == ZombiePhase::DancerDancingIn {
            self.pos_x += speed;
        } else {
            self.pos_x -= speed;
        }
        // The football zombie's feet and the running pole vaulter's kick up dust.
        let steps = if self.zombie_type == ZombieType::Football && !self.is_winner() {
            Some([(0.03, 81.0, 106.0), (0.61, 87.0, 110.0)])
        } else if self.phase == ZombiePhase::PolePreVault {
            Some([(0.16, 81.0, 106.0), (0.67, 87.0, 110.0)])
        } else {
            None
        };
        if let Some(steps) = steps
            && let Some(body) = body
        {
            let dust: Vec<_> = steps.iter().filter(|(t, _, _)| body.should_trigger_timed_event(*t)).collect();
            for &&(_, dx, dy) in &dust {
                let rng = fx.sounds.rng();
                let (x, y) = (self.x as f32 + dx, self.y as f32 + dy);
                fx.effects.add_particle(x, y, self.render_order - 1, ParticleEffect::DustFoot, fx.res, fx.images, rng);
            }
        }
    }

    /// `Zombie::CheckIfPreyCaught`: every few ticks, bite whatever's in reach.
    fn check_if_prey_caught(&mut self, plants: &mut DataArray<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        // The gargantuar smashes rather than bites.
        if matches!(self.zombie_type, ZombieType::Bungee | ZombieType::Gargantuar | ZombieType::RedeyeGargantuar) || self.is_flying() {
            return;
        }
        // A digger under the lawn walks straight past the plants, and so does
        // one still coming up (`Zombie::CheckIfPreyCaught`); a bouncing pogo
        // stick never stops to bite either.
        if matches!(
            self.phase,
            ZombiePhase::DiggerTunneling
                | ZombiePhase::DiggerCutscene
                | ZombiePhase::DiggerRising
                | ZombiePhase::DiggerTunnelingPauseWithoutAxe
                | ZombiePhase::DiggerRiseWithoutAxe
                | ZombiePhase::DiggerStunned
                | ZombiePhase::PogoBouncing
                | ZombiePhase::PogoHighBounce
                | ZombiePhase::PogoForwardBounce
        ) {
            return;
        }
        let ticks_between_eats = if self.chilled_counter > 0 { TICKS_BETWEEN_EATS * 2 } else { TICKS_BETWEEN_EATS };
        use ZombiePhase::*;
        let busy = matches!(
            self.phase,
            NewspaperMaddening
                | RisingFromGrave
                | DancerRising
                | DancerWalkToRaise
                | DancerRaiseLeft1
                | DancerRaiseRight1
                | DancerRaiseLeft2
                | DancerRaiseRight2
                | DancerSnappingFingers
                | DancerSnappingFingersWithLight
                | DancerSnappingFingersHold
                | SnorkelWalking
                | SnorkelIntoPool
                | DolphinWalking
                | DolphinWalkingWithoutDolphin
                | DolphinIntoPool
                | DolphinRiding
                | DolphinInJump
                | LadderPlacing
                | ImpGettingThrown
                | ImpLanding
        );
        let wading = self.height != ZombieHeight::Normal;
        if !self.has_head || self.age % ticks_between_eats != 0 || self.is_vaulting() || busy || wading {
            return;
        }
        // Zombies on opposite sides eat each other first (EatZombie).
        if let Some(target) = self.zombie_target {
            self.start_eating(fx);
            self.bitten_zombie = Some(target);
            return;
        }
        // Zombies ignore the rolling nuts on the bowling level (CanTargetPlant),
        // and hypnotized zombies leave plants alone.
        let target = if env.bowling || self.mind_controlled { None } else { self.find_plant_target(plants) };
        if let Some(id) = target {
            self.eat_plant(plants, id, env, fx);
        } else if self.is_eating {
            self.stop_eating(env, fx);
        }
    }

    /// `Zombie::FindPlantTarget`, for chewing.
    fn find_plant_target(&self, plants: &DataArray<Plant>) -> Option<sexy::data_array::Id<Plant>> {
        self.find_plant_target_for(plants, AttackType::Chew)
    }

    /// `Zombie::FindPlantTarget`.
    fn find_plant_target_for(&self, plants: &DataArray<Plant>, attack_type: AttackType) -> Option<sexy::data_array::Id<Plant>> {
        // Behaviour per ruslan831 decomp, Zombie::CanTargetPlant: a zombie that
        // isn't swimming can't bite plants on the water.
        if self.water_row && !self.in_pool {
            return None;
        }
        // A ladder zombie props its ladder against nuts (with no ladder yet)
        // and chews everything else.
        let ladder_rule = |plant: &Plant| {
            if !matches!(self.phase, ZombiePhase::LadderCarrying | ZombiePhase::LadderPlacing) || attack_type == AttackType::Vault {
                return true;
            }
            use crate::plant::SeedType;
            let place_ladder = matches!(plant.seed, SeedType::Wallnut | SeedType::Tallnut | SeedType::Pumpkinshell) && !self.ladder_at(plant.col, plant.row);
            place_ladder == (attack_type == AttackType::Ladder)
        };
        // A gargantuar smashes spikes too (CanTargetPlant).
        let smashes = matches!(self.zombie_type, ZombieType::Gargantuar | ZombieType::RedeyeGargantuar);
        let targetable = |plant: &Plant| {
            use crate::plant::SeedType;
            can_target(plant) || (smashes && matches!(plant.seed, SeedType::Spikeweed | SeedType::Spikerock) && !plant.not_on_ground())
        };
        let attack = self.attack_rect();
        let is_under = |p: &Plant| matches!(p.seed, crate::plant::SeedType::Lilypad | crate::plant::SeedType::Flowerpot);
        plants
            .iter()
            .find(|(_, plant)| {
                plant.row == self.row
                    && !plant.dead
                    && rect_overlap(attack, plant.rect()) >= 20
                    && targetable(plant)
                    && ladder_rule(plant)
                    // A pumpkin is eaten before the plant inside it (GetTopPlantAt).
                    && !(plant.seed != crate::plant::SeedType::Pumpkinshell
                        && !is_under(plant)
                        && plants.iter().any(|(_, other)| {
                            other.col == plant.col
                                && other.row == plant.row
                                && !other.dead
                                && other.seed == crate::plant::SeedType::Pumpkinshell
                                && can_target(other)
                        }))
                    // A lily pad or pot is eaten last: the plant on it goes first (GetTopPlantAt).
                    && !(is_under(plant)
                        && plants.iter().any(|(_, other)| {
                            other.col == plant.col && other.row == plant.row && !other.dead && !is_under(other) && can_target(other)
                        }))
            })
            .map(|(id, _)| id)
    }

    /// `Zombie::EatPlant`.
    fn eat_plant(&mut self, plants: &mut DataArray<Plant>, id: sexy::data_array::Id<Plant>, env: &ZombieEnv, fx: &mut Fx) {
        use crate::plant::SeedType;
        // A dancer still moonwalking in stops to point instead.
        if self.phase == ZombiePhase::DancerDancingIn {
            self.phase_counter = 1;
            return;
        }
        // Behaviour per ruslan831 decomp, Zombie::EatPlant: a zombie pulling a
        // face at garlic eats nothing.
        if self.yucky_face {
            return;
        }
        // Behaviour per ruslan831 decomp, Zombie::EatPlant: a plant with a
        // ladder against it is climbed, not eaten.
        if let Some(plant) = plants.get(id)
            && self.ladder_at(plant.col, plant.row)
        {
            let col = plant.col;
            self.stop_eating(env, fx);
            if self.height == ZombieHeight::Normal && self.use_ladder_col != col {
                self.height = ZombieHeight::UpLadder;
                self.use_ladder_col = col;
            }
            return;
        }
        self.start_eating(fx);
        let Some(plant) = plants.get_mut(id) else { return };
        // Plants that are about to go off can't be eaten (unless asleep), nor
        // can an armed potato mine.
        let going_off = matches!(plant.seed, SeedType::Cherrybomb | SeedType::Jalapeno | SeedType::Doomshroom | SeedType::Iceshroom | SeedType::Hypnoshroom);
        if (going_off && !plant.is_asleep()) || plant.is_squash_getting_ready() {
            return;
        }
        if plant.seed == SeedType::Potatomine && plant.is_armed_or_rising() {
            return;
        }
        if self.chilled_counter > 0 && self.age % 2 == 1 {
            return;
        }
        plant.health -= DAMAGE_PER_EAT;
        plant.recently_eaten_countdown = 50;
        if plant.health <= 0 {
            fx.sounds.play_sample("SOUND_GULP");
            plant.die(fx.effects);
            self.ate_plant = Some((plant.seed, plant.col));
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
            let (left, right) = match self.zombie_type {
                ZombieType::Polevaulter => (0.38, 0.8),
                ZombieType::Newspaper => (0.42, 0.42),
                _ => (0.14, 0.68),
            };
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
    fn animate_chew(&mut self, plants: &mut DataArray<Plant>, fx: &mut Fx) {
        let target = self.find_plant_target(plants).filter(|_| !self.mind_controlled);
        // A hypno-shroom that's awake turns the zombie that bites it.
        if let Some(plant) = target.and_then(|id| plants.get_mut(id))
            && plant.seed == crate::plant::SeedType::Hypnoshroom
            && !plant.is_asleep()
        {
            fx.sounds.play_foley(FoleyType::Floop);
            plant.die(fx.effects);
            self.start_mind_controlled(fx);
            return;
        }
        // Behaviour per ruslan831 decomp, Zombie::AnimateChewSound: garlic
        // makes it pull a face.
        if target.and_then(|id| plants.get(id)).is_some_and(|p| p.seed == crate::plant::SeedType::Garlic) {
            if !self.yucky_face {
                self.yucky_face = true;
                self.yucky_face_counter = 0;
                self.update_anim_speed(fx.effects);
                fx.sounds.play_foley(FoleyType::Chomp);
            }
            return;
        }
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
        // The body faces the way it walks: hypnotized zombies turn round,
        // and dancers turn to face right for some of their steps.
        let mut facing_right = self.is_walking_backwards();
        if matches!(self.zombie_type, ZombieType::Dancer | ZombieType::BackupDancer) {
            use ZombiePhase::*;
            facing_right = !self.is_eating && matches!(self.phase, DancerDancingIn | DancerRaiseRight1 | DancerRaiseRight2);
            facing_right ^= self.mind_controlled;
        }
        // Behaviour per ruslan831 decomp, Zombie::UpdateReanim: the body is
        // scaled about its middle, so it stays on the same spot on the lawn.
        let s = self.scale;
        let mut offset_x = pos.image_offset_x + 15.0;
        let mut offset_y = pos.image_offset_y + pos.body_y - 28.0 + 20.0;
        if self.zombie_type == ZombieType::Football && s < 1.0 {
            offset_y += 20.0 - 20.0 * s;
        }
        // Behaviour per ruslan831 decomp, Zombie::UpdateReanim: a Zamboni or
        // catapult shakes when it's nearly wrecked, and more as it dies.
        if matches!(self.zombie_type, ZombieType::Zamboni | ZombieType::Catapult) && self.phase != ZombiePhase::Burned {
            let anim_time = effects.get(body_id).map_or(0.0, |b| b.anim_time);
            let range = if self.phase == ZombiePhase::Dying {
                sexy::tod::animate_curve_float_time(0.7, 1.0, anim_time, 0.0, 1.0, TodCurve::EaseOut)
            } else if self.body_health < 200 {
                1.0
            } else {
                0.0
            };
            // Only how it looks, so a hash of the age stands in for the random generator.
            let jitter = |salt: u32| {
                let h = (self.age as u32).wrapping_add(salt).wrapping_mul(2_654_435_761);
                ((h >> 16) & 0xffff) as f32 / 32767.5 - 1.0
            };
            offset_x += jitter(1) * range;
            offset_y += jitter(7) * range;
        }
        if facing_right {
            offset_x += 90.0 * s;
        }
        let fade = if self.zombie_fade >= 0 { (255 * self.zombie_fade / 10).clamp(0, 255) as u8 } else { 255 };
        // DrawReanim's colors: chilled zombies are blue, burned ones black,
        // and a zombie that was just hit flashes white.
        let (tint, mut extra): ([u8; 3], Option<[u8; 3]>) = if self.phase == ZombiePhase::Burned {
            ([0, 0, 0], None)
        } else if self.mind_controlled {
            ([128, 0, 192], Some([128, 0, 192]))
        } else if self.chilled_counter > 0 || self.ice_trap_counter > 0 {
            ([75, 75, 255], Some([75, 75, 255]))
        } else {
            ([255, 255, 255], None)
        };
        // Behaviour per ruslan831 decomp, Zombie::DrawReanim: nearly dead, the
        // boss pulses gray.
        let mut tint = tint;
        if self.is_boss() && self.phase != ZombiePhase::Dying && self.body_health < self.body_max_health / 10 {
            let gray = animate_curve(0, 39, self.age % 40, 155, 255, TodCurve::Bounce) as u8;
            tint = if self.chilled_counter > 0 || self.ice_trap_counter > 0 {
                let cold = animate_curve(0, 39, self.age % 40, 65, 75, TodCurve::Bounce) as u8;
                [cold, cold, gray]
            } else {
                [255, gray, gray]
            };
        }
        if self.just_got_shot_counter > 0 {
            let flash = (self.just_got_shot_counter * 10).clamp(0, 255) as u8;
            let base = extra.unwrap_or([0, 0, 0]);
            extra = Some([base[0].saturating_add(flash), base[1].saturating_add(flash), base[2].saturating_add(flash)]);
        }
        if let Some(body) = effects.get_mut(body_id) {
            body.overlay.m01 = 0.0;
            body.overlay.m10 = 0.0;
            body.override_scale(s, s);
            body.set_position(offset_x + 30.0 - 30.0 * s, offset_y + 120.0 - 120.0 * s);
            if facing_right {
                body.overlay.m00 = -s;
            }
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
        // A popping jack-in-the-box can't be stopped (`Zombie::TakeDamage`).
        if self.is_dead_or_dying() || self.phase == ZombiePhase::JackPopping {
            return;
        }
        let mut remaining = damage;
        if self.is_flying() {
            remaining = self.take_flying_damage(remaining, flags, env, fx);
        }
        if remaining > 0 && self.shield_type != ShieldType::None && flags & damage::BYPASSES_SHIELD == 0 {
            remaining = self.take_shield_damage(remaining, flags, env, fx);
            if flags & damage::HITS_SHIELD_AND_BODY != 0 {
                remaining = damage;
            }
        }
        if remaining > 0 && self.helm_type != HelmType::None {
            remaining = self.take_helm_damage(remaining, flags, fx);
        }
        if remaining > 0 {
            self.take_body_damage(remaining, flags, env, fx);
        }
    }

    /// A rolling nut's hit (`Plant::UpdateBowling`): a giant wall-nut flattens
    /// anything; a bouncing nut flattens a screen door's holder; other shields and
    /// helmets take the blow, otherwise the zombie is flattened.
    pub fn bowling_damage(&mut self, giant: bool, bouncing: bool, env: &ZombieEnv, fx: &mut Fx) {
        if giant || (self.shield_type == ShieldType::Door && bouncing) {
            self.take_damage(1800, 0, env, fx);
        } else if self.shield_type != ShieldType::None {
            self.take_shield_damage(400, 0, env, fx);
        } else {
            match self.helm_type {
                HelmType::Pail => {
                    fx.sounds.play_foley(FoleyType::ShieldHit);
                    self.take_helm_damage(900, 0, fx);
                }
                HelmType::TrafficCone | HelmType::Football => {
                    fx.sounds.play_foley(FoleyType::PlasticHit);
                    self.take_helm_damage(900, 0, fx);
                }
                // A wall-nut, tall-nut or hard hat takes the blow silently.
                HelmType::Wallnut | HelmType::Tallnut | HelmType::Digger => {
                    self.take_helm_damage(900, 0, fx);
                }
                HelmType::None | HelmType::Bobsled => self.take_damage(1800, 0, env, fx),
            }
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

    /// `Zombie::GetShieldDamageIndex`.
    fn shield_damage_index(&self) -> i32 {
        if self.shield_health < self.shield_max_health / 3 {
            2
        } else if self.shield_health < self.shield_max_health * 2 / 3 {
            1
        } else {
            0
        }
    }

    /// `Zombie::TakeShieldDamage`: returns the damage the shield didn't absorb.
    fn take_shield_damage(&mut self, damage: i32, flags: u32, env: &ZombieEnv, fx: &mut Fx) -> i32 {
        if flags & damage::DOESNT_CAUSE_FLASH == 0 {
            self.shield_just_got_shot_counter = 25;
            self.just_got_shot_counter = self.just_got_shot_counter.max(0);
            if flags & damage::HITS_SHIELD_AND_BODY == 0 {
                self.shield_recoil_counter = 12;
                if self.shield_type == ShieldType::Door {
                    fx.sounds.play_foley(FoleyType::ShieldHit);
                }
            }
        }
        let index_before = self.shield_damage_index();
        let absorbed = self.shield_health.min(damage);
        self.shield_health -= absorbed;
        if self.shield_health == 0 {
            self.drop_shield(flags, env, fx);
            return damage - absorbed;
        }
        let index_after = self.shield_damage_index();
        if index_before != index_after && index_after >= 1 {
            let (track, images) = match self.shield_type {
                ShieldType::Door => ("anim_screendoor", ["IMAGE_REANIM_ZOMBIE_SCREENDOOR2", "IMAGE_REANIM_ZOMBIE_SCREENDOOR3"]),
                ShieldType::Newspaper => ("Zombie_paper_paper", ["IMAGE_REANIM_ZOMBIE_PAPER_PAPER2", "IMAGE_REANIM_ZOMBIE_PAPER_PAPER3"]),
                ShieldType::Ladder => ("Zombie_ladder_1", ["IMAGE_REANIM_ZOMBIE_LADDER_1_DAMAGE1", "IMAGE_REANIM_ZOMBIE_LADDER_1_DAMAGE2"]),
                ShieldType::None => return damage - absorbed,
            };
            if let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b)) {
                body.set_image_override(track, Some(images[index_after as usize - 1]));
            }
        }
        damage - absorbed
    }

    /// `Zombie::DropShield`: the door falls away; the newspaper zombie
    /// loses its paper and gets angry.
    fn drop_shield(&mut self, flags: u32, env: &ZombieEnv, fx: &mut Fx) {
        let leaves_body = flags & damage::DOESNT_LEAVE_BODY == 0;
        match self.shield_type {
            ShieldType::Ladder => {
                self.detach_shield(env, fx);
                if leaves_body {
                    let rng = fx.sounds.rng();
                    let (x, y) = (self.pos_x + 31.0, self.pos_y + 80.0);
                    let particle = fx.effects.add_particle(x, y, self.render_order + 1, ParticleEffect::ZombieLadder, fx.res, fx.images, rng);
                    self.scale_particle(fx, particle);
                }
            }
            ShieldType::Door => {
                self.detach_shield(env, fx);
                if leaves_body {
                    let (x, y) = self.track_position(fx.effects, "anim_screendoor");
                    let rng = fx.sounds.rng();
                    let particle = fx.effects.add_particle(x, y, self.render_order + 1, ParticleEffect::ZombieDoor, fx.res, fx.images, rng);
                    self.scale_particle(fx, particle);
                }
            }
            ShieldType::Newspaper => {
                self.stop_eating(env, fx);
                if self.yucky_face {
                    self.show_yucky_face(false, fx);
                    self.yucky_face = false;
                    self.yucky_face_counter = 0;
                }
                self.phase = ZombiePhase::NewspaperMaddening;
                self.play_zombie_reanim(fx.effects, "anim_gasp", LoopType::PlayOnceAndHold, 10, 8.0);
                self.detach_shield(env, fx);
                if leaves_body {
                    let (x, y) = self.track_position(fx.effects, "Zombie_paper_paper");
                    let rng = fx.sounds.rng();
                    let particle = fx.effects.add_particle(x, y, self.render_order + 1, ParticleEffect::ZombieNewspaper, fx.res, fx.images, rng);
                    self.scale_particle(fx, particle);
                }
                if leaves_body && flags & damage::BYPASSES_SHIELD == 0 {
                    fx.sounds.play_foley(FoleyType::NewspaperRip);
                    // AddAttachedReanim: the "!" follows the zombie.
                    let (x, y) = (self.pos_x - 11.0, self.pos_y);
                    if let Ok((id, surprise)) = fx.effects.add_reanimation(x, y, ReanimationType::ZombieSurprise, fx.res, fx.images) {
                        surprise.render_order = self.render_order + 1;
                        self.surprise = Some(id);
                    }
                }
            }
            ShieldType::None => {}
        }
        self.shield_type = ShieldType::None;
    }

    /// `Zombie::GetBodyDamageIndex`.
    #[allow(dead_code, reason = "damage states of the bigger zombies")]
    fn body_damage_index(&self) -> i32 {
        if self.is_boss() {
            return self.boss_damage_index();
        }
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
            self.apply_chill(false, fx);
        }
        if self.helm_health == 0 {
            self.drop_helm(flags, fx);
            return remaining;
        }
        let index_after = self.helm_damage_index();
        // The wall-nut and tall-nut crack on their own heads; the hard hat
        // cracks on the body (`Zombie::TakeHelmDamage`).
        match self.helm_type {
            HelmType::Wallnut | HelmType::Tallnut => {
                if index_before != index_after && index_after >= 1 {
                    let images = match self.helm_type {
                        HelmType::Wallnut => ["IMAGE_REANIM_WALLNUT_CRACKED1", "IMAGE_REANIM_WALLNUT_CRACKED2"],
                        _ => ["IMAGE_REANIM_TALLNUT_CRACKED1", "IMAGE_REANIM_TALLNUT_CRACKED2"],
                    };
                    let track = match self.helm_type {
                        HelmType::Wallnut => "anim_face",
                        _ => "anim_idle",
                    };
                    if let Some(head) = self.special_head.and_then(|id| fx.effects.get_mut(id)) {
                        head.set_image_override(track, Some(images[index_after as usize - 1]));
                    }
                }
                return remaining;
            }
            HelmType::Digger => {
                if index_before != index_after
                    && let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b))
                    && index_after >= 1
                {
                    body.set_image_override(
                        "Zombie_digger_hardhat",
                        Some(["IMAGE_REANIM_ZOMBIE_DIGGER_HARDHAT2", "IMAGE_REANIM_ZOMBIE_DIGGER_HARDHAT3"][index_after as usize - 1]),
                    );
                }
                return remaining;
            }
            _ => {}
        }
        if index_before != index_after
            && let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b))
        {
            let (track, images) = match self.helm_type {
                HelmType::TrafficCone => ("anim_cone", ["IMAGE_REANIM_ZOMBIE_CONE2", "IMAGE_REANIM_ZOMBIE_CONE3"]),
                HelmType::Pail => ("anim_bucket", ["IMAGE_REANIM_ZOMBIE_BUCKET2", "IMAGE_REANIM_ZOMBIE_BUCKET3"]),
                HelmType::Football => (
                    "zombie_football_helmet",
                    ["IMAGE_REANIM_ZOMBIE_FOOTBALL_HELMET2", "IMAGE_REANIM_ZOMBIE_FOOTBALL_HELMET3"],
                ),
                HelmType::Wallnut | HelmType::Tallnut | HelmType::Digger | HelmType::None | HelmType::Bobsled => {
                    return remaining
                }
            };
            if index_after >= 1 {
                body.set_image_override(track, Some(images[index_after as usize - 1]));
            }
        }
        remaining
    }

    /// `Zombie::DropHelm`.
    fn drop_helm(&mut self, flags: u32, fx: &mut Fx) {
        // The wall-nut and tall-nut stay on, cracked (`Zombie::DropHelm`
        // has no arm for them); the digger's hard hat comes off.
        if matches!(self.helm_type, HelmType::Wallnut | HelmType::Tallnut) {
            self.helm_type = HelmType::None;
            return;
        }
        if self.helm_type == HelmType::Digger {
            let (x, y) = self.track_position(fx.effects, "Zombie_digger_hardhat");
            self.reanim_show_track(fx, "Zombie_digger_hardhat", RENDER_GROUP_HIDDEN);
            if flags & damage::DOESNT_LEAVE_BODY == 0 {
                let rng = fx.sounds.rng();
                fx.effects.add_particle(x, y, self.render_order + 1, ParticleEffect::ZombieHeadlight, fx.res, fx.images, rng);
            }
            self.helm_type = HelmType::None;
            return;
        }
        let (track, effect) = match self.helm_type {
            HelmType::TrafficCone => ("anim_cone", ParticleEffect::ZombieTrafficCone),
            HelmType::Pail => ("anim_bucket", ParticleEffect::ZombiePail),
            HelmType::Football => ("zombie_football_helmet", ParticleEffect::ZombieHelmet),
            HelmType::Bobsled => {
                // The broken sled tips its team off (BobsledCrash, on the next update).
                if flags & damage::DOESNT_LEAVE_BODY == 0 {
                    self.bobsled_crash_pending = true;
                }
                self.helm_type = HelmType::None;
                return;
            }
            HelmType::Wallnut | HelmType::Tallnut | HelmType::Digger | HelmType::None => return,
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
            self.apply_chill(false, fx);
        }
        let index_before = self.body_damage_index();
        let health_before = self.body_health;
        self.body_health -= damage;
        if self.is_boss() {
            self.boss_take_damage(health_before, index_before, flags, fx);
            return;
        }
        if self.zombie_type == ZombieType::Zamboni {
            self.zamboni_damage(index_before, flags, fx);
            return;
        }
        if self.zombie_type == ZombieType::Catapult {
            self.catapult_damage(index_before, flags, fx);
            return;
        }
        if matches!(self.zombie_type, ZombieType::Gargantuar | ZombieType::RedeyeGargantuar) {
            self.gargantuar_damage(index_before, fx);
        }
        self.update_damage_states(flags, env, fx);
        if self.body_health <= 0 {
            self.body_health = 0;
            self.play_death_anim(flags, env, fx);
            self.drop_loot();
        }
    }

    /// The Zamboni's part of `Zombie::TakeBodyDamage`: it dents and smokes as
    /// it's hurt, and meets its end with `zamboni_death`.
    fn zamboni_damage(&mut self, index_before: i32, flags: u32, fx: &mut Fx) {
        let index = self.body_damage_index();
        let set = |this: &Self, fx: &mut Fx, level: u32| {
            if let Some(body) = this.body.and_then(|b| fx.effects.get_mut(b)) {
                body.set_image_override("Zombie_zamboni_1", Some(&format!("IMAGE_REANIM_ZOMBIE_ZAMBONI_1_DAMAGE{level}")));
                body.set_image_override("Zombie_zamboni_2", Some(&format!("IMAGE_REANIM_ZOMBIE_ZAMBONI_2_DAMAGE{level}")));
            }
        };
        if flags & damage::DOESNT_CAUSE_FLASH == 0 {
            fx.sounds.play_foley(FoleyType::ShieldHit);
        }
        if flags & damage::SPIKE != 0 {
            set(self, fx, 2);
            self.zamboni_death(flags, fx);
        } else if self.body_health <= 0 {
            self.body_health = 0;
            self.zamboni_death(flags, fx);
        } else if index != index_before {
            set(self, fx, index as u32);
            if index == 2 {
                self.add_attached_particle(27.0, 72.0, ParticleEffect::ZamboniSmoke, fx);
            }
        }
    }

    /// The catapult's part of `Zombie::TakeBodyDamage`: its siding dents,
    /// then it smokes, and `catapult_death` finishes it.
    fn catapult_damage(&mut self, index_before: i32, flags: u32, fx: &mut Fx) {
        let index = self.body_damage_index();
        let dent = |this: &Self, fx: &mut Fx| {
            if let Some(body) = this.body.and_then(|b| fx.effects.get_mut(b)) {
                body.set_image_override("Zombie_catapult_siding", Some("IMAGE_REANIM_ZOMBIE_CATAPULT_SIDING_DAMAGE"));
            }
        };
        if flags & damage::DOESNT_CAUSE_FLASH == 0 {
            fx.sounds.play_foley(FoleyType::ShieldHit);
        }
        if flags & damage::SPIKE != 0 || self.body_health <= 0 {
            self.body_health = self.body_health.max(0);
            dent(self, fx);
            self.catapult_death(flags, fx);
        } else if index != index_before {
            if index == 1 {
                dent(self, fx);
            } else if index == 2 {
                self.add_attached_particle(47.0, 77.0, ParticleEffect::ZamboniSmoke, fx);
            }
        }
    }

    /// `Zombie::CatapultDeath`: spikes pop a tire and it bounces to a stop
    /// before it blows up; anything else blows it up at once.
    fn catapult_death(&mut self, flags: u32, fx: &mut Fx) {
        if flags & damage::SPIKE != 0 {
            fx.sounds.play_sample("SOUND_BALLOON_POP");
            self.phase = ZombiePhase::Dying;
            let rng = fx.sounds.rng();
            let (x, y, order) = (self.pos_x + 29.0, self.pos_y + 114.0, self.render_order + 1);
            fx.effects.add_particle(x, y, order, ParticleEffect::ZamboniTire, fx.res, fx.images, rng);
            self.vel_x = 0.0;
            self.add_attached_particle(47.0, 77.0, ParticleEffect::ZamboniSmoke, fx);
            self.phase_counter = 280;
            self.play_zombie_reanim(fx.effects, "anim_bounce", LoopType::PlayOnceAndHold, 10, 12.0);
            self.drop_loot();
        } else {
            self.vehicle_explode(ParticleEffect::CatapultExplosion, fx);
        }
    }

    /// A Zamboni or catapult going up in smoke.
    fn vehicle_explode(&mut self, effect: ParticleEffect, fx: &mut Fx) {
        let rng = fx.sounds.rng();
        let (x, y, order) = (self.pos_x + 80.0, self.pos_y + 60.0, self.render_order + 1);
        fx.effects.add_particle(x, y, order, effect, fx.res, fx.images, rng);
        fx.sounds.play_sample("SOUND_EXPLOSION");
        self.die_with_loot(fx);
    }

    /// The gargantuar's part of `Zombie::TakeBodyDamage`: it looks more
    /// battered as it's hurt.
    fn gargantuar_damage(&mut self, index_before: i32, fx: &mut Fx) {
        let index = self.body_damage_index();
        if index == index_before {
            return;
        }
        let redeye = self.zombie_type == ZombieType::RedeyeGargantuar;
        let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b)) else { return };
        if index == 1 {
            body.set_image_override("Zombie_gargantua_body1", Some("IMAGE_REANIM_ZOMBIE_GARGANTUAR_BODY1_2"));
            body.set_image_override("Zombie_gargantuar_outerarm_lower", Some("IMAGE_REANIM_ZOMBIE_GARGANTUAR_OUTERARM_LOWER2"));
        } else if index == 2 {
            body.set_image_override("Zombie_gargantua_body1", Some("IMAGE_REANIM_ZOMBIE_GARGANTUAR_BODY1_3"));
            body.set_image_override("Zombie_gargantuar_outerleg_foot", Some("IMAGE_REANIM_ZOMBIE_GARGANTUAR_FOOT2"));
            body.set_image_override("Zombie_gargantuar_outerarm_lower", Some("IMAGE_REANIM_ZOMBIE_GARGANTUAR_OUTERARM_LOWER2"));
            let head = if redeye { "IMAGE_REANIM_ZOMBIE_GARGANTUAR_HEAD2_REDEYE" } else { "IMAGE_REANIM_ZOMBIE_GARGANTUAR_HEAD2" };
            body.set_image_override("anim_head1", Some(head));
        }
    }

    /// The Zamboni and the catapult drive rather than walk.
    pub(super) fn is_vehicle(&self) -> bool {
        matches!(self.zombie_type, ZombieType::Zamboni | ZombieType::Catapult)
    }

    /// `Zombie::UpdateDamageStates`: arm off at two thirds, head at one third.
    fn update_damage_states(&mut self, flags: u32, env: &ZombieEnv, fx: &mut Fx) {
        if !self.can_lose_body_parts() {
            return;
        }
        if self.has_arm && self.body_health < 2 * self.body_max_health / 3 && self.body_health > 0 {
            self.drop_arm(flags, fx);
        }
        if self.has_head && self.body_health < self.body_max_health / 3 {
            self.drop_head(flags, fx);
            self.drop_loot();
            // Once the level's won, a headless zombie falls over at once.
            if env.award_dropped {
                self.play_death_anim(flags, env, fx);
            }
        }
    }

    /// `Zombie::CanLoseBodyParts`, for the zombies ported so far.
    fn can_lose_body_parts(&self) -> bool {
        !self.is_flying()
            && !matches!(
            self.zombie_type,
            ZombieType::Zamboni | ZombieType::Bungee | ZombieType::Catapult | ZombieType::Gargantuar | ZombieType::RedeyeGargantuar | ZombieType::Boss
        )
    }

    /// `Zombie::DropArm` + `SetupReanimForLostArm`.
    fn drop_arm(&mut self, flags: u32, fx: &mut Fx) {
        // Zombies holding a shield (or reading) keep both arms.
        if !self.has_arm || self.shield_type != ShieldType::None || self.phase == ZombiePhase::NewspaperReading {
            return;
        }
        self.has_arm = false;
        // Tracks to hide, the track the arm flies from, the upper arm track
        // and its bare image, and what flies off.
        type LostArm<'a> = (&'a [&'a str], &'a str, Option<(&'a str, &'a str)>, Option<&'a str>);
        let (hide, from, upper, flying): LostArm = match self.zombie_type {
            ZombieType::Football => (
                &["Zombie_football_leftarm_lower", "Zombie_football_leftarm_hand"],
                "Zombie_football_leftarm_hand",
                Some(("Zombie_football_leftarm_upper", "IMAGE_REANIM_ZOMBIE_FOOTBALL_LEFTARM_UPPER2")),
                Some("IMAGE_REANIM_ZOMBIE_FOOTBALL_LEFTARM_HAND"),
            ),
            ZombieType::Newspaper => (
                &["Zombie_paper_hands", "Zombie_paper_leftarm_lower"],
                "Zombie_paper_leftarm_lower",
                Some(("Zombie_paper_leftarm_upper", "IMAGE_REANIM_ZOMBIE_PAPER_LEFTARM_UPPER2")),
                Some("IMAGE_REANIM_ZOMBIE_PAPER_LEFTARM_LOWER"),
            ),
            ZombieType::Polevaulter => (
                &["Zombie_polevaulter_outerarm_lower", "Zombie_outerarm_hand"],
                "Zombie_polevaulter_outerarm_lower",
                Some(("Zombie_polevaulter_outerarm_upper", "IMAGE_REANIM_ZOMBIE_POLEVAULTER_OUTERARM_UPPER2")),
                Some("IMAGE_REANIM_ZOMBIE_OUTERARM_HAND"),
            ),
            ZombieType::Dancer => (
                &["Zombie_outerarm_lower", "Zombie_outerarm_hand"],
                "Zombie_outerarm_lower",
                Some(("Zombie_Jackson_outerarm_upper", "IMAGE_REANIM_ZOMBIE_JACKSON_OUTERARM_UPPER2")),
                Some("IMAGE_REANIM_ZOMBIE_JACKSON_OUTERARM_HAND"),
            ),
            ZombieType::BackupDancer => (
                &["Zombie_outerarm_lower", "Zombie_outerarm_hand"],
                "Zombie_outerarm_lower",
                None,
                Some("IMAGE_REANIM_ZOMBIE_DANCER_INNERARM_HAND"),
            ),
            ZombieType::Imp => (
                &["Zombie_outerarm_lower"],
                "Zombie_outerarm_lower",
                Some(("Zombie_imp_outerarm_upper", "IMAGE_REANIM_ZOMBIE_IMP_ARM1_BONE")),
                Some("IMAGE_REANIM_ZOMBIE_IMP_ARM2"),
            ),
            ZombieType::Balloon => (
                &["Zombie_outerarm_lower", "Zombie_outerarm_hand"],
                "Zombie_outerarm_lower",
                Some(("Zombie_outerarm_upper", "IMAGE_REANIM_ZOMBIE_BALLOON_OUTERARM_UPPER2")),
                Some("IMAGE_REANIM_ZOMBIE_OUTERARM_HAND"),
            ),
            ZombieType::Ladder => (
                &["Zombie_outerarm_lower", "Zombie_outerarm_hand"],
                "Zombie_outerarm_hand",
                Some(("Zombie_ladder_outerarm_upper", "IMAGE_REANIM_ZOMBIE_LADDER_OUTERARM_UPPER2")),
                Some("IMAGE_REANIM_ZOMBIE_OUTERARM_HAND"),
            ),
            ZombieType::Digger => (
                &["Zombie_outerarm_lower", "Zombie_outerarm_hand"],
                "Zombie_outerarm_lower",
                Some(("Zombie_digger_outerarm_upper", "IMAGE_REANIM_ZOMBIE_DIGGER_OUTERARM_UPPER2")),
                Some("IMAGE_REANIM_ZOMBIE_OUTERARM_HAND"),
            ),
            ZombieType::JackInTheBox => (
                &["Zombie_outerarm_lower", "Zombie_outerarm_hand"],
                "Zombie_jackbox_outerarm_lower",
                Some(("Zombie_jackbox_outerarm_lower", "IMAGE_REANIM_ZOMBIE_JACKBOX_OUTERARM_LOWER2")),
                Some("IMAGE_REANIM_ZOMBIE_OUTERARM_HAND"),
            ),
            ZombieType::Pogo => (
                &["Zombie_outerarm_lower", "Zombie_outerarm_hand"],
                "Zombie_outerarm_lower",
                Some(("Zombie_outerarm_upper", "IMAGE_REANIM_ZOMBIE_POGO_OUTERARM_UPPER2")),
                Some("IMAGE_REANIM_ZOMBIE_OUTERARM_HAND"),
            ),
            ZombieType::Yeti => (
                &["Zombie_outerarm_lower", "Zombie_outerarm_hand"],
                "Zombie_outerarm_lower",
                Some(("Zombie_yeti_outerarm_upper", "IMAGE_REANIM_ZOMBIE_YETI_OUTERARM_UPPER2")),
                Some("IMAGE_REANIM_ZOMBIE_YETI_OUTERARM_HAND"),
            ),
            _ => (
                &["Zombie_outerarm_lower", "Zombie_outerarm_hand"],
                "Zombie_outerarm_lower",
                Some(("Zombie_outerarm_upper", "IMAGE_REANIM_ZOMBIE_OUTERARM_UPPER2")),
                None,
            ),
        };
        // The polevaulter's tracks are hidden one by one, the others by prefix.
        for track in hide {
            if self.zombie_type == ZombieType::Polevaulter || self.zombie_type == ZombieType::Newspaper {
                self.reanim_show_track(fx, track, RENDER_GROUP_HIDDEN);
            } else {
                self.reanim_show_prefix(fx, track, RENDER_GROUP_HIDDEN);
            }
        }
        let pos = self.draw_pos();
        let (mut x, mut y) = (self.pos_x + pos.image_offset_x + 45.0, self.pos_y + pos.image_offset_y + pos.body_y + 78.0);
        if self.body.is_some() {
            (x, y) = self.track_position(fx.effects, from);
            if let Some((track, image)) = upper
                && let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b))
            {
                body.set_image_override(track, Some(image));
            }
            if self.zombie_type == ZombieType::Flag
                && let Some(flag) = self.special_head.and_then(|f| fx.effects.get_mut(f))
            {
                flag.set_image_override("Zombie_flag", Some("IMAGE_REANIM_ZOMBIE_FLAG3"));
            }
            // The pogo stick takes its share of the beating (`Zombie::DropArm`).
            if self.zombie_type == ZombieType::Pogo
                && let Some(body) = self.body.and_then(|b| fx.effects.get_mut(b))
            {
                body.set_image_override("Zombie_pogo_stickhands", Some("IMAGE_REANIM_ZOMBIE_POGO_STICKHANDS2"));
                body.set_image_override("Zombie_pogo_stick", Some("IMAGE_REANIM_ZOMBIE_POGO_STICKDAMAGE2"));
                body.set_image_override("Zombie_pogo_stick2", Some("IMAGE_REANIM_ZOMBIE_POGO_STICK2DAMAGE2"));
            }
        }
        if flags & damage::DOESNT_LEAVE_BODY == 0 {
            let rng = fx.sounds.rng();
            let particle = fx.effects.add_particle(x, y, self.render_order + 1, ParticleEffect::ZombieArm, fx.res, fx.images, rng);
            self.scale_particle(fx, particle);
            if let Some(image) = flying
                && let Some(system) = particle.and_then(|p| fx.effects.particle_mut(p))
            {
                system.override_image(None, Some(image));
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
        // A zombotany head's plant head dies with it (`Zombie::DropHead`).
        if self.zombie_type.is_zombotany() {
            self.reanim_show_prefix(fx, "anim_head", RENDER_GROUP_HIDDEN);
            self.reanim_show_prefix(fx, "anim_hair", RENDER_GROUP_HIDDEN);
            self.reanim_show_prefix(fx, "anim_tongue", RENDER_GROUP_HIDDEN);
            if let Some(id) = self.special_head.take() {
                fx.effects.kill(id);
            }
            return;
        }
        self.reanim_show_prefix(fx, "anim_head", RENDER_GROUP_HIDDEN);
        self.reanim_show_prefix(fx, "anim_hair", RENDER_GROUP_HIDDEN);
        self.reanim_show_prefix(fx, "anim_tongue", RENDER_GROUP_HIDDEN);
        if flags & damage::DOESNT_LEAVE_BODY != 0 {
            return;
        }
        // Losing its head breaks the pogo stick too (`Zombie::DropHead`).
        if self.zombie_type == ZombieType::Pogo {
            self.pogo_break_on_head_loss(fx);
        }
        if self.zombie_type == ZombieType::Balloon {
            self.reanim_show_prefix(fx, "anim_hat", RENDER_GROUP_HIDDEN);
            self.reanim_show_prefix(fx, "hat", RENDER_GROUP_HIDDEN);
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
        let (effect, order) = match self.zombie_type {
            ZombieType::Newspaper => (ParticleEffect::ZombieNewspaperHead, self.render_order + 1),
            ZombieType::Dancer => (ParticleEffect::ZombieHead, self.render_order - 1),
            ZombieType::Balloon => (ParticleEffect::ZombieBalloonHead, self.render_order + 1),
            ZombieType::Pogo => (ParticleEffect::ZombiePogoHead, self.render_order + 1),
            _ => (ParticleEffect::ZombieHead, self.render_order + 1),
        };
        let image = match self.zombie_type {
            ZombieType::Polevaulter => Some("IMAGE_ZOMBIEPOLEVAULTERHEAD"),
            ZombieType::Football => Some("IMAGE_ZOMBIEFOOTBALLHEAD"),
            ZombieType::Ladder => Some("IMAGE_ZOMBIELADDERHEAD"),
            ZombieType::Imp => Some("IMAGE_ZOMBIEIMPHEAD"),
            ZombieType::Dancer => Some("IMAGE_ZOMBIEDANCERHEAD"),
            ZombieType::BackupDancer => {
                self.reanim_show_prefix(fx, "anim_earing", RENDER_GROUP_HIDDEN);
                Some("IMAGE_ZOMBIEBACKUPDANCERHEAD")
            }
            _ => None,
        };
        let rng = fx.sounds.rng();
        let particle = fx.effects.add_particle(x, y, order, effect, fx.res, fx.images, rng);
        self.scale_particle(fx, particle);
        if let Some(image) = image
            && let Some(system) = particle.and_then(|p| fx.effects.particle_mut(p))
        {
            system.override_image(None, Some(image));
        }
        fx.sounds.play_foley(FoleyType::LimbsPop);
    }

    /// `Zombie::OverrideParticleScale`: a shrunken zombie's bits shrink with it.
    fn scale_particle(&self, fx: &mut Fx, particle: Option<crate::reanim::ParticleSystemId>) {
        if let Some(system) = particle.and_then(|p| fx.effects.particle_mut(p)) {
            system.override_scale(None, self.scale);
        }
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
        let particle = fx.effects.add_particle(x + 6.0, y - 45.0, self.render_order + 1, ParticleEffect::ZombieFlag, fx.res, fx.images, rng);
        self.scale_particle(fx, particle);
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
        let gargantuar = matches!(self.zombie_type, ZombieType::Gargantuar | ZombieType::RedeyeGargantuar);
        if (flags & damage::DOESNT_LEAVE_BODY != 0 && !gargantuar && !self.is_boss())
            || (self.zombie_type == ZombieType::DolphinRider && self.phase != ZombiePhase::DolphinWalkingInPool)
            || matches!(self.phase, ZombiePhase::SnorkelIntoPool | ZombiePhase::SnorkelWalking)
        {
            self.die_no_loot(fx.effects);
            return;
        }
        if self.ice_trap_counter > 0 {
            self.add_attached_particle(75.0, 106.0, ParticleEffect::IceTrapRelease, fx);
            self.ice_trap_counter = 0;
        }
        if let Some(surprise) = self.surprise.take() {
            fx.effects.kill(surprise);
        }
        if self.yucky_face {
            self.show_yucky_face(false, fx);
            self.yucky_face = false;
            self.yucky_face_counter = 0;
        }
        self.stop_eating(env, fx);
        if self.shield_type != ShieldType::None {
            self.drop_shield(damage::BYPASSES_SHIELD, env, fx);
        }
        self.vel_x = 0.0;
        self.phase = ZombiePhase::Dying;
        if self.height == ZombieHeight::UpLadder {
            self.height = ZombieHeight::Falling;
        }

        let mut rate = if self.zombie_type == ZombieType::Football {
            24.0
        } else if gargantuar {
            fx.sounds.play_foley(FoleyType::GargantuDeath);
            14.0
        } else if self.is_boss() {
            self.boss_die(fx);
            self.driver_play(fx, "anim_death", LoopType::PlayOnceAndHold, 20, 18.0);
            18.0
        } else {
            fx.sounds.rng().range_f32(24.0, 30.0)
        };
        let rng = fx.sounds.rng();
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
        if self.body.and_then(|b| fx.effects.get(b)).is_none() {
            self.die_no_loot(fx.effects);
            return;
        }
        if self.height == ZombieHeight::Falling {
            self.update_zombie_falling();
        }
        // Behaviour per ruslan831 decomp, Zombie::UpdateDeath: a vehicle with a
        // popped tire blows up when it's done bouncing.
        let vehicle_countdown = match self.zombie_type {
            ZombieType::Zamboni => self.phase_counter > 0,
            ZombieType::Catapult => true,
            _ => false,
        };
        if vehicle_countdown {
            self.phase_counter -= 1;
            if self.phase_counter == 0 {
                let effect = match self.zombie_type {
                    ZombieType::Catapult => ParticleEffect::CatapultExplosion,
                    _ if self.body.and_then(|b| fx.effects.get(b)).is_some_and(|b| b.is_anim_playing("anim_wheelie2")) => {
                        ParticleEffect::ZamboniExplosion2
                    }
                    _ => ParticleEffect::ZamboniExplosion,
                };
                self.vehicle_explode(effect, fx);
            }
            return;
        }
        if self.is_boss() {
            self.update_boss_death(fx);
        }
        let Some(body) = self.body.and_then(|b| fx.effects.get(b)) else { return };
        let gargantuar = matches!(self.zombie_type, ZombieType::Gargantuar | ZombieType::RedeyeGargantuar);
        if gargantuar {
            if body.should_trigger_timed_event(0.89) {
                self.shake = Some((0, 3));
            } else if body.should_trigger_timed_event(0.98) {
                self.shake = Some((0, 1));
            }
        }
        let fall_time = if self.zombie_type == ZombieType::Polevaulter {
            0.68
        } else if self.zombie_type == ZombieType::Football {
            0.52
        } else if self.zombie_type == ZombieType::Newspaper {
            0.63
        } else if self.zombie_type == ZombieType::Ladder {
            0.62
        } else if gargantuar {
            0.86
        } else if matches!(self.zombie_type, ZombieType::Imp | ZombieType::Boss) {
            -1.0
        } else if matches!(self.zombie_type, ZombieType::Dancer | ZombieType::BackupDancer) {
            0.83
        } else if body.is_anim_playing("anim_superlongdeath") {
            0.788
        } else if body.is_anim_playing("anim_death2") {
            0.71
        } else {
            0.77
        };
        if fall_time > 0.0 && body.should_trigger_timed_event(fall_time) {
            fx.sounds.play_foley(FoleyType::ZombieFalling);
            if gargantuar {
                fx.sounds.play_foley(FoleyType::Thump);
            }
        }
        if self.zombie_fade == -1 && body.loop_count > 0 && !self.is_boss() {
            self.zombie_fade = 100;
        }
    }

    /// `Zombie::DieNoLoot`.
    pub fn die_no_loot(&mut self, effects: &mut EffectSystem) {
        for id in [self.body.take(), self.special_head.take(), self.propeller.take(), self.boss.fireball.take(), self.mowered_reanim.take(), self.surprise.take()].into_iter().flatten() {
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
        let edge = match self.zombie_type {
            ZombieType::Polevaulter | ZombieType::Gargantuar | ZombieType::RedeyeGargantuar => -150,
            ZombieType::Football | ZombieType::Catapult | ZombieType::Zamboni => -175,
            ZombieType::Dancer | ZombieType::BackupDancer | ZombieType::Snorkel => -130,
            _ => BOARD_EDGE,
        };
        self.has_head && self.x <= edge && !self.is_dead_or_dying() && !self.mind_controlled && !self.is_boss()
    }

    /// `CheckForBoardEdge`: a hypnotized zombie that walks off the right
    /// of the lawn is gone.
    pub fn walked_off_lawn(&self) -> bool {
        self.is_walking_backwards() && self.pos_x > 850.0
    }

    /// `FindZombieTarget`'s test: whether this zombie is on the other side
    /// from `other` and close enough to bite it.
    pub fn can_bite_zombie(&self, other: &Zombie) -> bool {
        self.mind_controlled != other.mind_controlled
            && !other.is_dead_or_dying()
            && other.row == self.row
            && {
                let overlap = rect_overlap(self.attack_rect(), other.zombie_rect());
                overlap >= 20 || (overlap >= 0 && other.is_eating)
            }
    }

    /// Running up to a plant, or in mid-jump: plants that bite or explode
    /// under a zombie can't get it now.
    pub fn is_vaulting(&self) -> bool {
        matches!(self.phase, ZombiePhase::PolePreVault | ZombiePhase::PoleInVault | ZombiePhase::DolphinIntoPool | ZombiePhase::DolphinInJump)
    }

    /// `Zombie::Draw`: the body (with its flag), in the zombie's own
    /// coordinates (its `mX`, `mY`). `carried` is the plant a bungee zombie
    /// has hold of.
    pub fn draw(&self, g: &mut Graphics, effects: &EffectSystem, res: &GameResources, carried: Option<&Plant>, night: bool, roof: bool) {
        g.push_state();
        g.translate(self.x as f32, self.y as f32);
        if self.is_winner() {
            // Winners vanish into the house (SetupDrawZombieWon).
            g.clip_rect(self.win_clip.0 - self.x as f32, -(self.y as f32), 800.0, self.win_clip.1);
        }
        // DrawReanim: a zombie climbing out of the ground is cut off at it.
        let pos = self.draw_pos();
        if pos.clip_height > CLIP_HEIGHT_LIMIT {
            let height = 120.0 - pos.clip_height + 71.0;
            g.clip_rect(pos.image_offset_x - 200.0, pos.image_offset_y + pos.body_y - 78.0, 520.0, height);
        }
        // DrawIceTrap: the zombie stands in a block of ice.
        let ice_y = self.draw_pos().body_y + 92.0;
        // Behaviour per ruslan831 decomp, Zombie::DrawIceTrap: the block
        // sits differently on a pogo stick or a digger.
        let (ice_x, ice_y) = match self.zombie_type {
            ZombieType::Pogo => (36.0, ice_y + 20.0),
            ZombieType::Digger => (19.0, ice_y),
            _ => (46.0, ice_y),
        };
        if self.ice_trap_counter > 0
            && let Some(back) = res.image("IMAGE_ICETRAP2")
        {
            g.draw_image(back, ice_x, ice_y);
        }
        let spotlight = self.dancer_spotlight();
        if let Some(color) = spotlight
            && let Some(light) = res.image("IMAGE_SPOTLIGHT2")
        {
            draw_spotlight(g, light, -30.0, 60.0, color);
        }
        if self.zombie_type == ZombieType::Bobsled {
            self.draw_bobsled(g, res, true);
        }
        if self.zombie_type == ZombieType::Bungee {
            self.draw_bungee_cord(g, effects, res);
        }
        if let Some(body) = self.body {
            effects.draw(g, body, res);
        }
        if self.zombie_type == ZombieType::Bungee {
            // DrawBungeeReanim: the plant it carries, under its lower arms.
            if let Some(plant) = carried {
                self.draw_carried_plant(g, plant, night, roof, effects, res);
            }
            if let Some(body) = self.body.and_then(|b| effects.get(b)) {
                draw_render_group(g, body, RENDER_GROUP_ARMS, res);
            }
        }
        if self.zombie_type == ZombieType::Bobsled {
            self.draw_bobsled(g, res, false);
        }
        if let Some(color) = spotlight
            && let Some(light) = res.image("IMAGE_SPOTLIGHT")
        {
            draw_spotlight(g, light, -30.0, -480.0, color);
        }
        if let Some(body) = self.body.and_then(|b| effects.get(b))
            && self.shield_type != ShieldType::None
        {
            // DrawReanim: the shield flashes and jolts on its own when hit.
            let fade = body.color_override[3];
            let (color, extra) = if self.phase == ZombiePhase::Burned {
                ([0, 0, 0, fade], None)
            } else if self.shield_just_got_shot_counter > 0 {
                let gray = (self.shield_just_got_shot_counter * 10).clamp(0, 255) as u8;
                ([gray, gray, gray, fade], Some([255, 255, 255, 255]))
            } else {
                ([255, 255, 255, fade], None)
            };
            let recoil = if self.shield_recoil_counter > 0 {
                animate_curve_float(12, 0, self.shield_recoil_counter, 3.0, 0.0, TodCurve::Linear)
            } else {
                0.0
            };
            g.translate(recoil, 0.0);
            draw_render_group_colored(g, body, RENDER_GROUP_SHIELD, res, color, extra);
            g.translate(-recoil, 0.0);
            draw_render_group(g, body, RENDER_GROUP_OVER_SHIELD, res);
        }
        if self.ice_trap_counter > 0
            && let Some(front) = res.image("IMAGE_ICETRAP")
        {
            g.draw_image(front, ice_x, ice_y);
        }
        if let Some((x, y, scale)) = self.butter_position(effects, res)
            && let Some(butter) = res.image("IMAGE_REANIM_CORNPULT_BUTTER_SPLAT")
        {
            g.draw_image_scaled(butter, x, y, scale, scale);
        }
        g.pop_state();
    }

    /// `Zombie::DrawBungeeCord`: the cord runs from its body up off the top
    /// of the screen.
    fn draw_bungee_cord(&self, g: &mut Graphics, effects: &EffectSystem, res: &GameResources) {
        let Some(cord) = res.image("IMAGE_BUNGEECORD") else { return };
        let Some((_, body_y)) = self.track_center(effects, res, "Zombie_bungi_body") else { return };
        let cel_h = cord.cel_height() as f32;
        // In the zombie's coordinates, until it's off the top of the board.
        let mut y = body_y - cel_h;
        // Behaviour per ruslan831 decomp, Zombie::DrawBungeeCord: on the boss
        // level the cords come out from under his arm.
        g.push_state();
        if let Some(clip) = self.cord_clip {
            g.clip_rect(-(self.x as f32), clip - self.y as f32, 800.0, 600.0);
        }
        while y + self.pos_y > -cel_h {
            g.draw_image(cord, -22.0 + 61.0 - 4.0, y);
            y -= cel_h;
        }
        g.pop_state();
    }

    /// `DrawBungeeReanim`'s plant: drawn hanging where the zombie is.
    fn draw_carried_plant(&self, g: &mut Graphics, plant: &Plant, night: bool, roof: bool, effects: &EffectSystem, res: &GameResources) {
        use crate::plant::SeedType;
        let mut dy = 30.0 - self.altitude;
        if self.phase == ZombiePhase::BungeeRising && matches!(plant.seed, SeedType::Spikeweed | SeedType::Spikerock) {
            dy -= 34.0;
        }
        if plant.col <= 4 && roof {
            dy += 10.0;
        }
        // Plant::draw places itself at its own square: draw it at ours instead.
        g.push_state();
        g.translate(-plant.x as f32, dy - plant.y as f32);
        plant.draw(g, night, effects, res);
        g.pop_state();
    }

    /// `Zombie::DrawBungeeTarget`: the target on the square a bungee zombie
    /// is dropping on, in board coordinates.
    pub fn draw_bungee_target(&self, g: &mut Graphics, res: &GameResources) {
        // Behaviour per ruslan831 decomp, Zombie::DrawBungeeTarget.
        if self.zombie_type != ZombieType::Bungee || !self.is_on_board() || self.phase == ZombiePhase::BungeeRising {
            return;
        }
        let Some(target) = res.image("IMAGE_BUNGEETARGET") else { return };
        let pos = self.draw_pos();
        let mut x = self.x as f32 + 10.0;
        let mut y = self.y as f32 + 60.0 + pos.body_y + pos.image_offset_y;
        // It swoops in from up and to the right as the zombie comes down.
        if matches!(self.phase, ZombiePhase::BungeeDiving | ZombiePhase::BungeeDivingScreaming) {
            let (top, bottom) = (BUNGEE_ZOMBIE_HEIGHT as i32, BUNGEE_ZOMBIE_HEIGHT as i32 - 400);
            x += animate_curve_float(top, bottom, self.altitude as i32, 30.0, 0.0, TodCurve::Linear);
            y += animate_curve_float(top, bottom, self.altitude as i32, -600.0, 0.0, TodCurve::Linear);
        }
        g.draw_image(target, x, y + self.altitude);
    }

    /// `GetTrackPosition`, less the zombie's position: where the middle of a
    /// body track's image is, in the zombie's own coordinates.
    fn track_center(&self, effects: &EffectSystem, res: &GameResources, track: &str) -> Option<(f32, f32)> {
        let body = self.body.and_then(|b| effects.get(b))?;
        let track = body.definition.find_track(track)?;
        // GetTrackMatrix measures from the middle of the track's image, and
        // the body's overlay already holds where the zombie is drawn.
        let (cel_w, cel_h) = body
            .current_transform(track)
            .image
            .as_deref()
            .and_then(|id| res.image(id))
            .map_or((0.0, 0.0), |image| (image.cel_width() as f32, image.cel_height() as f32));
        let m = body.track_matrix(track) * sexy::matrix::Matrix3::translation(cel_w * 0.5, cel_h * 0.5);
        Some((m.m02, m.m12))
    }

    /// `DrawButter`: where a pat of butter on its head is drawn, in the
    /// zombie's own coordinates (its `mX`, `mY`).
    pub(super) fn butter_position(&self, effects: &EffectSystem, res: &GameResources) -> Option<(f32, f32, f32)> {
        // Behaviour per ruslan831 decomp, Zombie::DrawButter.
        if self.buttered_counter <= 0 {
            return None;
        }
        let track = match self.zombie_type {
            _ if self.phase == ZombiePhase::NewspaperMaddening => "anim_head_look",
            ZombieType::Catapult => "Zombie_catapult_driver_head",
            _ => "anim_head1",
        };
        let (x, y) = self.track_center(effects, res, track)?;
        let (x, y) = (x - 29.0, y - 36.0);
        // A gargantuar's head takes a bigger pat.
        Some(match self.zombie_type {
            ZombieType::Gargantuar | ZombieType::RedeyeGargantuar => (x - 5.0, y - 15.0, 1.2),
            ZombieType::Pogo => (x, y - 5.0, 1.0),
            ZombieType::SquashHead => (x + 6.0, y - 9.0, 1.0),
            ZombieType::WallnutHead => (x - 6.0, y - 1.0, 1.0),
            ZombieType::TallnutHead => (x - 24.0, y - 39.0, 1.0),
            _ => (x, y, 1.0),
        })
    }

    /// Its body reanimation.
    #[cfg(test)]
    pub(super) fn body_reanim(&self) -> Option<ReanimationId> {
        self.body
    }

    /// How many basketballs a catapult has left, set directly.
    #[cfg(test)]
    pub(super) fn set_basketballs(&mut self, count: i32) {
        self.summon_counter = count;
    }

    /// Ticks left stuck in butter, set directly.
    #[cfg(test)]
    pub(super) fn set_buttered_counter(&mut self, ticks: i32) {
        self.buttered_counter = ticks;
    }

    /// `DrawDancerReanim`'s spotlight: dancers dance under a colored light
    /// that changes every so often.
    fn dancer_spotlight(&self) -> Option<SexyColor> {
        use ZombiePhase::*;
        if self.zombie_type != ZombieType::Dancer
            || matches!(self.phase, DancerDancingIn | DancerSnappingFingers | Normal | Dying)
            || self.is_winner()
        {
            return None;
        }
        let pick = if self.age >= 700 { self.age / 100 * 7 % 5 } else { 0 };
        Some(match pick {
            0 => SexyColor::rgba(250, 250, 160, 255),
            1 => SexyColor::rgba(114, 234, 170, 255),
            2 => SexyColor::rgba(216, 126, 202, 255),
            3 => SexyColor::rgba(90, 110, 140, 255),
            _ => SexyColor::rgba(240, 90, 130, 255),
        })
    }

    /// `Zombie::DrawShadow`.
    pub fn draw_shadow(&self, g: &mut Graphics, night: bool, res: &GameResources) {
        // `Zombie::HasShadow`.
        if matches!(
            self.phase,
            ZombiePhase::Dying | ZombiePhase::PoleInVault | ZombiePhase::RisingFromGrave | ZombiePhase::DancerRising
        ) {
            return;
        }
        if self.is_boss() || (self.zombie_type == ZombieType::Bungee && (!self.is_on_board() || self.hit_umbrella)) {
            return;
        }
        let pos = self.draw_pos();
        let s = self.scale;
        let backwards = self.is_walking_backwards();
        // Behaviour per ruslan831 decomp, Zombie::DrawShadow: the shadow
        // follows the zombie's scale, and most types shift it their own way.
        let mut dx = pos.image_offset_x + (s * 20.0 - 20.0);
        let mut dy = pos.image_offset_y + pos.body_y;
        let mut scale = s;
        match self.zombie_type {
            ZombieType::Football => dx += if backwards { -11.0 * s } else { 20.0 + 21.0 * s },
            ZombieType::Newspaper => dx += if backwards { 5.0 } else { 29.0 },
            ZombieType::Polevaulter => dx += if backwards { -5.0 } else { 36.0 },
            ZombieType::Bobsled => dx += if backwards { 13.0 } else { 20.0 },
            ZombieType::Imp => {
                scale *= 0.6;
                dx += if backwards { 13.0 } else { 25.0 };
            }
            ZombieType::Digger => dx += if backwards { 14.0 } else { 17.0 },
            ZombieType::Snorkel => dx += if backwards { -2.0 } else { 35.0 },
            ZombieType::DolphinRider => dx += if backwards { 15.0 } else { 19.0 },
            ZombieType::Yeti => dx += if backwards { 20.0 } else { 3.0 },
            ZombieType::Gargantuar | ZombieType::RedeyeGargantuar => {
                scale *= 1.5;
                dx += 27.0;
            }
            ZombieType::Balloon | ZombieType::Bungee => {}
            _ => dx += if backwards { 11.0 } else { 23.0 },
        }
        dy += match self.zombie_type {
            ZombieType::Football => 16.0,
            ZombieType::Newspaper => 4.0,
            ZombieType::Polevaulter => 11.0,
            ZombieType::Bobsled => 13.0,
            ZombieType::Imp => 7.0,
            ZombieType::Digger | ZombieType::Snorkel => 5.0,
            ZombieType::DolphinRider => 11.0,
            ZombieType::Yeti => 20.0,
            ZombieType::Gargantuar | ZombieType::RedeyeGargantuar => 7.0,
            ZombieType::Balloon => 13.0,
            _ => 0.0,
        };
        if self.zombie_type == ZombieType::Bungee {
            dx -= 12.0;
            let top = (BUNGEE_ZOMBIE_HEIGHT - 1000.0) as i32;
            scale = animate_curve_float(top, 100, self.altitude as i32, 0.1, 1.5, TodCurve::Linear);
        }
        // A climbing, falling, thrown, bouncing or flying zombie's shadow
        // stays on the ground.
        let airborne = matches!(self.height, ZombieHeight::UpLadder | ZombieHeight::Falling)
            || self.phase == ZombiePhase::ImpGettingThrown
            || self.zombie_type == ZombieType::Bungee
            || matches!(self.phase, ZombiePhase::PogoBouncing | ZombiePhase::PogoHighBounce | ZombiePhase::PogoForwardBounce)
            || self.is_flying();
        if airborne {
            dy += self.altitude;
        }
        g.push_state();
        g.translate(self.x as f32, self.y as f32);
        if self.in_pool {
            if let Some(shadow) = res.image("IMAGE_WHITEWATER_SHADOW") {
                g.draw_image_cel_center_scaled(shadow, dx, dy + 67.0, 0, scale, scale);
            }
        } else if let Some(shadow) = res.image(if night { "IMAGE_PLANTSHADOW2" } else { "IMAGE_PLANTSHADOW" }) {
            g.draw_image_cel_center_scaled(shadow, dx, dy + 92.0, 0, scale, scale);
        }
        g.pop_state();
    }
}

/// `TodDrawImageScaledF` at 4x, tinted: the dancer's spotlight, with its
/// top-left at (x, y).
fn draw_spotlight(g: &mut Graphics, image: &ImageRes, x: f32, y: f32, color: SexyColor) {
    let (w, h) = (image.width as f32, image.height as f32);
    let scale = 4.0;
    g.push_state();
    g.set_colorize_images(true);
    g.set_color(color);
    let transform = bevy::math::Affine2::from_cols_array(&[scale, 0.0, 0.0, scale, x + w * scale * 0.5, y + h * scale * 0.5]);
    g.draw_image_matrix(image, transform);
    g.pop_state();
}

/// `Zombie::CanTargetPlant`, for chewing: spiky plants and the like aren't
/// ported yet, so everything on the ground is food.
fn can_target(plant: &Plant) -> bool {
    use crate::plant::SeedType;
    // `Zombie::CanTargetPlant`: the spikes are walked over, not eaten.
    !plant.dead && !plant.not_on_ground() && !matches!(plant.seed, SeedType::Tanglekelp | SeedType::Spikeweed | SeedType::Spikerock)
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
