//! The Suburban Almanac: an index page, a page of seed packets with the
//! chosen plant growing on its patch of ground, and a page of zombie
//! portraits with the chosen zombie shuffling about, each with its card.
//!
//! Like the original's `AlmanacDialog`, it's a dialog over the screen: a
//! `DialogKind::Almanac` in [`Dialogs`] keeps it modal and on top, and this
//! module keeps its state ([`Almanac`]) and runs it. Its plant and zombie are
//! the game's own (`board::figures`), in an effect system of their own so
//! they never mix with a board underneath.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Widget/AlmanacDialog.cpp`. Not
//! ported: the pool's animated water behind the water plants, the hand
//! cursor, and live portraits of the zombies that aren't ported yet (they
//! stand still).

use bevy::prelude::*;
use sexy::data_array::DataArray;

use crate::audio::{Music, MusicTune, Sounds};
use crate::board::figures::{Fx, Plant, PlantAction, ZOMBIE_WAVE_UI, Zombie, ZombieEnv, draw_cached_zombie};
use crate::board::{MAX_GRID_SIZE_Y, draw_seed_packet_at};
use crate::dialog::{DialogId, DialogKind, Dialogs, LawnDialog};
use crate::graphics::{Graphics, Justify, SexyColor, SexyRect};
use crate::input::GameCursor;
use crate::lawn_app::PlayerChanged;
use crate::plant::{SeedType, has_seed_type};
use crate::player::{PlayerInfo, ProfileMgr};
use crate::reanim::{EffectSystem, ReanimationType};
use crate::resources::GameResources;
use crate::widgets::{LabelButton, StoneButton};
use crate::zombie::{NUM_ZOMBIE_TYPES, ZombieType};
use crate::{AppState, UiSet};

pub struct AlmanacPlugin;

impl Plugin for AlmanacPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ZombiesDefeated>()
            .add_systems(FixedUpdate, update_almanac.run_if(resource_exists::<Almanac>))
            .add_systems(Update, (sync_almanac, almanac_input, player_changed).chain().in_set(UiSet::DialogActions));
    }
}

/// `NUM_ALMANAC_SEEDS`: the plants the almanac lists (all but the
/// bowling nuts, the sprout and the left-facing repeater).
const NUM_ALMANAC_SEEDS: usize = 49;
/// `NUM_ALMANAC_ZOMBIES`: normal zombie to Dr. Zomboss.
const NUM_ALMANAC_ZOMBIES: usize = 26;
const PLANT_POSITION: (f32, f32) = (578.0, 140.0);
const ZOMBIE_POSITION: (f32, f32) = (559.0, 175.0);
const INDEX_PLANT_POSITION: (f32, f32) = (167.0, 255.0);
const INDEX_ZOMBIE_POSITION: (f32, f32) = (535.0, 215.0);
const SEED_PACKET_SIZE: (i32, i32) = (50, 70);
/// The button labels' colour.
const BUTTON_TEXT: SexyColor = SexyColor::rgba(42, 42, 90, 255);
const CARD_TEXT: SexyColor = SexyColor::rgba(40, 50, 90, 255);

/// `gZombieDefeated`: zombie types the player has beaten since choosing
/// this player (`AlmanacInitForPlayer` clears it). Only matters on the level
/// a zombie first appears, before which the almanac keeps it a mystery.
#[derive(Resource)]
pub struct ZombiesDefeated(pub [bool; NUM_ZOMBIE_TYPES]);

impl Default for ZombiesDefeated {
    fn default() -> Self {
        Self([false; NUM_ZOMBIE_TYPES])
    }
}

/// Which page the almanac opens on (`ShowPlant`/`ShowZombie`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlmanacStart {
    Index,
    Plant(SeedType),
    Zombie(ZombieType),
}

/// `LawnApp::DoAlmanacDialog`. The almanac is set up on the next frame.
pub fn do_almanac_dialog(dialogs: &mut Dialogs, res: &GameResources, start: AlmanacStart) {
    dialogs.add_dialog(LawnDialog::almanac(res, start));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Index,
    Plants,
    Zombies,
}

/// What the player has seen, for which zombies the almanac shows.
#[derive(Debug, Clone, Copy)]
pub struct Knowledge {
    pub level: i32,
    /// `mFinishedAdventure`: how many times adventure has been finished.
    pub finished_adventure: i32,
    pub defeated: [bool; NUM_ZOMBIE_TYPES],
}

impl Knowledge {
    fn has_finished_adventure(&self) -> bool {
        self.finished_adventure > 0
    }

    /// `LawnApp::CanSpawnYetis`.
    fn can_spawn_yetis(&self) -> bool {
        self.has_finished_adventure()
            && (self.finished_adventure >= 2 || self.level >= ZombieType::Yeti.definition().starting_level)
    }

    /// `ZombieHasSilhouette`: the yeti is a shadow once its level is
    /// passed, until it can really turn up.
    pub fn zombie_has_silhouette(&self, ty: ZombieType) -> bool {
        if ty != ZombieType::Yeti || self.can_spawn_yetis() {
            return false;
        }
        self.has_finished_adventure() || self.level > ZombieType::Yeti.definition().starting_level
    }

    /// `ZombieIsShown`: reached its first level (and, for those that only
    /// come with another, beaten it or passed that level); everything once
    /// adventure is finished.
    pub fn zombie_is_shown(&self, ty: ZombieType) -> bool {
        if ty == ZombieType::Yeti {
            return self.can_spawn_yetis() || self.zombie_has_silhouette(ty);
        }
        if ty as usize > ZombieType::Boss as usize {
            return false;
        }
        if self.has_finished_adventure() {
            return true;
        }
        let start = ty.definition().starting_level;
        start <= self.level && (start != self.level || !ty.is_spawned_only() || self.defeated[ty as usize])
    }

    /// `ZombieHasDescription`: its card stays "???" until its first level
    /// is passed or one is beaten.
    pub fn zombie_has_description(&self, ty: ZombieType) -> bool {
        if ty == ZombieType::Yeti {
            if !self.can_spawn_yetis() {
                return false;
            }
            if self.finished_adventure >= 2 {
                return true;
            }
        } else if self.has_finished_adventure() {
            return true;
        }
        let start = ty.definition().starting_level;
        start <= self.level && (start != self.level || self.defeated[ty as usize])
    }
}

/// `AlmanacDialog`'s state.
#[derive(Resource)]
pub struct Almanac {
    /// The effect system its plant and zombie animate in.
    effects: EffectSystem,
    page: Page,
    selected_seed: SeedType,
    selected_zombie: ZombieType,
    plant: Option<Plant>,
    zombie: Option<Zombie>,
    close: LabelButton,
    index: LabelButton,
    plants: LabelButton,
    zombies: StoneButton<()>,
    knowledge: Knowledge,
    /// `SeedTypeAvailable` for each listed plant.
    has_seed: [bool; NUM_ALMANAC_SEEDS],
    mouse: Option<IVec2>,
}

/// The player as the almanac sees them: `--lvl` stands in for their level.
fn player_for_almanac(player: Option<&PlayerInfo>) -> PlayerInfo {
    let mut player = player.cloned().unwrap_or_default();
    if let Some(level) = crate::level_flag() {
        player.level = level;
    }
    player
}

impl Almanac {
    fn new(
        start: AlmanacStart,
        player: &PlayerInfo,
        defeated: &ZombiesDefeated,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) -> Self {
        if let Err(e) = res.load_group_now("DelayLoad_Almanac", images) {
            error!("{e}");
        }
        let has_seed = std::array::from_fn(|i| SeedType::from_index(i as i32).is_some_and(|s| has_seed_type(player, s)));
        // Seed packets show their plant, and portraits their zombie: load
        // the reanimations they're drawn from.
        let seeds = (0..NUM_ALMANAC_SEEDS).filter(|&i| has_seed[i]).filter_map(|i| SeedType::from_index(i as i32));
        let plant_reanims = seeds.map(|s| s.definition().reanim);
        let zombie_reanims = (0..NUM_ALMANAC_ZOMBIES).filter_map(|i| ZombieType::from_index(i).almanac_reanim());
        let extra = [ReanimationType::Flag, ReanimationType::BossDriver];
        for ty in plant_reanims.chain(zombie_reanims).chain(extra) {
            if let Err(e) = res.reanim_definition(ty.file(), images) {
                error!("{e}");
            }
        }

        let label_button = |label: &str, image, over, (x, y, w, h), text_offset| {
            let mut b = LabelButton::new(res.translate(label), "FONT_BRIANNETOD12", BUTTON_TEXT, image);
            b.over_image = Some(over);
            b.text_offset = text_offset;
            b.resize(x, y, w, h);
            b
        };
        let close = label_button(
            "[CLOSE_BUTTON]",
            "IMAGE_ALMANAC_CLOSEBUTTON",
            "IMAGE_ALMANAC_CLOSEBUTTONHIGHLIGHT",
            (676, 567, 89, 26),
            (-8, 1),
        );
        let index = label_button(
            "[ALMANAC_INDEX]",
            "IMAGE_ALMANAC_INDEXBUTTON",
            "IMAGE_ALMANAC_INDEXBUTTONHIGHLIGHT",
            (32, 567, 164, 26),
            (8, 1),
        );
        let mut plants = LabelButton::new(
            res.translate("[VIEW_PLANTS]"),
            "FONT_DWARVENTODCRAFT18YELLOW",
            SexyColor::WHITE,
            "IMAGE_SEEDCHOOSER_BUTTON",
        );
        plants.over_overlay = Some("IMAGE_SEEDCHOOSER_BUTTON_GLOW");
        plants.text_offset = (0, -1);
        plants.resize(130, 345, 156, 42);
        let mut zombies = StoneButton::new((), res.translate("[VIEW_ZOMBIES]"));
        zombies.resize(487, 345, 210, 48);

        let mut almanac = Self {
            effects: EffectSystem::default(),
            page: Page::Index,
            selected_seed: SeedType::Peashooter,
            selected_zombie: ZombieType::Normal,
            plant: None,
            zombie: None,
            close,
            index,
            plants,
            zombies,
            knowledge: Knowledge { level: player.level, finished_adventure: player.finished_adventure, defeated: defeated.0 },
            has_seed,
            mouse: None,
        };
        match start {
            AlmanacStart::Index => almanac.set_page(Page::Index, res, images, sounds),
            AlmanacStart::Plant(seed) => {
                almanac.selected_seed = seed;
                almanac.set_page(Page::Plants, res, images, sounds);
            }
            AlmanacStart::Zombie(ty) => {
                almanac.selected_zombie = ty;
                almanac.set_page(Page::Zombies, res, images, sounds);
            }
        }
        almanac
    }

    /// `ClearPlantsAndZombies`.
    fn clear_plants_and_zombies(&mut self) {
        if let Some(mut plant) = self.plant.take() {
            plant.die(&mut self.effects);
        }
        if let Some(mut zombie) = self.zombie.take() {
            zombie.die_no_loot(&mut self.effects);
        }
    }

    fn make_plant(&mut self, seed: SeedType, (x, y): (f32, f32), res: &mut GameResources, images: &mut Assets<Image>, sounds: &mut Sounds) {
        let plant = Plant::new(0, 0, x as i32, y as i32, seed, false, &mut self.effects, res, images, sounds.rng());
        self.plant = Some(plant);
    }

    /// Zombies not ported yet are shown as still pictures instead.
    fn make_zombie(&mut self, ty: ZombieType, (x, y): (f32, f32), res: &mut GameResources, images: &mut Assets<Image>, sounds: &mut Sounds) {
        if !ty.is_implemented() {
            return;
        }
        let mut fx = Fx { effects: &mut self.effects, res, images, sounds };
        let mut zombie = Zombie::new(0, ty, false, ZOMBIE_WAVE_UI, false, &ZombieEnv::ui(), &mut fx);
        zombie.place(x, y);
        self.zombie = Some(zombie);
    }

    /// `SetupPlant`: the chosen plant on its patch, nudged for the odd shapes.
    fn setup_plant(&mut self, res: &mut GameResources, images: &mut Assets<Image>, sounds: &mut Sounds) {
        self.clear_plants_and_zombies();
        let (mut x, mut y) = PLANT_POSITION;
        match self.selected_seed {
            SeedType::Tallnut => y += 18.0,
            SeedType::Cobcannon => x -= 40.0,
            SeedType::Flowerpot => y -= 20.0,
            SeedType::InstantCoffee => y += 20.0,
            SeedType::Gravebuster => y += 55.0,
            _ => {}
        }
        self.make_plant(self.selected_seed, (x, y), res, images, sounds);
    }

    /// `SetupZombie`.
    fn setup_zombie(&mut self, res: &mut GameResources, images: &mut Assets<Image>, sounds: &mut Sounds) {
        self.clear_plants_and_zombies();
        self.make_zombie(self.selected_zombie, ZOMBIE_POSITION, res, images, sounds);
    }

    /// `SetPage`.
    fn set_page(&mut self, page: Page, res: &mut GameResources, images: &mut Assets<Image>, sounds: &mut Sounds) {
        self.page = page;
        self.clear_plants_and_zombies();
        match page {
            Page::Index => {
                self.make_plant(SeedType::Sunflower, INDEX_PLANT_POSITION, res, images, sounds);
                self.make_zombie(ZombieType::Normal, INDEX_ZOMBIE_POSITION, res, images, sounds);
            }
            Page::Plants => self.setup_plant(res, images, sounds),
            Page::Zombies => self.setup_zombie(res, images, sounds),
        }
        let index = page == Page::Index;
        self.index.visible = !index;
        self.plants.visible = index;
        self.zombies.visible = index;
    }

    /// `GetSeedPosition`.
    fn seed_position(seed: usize) -> (i32, i32) {
        if seed == SeedType::Imitater as usize {
            (20, 23)
        } else {
            (seed as i32 % 8 * 52 + 26, seed as i32 / 8 * 78 + 92)
        }
    }

    /// `SeedHitTest`.
    fn seed_hit_test(&self, p: IVec2) -> Option<SeedType> {
        if self.page != Page::Plants {
            return None;
        }
        (0..NUM_ALMANAC_SEEDS).filter(|&i| self.has_seed[i]).find_map(|i| {
            let (x, y) = Self::seed_position(i);
            let (w, h) = if i == SeedType::Imitater as usize { (34, 46) } else { SEED_PACKET_SIZE };
            (p.x >= x && p.y >= y && p.x < x + w && p.y < y + h).then(|| SeedType::from_index(i as i32)).flatten()
        })
    }

    /// `GetZombiePosition`.
    fn zombie_position(ty: ZombieType) -> (i32, i32) {
        if ty == ZombieType::Boss {
            (192, 486)
        } else {
            let i = ty as i32;
            (i % 5 * 85 + 22, i / 5 * 80 + 86)
        }
    }

    /// `ZombieHitTest`. Hidden zombies can be picked too, and show "???".
    fn zombie_hit_test(&self, p: IVec2) -> Option<ZombieType> {
        if self.page != Page::Zombies {
            return None;
        }
        (0..NUM_ALMANAC_ZOMBIES).map(ZombieType::from_index).find(|&ty| {
            let (x, y) = Self::zombie_position(ty);
            p.x >= x && p.y >= y && p.x < x + 76 && p.y < y + 76
        })
    }

    /// `MouseDown`.
    fn mouse_down(&mut self, p: IVec2, res: &mut GameResources, images: &mut Assets<Image>, sounds: &mut Sounds) {
        if self.plants.contains(p) || self.close.contains(p) || self.index.contains(p) {
            sounds.play_sample("SOUND_TAP");
        }
        if self.zombies.contains(p) {
            sounds.play_sample("SOUND_GRAVEBUTTON");
        }
        for b in [&mut self.plants, &mut self.close, &mut self.index] {
            b.is_down = b.contains(p);
        }
        self.zombies.is_down = self.zombies.contains(p);
        if let Some(seed) = self.seed_hit_test(p)
            && seed != self.selected_seed
        {
            self.selected_seed = seed;
            self.setup_plant(res, images, sounds);
            sounds.play_sample("SOUND_TAP");
        }
        if let Some(ty) = self.zombie_hit_test(p)
            && ty != self.selected_zombie
        {
            self.selected_zombie = ty;
            self.setup_zombie(res, images, sounds);
            sounds.play_sample("SOUND_TAP");
        }
    }

    /// `MouseUp`: returns true when Close was clicked.
    fn mouse_up(&mut self, p: IVec2, res: &mut GameResources, images: &mut Assets<Image>, sounds: &mut Sounds) -> bool {
        for b in [&mut self.plants, &mut self.close, &mut self.index] {
            b.is_down = false;
        }
        self.zombies.is_down = false;
        if self.plants.contains(p) {
            self.set_page(Page::Plants, res, images, sounds);
        } else if self.zombies.contains(p) {
            self.set_page(Page::Zombies, res, images, sounds);
        } else if self.close.contains(p) {
            return true;
        } else if self.index.contains(p) {
            self.set_page(Page::Index, res, images, sounds);
        }
        false
    }

    fn hover(&mut self, mouse: Option<IVec2>) {
        self.mouse = mouse;
        let over = |b: &LabelButton| mouse.is_some_and(|p| b.contains(p));
        self.close.is_over = over(&self.close);
        self.index.is_over = over(&self.index);
        self.plants.is_over = over(&self.plants);
        self.zombies.is_over = mouse.is_some_and(|p| self.zombies.contains(p));
    }

    /// `Update`: the plant and zombie animate.
    fn update(&mut self, res: &mut GameResources, images: &mut Assets<Image>, sounds: &mut Sounds) {
        if let Some(plant) = &mut self.plant {
            // Off the board a plant finds nothing to do, and asks for nothing.
            let mut actions: Vec<PlantAction> = Vec::new();
            let zombies = DataArray::default();
            let rows = [0; MAX_GRID_SIZE_Y];
            plant.update(&zombies, &mut actions, &mut self.effects, res, images, &rows, &[false; MAX_GRID_SIZE_Y], /* no water to bob on */ 0, sounds.rng());
        }
        if let Some(zombie) = &mut self.zombie {
            zombie.update_street(&mut self.effects);
        }
    }

    /// `Draw`.
    pub fn draw(&self, g: &mut Graphics, res: &GameResources) {
        match self.page {
            Page::Index => self.draw_index(g, res),
            Page::Plants => self.draw_plants(g, res),
            Page::Zombies => self.draw_zombies(g, res),
        }
        self.close.draw(g, res);
        self.index.draw(g, res);
        self.plants.draw(g, res);
        if self.zombies.visible {
            self.zombies.draw(g, res);
        }
    }

    /// `DrawIndex`.
    fn draw_index(&self, g: &mut Graphics, res: &GameResources) {
        if let Some(back) = res.image("IMAGE_ALMANAC_INDEXBACK") {
            g.draw_image(back, 0.0, 0.0);
        }
        if let Some(font) = res.font("FONT_HOUSEOFTERROR28") {
            let title = res.translate("[SUBURBAN_ALMANAC_INDEX]");
            g.tod_draw_string(font, 400, 60, &title, SexyColor::rgba(220, 220, 220, 255), Justify::Center);
        }
        if let Some(plant) = &self.plant {
            plant.draw(g, false, &self.effects, res);
        }
        if let Some(zombie) = &self.zombie {
            zombie.draw(g, &self.effects, res, None, false, false);
        }
    }

    /// `DrawPlants`.
    fn draw_plants(&self, g: &mut Graphics, res: &GameResources) {
        if let Some(back) = res.image("IMAGE_ALMANAC_PLANTBACK") {
            g.draw_image(back, 0.0, 0.0);
        }
        if let Some(font) = res.font("FONT_HOUSEOFTERROR20") {
            let title = res.translate("[SUBURBAN_ALMANAC_PLANTS]");
            g.tod_draw_string(font, 400, 48, &title, SexyColor::rgba(213, 159, 43, 255), Justify::Center);
        }
        let mouse_on = self.mouse.and_then(|p| self.seed_hit_test(p));
        for i in (0..NUM_ALMANAC_SEEDS).filter(|&i| self.has_seed[i]) {
            let Some(seed) = SeedType::from_index(i as i32) else { continue };
            let (x, y) = Self::seed_position(i);
            if seed == SeedType::Imitater {
                if let Some(image) = res.image("IMAGE_ALMANAC_IMITATER") {
                    g.draw_image(image, x as f32, y as f32);
                }
            } else {
                draw_seed_packet_at(g, res, x as f32, y as f32, seed);
                if mouse_on == Some(seed)
                    && let Some(flash) = res.image("IMAGE_SEEDPACKETFLASH")
                {
                    g.draw_image(flash, x as f32, y as f32);
                }
            }
        }

        let seed = self.selected_seed;
        let night = seed.is_nocturnal() || matches!(seed, SeedType::Gravebuster | SeedType::Plantern);
        let ground = match seed {
            // TODO: the pool's animated water (PoolEffectDraw) over the pool ground.
            SeedType::Seashroom => "IMAGE_ALMANAC_GROUNDNIGHTPOOL",
            SeedType::Lilypad | SeedType::Tanglekelp | SeedType::Cattail => "IMAGE_ALMANAC_GROUNDPOOL",
            _ if night => "IMAGE_ALMANAC_GROUNDNIGHT",
            SeedType::Flowerpot => "IMAGE_ALMANAC_GROUNDROOF",
            _ => "IMAGE_ALMANAC_GROUNDDAY",
        };
        if let Some(ground) = res.image(ground) {
            g.draw_image(ground, 521.0, 107.0);
        }
        if let Some(plant) = &self.plant {
            plant.draw(g, false, &self.effects, res);
        }
        if let Some(card) = res.image("IMAGE_ALMANAC_PLANTCARD") {
            g.draw_image(card, 459.0, 86.0);
        }
        let def = seed.definition();
        if let Some(font) = res.font("FONT_DWARVENTODCRAFT18YELLOW") {
            g.tod_draw_string(font, 617, 288, &res.translate(&format!("[{}]", def.name)), SexyColor::WHITE, Justify::Center);
        }
        let Some(font) = res.font("FONT_BRIANNETOD12") else { return };
        let description = res.translate(&format!("[{}_DESCRIPTION]", def.name));
        g.tod_draw_string_wrapped(font, SexyRect::new(485.0, 309.0, 258.0, 230.0), &description, CARD_TEXT, Justify::Left);
        if seed != SeedType::Imitater {
            let cost = format!("{{KEYWORD}}{}:{{STAT}} {}", res.translate("[COST]"), def.cost);
            g.tod_draw_string_wrapped(font, SexyRect::new(485.0, 520.0, 134.0, 50.0), &cost, SexyColor::WHITE, Justify::Left);
            let wait = match def.refresh_time {
                750 => "[WAIT_TIME_SHORT]",
                3000 => "[WAIT_TIME_LONG]",
                _ => "[WAIT_TIME_VERY_LONG]",
            };
            let recharge = format!("{{KEYWORD}}{}:{{STAT}}{}", res.translate("[WAIT_TIME]"), res.translate(wait));
            g.tod_draw_string_wrapped(font, SexyRect::new(600.0, 520.0, 139.0, 50.0), &recharge, CARD_TEXT, Justify::Right);
        }
    }

    /// `DrawZombies`.
    fn draw_zombies(&self, g: &mut Graphics, res: &GameResources) {
        if let Some(back) = res.image("IMAGE_ALMANAC_ZOMBIEBACK") {
            g.draw_image(back, 0.0, 0.0);
        }
        if let Some(font) = res.font("FONT_DWARVENTODCRAFT24") {
            let title = res.translate("[SUBURBAN_ALMANAC_ZOMBIES]");
            g.tod_draw_string(font, 400, 54, &title, SexyColor::rgba(0, 196, 0, 255), Justify::Center);
        }
        let mouse_on = self.mouse.and_then(|p| self.zombie_hit_test(p));
        for ty in (0..NUM_ALMANAC_ZOMBIES).map(ZombieType::from_index) {
            let (x, y) = Self::zombie_position(ty);
            let (x, y) = (x as f32, y as f32);
            if !self.knowledge.zombie_is_shown(ty) {
                if let Some(blank) = res.image("IMAGE_ALMANAC_ZOMBIEBLANK") {
                    g.draw_image(blank, x, y);
                }
                continue;
            }
            draw_window(g, res, "IMAGE_ALMANAC_ZOMBIEWINDOW", x, y, mouse_on == Some(ty));
            g.push_state();
            g.clip_rect(x + 2.0, y + 2.0, 72.0, 72.0);
            let (dx, dy) = match ty {
                ZombieType::Polevaulter => (2.0, -3.0),
                ZombieType::Flag => (2.0, 10.0),
                ZombieType::TrafficCone => (0.0, 12.0),
                ZombieType::Pail => (0.0, 9.0),
                ZombieType::Football => (-15.0, -1.0),
                ZombieType::Zamboni => (0.0, 3.0),
                ZombieType::DolphinRider => (-2.0, -10.0),
                ZombieType::Pogo => (0.0, -3.0),
                ZombieType::Gargantuar => (15.0, 17.0),
                ZombieType::Imp => (-8.0, -7.0),
                ZombieType::Bungee => (-4.0, 3.0),
                ZombieType::BackupDancer => (-8.0, 5.0),
                ZombieType::Snorkel => (-10.0, 0.0),
                ZombieType::Yeti => (0.0, 4.0),
                ZombieType::Catapult => (-24.0, -1.0),
                ZombieType::Bobsled => (0.0, -8.0),
                ZombieType::Ladder => (0.0, -3.0),
                _ => (0.0, 0.0),
            };
            g.translate(x + 1.0 + dx, y - 6.0 + dy);
            g.set_scale(0.5, 0.0, 0.0);
            if self.knowledge.zombie_has_silhouette(ty) {
                g.set_color(SexyColor::rgba(0, 0, 0, 40));
                g.set_colorize_images(true);
            }
            draw_cached_zombie(g, res, 0.0, 0.0, ty, ty == ZombieType::Polevaulter);
            g.pop_state();
            draw_window(g, res, "IMAGE_ALMANAC_ZOMBIEWINDOW2", x, y, mouse_on == Some(ty));
        }

        let ty = self.selected_zombie;
        let ice = matches!(ty, ZombieType::Zamboni | ZombieType::Bobsled);
        if let Some(ground) = res.image(if ice { "IMAGE_ALMANAC_GROUNDICE" } else { "IMAGE_ALMANAC_GROUNDDAY" }) {
            g.draw_image(ground, 518.0, 110.0);
        }
        if !self.knowledge.zombie_has_silhouette(ty) {
            let (zx, zy) = ZOMBIE_POSITION;
            g.push_state();
            g.clip_rect(zx - 42.0, zy - 51.0, 197.0, 187.0);
            let (dx, dy) = match ty {
                ZombieType::Zamboni => (-30.0, 5.0),
                ZombieType::Gargantuar => (0.0, 40.0),
                ZombieType::Football => (-10.0, 0.0),
                ZombieType::Balloon => (0.0, -20.0),
                ZombieType::Bungee => (15.0, 0.0),
                ZombieType::Catapult => (-10.0, 0.0),
                _ => (0.0, 0.0),
            };
            g.translate(dx, dy);
            match &self.zombie {
                Some(zombie) => {
                    if !matches!(ty, ZombieType::Bungee | ZombieType::Boss | ZombieType::Zamboni | ZombieType::Catapult) {
                        zombie.draw_shadow(g, false, res);
                    }
                    zombie.draw(g, &self.effects, res, None, false, false);
                }
                // Not ported yet: a still picture where the zombie would stand.
                None => draw_cached_zombie(g, res, zx - 40.0, zy - 40.0, ty, false),
            }
            g.pop_state();
        }
        if let Some(card) = res.image("IMAGE_ALMANAC_ZOMBIECARD") {
            g.draw_image(card, 455.0, 78.0);
        }
        let name = if self.knowledge.zombie_has_silhouette(ty) { "???".to_owned() } else { res.translate(&format!("[{}]", ty.name())) };
        if let Some(font) = res.font("FONT_DWARVENTODCRAFT18GREENINSET") {
            g.tod_draw_string(font, 613, 362, &name, SexyColor::rgba(190, 255, 235, 255), Justify::Center);
        }
        let (description, justify) = if self.knowledge.zombie_has_description(ty) {
            (res.translate(&format!("[{}_DESCRIPTION]", ty.name())), Justify::Left)
        } else {
            (res.translate("[NOT_ENCOUNTERED_YET]"), Justify::CenterMiddle)
        };
        if let Some(font) = res.font("FONT_BRIANNETOD12") {
            let show_metal = self.has_seed[SeedType::Magnetshroom as usize];
            let rect = SexyRect::new(484.0, 377.0, 258.0, 170.0);
            g.tod_draw_string_wrapped_ex(font, rect, &description, CARD_TEXT, justify, show_metal);
        }
    }
}

/// A portrait's window, brightened (additively) under the mouse.
fn draw_window(g: &mut Graphics, res: &GameResources, id: &str, x: f32, y: f32, highlighted: bool) {
    let Some(image) = res.image(id) else { return };
    g.draw_image(image, x, y);
    if highlighted {
        g.push_state();
        g.set_additive(true);
        g.set_colorize_images(true);
        g.set_color(SexyColor::rgba(255, 255, 255, 48));
        let (w, h) = (image.width as f32, image.height as f32);
        g.draw_image_matrix(image, bevy::math::Affine2::from_translation(Vec2::new(x + w / 2.0, y + h / 2.0)));
        g.pop_state();
    }
}

/// Builds the almanac when its dialog opens and lets it go when it closes:
/// back on the main menu the title theme returns, and from the award
/// screen the next level starts (`AwardScreen::StartButtonPressed`).
#[allow(clippy::too_many_arguments)]
fn sync_almanac(
    mut commands: Commands,
    almanac: Option<ResMut<Almanac>>,
    dialogs: Res<Dialogs>,
    profiles: Option<Res<ProfileMgr>>,
    defeated: Res<ZombiesDefeated>,
    res: Option<ResMut<GameResources>>,
    mut images: ResMut<Assets<Image>>,
    mut music: Option<ResMut<Music>>,
    mut sounds: Sounds,
    state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let start = dialogs.get(DialogId::Almanac).and_then(|d| match d.kind {
        DialogKind::Almanac(start) => Some(start),
        _ => None,
    });
    match (start, almanac) {
        (Some(start), None) => {
            let Some(mut res) = res else { return };
            let player = player_for_almanac(profiles.as_deref().and_then(ProfileMgr::player));
            commands.insert_resource(Almanac::new(start, &player, &defeated, &mut res, &mut images, &mut sounds));
            // A paused level keeps its music.
            if *state.get() != AppState::Board
                && let Some(music) = music.as_deref_mut()
            {
                music.make_sure_music_is_playing(MusicTune::ChooseYourSeeds);
            }
        }
        (None, Some(mut almanac)) => {
            almanac.clear_plants_and_zombies();
            commands.remove_resource::<Almanac>();
            match state.get() {
                AppState::GameSelector => {
                    if let Some(music) = music.as_deref_mut() {
                        music.make_sure_music_is_playing(MusicTune::TitleCrazyDaveMainTheme);
                    }
                }
                AppState::Award => next_state.set(AppState::Board),
                _ => {}
            }
        }
        _ => {}
    }
}

/// The almanac's mouse, while it's the topmost dialog.
fn almanac_input(
    almanac: Option<ResMut<Almanac>>,
    mut dialogs: ResMut<Dialogs>,
    cursor: Res<GameCursor>,
    mouse: Res<ButtonInput<MouseButton>>,
    res: Option<ResMut<GameResources>>,
    mut images: ResMut<Assets<Image>>,
    mut sounds: Sounds,
) {
    let (Some(mut almanac), Some(mut res)) = (almanac, res) else { return };
    let on_top = dialogs.top().is_some_and(|d| d.id == DialogId::Almanac);
    let p = cursor.0.filter(|_| on_top).map(|p| p.floor().as_ivec2());
    almanac.hover(p);
    let Some(p) = p else { return };
    if mouse.just_pressed(MouseButton::Left) {
        almanac.mouse_down(p, &mut res, &mut images, &mut sounds);
    }
    if mouse.just_released(MouseButton::Left) && almanac.mouse_up(p, &mut res, &mut images, &mut sounds) {
        // LawnApp::KillAlmanacDialog.
        dialogs.kill_dialog(DialogId::Almanac);
    }
}

fn update_almanac(
    mut almanac: ResMut<Almanac>,
    res: Option<ResMut<GameResources>>,
    mut images: ResMut<Assets<Image>>,
    mut sounds: Sounds,
) {
    let Some(mut res) = res else { return };
    almanac.update(&mut res, &mut images, &mut sounds);
}

/// `GameSelector::SyncProfile` calls `AlmanacInitForPlayer`.
fn player_changed(mut changed: MessageReader<PlayerChanged>, mut defeated: ResMut<ZombiesDefeated>) {
    if changed.read().count() > 0 {
        defeated.0 = [false; NUM_ZOMBIE_TYPES];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn knowledge(level: i32, finished_adventure: i32) -> Knowledge {
        Knowledge { level, finished_adventure, defeated: [false; NUM_ZOMBIE_TYPES] }
    }

    #[test]
    fn zombies_appear_as_the_player_reaches_them() {
        let k = knowledge(1, 0);
        assert!(k.zombie_is_shown(ZombieType::Normal));
        assert!(!k.zombie_is_shown(ZombieType::TrafficCone));
        // On its first level a zombie's card waits until one is beaten.
        assert!(!k.zombie_has_description(ZombieType::Normal));
        let mut k = knowledge(3, 0);
        assert!(k.zombie_is_shown(ZombieType::TrafficCone) && k.zombie_has_description(ZombieType::Normal));
        k.defeated[ZombieType::TrafficCone as usize] = true;
        assert!(k.zombie_has_description(ZombieType::TrafficCone));
    }

    #[test]
    fn zombies_that_only_come_with_others_wait_until_beaten() {
        let level = ZombieType::BackupDancer.definition().starting_level;
        let mut k = knowledge(level, 0);
        assert!(!k.zombie_is_shown(ZombieType::BackupDancer));
        k.defeated[ZombieType::BackupDancer as usize] = true;
        assert!(k.zombie_is_shown(ZombieType::BackupDancer));
        assert!(knowledge(level + 1, 0).zombie_is_shown(ZombieType::BackupDancer));
    }

    #[test]
    fn the_yeti_is_a_shadow_until_it_can_turn_up() {
        let yeti = ZombieType::Yeti;
        let start = yeti.definition().starting_level;
        assert!(!knowledge(start, 0).zombie_is_shown(yeti));
        // Finished once, but not back to its level yet: a silhouette, no card.
        let k = knowledge(1, 1);
        assert!(k.zombie_is_shown(yeti) && k.zombie_has_silhouette(yeti) && !k.zombie_has_description(yeti));
        let k = knowledge(start, 1);
        assert!(k.zombie_is_shown(yeti) && !k.zombie_has_silhouette(yeti));
        assert!(knowledge(1, 2).zombie_has_description(yeti));
        // Everyone else is known once adventure is finished.
        assert!(knowledge(1, 1).zombie_has_description(ZombieType::Gargantuar));
        assert!(knowledge(1, 1).zombie_is_shown(ZombieType::Boss));
    }

    #[test]
    fn portraits_and_packets_sit_in_a_grid() {
        assert_eq!(Almanac::seed_position(0), (26, 92));
        assert_eq!(Almanac::seed_position(9), (78, 170));
        assert_eq!(Almanac::seed_position(SeedType::Imitater as usize), (20, 23));
        assert_eq!(Almanac::zombie_position(ZombieType::Pail), (362, 86));
        assert_eq!(Almanac::zombie_position(ZombieType::Boss), (192, 486));
    }
}
