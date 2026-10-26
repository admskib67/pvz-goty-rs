//! Unit tests for the board (`board::tests`, kept out of `src/`): plants and
//! zombies run through the real game code, with the real animations and
//! timings from the player's own `main.pak`.
//!
//! The pak is found like the app finds it (`PVZ_GAME_PATH`, else the
//! saved install). Without it every test here prints that it was skipped
//! and passes, so `cargo test` still works on a machine without the game.
//!
//! A [`TestBoard`] is a level with its intro skipped and zombie waves held
//! back, so a test decides which zombies come and where. Sounds are silent
//! (none are loaded) and the random generator has a fixed seed, so runs
//! repeat exactly.

mod almanac;
mod balloon;
mod boss;
mod butter;
mod fog;
mod levels;
mod options;
mod plants;
mod roof;
mod roof_zombies;
mod world3;
mod zombies;

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use sexy::data_array::Id;
use sexy::files::GameFiles;
use sexy::rand::MtRand;

use crate::audio::{Music, SoundSystem, Sounds, UpdateCount};
use crate::install;
use crate::plant::SeedType;
use crate::player::{GameRng, PlayerInfo};
use crate::reanim::EffectSystem;
use crate::resources::{GameResources, INIT_GROUP};
use crate::zombie::ZombieType;

use super::plant::Plant;
use super::zombie::Zombie;
use super::{Board, GameMode};

#[test]
fn render_order_sorts_rows_within_layers() {
    use super::{make_render_order, render_layer};
    assert!(make_render_order(render_layer::LAWN_MOWER, 0, 0) < make_render_order(render_layer::PLANT, 1, 0));
    assert_eq!(make_render_order(render_layer::TOP, 0, 1), 400001);
}

/// The game's resources, loaded once and shared by every test (decoding
/// every image again for each test would take far too long). Tests take
/// turns with them.
struct Shared {
    resources: GameResources,
    images: Assets<Image>,
}

fn shared() -> Option<MutexGuard<'static, Shared>> {
    static SHARED: OnceLock<Option<Mutex<Shared>>> = OnceLock::new();
    let shared = SHARED.get_or_init(|| {
        let files = game_files()?;
        let mut resources = GameResources::new(files).map_err(|e| eprintln!("can't read the game's resources: {e}")).ok()?;
        let mut images = Assets::<Image>::default();
        resources.load_strings_now().map_err(|e| eprintln!("can't load the strings: {e}")).ok()?;
        for group in [INIT_GROUP, "LoadingImages"] {
            if let Err(e) = resources.load_group_now(group, &mut images) {
                eprintln!("can't load {group}: {e}");
                return None;
            }
        }
        Some(Mutex::new(Shared { resources, images }))
    });
    // A test that failed while holding the lock leaves nothing broken behind.
    shared.as_ref().map(|m| m.lock().unwrap_or_else(|e| e.into_inner()))
}

/// The game's files, found like `boot` finds them.
fn game_files() -> Option<GameFiles> {
    let chosen = std::env::var_os(install::ENV_GAME_PATH).map(Into::into).or(install::Config::load().game_path)?;
    let (path, pak) = install::open_install(&chosen).map_err(|e| eprintln!("can't open the game: {e}")).ok()?;
    let root = path.parent().map(Into::into).unwrap_or_default();
    Some(GameFiles { pak: Arc::new(pak), root })
}

/// Makes a [`TestBoard`], or skips the test when the game isn't installed.
/// The optional second argument adjusts the player first, as
/// [`TestBoard::with_player`] does.
macro_rules! test_board {
    ($stage:expr) => {
        match $crate::board::tests::TestBoard::new($stage) {
            Some(board) => board,
            None => {
                eprintln!("skipped: no main.pak (set PVZ_GAME_PATH or pick the game in the app)");
                return;
            }
        }
    };
    ($stage:expr, $adjust:expr) => {
        match $crate::board::tests::TestBoard::with_player($stage, $adjust) {
            Some(board) => board,
            None => {
                eprintln!("skipped: no main.pak (set PVZ_GAME_PATH or pick the game in the app)");
                return;
            }
        }
    };
}
pub(crate) use test_board;

/// A level ready to play, and what it takes to run it.
pub struct TestBoard {
    pub board: Board,
    pub effects: EffectSystem,
    music: Music,
    world: World,
    sounds: SystemState<Sounds<'static, 'static>>,
    shared: MutexGuard<'static, Shared>,
    /// Whether the level's own zombie waves come.
    waves: bool,
    /// The countdown to the first wave, kept while waves are held back.
    first_wave_count_down: i32,
}

impl TestBoard {
    /// Adventure level `stage` ("2-1" and so on), played by a player who has
    /// reached it for the first time. The intro is skipped: the lawn mowers
    /// are out, the graves are up and play has started.
    pub fn new(stage: &str) -> Option<Self> {
        Self::with_player(stage, |_| {})
    }

    /// Like [`Self::new`], with the player adjusted first (say, what they've
    /// bought from the store).
    pub fn with_player(stage: &str, adjust: impl FnOnce(&mut PlayerInfo)) -> Option<Self> {
        Self::build(stage, adjust, false)
    }

    /// Adventure level `stage` as the intro starts, to be run through.
    pub fn at_intro(stage: &str) -> Option<Self> {
        Self::build(stage, |_| {}, true)
    }

    fn build(stage: &str, adjust: impl FnOnce(&mut PlayerInfo), intro: bool) -> Option<Self> {
        let (area, sub) = stage.split_once('-').expect("stage like \"2-1\"");
        let level = (area.parse::<i32>().unwrap() - 1) * 10 + sub.parse::<i32>().unwrap();
        let mut shared = shared()?;
        let mut world = World::new();
        world.insert_resource(SoundSystem::default());
        world.insert_resource(GameRng(MtRand::new(2009)));
        world.insert_resource(UpdateCount::default());
        let sounds = SystemState::<Sounds>::new(&mut world);

        let mut player = PlayerInfo { level, ..Default::default() };
        adjust(&mut player);
        let Shared { resources, images } = &mut *shared;
        let mut rng = MtRand::new(1234);
        let mut board = Board::new(GameMode::Adventure, Some(&player), resources, images, &mut rng);
        if intro {
            board.draw_count = 1;
            let mut music = Music::silent();
            super::cutscene::start_level_intro(&mut board, resources, &mut music, "Tester");
            let first_wave_count_down = board.waves.zombie_count_down;
            return Some(Self {
                board,
                effects: EffectSystem::default(),
                music,
                world,
                sounds,
                shared,
                waves: false,
                first_wave_count_down,
            });
        }
        board.init_lawn_mowers();
        // Where the intro leaves the mowers.
        for mower in &mut board.lawn_mowers {
            mower.visible = true;
            mower.pos_x = -21.0;
        }
        board.enable_grave_stones = true;
        board.seed_bank.y = 0;
        board.show_shovel = !board.is_whack_a_zombie_level();
        board.draw_count = 1;
        let mut music = Music::silent();
        // LawnApp::StartPlaying (whack-a-zombie's mallet and timer too).
        board.start_playing(&mut music);
        let mut effects = EffectSystem::default();
        if board.is_whack_a_zombie_level() {
            board.add_hammer(&mut effects, resources, images);
        }
        let first_wave_count_down = board.waves.zombie_count_down;
        let mut test = Self {
            board,
            effects,
            music,
            world,
            sounds,
            shared,
            waves: false,
            first_wave_count_down,
        };
        // Graves come up fully (from as far as 50 ticks underground) and
        // the mowers finish rolling in before the test starts.
        test.ticks(160);
        Some(test)
    }

    /// Lets the level's zombie waves come as they would in the game.
    pub fn with_waves(mut self) -> Self {
        self.waves = true;
        self.board.waves.zombie_count_down = self.first_wave_count_down;
        self.board.waves.zombie_count_down_start = self.first_wave_count_down;
        self
    }

    /// Runs `f` with everything a board method needs.
    pub fn with<R>(
        &mut self,
        f: impl FnOnce(&mut Board, &mut EffectSystem, &mut GameResources, &mut Assets<Image>, &mut Sounds) -> R,
    ) -> R {
        let Shared { resources, images } = &mut *self.shared;
        let mut sounds = self.sounds.get_mut(&mut self.world).expect("sounds without an audio device");
        let result = f(&mut self.board, &mut self.effects, resources, images, &mut sounds);
        self.sounds.apply(&mut self.world);
        result
    }

    /// One 100 Hz tick of the game.
    pub fn tick(&mut self) {
        if !self.waves {
            // Hold the next wave back: the countdown never gets going.
            let waves = &mut self.board.waves;
            waves.zombie_count_down = 100_000;
            waves.zombie_count_down_start = 100_000;
        }
        let Shared { resources, images } = &mut *self.shared;
        let mut sounds = self.sounds.get_mut(&mut self.world).expect("sounds without an audio device");
        self.board.tick(&mut self.effects, resources, images, &mut self.music, &mut sounds);
        self.sounds.apply(&mut self.world);
        self.world.resource_mut::<UpdateCount>().0 += 1;
    }

    pub fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.tick();
        }
    }

    /// Ticks until `done` holds, for at most `max` ticks. Returns whether it did.
    pub fn run_until(&mut self, max: u32, mut done: impl FnMut(&Board) -> bool) -> bool {
        for _ in 0..max {
            if done(&self.board) {
                return true;
            }
            self.tick();
        }
        done(&self.board)
    }

    pub fn give_sun(&mut self, sun: i32) {
        self.board.sun_money = sun;
    }

    /// Fills the seed bank with these plants, ready to be picked.
    pub fn set_bank(&mut self, seeds: &[SeedType]) {
        let bank = &mut self.board.seed_bank;
        bank.num_packets = seeds.len();
        for (packet, &seed) in bank.packets.iter_mut().zip(seeds) {
            packet.set_packet_type(Some(seed));
            packet.active = true;
            packet.refreshing = false;
        }
        let shared = &mut *self.shared;
        self.board.seed_bank.update_width(&shared.resources);
    }

    /// Puts a plant straight on the lawn (`Board::AddPlant`, no sun spent).
    pub fn plant(&mut self, seed: SeedType, col: i32, row: i32) -> Id<Plant> {
        self.with(|board, effects, res, images, sounds| board.add_plant(col, row, seed, effects, res, images, sounds));
        self.plant_at(col, row).expect("the plant was just planted")
    }

    /// The (living) plant on a square.
    pub fn plant_at(&self, col: i32, row: i32) -> Option<Id<Plant>> {
        self.board.plants.iter().find(|(_, p)| p.col == col && p.row == row && !p.dead).map(|(id, _)| id)
    }

    pub fn plant_ref(&self, id: Id<Plant>) -> Option<&Plant> {
        self.board.plants.get(id).filter(|p| !p.dead)
    }

    /// A click at a point on the board (`Board::MouseDown`).
    pub fn click(&mut self, x: i32, y: i32) {
        let p = Vec2::new(x as f32, y as f32);
        self.with(|board, effects, res, images, sounds| board.mouse_down(p, false, effects, res, images, sounds));
    }

    /// The middle of a lawn square, in board coordinates.
    pub fn square_center(&self, col: i32, row: i32) -> (i32, i32) {
        (self.board.grid_to_pixel_x(col) + 40, self.board.grid_to_pixel_y(col, row) + 40)
    }

    /// Picks packet `slot` from the seed bank and plants it on a square, by clicking.
    pub fn plant_from_bank(&mut self, slot: usize, col: i32, row: i32) {
        let bank = &self.board.seed_bank;
        let packet = &bank.packets[slot];
        let (x, y) = (bank.x + packet.x + packet.offset_x + 25, bank.y + packet.y + 35);
        self.click(x, y);
        let (x, y) = self.square_center(col, row);
        self.click(x, y);
    }

    /// Picks up the shovel and digs up whatever's on a square, by clicking.
    pub fn dig(&mut self, col: i32, row: i32) {
        let (x, y, w, h) = {
            let shared = &*self.shared;
            self.board.shovel_button_rect(&shared.resources)
        };
        self.click(x + w / 2, y + h / 2);
        let (x, y) = self.square_center(col, row);
        self.click(x, y);
    }

    /// A zombie of this type in `row`, its left edge at `x` (the lawn's
    /// right edge is about 800).
    pub fn zombie(&mut self, ty: ZombieType, row: i32, x: f32) -> Id<Zombie> {
        let id = self.with(|board, effects, res, images, sounds| {
            let mut fx = super::zombie::Fx { effects, res, images, sounds };
            board.add_zombie_in_row(ty, row, 0, &mut fx)
        });
        if let Some(zombie) = self.board.zombies.get_mut(id) {
            zombie.place_x(x);
        }
        id
    }

    /// A zombie that's still on the board and not finished dying.
    pub fn zombie_ref(&self, id: Id<Zombie>) -> Option<&Zombie> {
        self.board.zombies.get(id).filter(|z| !z.dead)
    }
}
