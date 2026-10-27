//! The Zen Garden: the main greenhouse, the mushroom garden, the aquarium,
//! potted plants, their needs, tools, coins, Stinky and selling.
//!
//! Behaviour per ruslan831 decomp, `Lawn/ZenGarden.cpp`
//! (`ZenGardenInitLevel`, `PlacePottedPlant`, `GetPlantsNeed`, `PlantWatered`,
//! `PlantFertilized`, `PlantFulfillNeed`, `GetPlantSellPrice`, `GotoNextGarden`,
//! `AddStinky`, `StinkyUpdate`, `LeaveGarden`) and `Lawn/System/PlayerInfo.h`
//! (`PottedPlant`'s 0x58-byte layout).
//!
//! `GARDEN_MAIN`/`_MUSHROOM`/`_AQUARIUM` and the wheelbarrow's `_WHEELBARROW`
//! are the plant record's `mWhichZenGarden`. The original runs the garden as a
//! `Board`; this port keeps it a screen resource, but the simulation (needs,
//! growth, coin production, Stinky, selling) runs at the same 100 Hz.
//!
//! Timers are scaled down from the original's real-time hours: a need appears
//! every 30 seconds, a grown plant makes a coin every minute, and fertilizer,
//! bug spray or the phonograph fulfil a need, and chocolate doubles a plant's
//! production for five minutes. The store opens over the garden and comes
//! back to it. Dave's first-visit tutorial (at level 45, before the adventure
//! is finished) walks through watering, the store and fertilizer.

use bevy::prelude::*;
use sexy::reanim::LoopType;

use crate::audio::{FoleyType, Sounds};
use crate::graphics::{Graphics, Justify, SexyColor, SexyRect};
use crate::input::GameCursor;
use crate::player::{PlayerInfo, ProfileMgr};
use crate::plant::SeedType;
use crate::reanim::{EffectSystem, ReanimationId, ReanimationType};
use crate::resources::GameResources;
use crate::store::{PURCHASE_COUNT_OFFSET, StoreItem};
use crate::widgets::draw_stone_button;
use crate::{AppState, UiSet};

const COLS: i32 = 8;
const MAIN_GRID: [(i32, i32); 32] = [
    (73, 73), (155, 71), (239, 68), (321, 73), (406, 71), (484, 67), (566, 70), (648, 72),
    (67, 168), (150, 165), (232, 170), (314, 175), (416, 173), (497, 170), (578, 164), (660, 168),
    (41, 268), (130, 266), (219, 260), (310, 266), (416, 267), (504, 261), (594, 265), (684, 269),
    (37, 371), (124, 369), (211, 368), (302, 369), (425, 375), (512, 368), (602, 365), (691, 368),
];
const MUSHROOM_GRID: [(i32, i32); 8] = [
    (110, 441), (237, 360), (298, 458), (355, 296), (387, 203), (460, 385), (486, 478), (552, 283),
];
const AQUARIUM_GRID: [(i32, i32); 8] = [
    (113, 185), (306, 120), (356, 270), (622, 120), (669, 270), (122, 355), (365, 458), (504, 417),
];

// `PottedPlant` field offsets in its 0x58-byte save record.
const OFF_SEED: usize = 0;
const OFF_GARDEN: usize = 4;
const OFF_X: usize = 8;
const OFF_Y: usize = 12;
const OFF_LAST_WATERED: usize = 0x18;
const OFF_AGE: usize = 0x24;
const OFF_TIMES_FED: usize = 0x28;
const OFF_FEEDINGS: usize = 0x2C;
const OFF_NEED: usize = 0x30;
const OFF_LAST_FULFILLED: usize = 0x38;
const OFF_LAST_FERTILIZED: usize = 0x40;
const OFF_LAST_CHOCOLATE: usize = 0x48;

const NEED_NONE: i32 = 0;
const NEED_WATER: i32 = 1;
const NEED_FERTILIZER: i32 = 2;
const NEED_BUGSPRAY: i32 = 3;
const NEED_PHONOGRAPH: i32 = 4;

const FULL_AGE: i32 = 3;
/// Ticks between needs and between a grown plant's coins (scaled from the
/// original's hours to half a minute / a minute).
const NEED_TICKS: i32 = 3_000;
const COIN_TICKS: i32 = 6_000;

pub struct ZenGardenPlugin;

impl Plugin for ZenGardenPlugin {
    fn build(&self, app: &mut App) {
        let open = resource_exists::<ZenGarden>;
        app.init_resource::<ZenTutorial>()
            .add_systems(OnEnter(AppState::ZenGarden), start_garden)
            .add_systems(OnExit(AppState::ZenGarden), leave_garden)
            .add_systems(FixedUpdate, (tick_effects, update_garden).chain().run_if(open))
            .add_systems(Update, garden_input.in_set(UiSet::ScreenInput).run_if(open))
            .add_systems(Update, draw_garden.in_set(UiSet::ScreenDraw).run_if(open));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GardenKind {
    Main,
    Mushroom,
    Aquarium,
}

impl GardenKind {
    fn index(self) -> i32 {
        match self {
            Self::Main => 0,
            Self::Mushroom => 1,
            Self::Aquarium => 3,
        }
    }

    fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Main),
            1 => Some(Self::Mushroom),
            3 => Some(Self::Aquarium),
            _ => None,
        }
    }

    fn rows(self) -> i32 {
        if self == Self::Main { 4 } else { 1 }
    }

    fn pos(self, col: i32, row: i32) -> (i32, i32) {
        let col = col.clamp(0, COLS - 1);
        let row = row.clamp(0, self.rows() - 1);
        match self {
            Self::Main => MAIN_GRID[(row * COLS + col) as usize],
            Self::Mushroom => MUSHROOM_GRID[col as usize],
            Self::Aquarium => AQUARIUM_GRID[col as usize],
        }
    }

    fn accepts(self, seed: SeedType) -> bool {
        match self {
            Self::Main => !seed.is_aquatic(),
            Self::Mushroom => seed.is_nocturnal(),
            Self::Aquarium => seed.is_aquatic(),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Main => "Main Garden",
            Self::Mushroom => "Mushroom Garden",
            Self::Aquarium => "Aquarium Garden",
        }
    }

    fn background(self) -> (&'static str, Option<&'static str>, &'static str) {
        match self {
            Self::Main => ("IMAGE_BACKGROUND_GREENHOUSE", Some("IMAGE_BACKGROUND_GREENHOUSE_OVERLAY"), "DelayLoad_GreenHouseGarden"),
            Self::Mushroom => ("IMAGE_BACKGROUND_MUSHROOMGARDEN", None, "DelayLoad_MushroomGarden"),
            Self::Aquarium => ("IMAGE_AQUARIUM1", Some("IMAGE_BACKGROUND_GREENHOUSE_OVERLAY"), "DelayLoad_Zombiquarium"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    None,
    Water,
    Fertilizer,
    BugSpray,
    Phonograph,
    Chocolate,
    Sell,
    Move,
}

/// `TutorialState::TUTORIAL_ZEN_GARDEN_*`: Dave's first-visit lesson.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum TutorialStep {
    #[default]
    NotStarted,
    Intro(usize),
    PickUpWater,
    WaterPlant,
    KeepWatering,
    VisitStore,
    Fertilize,
    Completed,
}

/// Kept across the store round trip (leaving the garden clears its resource).
#[derive(Resource, Default)]
pub struct ZenTutorial {
    step: TutorialStep,
}

struct GardenPlant {
    data: [u8; 88],
    body: Option<ReanimationId>,
    pot: Option<ReanimationId>,
    need_counter: i32,
    coin_counter: i32,
}

struct GardenCoin {
    x: f32,
    y: f32,
    value: i32,
    age: i32,
}

struct Stinky {
    x: f32,
    y: f32,
    body: Option<ReanimationId>,
    target: Option<usize>,
}

#[derive(Resource)]
pub struct ZenGarden {
    garden: GardenKind,
    plants: Vec<GardenPlant>,
    coins: Vec<GardenCoin>,
    tool: Tool,
    /// The plant in the wheelbarrow, if any (`GARDEN_WHEELBARROW`).
    carried: Option<[u8; 88]>,
    carried_body: Option<ReanimationId>,
    stinky: Option<Stinky>,
    advice: Option<(String, i32)>,
    mouse: Option<Vec2>,
}

fn tick_effects(mut effects: ResMut<EffectSystem>, mut sounds: Sounds) {
    effects.update(sounds.rng());
}

fn get_i32(data: &[u8; 88], off: usize) -> i32 {
    i32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

fn set_i32(data: &mut [u8; 88], off: usize, value: i32) {
    data[off..off + 4].copy_from_slice(&value.to_le_bytes());
}

fn seed_of(data: &[u8; 88]) -> Option<SeedType> {
    SeedType::from_index(get_i32(data, OFF_SEED))
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

fn age_scale(age: i32) -> f32 {
    match age {
        0 => 0.4,
        1 => 0.6,
        2 => 0.8,
        _ => 1.0,
    }
}

/// `ZenGarden::DrawPottedPlantIcon`: a pot with a full marigold, for the
/// store's potted-plant item.
pub(crate) fn draw_potted_plant_icon(
    g: &mut Graphics,
    res: &GameResources,
    x: f32,
    y: f32,
) {
    let draw = |g: &mut Graphics, reanim_type: crate::reanim::ReanimationType, x: f32, y: f32, track: &str| {
        let Some(definition) = res.loaded_reanim_definition(reanim_type.file()) else { return };
        let mut reanim = crate::reanim::Reanimation::new(x, y, definition);
        reanim.override_scale(0.7, 0.7);
        reanim.set_frames_for_layer(track);
        crate::reanim::draw_reanimation(g, &reanim, res);
    };
    draw(g, SeedType::Flowerpot.definition().reanim, x, y, "anim_zengarden");
    draw(g, SeedType::Marigold.definition().reanim, x, y + 20.0, "anim_idle");
}

fn make_plant(seed: SeedType, garden: GardenKind, col: i32, row: i32) -> [u8; 88] {
    let mut data = [0u8; 88];
    set_i32(&mut data, OFF_SEED, seed as i32);
    set_i32(&mut data, OFF_GARDEN, garden.index());
    set_i32(&mut data, OFF_X, col);
    set_i32(&mut data, OFF_Y, row);
    set_i32(&mut data, OFF_AGE, 0);
    set_i32(&mut data, OFF_FEEDINGS, 3);
    set_i32(&mut data, OFF_LAST_WATERED, now() as i32);
    set_i32(&mut data, OFF_LAST_FULFILLED, now() as i32);
    data
}

fn needs_pot(garden: GardenKind, seed: SeedType) -> bool {
    garden == GardenKind::Main && !seed.is_aquatic()
}

fn spawn_body(plant: &mut GardenPlant, garden: GardenKind, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>) {
    if let Some(id) = plant.body.take() {
        effects.kill(id);
    }
    let Some(seed) = seed_of(&plant.data) else { return };
    let (col, row) = (get_i32(&plant.data, OFF_X), get_i32(&plant.data, OFF_Y));
    let (px, py) = garden.pos(col, row);
    let age = get_i32(&plant.data, OFF_AGE);
    let draw_seed = if age == 0 { SeedType::Sprout } else { seed };
    let scale = age_scale(age);
    if let Ok((id, reanim)) = effects.add_reanimation(px as f32, py as f32 + 28.0 * scale, draw_seed.definition().reanim, res, images) {
        reanim.override_scale(scale, scale);
        reanim.loop_type = LoopType::Loop;
        let anim = if age == 0 && seed != SeedType::Marigold { "anim_idle_noflower" } else { "anim_idle" };
        reanim.play(anim, LoopType::Loop, 0, 6.0);
        plant.body = Some(id);
    }
}

fn spawn_pot(plant: &mut GardenPlant, garden: GardenKind, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>) {
    let Some(seed) = seed_of(&plant.data) else { return };
    if !needs_pot(garden, seed) {
        return;
    }
    let (col, row) = (get_i32(&plant.data, OFF_X), get_i32(&plant.data, OFF_Y));
    let (px, py) = garden.pos(col, row);
    if let Ok((id, reanim)) = effects.add_reanimation(px as f32, py as f32, SeedType::Flowerpot.definition().reanim, res, images) {
        reanim.override_scale(0.9, 0.9);
        reanim.play("anim_zengarden", LoopType::Loop, 0, 0.0);
        plant.pot = Some(id);
    }
}

fn spawn_plant(
    data: [u8; 88],
    garden: GardenKind,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
) -> GardenPlant {
    let mut plant = GardenPlant { data, body: None, pot: None, need_counter: NEED_TICKS / 2, coin_counter: COIN_TICKS };
    spawn_pot(&mut plant, garden, effects, res, images);
    spawn_body(&mut plant, garden, effects, res, images);
    plant
}

fn add_stinky(garden: &mut ZenGarden, player: &PlayerInfo, effects: &mut EffectSystem, res: &mut GameResources, images: &mut Assets<Image>) {
    if garden.garden != GardenKind::Main || player.purchases[StoreItem::StinkyTheSnail.slot()] == 0 {
        garden.stinky = None;
        return;
    }
    let (x, y) = if player.stinky_pos_x != 0 || player.stinky_pos_y != 0 {
        (player.stinky_pos_x as f32, player.stinky_pos_y as f32)
    } else {
        (200.0, 400.0)
    };
    let body = effects
        .add_reanimation(x, y, ReanimationType::Stinky, res, images)
        .ok()
        .map(|(id, reanim)| {
            reanim.override_scale(0.8, 0.8);
            reanim.play("anim_crawl", LoopType::Loop, 0, 6.0);
            id
        });
    garden.stinky = Some(Stinky { x, y, body, target: None });
}

fn load_garden(
    garden: &mut ZenGarden,
    kind: GardenKind,
    player: &PlayerInfo,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
) {
    effects.clear();
    garden.garden = kind;
    garden.plants.clear();
    garden.coins.clear();
    garden.tool = Tool::None;
    let (_, _, group) = kind.background();
    if let Err(e) = res.load_group_now(group, images) {
        error!("{e}");
    }
    if let Err(e) = res.load_image("IMAGE_COINBANK", images) {
        error!("{e}");
    }
    for data in player.potted_plants.iter().filter(|p| get_i32(p, OFF_GARDEN) == kind.index()) {
        garden.plants.push(spawn_plant(*data, kind, effects, res, images));
    }
    let carried = player.potted_plants.iter().find(|p| get_i32(p, OFF_GARDEN) == 2).copied();
    garden.carried = carried;
    if let Some(data) = carried
        && let Some(seed) = seed_of(&data)
        && let Ok((id, reanim)) = effects.add_reanimation(0.0, 0.0, seed.definition().reanim, res, images)
    {
        reanim.override_scale(0.5, 0.5);
        garden.carried_body = Some(id);
    }
    add_stinky(garden, player, effects, res, images);
}

fn start_garden(
    mut commands: Commands,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut effects: ResMut<EffectSystem>,
    mut music: ResMut<crate::audio::Music>,
    mut profiles: ResMut<ProfileMgr>,
    tutorial: Option<ResMut<ZenTutorial>>,
) {
    for id in [
        "IMAGE_WATERDROP",
        "IMAGE_WATERINGCAN",
        "IMAGE_ZENGARDEN_BUGSPRAY_BOTTLE",
        "IMAGE_FERTILIZER",
        "IMAGE_PHONOGRAPH",
        "IMAGE_REANIM_COIN_GOLD_DOLLAR",
    ] {
        if let Err(e) = resources.load_image(id, &mut images) {
            debug!("{e}");
        }
    }
    music.make_sure_music_is_playing(crate::audio::MusicTune::ZenGarden);
    let Some(player) = profiles.player_mut() else { return };
    // `ShouldDoZenTuturialBeforeAdventure`: level 45, no plants, no finish yet.
    let tutorial_eligible = std::env::var("PVZ_ZEN_TUTORIAL").is_ok() || tutorial_should_start(player);
    let Some(mut tutorial) = tutorial else {
        if player.potted_plants.is_empty() {
            for i in 0..2 {
                player.potted_plants.push(make_plant(SeedType::Marigold, GardenKind::Main, i * 3, 1));
            }
            profiles.save_current_details();
        }
        return;
    };
    if tutorial_eligible && tutorial.step == TutorialStep::NotStarted {
        tutorial.step = TutorialStep::Intro(0);
    }
    let start_tutorial = tutorial_eligible && tutorial.step != TutorialStep::NotStarted;
    if player.potted_plants.is_empty() && !start_tutorial {
        for i in 0..2 {
            player.potted_plants.push(make_plant(SeedType::Marigold, GardenKind::Main, i * 3, 1));
        }
        profiles.save_current_details();
    }
    let Some(player) = profiles.player() else { return };
    let initial = match std::env::var("PVZ_ZEN_GARDEN").as_deref() {
        Ok("mushroom") => GardenKind::Mushroom,
        Ok("aquarium") => GardenKind::Aquarium,
        _ => GardenKind::Main,
    };
    let mut garden = ZenGarden {
        garden: GardenKind::Main,
        plants: Vec::new(),
        coins: Vec::new(),
        tool: Tool::None,
        carried: None,
        carried_body: None,
        stinky: None,
        advice: None,
        mouse: None,
    };
    load_garden(&mut garden, initial, player, &mut effects, &mut resources, &mut images);
    commands.insert_resource(garden);
}

/// Writes the current plants back into the profile's raw records.
fn sync_profile(player: &mut PlayerInfo, garden: &ZenGarden) {
    for plant in &garden.plants {
        let (g, x, y) = (get_i32(&plant.data, OFF_GARDEN), get_i32(&plant.data, OFF_X), get_i32(&plant.data, OFF_Y));
        if let Some(slot) = player
            .potted_plants
            .iter_mut()
            .find(|p| get_i32(p, OFF_GARDEN) == g && get_i32(p, OFF_X) == x && get_i32(p, OFF_Y) == y)
        {
            *slot = plant.data;
        }
    }
    if let Some(carried) = garden.carried
        && let Some(slot) = player.potted_plants.iter_mut().find(|p| get_i32(p, OFF_GARDEN) == 2)
    {
        *slot = carried;
    }
    if let Some(stinky) = &garden.stinky {
        player.stinky_pos_x = stinky.x as i32;
        player.stinky_pos_y = stinky.y as i32;
    }
}

fn leave_garden(
    mut commands: Commands,
    garden: Option<Res<ZenGarden>>,
    mut profiles: ResMut<ProfileMgr>,
    mut effects: ResMut<EffectSystem>,
    mut tutorial: ResMut<ZenTutorial>,
) {
    if let Some(garden) = garden
        && let Some(player) = profiles.player_mut()
    {
        sync_profile(player, &garden);
        profiles.save_current_details();
    }
    if tutorial.step == TutorialStep::Completed {
        tutorial.step = TutorialStep::NotStarted;
    }
    effects.clear();
    commands.remove_resource::<ZenGarden>();
}

fn pos(garden: GardenKind, col: i32, row: i32) -> (i32, i32) {
    garden.pos(col, row)
}

fn plant_at(garden: &ZenGarden, point: Vec2) -> Option<usize> {
    garden.plants.iter().position(|plant| {
        let (col, row) = (get_i32(&plant.data, OFF_X), get_i32(&plant.data, OFF_Y));
        let (x, y) = pos(garden.garden, col, row);
        point.x >= x as f32 && point.x < x as f32 + 80.0 && point.y >= y as f32 && point.y < y as f32 + 85.0
    })
}

fn cell_at(garden: GardenKind, point: Vec2) -> Option<(i32, i32)> {
    for row in 0..garden.rows() {
        for col in 0..COLS {
            let (x, y) = garden.pos(col, row);
            if point.x >= x as f32 && point.x < x as f32 + 80.0 && point.y >= y as f32 && point.y < y as f32 + 85.0 {
                return Some((col, row));
            }
        }
    }
    None
}

fn coin_for_need(need: i32, age: i32, seed: SeedType) -> i32 {
    match need {
        NEED_WATER => 10,
        NEED_FERTILIZER => match age {
            1 => 25,
            2 => 50,
            _ if seed == SeedType::Marigold => 100,
            _ => 100,
        },
        _ => 50,
    }
}

fn add_coin(garden: &mut ZenGarden, index: usize, value: i32) {
    let plant = &garden.plants[index];
    let (col, row) = (get_i32(&plant.data, OFF_X), get_i32(&plant.data, OFF_Y));
    let (x, y) = garden.garden.pos(col, row);
    garden.coins.push(GardenCoin { x: x as f32 + 35.0, y: y as f32 + 15.0, value, age: 0 });
}

fn refresh_need(garden: &mut ZenGarden, index: usize, player: &PlayerInfo) {
    let plant = &mut garden.plants[index];
    let age = get_i32(&plant.data, OFF_AGE);
    let fed = get_i32(&plant.data, OFF_TIMES_FED);
    let feedings = get_i32(&plant.data, OFF_FEEDINGS).max(1);
    let seed = seed_of(&plant.data).unwrap_or(SeedType::Marigold);
    if fed < feedings {
        set_i32(&mut plant.data, OFF_NEED, NEED_WATER);
    } else if age < FULL_AGE {
        set_i32(&mut plant.data, OFF_NEED, NEED_FERTILIZER);
    } else if seed == SeedType::Marigold && player.purchases[StoreItem::Phonograph.slot()] == 0 {
        set_i32(&mut plant.data, OFF_NEED, NEED_WATER);
    } else {
        // Grown plants ask for a random extra in the original; keep it to the
        // tools the player owns (water is always available).
        let mut choices = vec![NEED_WATER];
        if player.purchases[StoreItem::BugSpray.slot()] > PURCHASE_COUNT_OFFSET {
            choices.push(NEED_BUGSPRAY);
        }
        if player.purchases[StoreItem::Phonograph.slot()] != 0 {
            choices.push(NEED_PHONOGRAPH);
        }
        let pick = (index * 7 + age as usize + player.coins as usize) % choices.len();
        set_i32(&mut plant.data, OFF_NEED, choices[pick]);
    }
    plant.need_counter = NEED_TICKS;
}

fn update_garden(mut garden: ResMut<ZenGarden>, mut profiles: ResMut<ProfileMgr>, mut effects: ResMut<EffectSystem>, mut res: ResMut<GameResources>, mut images: ResMut<Assets<Image>>) {
    let Some(player) = profiles.player().cloned() else { return };
    let garden = &mut *garden;
    let mut spawned: Vec<GardenCoin> = Vec::new();
    for i in 0..garden.plants.len() {
        let needs_refresh = {
            let plant = &mut garden.plants[i];
            if get_i32(&plant.data, OFF_NEED) == NEED_NONE {
                plant.need_counter -= 1;
                plant.need_counter <= 0
            } else {
                false
            }
        };
        if needs_refresh {
            refresh_need(garden, i, &player);
        }
        let plant = &mut garden.plants[i];
        let age = get_i32(&plant.data, OFF_AGE);
        let high_on_chocolate = now() as i32 - get_i32(&plant.data, OFF_LAST_CHOCOLATE) < 300;
        if age == FULL_AGE && get_i32(&plant.data, OFF_NEED) == NEED_NONE {
            plant.coin_counter -= if high_on_chocolate { 2 } else { 1 };
            if plant.coin_counter <= 0 {
                plant.coin_counter = COIN_TICKS;
                let (col, row) = (get_i32(&plant.data, OFF_X), get_i32(&plant.data, OFF_Y));
                let (x, y) = pos(garden.garden, col, row);
                spawned.push(GardenCoin { x: x as f32 + 30.0, y: y as f32 - 10.0, value: 10, age: 0 });
            }
        }
    }
    garden.coins.extend(spawned);
    for coin in &mut garden.coins {
        coin.age += 1;
    }
    if let Some(body) = garden.carried_body
        && let Some(point) = garden.mouse
        && let Some(reanim) = effects.get_mut(body)
    {
        reanim.set_position(point.x - 40.0, point.y - 60.0);
    }
    // Stinky walks toward the nearest coin and eats it.
    if garden.garden == GardenKind::Main
        && let Some(stinky) = &mut garden.stinky
    {
        if let Some(i) = garden.coins.iter().position(|c| (c.x - stinky.x).abs() < 40.0 && (c.y - stinky.y).abs() < 50.0) {
            let coin = garden.coins.remove(i);
            if let Some(p) = profiles.player_mut() {
                p.coins = (p.coins + coin.value).min(99_999);
            }
            if let Some(body) = stinky.body
                && let Some(reanim) = effects.get_mut(body)
            {
                reanim.play("anim_idle", LoopType::Loop, 0, 6.0);
            }
            stinky.target = None;
        } else if stinky.target.is_none() {
            stinky.target = (0..garden.coins.len()).min_by(|&a, &b| {
                let da = (garden.coins[a].x - stinky.x).abs() + (garden.coins[a].y - stinky.y).abs();
                let db = (garden.coins[b].x - stinky.x).abs() + (garden.coins[b].y - stinky.y).abs();
                da.total_cmp(&db)
            });
        }
        if let Some(target) = stinky.target
            && let Some(coin) = garden.coins.get(target)
        {
            let (tx, ty) = (coin.x - 20.0, coin.y - 20.0);
            let (dx, dy) = (tx - stinky.x, ty - stinky.y);
            let distance = (dx * dx + dy * dy).sqrt().max(1.0);
            stinky.x += dx / distance * 1.2;
            stinky.y += dy / distance * 0.9;
            if let Some(body) = stinky.body
                && let Some(reanim) = effects.get_mut(body)
            {
                reanim.set_position(stinky.x, stinky.y);
                let facing = if dx < 0.0 { -0.8 } else { 0.8 };
                reanim.override_scale(facing, 0.8);
            }
        }
        let _ = (&mut res, &mut images);
    }
}

/// `ZenGarden::FeedChocolateToPlant`: five minutes of double production.
fn feed_chocolate(garden: &mut ZenGarden, index: usize, profiles: &mut ProfileMgr, effects: &mut EffectSystem, sounds: &mut Sounds) {
    let Some(player) = profiles.player_mut() else { return };
    let slot = StoreItem::Chocolate.slot();
    if player.purchases[slot] <= PURCHASE_COUNT_OFFSET {
        return;
    }
    player.purchases[slot] -= 1;
    set_i32(&mut garden.plants[index].data, OFF_LAST_CHOCOLATE, now() as i32);
    if let Some(body) = garden.plants[index].body
        && let Some(reanim) = effects.get_mut(body)
    {
        // `SetPlantAnimSpeed`: visibly excited without changing the track.
        reanim.anim_rate = 18.0;
    }
    profiles.save_current_details();
    sounds.play_foley(FoleyType::Chime);
}

fn apply_tool(
    garden: &mut ZenGarden,
    index: usize,
    profiles: &mut ProfileMgr,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
    sounds: &mut Sounds,
) {
    let tool = garden.tool;
    let need = get_i32(&garden.plants[index].data, OFF_NEED);
    let age = get_i32(&garden.plants[index].data, OFF_AGE);
    let seed = seed_of(&garden.plants[index].data).unwrap_or(SeedType::Marigold);
    let mut grew = false;
    match tool {
        Tool::Water if need == NEED_WATER => {
            {
                let plant = &mut garden.plants[index];
                let fed = get_i32(&plant.data, OFF_TIMES_FED) + 1;
                set_i32(&mut plant.data, OFF_TIMES_FED, fed);
                set_i32(&mut plant.data, OFF_LAST_WATERED, now() as i32);
                set_i32(&mut plant.data, OFF_NEED, NEED_NONE);
                plant.need_counter = NEED_TICKS;
                if fed >= get_i32(&plant.data, OFF_FEEDINGS).max(1) && age < FULL_AGE {
                    set_i32(&mut plant.data, OFF_NEED, NEED_FERTILIZER);
                }
            }
            sounds.play_foley(FoleyType::PlantWater);
            let coin = coin_for_need(NEED_WATER, age, seed);
            add_coin(garden, index, coin);
        }
        Tool::Fertilizer if need == NEED_FERTILIZER => {
            if let Some(player) = profiles.player_mut() {
                if player.purchases[StoreItem::Fertilizer.slot()] <= PURCHASE_COUNT_OFFSET {
                    return;
                }
                player.purchases[StoreItem::Fertilizer.slot()] -= 1;
            }
            let plant = &mut garden.plants[index];
            let age = (get_i32(&plant.data, OFF_AGE) + 1).min(FULL_AGE);
            set_i32(&mut plant.data, OFF_AGE, age);
            set_i32(&mut plant.data, OFF_TIMES_FED, 0);
            set_i32(&mut plant.data, OFF_NEED, NEED_NONE);
            set_i32(&mut plant.data, OFF_LAST_FERTILIZED, now() as i32);
            plant.need_counter = NEED_TICKS;
            grew = true;
            sounds.play_foley(FoleyType::PlantGrow);
            let coin = coin_for_need(NEED_FERTILIZER, age, seed);
            add_coin(garden, index, coin);
        }
        Tool::BugSpray if need == NEED_BUGSPRAY => {
            if let Some(player) = profiles.player_mut() {
                if player.purchases[StoreItem::BugSpray.slot()] <= PURCHASE_COUNT_OFFSET {
                    return;
                }
                player.purchases[StoreItem::BugSpray.slot()] -= 1;
            }
            clear_need(garden, index, NEED_TICKS);
            sounds.play_foley(FoleyType::SpawnSun);
            add_coin(garden, index, 50);
        }
        Tool::Phonograph if need == NEED_PHONOGRAPH => {
            clear_need(garden, index, NEED_TICKS);
            sounds.play_foley(FoleyType::Chime);
            add_coin(garden, index, 50);
        }
        _ => return,
    }
    set_i32(&mut garden.plants[index].data, OFF_LAST_FULFILLED, now() as i32);
    profiles.save_current_details();
    if grew {
        spawn_body(&mut garden.plants[index], garden.garden, effects, res, images);
    }
}

fn clear_need(garden: &mut ZenGarden, index: usize, timer: i32) {
    let plant = &mut garden.plants[index];
    set_i32(&mut plant.data, OFF_NEED, NEED_NONE);
    plant.need_counter = timer;
}

fn sell_price(plant: &GardenPlant) -> i32 {
    let age = get_i32(&plant.data, OFF_AGE);
    let seed = seed_of(&plant.data).unwrap_or(SeedType::Marigold);
    if seed == SeedType::Marigold {
        return [150, 200, 250, 300][age.clamp(0, 3) as usize];
    }
    match age {
        0 => 150,
        1 => 300,
        2 => 500,
        _ if seed.is_nocturnal() || seed.is_aquatic() => 1000,
        _ => 800,
    }
}

fn sell_plant(
    garden: &mut ZenGarden,
    index: usize,
    profiles: &mut ProfileMgr,
    effects: &mut EffectSystem,
    sounds: &mut Sounds,
) {
    let data = garden.plants[index].data;
    let price = sell_price(&garden.plants[index]);
    if let Some(player) = profiles.player_mut() {
        player.coins = (player.coins + price).min(99_999);
        // Original: later records shift down; our key-based lookup just
        // removes this one.
        if let Some(slot) = player.potted_plants.iter().position(|p| {
            get_i32(p, OFF_GARDEN) == get_i32(&data, OFF_GARDEN)
                && get_i32(p, OFF_X) == get_i32(&data, OFF_X)
                && get_i32(p, OFF_Y) == get_i32(&data, OFF_Y)
        }) {
            player.potted_plants.remove(slot);
        }
        profiles.save_current_details();
    }
    let plant = garden.plants.remove(index);
    for id in [plant.body, plant.pot].into_iter().flatten() {
        effects.kill(id);
    }
    sounds.play_foley(FoleyType::UseShovel);
}

fn pick_up(garden: &mut ZenGarden, index: usize, profiles: &mut ProfileMgr, effects: &mut EffectSystem, sounds: &mut Sounds) {
    let plant = garden.plants.remove(index);
    let mut data = plant.data;
    let (old_garden, old_x, old_y) = (get_i32(&data, OFF_GARDEN), get_i32(&data, OFF_X), get_i32(&data, OFF_Y));
    for id in [plant.body, plant.pot].into_iter().flatten() {
        effects.kill(id);
    }
    set_i32(&mut data, OFF_GARDEN, 2);
    set_i32(&mut data, OFF_X, 0);
    set_i32(&mut data, OFF_Y, 0);
    garden.carried = Some(data);
    if let Some(player) = profiles.player_mut() {
        if let Some(slot) = player.potted_plants.iter_mut().find(|p| {
            get_i32(p, OFF_GARDEN) == old_garden && get_i32(p, OFF_X) == old_x && get_i32(p, OFF_Y) == old_y
        }) {
            *slot = data;
        }
        profiles.save_current_details();
    }
    sounds.play_foley(FoleyType::Plant);
}

#[allow(clippy::too_many_arguments)]
fn place_carried(
    garden: &mut ZenGarden,
    col: i32,
    row: i32,
    profiles: &mut ProfileMgr,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
    sounds: &mut Sounds,
) {
    let Some(mut data) = garden.carried.take() else { return };
    let Some(seed) = seed_of(&data) else { return };
    if !garden.garden.accepts(seed) {
        garden.carried = Some(data);
        garden.advice = Some((only_plant_advice(garden.garden).to_owned(), 200));
        return;
    }
    if garden.plants.iter().any(|plant| get_i32(&plant.data, OFF_X) == col && get_i32(&plant.data, OFF_Y) == row) {
        garden.carried = Some(data);
        return;
    }
    set_i32(&mut data, OFF_GARDEN, garden.garden.index());
    set_i32(&mut data, OFF_X, col);
    set_i32(&mut data, OFF_Y, row);
    if let Some(body) = garden.carried_body.take() {
        effects.kill(body);
    }
    garden.plants.push(spawn_plant(data, garden.garden, effects, res, images));
    if let Some(player) = profiles.player_mut() {
        if let Some(slot) = player.potted_plants.iter_mut().find(|p| get_i32(p, OFF_GARDEN) == 2) {
            *slot = data;
        }
        profiles.save_current_details();
    }
    sounds.play_foley(FoleyType::Plant);
}

fn only_plant_advice(garden: GardenKind) -> &'static str {
    match garden {
        GardenKind::Aquarium => "Only aquatic plants can go in the aquarium.",
        GardenKind::Mushroom => "Only mushrooms can go in the mushroom garden.",
        GardenKind::Main => "Only land plants can go in the main garden.",
    }
}

/// `ShouldDoZenTuturialBeforeAdventure`.
fn tutorial_should_start(player: &PlayerInfo) -> bool {
    !player.has_finished_adventure() && player.level == 45 && player.potted_plants.is_empty()
}

/// Dave's message 2102: two marigolds appear for the lesson
/// (`ZenGarden::AdvanceCrazyDaveDialog`).
fn tutorial_add_marigolds(
    garden: &mut ZenGarden,
    profiles: &mut ProfileMgr,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
) {
    if !garden.plants.is_empty() {
        return;
    }
    let mut added = Vec::new();
    if let Some(player) = profiles.player_mut() {
        for _ in 0..2 {
            let before = player.potted_plants.len();
            if crate::zen_garden::add_potted_plant(player, SeedType::Marigold) {
                added.extend_from_slice(&player.potted_plants[before..]);
            }
        }
        profiles.save_current_details();
    }
    for mut data in added {
        set_i32(&mut data, OFF_NEED, NEED_WATER);
        garden.plants.push(spawn_plant(data, GardenKind::Main, effects, res, images));
    }
}

fn tutorial_text(res: &GameResources, step: TutorialStep) -> Option<String> {
    let key = match step {
        TutorialStep::PickUpWater => "[ADVICE_ZEN_GARDEN_PICK_UP_WATER]",
        TutorialStep::WaterPlant => "[ADVICE_ZEN_GARDEN_WATER_PLANT]",
        TutorialStep::KeepWatering => "[ADVICE_ZEN_GARDEN_KEEP_WATERING]",
        TutorialStep::VisitStore => "[ADVICE_ZEN_GARDEN_VISIT_STORE]",
        TutorialStep::Fertilize => "[ADVICE_ZEN_GARDEN_FERTILIZE]",
        TutorialStep::Completed => "[ADVICE_ZEN_GARDEN_CONTINUE_ADVENTURE]",
        _ => return None,
    };
    Some(res.translate(key))
}

#[allow(clippy::too_many_arguments)]
fn garden_input(
    mut commands: Commands,
    mut garden: ResMut<ZenGarden>,
    cursor: Res<GameCursor>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut profiles: ResMut<ProfileMgr>,
    mut effects: ResMut<EffectSystem>,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut sounds: Sounds,
    mut tutorial: ResMut<ZenTutorial>,
) {
    let garden = &mut *garden;
    garden.mouse = cursor.0;
    let Some(point) = cursor.0 else { return };
    let in_rect = |x: f32, y: f32, w: f32, h: f32| point.x >= x && point.y >= y && point.x < x + w && point.y < y + h;

    if let TutorialStep::Intro(line) = tutorial.step {
        if mouse.just_released(MouseButton::Left) {
            if line == 2 {
                tutorial_add_marigolds(garden, &mut profiles, &mut effects, &mut resources, &mut images);
            }
            tutorial.step = if line >= 4 { TutorialStep::PickUpWater } else { TutorialStep::Intro(line + 1) };
            sounds.play_sample("SOUND_TAP");
        }
        return;
    }

    let buttons: [(Tool, &str, f32); 5] = [
        (Tool::Water, "Water", 130.0),
        (Tool::Fertilizer, "Fertilizer", 240.0),
        (Tool::BugSpray, "Bug Spray", 350.0),
        (Tool::Phonograph, "Music", 460.0),
        (Tool::Chocolate, "Chocolate", 570.0),
    ];
    if mouse.just_released(MouseButton::Left) {
        if in_rect(10.0, 555.0, 110.0, 36.0) {
            next_state.set(AppState::GameSelector);
            return;
        }
        if in_rect(580.0, 510.0, 100.0, 36.0) {
            garden.tool = if garden.tool == Tool::Sell { Tool::None } else { Tool::Sell };
            sounds.play_sample("SOUND_TAP");
            return;
        }
        if in_rect(690.0, 510.0, 100.0, 36.0) {
            next_garden(garden, &mut profiles, &mut effects, &mut resources, &mut images, &mut sounds, &mut next_state);
            return;
        }
        if in_rect(300.0, 510.0, 100.0, 36.0) {
            if tutorial.step == TutorialStep::VisitStore {
                if let Some(player) = profiles.player_mut()
                    && player.purchases[StoreItem::Fertilizer.slot()] <= PURCHASE_COUNT_OFFSET
                {
                    player.purchases[StoreItem::Fertilizer.slot()] = PURCHASE_COUNT_OFFSET + 5;
                }
                tutorial.step = TutorialStep::Fertilize;
                profiles.save_current_details();
            }
            commands.insert_resource(crate::store::StoreReturn(Some(AppState::ZenGarden)));
            next_state.set(AppState::Store);
            return;
        }
        if in_rect(470.0, 510.0, 100.0, 36.0) {
            if profiles.player().is_some_and(|p| p.purchases[StoreItem::WheelBarrow.slot()] == 0) {
                garden.advice = Some(("You need a wheelbarrow to move plants between gardens.".to_owned(), 300));
            } else {
                garden.tool = if garden.tool == Tool::Move { Tool::None } else { Tool::Move };
                sounds.play_sample("SOUND_TAP");
            }
            return;
        }
        for (tool, _, x) in buttons {
            if in_rect(x, 555.0, 100.0, 36.0) {
                let available = tool_available(profiles.player(), tool);
                if !available {
                    garden.advice = Some((format!("You need {} first.", match tool {
                        Tool::Fertilizer => "fertilizer",
                        Tool::BugSpray => "bug spray",
                        Tool::Phonograph => "a phonograph",
                        _ => "that tool",
                    }), 250));
                    return;
                }
                garden.tool = if garden.tool == tool { Tool::None } else { tool };
                if garden.tool == Tool::Water && tutorial.step == TutorialStep::PickUpWater {
                    tutorial.step = TutorialStep::WaterPlant;
                }
                sounds.play_sample("SOUND_TAP");
                return;
            }
        }
        if let Some(i) = garden.coins.iter().position(|c| in_rect(c.x, c.y, 40.0, 40.0)) {
            let coin = garden.coins.remove(i);
            if let Some(player) = profiles.player_mut() {
                player.coins = (player.coins + coin.value).min(99_999);
            }
            profiles.save_current_details();
            sounds.play_foley(FoleyType::Coin);
            return;
        }
        if garden.tool == Tool::Move {
            if garden.carried.is_some() {
                if let Some((col, row)) = cell_at(garden.garden, point) {
                    place_carried(garden, col, row, &mut profiles, &mut effects, &mut resources, &mut images, &mut sounds);
                }
            } else if let Some(index) = plant_at(garden, point) {
                pick_up(garden, index, &mut profiles, &mut effects, &mut sounds);
            }
            return;
        }
        if let Some(index) = plant_at(garden, point) {
            if garden.tool == Tool::Sell {
                sell_plant(garden, index, &mut profiles, &mut effects, &mut sounds);
            } else if garden.tool == Tool::Chocolate {
                feed_chocolate(garden, index, &mut profiles, &mut effects, &mut sounds);
            } else if garden.tool != Tool::None {
                let need = get_i32(&garden.plants[index].data, OFF_NEED);
                apply_tool(garden, index, &mut profiles, &mut effects, &mut resources, &mut images, &mut sounds);
                if garden.tool == Tool::Water && need == NEED_WATER {
                    if tutorial.step == TutorialStep::WaterPlant {
                        tutorial.step = TutorialStep::KeepWatering;
                    }
                    if tutorial.step == TutorialStep::KeepWatering
                        && garden.plants.iter().any(|p| get_i32(&p.data, OFF_NEED) == NEED_FERTILIZER)
                    {
                        tutorial.step = TutorialStep::VisitStore;
                    }
                }
                if garden.tool == Tool::Fertilizer
                    && need == NEED_FERTILIZER
                    && tutorial.step == TutorialStep::Fertilize
                    && garden.plants.iter().all(|p| get_i32(&p.data, OFF_AGE) > 0)
                {
                    tutorial.step = TutorialStep::Completed;
                }
            }
        }
    }
}

fn next_garden(
    garden: &mut ZenGarden,
    profiles: &mut ProfileMgr,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
    sounds: &mut Sounds,
    next_state: &mut NextState<AppState>,
) {
    let Some(player) = profiles.player() else { return };
    let bought = |item: StoreItem| player.purchases[item.slot()] != 0;
    let current = garden.garden;
    let mut go_to_tree = false;
    let next = match current {
        GardenKind::Main => {
            if bought(StoreItem::MushroomGarden) {
                Some(GardenKind::Mushroom)
            } else if bought(StoreItem::AquariumGarden) {
                Some(GardenKind::Aquarium)
            } else if bought(StoreItem::TreeOfWisdom) {
                go_to_tree = true;
                None
            } else {
                None
            }
        }
        GardenKind::Mushroom => {
            if bought(StoreItem::AquariumGarden) {
                Some(GardenKind::Aquarium)
            } else if bought(StoreItem::TreeOfWisdom) {
                go_to_tree = true;
                None
            } else {
                Some(GardenKind::Main)
            }
        }
        GardenKind::Aquarium => {
            if bought(StoreItem::TreeOfWisdom) {
                go_to_tree = true;
                None
            } else {
                Some(GardenKind::Main)
            }
        }
    };
    if go_to_tree {
        if let Some(player) = profiles.player_mut() {
            sync_profile(player, garden);
            profiles.save_current_details();
        }
        next_state.set(AppState::TreeOfWisdom);
        return;
    }
    let Some(next) = next else {
        garden.advice = Some(("Buy another garden in Crazy Dave's store first.".to_owned(), 250));
        return;
    };
    if let Some(player) = profiles.player_mut() {
        sync_profile(player, garden);
        profiles.save_current_details();
    }
    sounds.play_foley(FoleyType::UseShovel);
    let Some(player) = profiles.player() else { return };
    load_garden(garden, next, player, effects, res, images);
}

fn tool_available(player: Option<&PlayerInfo>, tool: Tool) -> bool {
    let Some(player) = player else { return false };
    let count = |item: StoreItem| player.purchases[item.slot()];
    match tool {
        Tool::None => false,
        Tool::Water | Tool::Sell | Tool::Move => true,
        Tool::Fertilizer => count(StoreItem::Fertilizer) > PURCHASE_COUNT_OFFSET,
        Tool::BugSpray => count(StoreItem::BugSpray) > PURCHASE_COUNT_OFFSET,
        Tool::Phonograph => count(StoreItem::Phonograph) != 0,
        Tool::Chocolate => count(StoreItem::Chocolate) > PURCHASE_COUNT_OFFSET,
    }
}

fn need_image(need: i32) -> Option<&'static str> {
    match need {
        NEED_WATER => Some("IMAGE_WATERDROP"),
        NEED_FERTILIZER => Some("IMAGE_FERTILIZER"),
        NEED_BUGSPRAY => Some("IMAGE_ZENGARDEN_BUGSPRAY_BOTTLE"),
        NEED_PHONOGRAPH => Some("IMAGE_PHONOGRAPH"),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_garden(
    garden: Res<ZenGarden>,
    tutorial: Res<ZenTutorial>,
    resources: Res<GameResources>,
    effects: Res<EffectSystem>,
    profiles: Res<ProfileMgr>,
    mut g: ResMut<Graphics>,
) {
    let res = &*resources;
    let g = &mut *g;
    let (background, overlay, _) = garden.garden.background();
    if let Some(image) = res.image(background) {
        g.draw_image(image, 0.0, 0.0);
    }
    if let Some(bank) = res.image("IMAGE_COINBANK") {
        g.draw_image(bank, 0.0, 0.0);
    }
    for plant in &garden.plants {
        for id in [plant.pot, plant.body].into_iter().flatten() {
            effects.draw(g, id, res);
        }
        if let Some(id) = need_image(get_i32(&plant.data, OFF_NEED))
            && let Some(image) = res.image(id)
        {
            let (col, row) = (get_i32(&plant.data, OFF_X), get_i32(&plant.data, OFF_Y));
            let (x, y) = pos(garden.garden, col, row);
            g.draw_image(image, x as f32 + 42.0, y as f32 - 14.0);
        }
    }
    if let Some(overlay) = overlay
        && let Some(image) = res.image(overlay)
    {
        g.draw_image(image, 0.0, 0.0);
    }
    if let Some(coin) = res.image("IMAGE_REANIM_COIN_GOLD_DOLLAR") {
        for c in &garden.coins {
            g.draw_image_cel_center_scaled(coin, c.x + 20.0, c.y + 20.0, 0, 1.0, 1.0);
        }
    }
    if let Some(font) = res.font("FONT_HOUSEOFTERROR16")
        && let Some(player) = profiles.player()
    {
        g.tod_draw_string(font, 95, 24, &player.coins.to_string(), SexyColor::rgba(255, 240, 0, 255), Justify::Center);
    }
    if let Some(font) = res.font("FONT_BRIANNETOD12") {
        g.tod_draw_string(font, 400, 470, garden.garden.name(), SexyColor::rgba(255, 255, 255, 255), Justify::Center);
        if let Some((advice, _)) = &garden.advice {
            g.tod_draw_string(font, 400, 495, advice, SexyColor::rgba(255, 240, 0, 255), Justify::Center);
        }
    }

    if let TutorialStep::Intro(line) = tutorial.step {
        g.set_color(SexyColor::rgba(20, 20, 30, 220));
        g.fill_rect(110.0, 120.0, 580.0, 330.0);
        g.set_color(SexyColor::rgba(255, 255, 255, 255));
        if let Some(font) = res.font("FONT_BRIANNETOD16") {
            let text = res.translate(&format!("[CRAZY_DAVE_210{line}]"));
            g.tod_draw_string_wrapped(font, SexyRect::new(140.0, 150.0, 520.0, 210.0), &text, SexyColor::rgba(240, 240, 190, 255), Justify::Center);
            g.tod_draw_string(font, 400, 410, "Click to continue", SexyColor::rgba(255, 240, 0, 255), Justify::Center);
        }
    } else if let Some(text) = tutorial_text(res, tutorial.step) {
        g.set_color(SexyColor::rgba(20, 20, 30, 200));
        g.fill_rect(180.0, 430.0, 440.0, 56.0);
        g.set_color(SexyColor::rgba(255, 255, 255, 255));
        if let Some(font) = res.font("FONT_BRIANNETOD12") {
            g.tod_draw_string_wrapped(font, SexyRect::new(190.0, 438.0, 420.0, 44.0), &text, SexyColor::rgba(255, 240, 0, 255), Justify::Center);
        }
    }
    draw_stone_button(g, res, 10, 555, 110, 36, false, false, "");
    draw_stone_button(g, res, 130, 555, 100, 36, false, false, "");
    draw_stone_button(g, res, 240, 555, 100, 36, false, false, "");
    draw_stone_button(g, res, 350, 555, 100, 36, false, false, "");
    draw_stone_button(g, res, 460, 555, 100, 36, false, false, "");
    draw_stone_button(g, res, 570, 555, 100, 36, false, false, "");
    draw_stone_button(g, res, 300, 510, 100, 36, false, false, "");
    draw_stone_button(g, res, 470, 510, 100, 36, false, false, "");
    draw_stone_button(g, res, 580, 510, 100, 36, false, false, "");
    draw_stone_button(g, res, 690, 510, 100, 36, false, false, "");

    if let Some(font) = res.font("FONT_BRIANNETOD12") {
        let labels = [
            ("Menu".to_owned(), 10.0, 573.0, None),
            ("Water".to_owned(), 130.0, 573.0, Some(Tool::Water)),
            ("Fertil.".to_owned(), 240.0, 573.0, Some(Tool::Fertilizer)),
            ("Spray".to_owned(), 350.0, 573.0, Some(Tool::BugSpray)),
            ("Music".to_owned(), 460.0, 573.0, Some(Tool::Phonograph)),
            ("Choc.".to_owned(), 570.0, 573.0, Some(Tool::Chocolate)),
            ("Shop".to_owned(), 300.0, 528.0, None),
            ("Move".to_owned(), 470.0, 528.0, Some(Tool::Move)),
            ("Sell".to_owned(), 580.0, 528.0, Some(Tool::Sell)),
            ("Next".to_owned(), 690.0, 528.0, None),
        ];
        for (label, x, y, tool) in labels {
            let tutorial_wants = || match tutorial.step {
                TutorialStep::PickUpWater => tool == Some(Tool::Water),
                TutorialStep::VisitStore => label == "Shop",
                TutorialStep::Fertilize => tool == Some(Tool::Fertilizer),
                _ => false,
            };
            let active = tool.is_some_and(|t| garden.tool == t) || tutorial_wants();
            let available = tool.is_none_or(|t| tool_available(profiles.player(), t));
            let color = if active {
                SexyColor::rgba(255, 220, 0, 255)
            } else if !available {
                SexyColor::rgba(120, 120, 120, 255)
            } else {
                SexyColor::rgba(255, 245, 200, 255)
            };
            g.tod_draw_string(font, x as i32 + 50, y as i32, &label, color, Justify::Center);
        }
    }
    // The carried plant follows the cursor.
    if let Some(body) = garden.carried_body {
        effects.draw(g, body, res);
    }
}

/// Any potted plant wants water (the selector's Zen bubble).
pub fn plants_need_water(player: &PlayerInfo) -> bool {
    player.potted_plants.iter().any(|p| {
        let garden = GardenKind::from_index(get_i32(p, OFF_GARDEN));
        garden.is_some() && get_i32(p, OFF_NEED) == NEED_WATER
    })
}

/// `ZenGarden::IsZenGardenFull`: 32 main-garden pots in use.
pub fn main_garden_full(player: &PlayerInfo) -> bool {
    player.potted_plants.iter().filter(|p| get_i32(p, OFF_GARDEN) == 0).count() >= 32
}

/// The store's potted marigold purchase: put it in the first free main-garden
/// spot (`ZenGarden::AddPottedPlant`).
pub fn add_potted_plant(player: &mut PlayerInfo, seed: SeedType) -> bool {
    let mut occupied = [false; 32];
    for p in &player.potted_plants {
        if get_i32(p, OFF_GARDEN) == 0 {
            let index = get_i32(p, OFF_Y) * COLS + get_i32(p, OFF_X);
            if (0..32).contains(&index) {
                occupied[index as usize] = true;
            }
        }
    }
    let Some(index) = occupied.iter().position(|taken| !taken) else { return false };
    let plant = make_plant(seed, GardenKind::Main, index as i32 % COLS, index as i32 / COLS);
    player.potted_plants.push(plant);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn potted_plant_records_round_trip() {
        let mut data = make_plant(SeedType::Marigold, GardenKind::Main, 3, 2);
        set_i32(&mut data, OFF_TIMES_FED, 2);
        assert_eq!(get_i32(&data, OFF_X), 3);
        assert_eq!(get_i32(&data, OFF_Y), 2);
        assert_eq!(get_i32(&data, OFF_TIMES_FED), 2);
        assert_eq!(seed_of(&data), Some(SeedType::Marigold));
    }

    #[test]
    fn watering_fills_feeding_and_fertilizer_grows() {
        let mut plant = GardenPlant { data: make_plant(SeedType::Marigold, GardenKind::Main, 0, 0), body: None, pot: None, need_counter: 0, coin_counter: 0 };
        let faces = get_i32(&plant.data, OFF_FEEDINGS);
        for _ in 0..faces {
            let fed = get_i32(&plant.data, OFF_TIMES_FED) + 1;
            set_i32(&mut plant.data, OFF_TIMES_FED, fed);
        }
        assert_eq!(get_i32(&plant.data, OFF_TIMES_FED), 3);
        let age = (get_i32(&plant.data, OFF_AGE) + 1).min(FULL_AGE);
        set_i32(&mut plant.data, OFF_AGE, age);
        set_i32(&mut plant.data, OFF_TIMES_FED, 0);
        assert_eq!(get_i32(&plant.data, OFF_AGE), 1);
    }

    #[test]
    fn selling_prices_match_the_original() {
        let mut plant = GardenPlant { data: make_plant(SeedType::Peashooter, GardenKind::Main, 0, 0), body: None, pot: None, need_counter: 0, coin_counter: 0 };
        assert_eq!(sell_price(&plant), 150);
        for (age, price) in [(1, 300), (2, 500), (3, 800)] {
            set_i32(&mut plant.data, OFF_AGE, age);
            assert_eq!(sell_price(&plant), price);
        }
    }

    #[test]
    fn garden_accepts_only_its_plants() {
        assert!(GardenKind::Aquarium.accepts(SeedType::Lilypad));
        assert!(!GardenKind::Aquarium.accepts(SeedType::Peashooter));
        assert!(GardenKind::Main.accepts(SeedType::Peashooter));
        assert!(!GardenKind::Main.accepts(SeedType::Lilypad));
    }

    #[test]
    fn need_water_detection_ignores_other_gardens_and_needs() {
        let mut player = PlayerInfo::default();
        assert!(!plants_need_water(&player));
        let mut plant = make_plant(SeedType::Marigold, GardenKind::Main, 0, 0);
        set_i32(&mut plant, OFF_NEED, NEED_WATER);
        player.potted_plants.push(plant);
        assert!(plants_need_water(&player));
        assert_eq!(NEED_WATER, 1);
    }

    #[test]
    fn tutorial_only_starts_at_level_45_before_the_finish() {
        let mut player = PlayerInfo::default();
        assert!(!tutorial_should_start(&player));
        player.level = 45;
        assert!(tutorial_should_start(&player));
        player.potted_plants.push(make_plant(SeedType::Marigold, GardenKind::Main, 0, 0));
        assert!(!tutorial_should_start(&player));
        player.potted_plants.clear();
        player.finished_adventure = 1;
        assert!(!tutorial_should_start(&player));
    }

    #[test]
    fn sync_writes_growth_back_and_fills_up() {
        let mut player = PlayerInfo::default();
        let data = make_plant(SeedType::Marigold, GardenKind::Main, 0, 0);
        player.potted_plants.push(data);
        let mut grown = data;
        set_i32(&mut grown, OFF_AGE, 2);
        let garden = ZenGarden {
            garden: GardenKind::Main,
            plants: vec![GardenPlant { data: grown, body: None, pot: None, need_counter: 0, coin_counter: 0 }],
            coins: Vec::new(),
            tool: Tool::None,
            carried: None,
            carried_body: None,
            stinky: None,
            advice: None,
            mouse: None,
        };
        sync_profile(&mut player, &garden);
        assert_eq!(get_i32(&player.potted_plants[0], OFF_AGE), 2);
        for i in 0..32 {
            let mut plant = make_plant(SeedType::Marigold, GardenKind::Main, i % COLS, i / COLS);
            set_i32(&mut plant, OFF_X, i % COLS);
            set_i32(&mut plant, OFF_Y, i / COLS);
            player.potted_plants.push(plant);
        }
        player.potted_plants.truncate(33);
        assert!(main_garden_full(&player));
        assert!(!add_potted_plant(&mut player, SeedType::Marigold));
    }

    #[test]
    fn potted_plants_fill_open_main_garden_spots() {
        let mut player = PlayerInfo::default();
        assert!(add_potted_plant(&mut player, SeedType::Marigold));
        assert!(add_potted_plant(&mut player, SeedType::Marigold));
        assert_eq!(player.potted_plants.len(), 2);
        assert_ne!(
            (get_i32(&player.potted_plants[0], OFF_X), get_i32(&player.potted_plants[0], OFF_Y)),
            (get_i32(&player.potted_plants[1], OFF_X), get_i32(&player.potted_plants[1], OFF_Y))
        );
    }
}
