//! The Zen Garden's main greenhouse: potted plants, watering, growth, coins
//! and selling.
//!
//! Behaviour per ruslan831 decomp, `Lawn/ZenGarden.cpp`
//! (`ZenGardenInitLevel`, `PlacePottedPlant`, `PlantWatered`, `PlantFulfillNeed`,
//! `PottedPlantUpdate`, `GetPlantsNeed`, `GetPlantSellPrice`, `LeaveGarden`)
//! and `Lawn/System/PlayerInfo.h` (`PottedPlant`'s 0x58-byte layout).
//!
//! Scope of this first pass (phase 9a): the main greenhouse, watering as the
//! only need, sprout→small→medium→full growth, sun production, selling and
//! saving back into the profile. Not yet: the mushroom and aquarium gardens,
//! fertilizer/bug-spray/phonograph needs, chocolate, Stinky, the wheelbarrow,
//! the Zen shop/tutorial, and the main menu's "needs water" bubble.

use bevy::prelude::*;
use sexy::reanim::LoopType;

use crate::audio::Sounds;
use crate::graphics::{Graphics, Justify, SexyColor};
use crate::input::GameCursor;
use crate::player::ProfileMgr;
use crate::reanim::EffectSystem;
use crate::resources::GameResources;
use crate::widgets::draw_stone_button;
use crate::{AppState, UiSet};

/// `ZEN_MAX_GRIDSIZE_X`/`_Y`.
const COLS: i32 = 8;
/// `gGreenhouseGridPlacement`: each grid cell's top-left.
const GREENHOUSE_GRID: [(i32, i32); 32] = [
    (73, 73), (155, 71), (239, 68), (321, 73), (406, 71), (484, 67), (566, 70), (648, 72),
    (67, 168), (150, 165), (232, 170), (314, 175), (416, 173), (497, 170), (578, 164), (660, 168),
    (41, 268), (130, 266), (219, 260), (310, 266), (416, 267), (504, 261), (594, 265), (684, 269),
    (37, 371), (124, 369), (211, 368), (302, 369), (425, 375), (512, 368), (602, 365), (691, 368),
];
/// `PottedPlant` offsets in its 0x58-byte save record.
const OFF_SEED: usize = 0;
const OFF_GARDEN: usize = 4;
const OFF_X: usize = 8;
const OFF_Y: usize = 12;
const OFF_AGE: usize = 0x24;
const OFF_TIMES_FED: usize = 0x28;
const OFF_FEEDINGS: usize = 0x2C;
const OFF_NEED: usize = 0x30;
const OFF_LAST_WATERED: usize = 0x18;
const OFF_LAST_FULFILLED: usize = 0x38;
const FULL_AGE: i32 = 3;
/// Ticks between a plant's watering needs (roughly the original's hours,
/// scaled for playability).
const NEED_TICKS: i32 = 12_000;
/// Ticks between a grown plant's coins.
const COIN_TICKS: i32 = 9_000;

pub struct ZenGardenPlugin;

impl Plugin for ZenGardenPlugin {
    fn build(&self, app: &mut App) {
        let open = resource_exists::<ZenGarden>;
        app.add_systems(OnEnter(AppState::ZenGarden), start_garden)
            .add_systems(OnExit(AppState::ZenGarden), leave_garden)
            .add_systems(FixedUpdate, update_garden.run_if(open))
            .add_systems(Update, garden_input.in_set(UiSet::ScreenInput).run_if(open))
            .add_systems(Update, draw_garden.in_set(UiSet::ScreenDraw).run_if(open));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    None,
    Water,
    Sell,
}

struct GardenPlant {
    data: [u8; 88],
    body: Option<crate::reanim::ReanimationId>,
    pot: Option<crate::reanim::ReanimationId>,
    need_counter: i32,
    coin_counter: i32,
}

struct GardenCoin {
    x: f32,
    y: f32,
    value: i32,
    age: i32,
}

#[derive(Resource)]
pub struct ZenGarden {
    plants: Vec<GardenPlant>,
    coins: Vec<GardenCoin>,
    tool: Tool,
    mouse: Option<Vec2>,
}

fn pos(col: i32, row: i32) -> (i32, i32) {
    let index = (row * COLS + col).clamp(0, 31) as usize;
    GREENHOUSE_GRID[index]
}

fn get_i32(data: &[u8; 88], off: usize) -> i32 {
    i32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

fn set_i32(data: &mut [u8; 88], off: usize, value: i32) {
    data[off..off + 4].copy_from_slice(&value.to_le_bytes());
}

fn seed_of(data: &[u8; 88]) -> Option<crate::plant::SeedType> {
    crate::plant::SeedType::from_index(get_i32(data, OFF_SEED))
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

fn make_plant(seed: crate::plant::SeedType, col: i32, row: i32) -> [u8; 88] {
    let mut data = [0u8; 88];
    set_i32(&mut data, OFF_SEED, seed as i32);
    set_i32(&mut data, OFF_GARDEN, 0);
    set_i32(&mut data, OFF_X, col);
    set_i32(&mut data, OFF_Y, row);
    set_i32(&mut data, OFF_AGE, 0);
    set_i32(&mut data, OFF_FEEDINGS, 3);
    set_i32(&mut data, OFF_LAST_WATERED, now() as i32);
    set_i32(&mut data, OFF_LAST_FULFILLED, now() as i32);
    data
}

fn spawn_body(
    plant: &mut GardenPlant,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
) {
    if let Some(id) = plant.body.take() {
        effects.kill(id);
    }
    let Some(seed) = seed_of(&plant.data) else { return };
    let (col, row) = (get_i32(&plant.data, OFF_X), get_i32(&plant.data, OFF_Y));
    let (px, py) = pos(col, row);
    let age = get_i32(&plant.data, OFF_AGE);
    let draw_seed = if age == 0 { crate::plant::SeedType::Sprout } else { seed };
    let scale = age_scale(age);
    if let Ok((id, reanim)) = effects.add_reanimation(px as f32, py as f32 + 28.0 * scale, draw_seed.definition().reanim, res, images) {
        reanim.override_scale(scale, scale);
        reanim.loop_type = LoopType::Loop;
        reanim.play("anim_idle", LoopType::Loop, 0, 6.0);
        plant.body = Some(id);
    }
}

fn spawn_reanims(
    plant: &mut GardenPlant,
    effects: &mut EffectSystem,
    res: &mut GameResources,
    images: &mut Assets<Image>,
) {
    spawn_body(plant, effects, res, images);
    let (col, row) = (get_i32(&plant.data, OFF_X), get_i32(&plant.data, OFF_Y));
    let (px, py) = pos(col, row);
    if let Ok((id, reanim)) = effects.add_reanimation(px as f32, py as f32, crate::plant::SeedType::Flowerpot.definition().reanim, res, images) {
        reanim.override_scale(0.9, 0.9);
        plant.pot = Some(id);
    }
}

fn start_garden(
    mut commands: Commands,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut effects: ResMut<EffectSystem>,
    mut music: ResMut<crate::audio::Music>,
    mut profiles: ResMut<ProfileMgr>,
) {
    for id in [
        "IMAGE_BACKGROUND_GREENHOUSE",
        "IMAGE_BACKGROUND_GREENHOUSE_OVERLAY",
        "IMAGE_COINBANK",
        "IMAGE_REANIM_COIN_GOLD_DOLLAR",
        "IMAGE_WATERDROP",
        "IMAGE_WATERINGCAN",
    ] {
        if let Err(e) = resources.load_image(id, &mut images) {
            error!("{e}");
        }
    }
    music.make_sure_music_is_playing(crate::audio::MusicTune::ZenGarden);
    let Some(player) = profiles.player_mut() else { return };
    // The tutorial's two marigolds when the garden is empty.
    if !player.potted_plants.iter().any(|p| get_i32(p, OFF_GARDEN) == 0) {
        for i in 0..2 {
            let plant = make_plant(crate::plant::SeedType::Marigold, i * 3, 1);
            player.potted_plants.push(plant);
        }
        profiles.save_current_details();
    }
    let Some(player) = profiles.player() else { return };
    let mut garden = ZenGarden { plants: Vec::new(), coins: Vec::new(), tool: Tool::None, mouse: None };
    for data in player.potted_plants.iter().filter(|p| get_i32(p, OFF_GARDEN) == 0) {
        let mut plant = GardenPlant { data: *data, body: None, pot: None, need_counter: NEED_TICKS / 2, coin_counter: COIN_TICKS };
        spawn_reanims(&mut plant, &mut effects, &mut resources, &mut images);
        garden.plants.push(plant);
    }
    commands.insert_resource(garden);
}

fn leave_garden(mut commands: Commands, garden: Option<Res<ZenGarden>>, mut profiles: ResMut<ProfileMgr>, mut effects: ResMut<EffectSystem>) {
    let Some(garden) = garden else { return };
    if let Some(player) = profiles.player_mut() {
        for plant in &garden.plants {
            let (x, y) = (get_i32(&plant.data, OFF_X), get_i32(&plant.data, OFF_Y));
            if let Some(slot) = player
                .potted_plants
                .iter_mut()
                .find(|p| get_i32(p, OFF_GARDEN) == 0 && get_i32(p, OFF_X) == x && get_i32(p, OFF_Y) == y)
            {
                *slot = plant.data;
            }
        }
        profiles.save_current_details();
    }
    effects.clear();
    commands.remove_resource::<ZenGarden>();
}

fn plant_at(garden: &ZenGarden, point: Vec2) -> Option<usize> {
    for (i, plant) in garden.plants.iter().enumerate() {
        let (col, row) = (get_i32(&plant.data, OFF_X), get_i32(&plant.data, OFF_Y));
        let (x, y) = pos(col, row);
        if point.x >= x as f32 && point.x < x as f32 + 80.0 && point.y >= y as f32 && point.y < y as f32 + 85.0 {
            return Some(i);
        }
    }
    None
}

fn update_garden(mut garden: ResMut<ZenGarden>) {
    let mut spawned: Vec<GardenCoin> = Vec::new();
    for plant in &mut garden.plants {
        let age = get_i32(&plant.data, OFF_AGE);
        if get_i32(&plant.data, OFF_NEED) == 0 {
            plant.need_counter -= 1;
            if plant.need_counter <= 0 {
                set_i32(&mut plant.data, OFF_NEED, 1);
            }
        }
        if age == FULL_AGE && get_i32(&plant.data, OFF_NEED) == 0 {
            plant.coin_counter -= 1;
            if plant.coin_counter <= 0 {
                plant.coin_counter = COIN_TICKS;
                let (col, row) = (get_i32(&plant.data, OFF_X), get_i32(&plant.data, OFF_Y));
                let (x, y) = pos(col, row);
                spawned.push(GardenCoin { x: x as f32 + 30.0, y: y as f32 - 10.0, value: 10, age: 0 });
            }
        }
    }
    garden.coins.extend(spawned);
    for coin in &mut garden.coins {
        coin.age += 1;
    }
}

fn water_plant(plant: &mut GardenPlant) -> bool {
    set_i32(&mut plant.data, OFF_NEED, 0);
    set_i32(&mut plant.data, OFF_LAST_WATERED, now() as i32);
    set_i32(&mut plant.data, OFF_LAST_FULFILLED, now() as i32);
    plant.need_counter = NEED_TICKS;
    let fed = get_i32(&plant.data, OFF_TIMES_FED) + 1;
    let per_grow = get_i32(&plant.data, OFF_FEEDINGS).max(1);
    if fed >= per_grow {
        let age = (get_i32(&plant.data, OFF_AGE) + 1).min(FULL_AGE);
        set_i32(&mut plant.data, OFF_AGE, age);
        set_i32(&mut plant.data, OFF_TIMES_FED, 0);
        set_i32(&mut plant.data, OFF_FEEDINGS, 3);
        true
    } else {
        set_i32(&mut plant.data, OFF_TIMES_FED, fed);
        false
    }
}

fn sell_price(plant: &GardenPlant) -> i32 {
    let seed = seed_of(&plant.data).map_or(0, |s| s.cost());
    let age = get_i32(&plant.data, OFF_AGE);
    (seed / 2).max(10) + age * 25
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
) {
    let garden = &mut *garden;
    garden.mouse = cursor.0;
    let Some(point) = cursor.0 else { return };
    let in_rect = |x: f32, y: f32, w: f32, h: f32| point.x >= x && point.y >= y && point.x < x + w && point.y < y + h;
    let buttons: [(Tool, &str, f32); 2] = [(Tool::Water, "[ADVICE_ZEN_GARDEN_PICK_UP_WATER]", 300.0), (Tool::Sell, "[ZEN_SELL_HEADER]", 430.0)];
    if mouse.just_released(MouseButton::Left) {
        if in_rect(20.0, 550.0, 150.0, 40.0) {
            next_state.set(AppState::GameSelector);
            return;
        }
        for (tool, _, x) in buttons {
            if in_rect(x, 550.0, 120.0, 40.0) {
                garden.tool = if garden.tool == tool { Tool::None } else { tool };
                sounds.play_sample("SOUND_TAP");
                return;
            }
        }
        // A coin above a plant.
        if let Some(i) = garden.coins.iter().position(|c| in_rect(c.x, c.y, 40.0, 40.0)) {
            let coin = garden.coins.remove(i);
            if let Some(player) = profiles.player_mut() {
                player.coins += coin.value;
            }
            profiles.save_current_details();
            sounds.play_foley(crate::audio::FoleyType::Coin);
            return;
        }
        let Some(index) = plant_at(garden, point) else { return };
        match garden.tool {
            Tool::Water => {
                let grew = water_plant(&mut garden.plants[index]);
                if grew {
                    spawn_body(&mut garden.plants[index], &mut effects, &mut resources, &mut images);
                }
                sounds.play_foley(crate::audio::FoleyType::PlantWater);
            }
            Tool::Sell => {
                let price = sell_price(&garden.plants[index]);
                if let Some(player) = profiles.player_mut() {
                    player.coins += price;
                    // Remove this plant from the profile's matching slot.
                    let data = garden.plants[index].data;
                    if let Some(slot) = player
                        .potted_plants
                        .iter()
                        .position(|p| get_i32(p, OFF_GARDEN) == 0 && get_i32(p, OFF_X) == get_i32(&data, OFF_X) && get_i32(p, OFF_Y) == get_i32(&data, OFF_Y))
                    {
                        player.potted_plants.remove(slot);
                    }
                    profiles.save_current_details();
                }
                if let Some(id) = garden.plants[index].body {
                    effects.kill(id);
                }
                if let Some(id) = garden.plants[index].pot {
                    effects.kill(id);
                }
                garden.plants.remove(index);
                sounds.play_foley(crate::audio::FoleyType::Coin);
            }
            Tool::None => {}
        }
    }
    let _ = &mut commands;
}

fn draw_garden(
    garden: Res<ZenGarden>,
    resources: Res<GameResources>,
    effects: Res<EffectSystem>,
    profiles: Res<ProfileMgr>,
    mut g: ResMut<Graphics>,
) {
    let res = &*resources;
    let g = &mut *g;
    let image = |g: &mut Graphics, id: &str| {
        if let Some(image) = res.image(id) {
            g.draw_image(image, 0.0, 0.0);
        }
    };
    if let Some(bank) = res.image("IMAGE_COINBANK") {
        g.draw_image(bank, 0.0, 0.0);
    }
    image(g, "IMAGE_BACKGROUND_GREENHOUSE");
    for plant in &garden.plants {
        for id in [plant.pot, plant.body].into_iter().flatten() {
            effects.draw(g, id, res);
        }
        if get_i32(&plant.data, OFF_NEED) != 0
            && let Some(drop) = res.image("IMAGE_WATERDROP")
        {
            let (col, row) = (get_i32(&plant.data, OFF_X), get_i32(&plant.data, OFF_Y));
            let (x, y) = pos(col, row);
            g.draw_image(drop, x as f32 + 45.0, y as f32 - 10.0);
        }
    }
    image(g, "IMAGE_BACKGROUND_GREENHOUSE_OVERLAY");
    if let Some(coin) = res.image("IMAGE_REANIM_COIN_GOLD_DOLLAR") {
        for c in &garden.coins {
            g.draw_image_cel_center_scaled(coin, c.x + 20.0, c.y + 20.0, 0, 1.0, 1.0);
        }
    }
    if let Some(font) = res.font("FONT_HOUSEOFTERROR16") {
        let coins = profiles.player().map_or(0, |p| p.coins);
        g.tod_draw_string(font, 95, 24, &coins.to_string(), SexyColor::rgba(255, 240, 0, 255), Justify::Center);
    }
    draw_stone_button(g, res, 20, 550, 150, 40, false, false, "");
    draw_stone_button(g, res, 300, 550, 120, 40, false, false, "");
    draw_stone_button(g, res, 430, 550, 120, 40, false, false, "");
    if let Some(font) = res.font("FONT_BRIANNETOD12") {
        let water = "Water".to_owned();
        let sell = "Sell".to_owned();
        let back = res.translate("[BACK_TO_MENU]");
        for (label, x, active) in [
            (water.as_str(), 300.0, garden.tool == Tool::Water),
            (sell.as_str(), 430.0, garden.tool == Tool::Sell),
            (back.as_str(), 20.0, false),
        ] {
            let color = if active { SexyColor::rgba(255, 220, 0, 255) } else { SexyColor::rgba(42, 42, 90, 255) };
            g.tod_draw_string(font, x as i32 + 60, 574, label, color, Justify::Center);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn potted_plant_records_round_trip() {
        let mut data = make_plant(crate::plant::SeedType::Marigold, 3, 2);
        set_i32(&mut data, OFF_TIMES_FED, 2);
        let copy = data;
        assert_eq!(get_i32(&copy, OFF_X), 3);
        assert_eq!(get_i32(&copy, OFF_Y), 2);
        assert_eq!(get_i32(&copy, OFF_TIMES_FED), 2);
        assert_eq!(seed_of(&copy), Some(crate::plant::SeedType::Marigold));
    }

    #[test]
    fn three_waterings_grow_a_sprout() {
        let mut plant = GardenPlant { data: make_plant(crate::plant::SeedType::Marigold, 0, 0), body: None, pot: None, need_counter: 0, coin_counter: 0 };
        for _ in 0..3 {
            water_plant(&mut plant);
        }
        assert_eq!(get_i32(&plant.data, OFF_AGE), 1);
    }
}
