//! Beghouled: an 8×5 board of plants that is played like a match-3 puzzle
//! while zombies walk in and eat the pieces.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Challenge.cpp` (`Beghouled*`,
//! `UpdateBeghouled`, `ZombieAtePlant`), `Lawn/Board.cpp` (`AddPlant`'s
//! Beghouled rules) and `Lawn/Plant.cpp` (`GetCost`'s Beghouled prices).
//!
//! Two games share this engine: `Beghouled` swaps adjacent pieces until the
//! score reaches 75; `Beghouled Twist` uses the same board but rotates its
//! target pieces, which isn't ported yet (`MiniGame::is_ported` keeps it off
//! the page).
//!
//! Simplified against the original: pieces fall and refill instantly instead
//! of animating, the "a match is here" flashing hint only runs when the
//! board has no moves, and the point particles are not drawn.

use bevy::prelude::*;
use sexy::rand::MtRand;

use crate::audio::{FoleyType, Sounds};
use crate::plant::SeedType;
use crate::reanim::EffectSystem;
use crate::resources::GameResources;

use super::coin::{CoinMotion, CoinType};
use super::message::MessageStyle;
use super::plant::Plant;
use super::{Board, MiniGame, make_render_order, render_layer};

/// `BEGHOULED_MAX_GRIDSIZEX`.
pub const COLS: usize = 8;
/// `BEGHOULED_MAX_GRIDSIZEY`.
pub const ROWS: usize = 5;
/// `BEGHOULED_WINNING_SCORE`.
pub const WINNING_SCORE: i32 = 75;

/// One match: its squares, its length, and whether it runs horizontally.
type MatchRun = (Vec<(i32, i32)>, i32, bool);

/// A rotated plant's from/to squares and the new pixel position (`x`, `y`).
type TwistMove = ((i32, i32), (i32, i32), i32, i32);

/// The six pieces, in `BeghouledPickSeed`'s order.
pub const PIECES: [SeedType; 6] = [
    SeedType::Puffshroom,
    SeedType::Starfruit,
    SeedType::Magnetshroom,
    SeedType::Snowpea,
    SeedType::Wallnut,
    SeedType::Peashooter,
];

/// The seed bank's two special buttons (`SEED_BEGHOULED_BUTTON_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeghouledButton {
    Shuffle,
    Crater,
}

/// The challenge's own state (`Challenge`'s Beghouled fields).
#[derive(Debug, Clone)]
pub struct Beghouled {
    grid: [[Option<SeedType>; COLS]; ROWS],
    eated: [[bool; COLS]; ROWS],
    pub score: i32,
    matches_this_move: i32,
    purchased: [bool; 3],
    /// The square a drag started on (`mBeghouledMouseCapture`).
    pub drag: Option<(i32, i32)>,
    /// Ticks until the no-moves shuffle (`mChallengeStateCounter`).
    counter: i32,
}

impl Default for Beghouled {
    fn default() -> Self {
        Self {
            grid: [[None; COLS]; ROWS],
            eated: [[false; COLS]; ROWS],
            score: 0,
            matches_this_move: 0,
            purchased: [false; 3],
            drag: None,
            counter: 1500,
        }
    }
}

impl Beghouled {
    fn seed_at(&self, col: i32, row: i32) -> Option<SeedType> {
        self.grid.get(row as usize).and_then(|r| r.get(col as usize)).copied().flatten()
    }

    fn set(&mut self, col: i32, row: i32, seed: Option<SeedType>) {
        if let Some(cell) = self.grid.get_mut(row as usize).and_then(|r| r.get_mut(col as usize)) {
            *cell = seed;
        }
    }

    pub fn is_eated(&self, col: i32, row: i32) -> bool {
        self.eated.get(row as usize).and_then(|r| r.get(col as usize)).copied().unwrap_or(false)
    }
}

impl Board {
    /// `Plant::GetCost` with the Beghouled prices.
    pub fn plant_cost(&self, seed: SeedType) -> i32 {
        if matches!(self.minigame(), Some(MiniGame::Beghouled | MiniGame::BeghouledTwist)) {
            match seed {
                SeedType::Repeater => 1000,
                SeedType::Fumeshroom => 500,
                SeedType::Tallnut => 250,
                _ => seed.cost(),
            }
        } else {
            seed.cost()
        }
    }

    /// Whether one of the Beghouled games is being played.
    pub fn is_beghouled(&self) -> bool {
        matches!(self.minigame(), Some(MiniGame::Beghouled | MiniGame::BeghouledTwist))
    }

    /// The puzzle state, while playing.
    pub fn beghouled(&self) -> Option<&Beghouled> {
        self.beghouled.as_ref()
    }

    /// Fills the board with pieces and creates their plants (`BeghouledMakeStartBoard`).
    pub(super) fn start_beghouled(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        rng: &mut MtRand,
    ) {
        if !self.is_beghouled() {
            return;
        }
        let mut state = Beghouled::default();
        // Fill in random order, never making a match.
        let mut cells: Vec<(i32, i32)> = (0..ROWS as i32).flat_map(|row| (0..COLS as i32).map(move |col| (col, row))).collect();
        for i in (1..cells.len()).rev() {
            cells.swap(i, rng.next_range((i + 1) as u32) as usize);
        }
        for (col, row) in cells {
            let seed = self.beghouled_pick_seed(&state, col, row, false, rng);
            state.set(col, row, Some(seed));
            self.spawn_beghouled_plant(col, row, seed, effects, res, images, rng);
        }
        self.beghouled = Some(state);
        self.seed_bank.num_packets = 0;
        self.seed_bank.update_width(res);
        self.sun_money = 0;
        self.show_shovel = false;
        let message = res.translate("[ADVICE_BEGHOULED_DRAG_TO_MATCH_3]");
        self.display_advice(res, &message, MessageStyle::HintFast, None);
    }

    /// `BeghouledPickSeed`: a random piece that doesn't make a match (unless
    /// cascades are allowed), with purchased upgrades applied.
    fn beghouled_pick_seed(&self, state: &Beghouled, col: i32, row: i32, allow_matches: bool, rng: &mut MtRand) -> SeedType {
        let mut grid = state.grid;
        for _ in 0..32 {
            let mut seed = PIECES[rng.next_range(PIECES.len() as u32) as usize];
            seed = self.beghouled_upgraded(seed);
            grid[row as usize][col as usize] = Some(seed);
            let fit = allow_matches || !beghouled_has_match(&grid);
            grid[row as usize][col as usize] = None;
            if fit {
                return seed;
            }
        }
        self.beghouled_upgraded(PIECES[rng.next_range(PIECES.len() as u32) as usize])
    }

    /// The upgrade a piece becomes once it's bought (`BeghouledPickSeed`).
    fn beghouled_upgraded(&self, seed: SeedType) -> SeedType {
        let Some(state) = &self.beghouled else { return seed };
        if state.purchased[0] && seed == SeedType::Peashooter {
            SeedType::Repeater
        } else if state.purchased[1] && seed == SeedType::Puffshroom {
            SeedType::Fumeshroom
        } else if state.purchased[2] && seed == SeedType::Wallnut {
            SeedType::Tallnut
        } else {
            seed
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_beghouled_plant(
        &mut self,
        col: i32,
        row: i32,
        seed: SeedType,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        rng: &mut MtRand,
    ) {
        let (x, y) = (self.grid_to_pixel_x(col), self.grid_to_pixel_y(col, row));
        let plant = Plant::new(col, row, x, y, seed, false, effects, res, images, rng);
        self.plants.alloc(plant);
    }

    /// `BeghouledIsValidMove`: swapping these two cells makes a match.
    pub fn beghouled_valid_move(&self, from: (i32, i32), to: (i32, i32)) -> bool {
        let Some(state) = &self.beghouled else { return false };
        if state.is_eated(from.0, from.1) || state.is_eated(to.0, to.1) {
            return false;
        }
        let (Some(a), Some(b)) = (state.seed_at(from.0, from.1), state.seed_at(to.0, to.1)) else { return false };
        let mut grid = state.grid;
        grid[from.1 as usize][from.0 as usize] = Some(b);
        grid[to.1 as usize][to.0 as usize] = Some(a);
        beghouled_has_match(&grid)
    }

    /// `BeghouledDragUpdate`: swaps two adjacent pieces if the swap makes a
    /// match, then resolves the board.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn beghouled_swap(
        &mut self,
        from: (i32, i32),
        to: (i32, i32),
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        if self.has_level_award_dropped() || !self.beghouled_valid_move(from, to) {
            sounds.play_foley(FoleyType::Floop);
            return;
        }
        let Some(state) = &mut self.beghouled else { return };
        let Some(a) = state.seed_at(from.0, from.1) else { return };
        let Some(b) = state.seed_at(to.0, to.1) else { return };
        state.set(from.0, from.1, Some(b));
        state.set(to.0, to.1, Some(a));
        // Swap the plants too.
        let from_pos = (self.grid_to_pixel_x(from.0), self.grid_to_pixel_y(from.0, from.1));
        let to_pos = (self.grid_to_pixel_x(to.0), self.grid_to_pixel_y(to.0, to.1));
        for (_, plant) in self.plants.iter_mut() {
            if plant.dead {
                continue;
            }
            let (target, (x, y)) = if (plant.col, plant.row) == from {
                (to, to_pos)
            } else if (plant.col, plant.row) == to {
                (from, from_pos)
            } else {
                continue;
            };
            (plant.col, plant.row) = target;
            plant.x = x;
            plant.y = y;
            plant.render_order = make_render_order(render_layer::PLANT, plant.row, 0);
        }
        self.resolve_beghouled(effects, res, images, sounds);
    }

    /// Removes every match, lets the pieces fall, refills, and repeats while
    /// the board has matches (`BeghouledRemoveMatches` + `MakePlantsFall` +
    /// `PopulateBoard`).
    fn resolve_beghouled(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        if self.beghouled.is_none() {
            return;
        }
        for _ in 0..24 {
            let runs = self.beghouled_match_runs();
            if runs.is_empty() {
                break;
            }
            for (cells, length, horizontal) in runs {
                let (col, row) = cells[0];
                if let Some(state) = &mut self.beghouled {
                    for (c, r) in &cells {
                        state.set(*c, *r, None);
                    }
                    state.score += 1;
                    state.matches_this_move += 1;
                }
                for (c, r) in cells {
                    for (_, plant) in self.plants.iter_mut() {
                        if !plant.dead && plant.col == c && plant.row == r {
                            plant.die(effects);
                        }
                    }
                }
                let (x, y) = (self.grid_to_pixel_x(col) + 40, self.grid_to_pixel_y(col, row) + 40);
                let mut suns = length - 2 + self.beghouled.as_ref().map_or(0, |b| b.matches_this_move);
                if length >= 5 {
                    suns += 2;
                }
                for i in 0..suns.clamp(1, 5) {
                    self.add_coin(
                        (x + 20 * i - 10) as f32,
                        y as f32,
                        CoinType::Sun,
                        CoinMotion::Coin,
                        effects,
                        res,
                        images,
                        sounds,
                    );
                }
                sounds.play_foley(FoleyType::Chime);
                let _ = horizontal;
                if self.beghouled.as_ref().is_some_and(|b| b.score >= WINNING_SCORE) {
                    self.beghouled_award(effects, res, images, sounds);
                    return;
                }
                self.beghouled_show_score_advice(res);
            }
            self.beghouled_fall_and_refill(effects, res, images, sounds);
        }
        if let Some(state) = &mut self.beghouled {
            state.matches_this_move = 0;
            state.counter = 1500;
        }
        self.beghouled_check_stuck(effects, res, images, sounds);
    }

    /// Every horizontal and vertical run of three or more.
    fn beghouled_match_runs(&self) -> Vec<MatchRun> {
        let Some(state) = &self.beghouled else { return Vec::new() };
        let mut runs = Vec::new();
        for row in 0..ROWS as i32 {
            let mut col = 0;
            while col < COLS as i32 {
                let Some(seed) = state.seed_at(col, row) else {
                    col += 1;
                    continue;
                };
                let start = col;
                while col < COLS as i32 && state.seed_at(col, row) == Some(seed) {
                    col += 1;
                }
                let length = col - start;
                if length >= 3 {
                    runs.push(((start..col).map(|c| (c, row)).collect(), length, true));
                }
            }
        }
        for col in 0..COLS as i32 {
            let mut row = 0;
            while row < ROWS as i32 {
                let Some(seed) = state.seed_at(col, row) else {
                    row += 1;
                    continue;
                };
                let start = row;
                while row < ROWS as i32 && state.seed_at(col, row) == Some(seed) {
                    row += 1;
                }
                let length = row - start;
                if length >= 3 {
                    runs.push(((start..row).map(|r| (col, r)).collect(), length, false));
                }
            }
        }
        runs
    }

    /// Pieces fall into holes and the holes at the top refill.
    fn beghouled_fall_and_refill(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        for col in 0..COLS as i32 {
            // Settle.
            loop {
                let mut moved = false;
                for row in (1..ROWS as i32).rev() {
                    let above = self.beghouled.as_ref().and_then(|b| b.seed_at(col, row - 1));
                    let here = self.beghouled.as_ref().and_then(|b| b.seed_at(col, row));
                    let blocked = self.beghouled.as_ref().is_some_and(|b| b.is_eated(col, row));
                    if here.is_none() && !blocked && let Some(seed) = above {
                        if let Some(state) = &mut self.beghouled {
                            state.set(col, row, Some(seed));
                            state.set(col, row - 1, None);
                        }
                        let (x, y) = (self.grid_to_pixel_x(col), self.grid_to_pixel_y(col, row));
                        for (_, plant) in self.plants.iter_mut() {
                            if !plant.dead && plant.col == col && plant.row == row - 1 {
                                plant.row = row;
                                plant.x = x;
                                plant.y = y;
                                plant.render_order = make_render_order(render_layer::PLANT, row, 0);
                            }
                        }
                        moved = true;
                    }
                }
                if !moved {
                    break;
                }
            }
            // Refill the top.
            for row in 0..ROWS as i32 {
                let here = self.beghouled.as_ref().and_then(|b| b.seed_at(col, row));
                let blocked = self.beghouled.as_ref().is_some_and(|b| b.is_eated(col, row));
                if here.is_none() && !blocked {
                    let state = self.beghouled.clone().unwrap_or_default();
                    let seed = self.beghouled_pick_seed(&state, col, row, true, sounds.rng());
                    if let Some(state) = &mut self.beghouled {
                        state.set(col, row, Some(seed));
                    }
                    self.spawn_beghouled_plant(col, row, seed, effects, res, images, sounds.rng());
                }
            }
        }
        let _ = images;
    }

    /// `BeghouledScore`'s advice and the seed bank's upgrade packets once
    /// the first match is made.
    fn beghouled_show_score_advice(&mut self, res: &GameResources) {
        let Some(state) = &self.beghouled else { return };
        if self.seed_bank.num_packets == 0 {
            self.seed_bank.num_packets = 4;
            let packets = &mut self.seed_bank.packets;
            packets[0].packet_type = Some(SeedType::Repeater);
            packets[1].packet_type = Some(SeedType::Fumeshroom);
            packets[2].packet_type = Some(SeedType::Tallnut);
            packets[3].packet_type = None;
            packets[3].button = Some(BeghouledButton::Shuffle);
            for packet in &mut packets[..4] {
                packet.active = true;
                packet.refreshing = false;
            }
            self.seed_bank.update_width(res);
            let message = res.translate("[ADVICE_BEGHOULED_SAVE_SUN]");
            self.display_advice(res, &message, MessageStyle::HintFast, None);
        } else if state.score >= WINNING_SCORE - 5 {
            let message = res.translate("[ADVICE_BEGHOULED_ALMOST_THERE]");
            self.display_advice(res, &message, MessageStyle::HintFast, None);
        }
    }

    /// `SpawnLevelAward` for a finished puzzle.
    fn beghouled_award(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        if self.level_award_spawned {
            return;
        }
        self.level_award_spawned = true;
        sounds.play_foley(FoleyType::SpawnSun);
        let (x, y) = (self.grid_to_pixel_x(3) + 40, self.grid_to_pixel_y(3, 2) + 40);
        self.add_coin(x as f32, y as f32, CoinType::SilverSunflower, CoinMotion::Coin, effects, res, images, sounds);
        self.clear_advice(None);
    }

    /// `BeghouledCheckStuckState` + `BeghouledShuffle`.
    fn beghouled_check_stuck(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        if self.has_level_award_dropped() || self.beghouled_has_move() {
            return;
        }
        if let Some(state) = &mut self.beghouled {
            state.counter = 500;
        }
        let message = res.translate("[ADVICE_BEGHOULED_NO_MOVES]");
        self.display_advice(res, &message, MessageStyle::HintFast, None);
        // The board waits out the timer in the original; the shuffle happens
        // when it ends, which the counter below does.
        self.beghouled_shuffle(effects, res, images, sounds);
    }

    fn beghouled_has_move(&self) -> bool {
        let Some(state) = &self.beghouled else { return false };
        if self.minigame() == Some(MiniGame::BeghouledTwist) {
            return (0..4).any(|row| (0..7).any(|col| self.beghouled_twist_causes_match(col, row)));
        }
        for row in 0..ROWS as i32 {
            for col in 0..COLS as i32 {
                if col + 1 < COLS as i32 && self.beghouled_valid_move((col, row), (col + 1, row)) {
                    return true;
                }
                if row + 1 < ROWS as i32 && self.beghouled_valid_move((col, row), (col, row + 1)) {
                    return true;
                }
            }
        }
        let _ = state;
        false
    }

    /// `BeghouledShuffle`: replaces every piece, with no matches and at least
    /// one possible move.
    pub(super) fn beghouled_shuffle(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        for _ in 0..16 {
            // Tear the board up.
            for (_, plant) in self.plants.iter_mut() {
                if !plant.dead && (plant.col, plant.row) < (COLS as i32, ROWS as i32) {
                    plant.die(effects);
                }
            }
            if let Some(state) = &mut self.beghouled {
                for row in 0..ROWS {
                    for col in 0..COLS {
                        state.grid[row][col] = None;
                    }
                }
            }
            let mut cells: Vec<(i32, i32)> = (0..ROWS as i32).flat_map(|row| (0..COLS as i32).map(move |col| (col, row))).collect();
            for i in (1..cells.len()).rev() {
                cells.swap(i, sounds.rng().next_range((i + 1) as u32) as usize);
            }
            for (col, row) in cells {
                let state = self.beghouled.clone().unwrap_or_default();
                if self.beghouled.as_ref().is_some_and(|b| b.is_eated(col, row)) {
                    continue;
                }
                let seed = self.beghouled_pick_seed(&state, col, row, false, sounds.rng());
                if let Some(state) = &mut self.beghouled {
                    state.set(col, row, Some(seed));
                }
                self.spawn_beghouled_plant(col, row, seed, effects, res, images, sounds.rng());
            }
            if self.beghouled_has_move() {
                break;
            }
        }
    }

    /// `Challenge::ZombieAtePlant`: the square can't be refilled until a
    /// crater button is used.
    pub(super) fn beghouled_plant_eaten(&mut self, col: i32, row: i32, res: &GameResources) {
        let Some(state) = &mut self.beghouled else { return };
        if !(0..COLS as i32).contains(&col) || !(0..ROWS as i32).contains(&row) {
            return;
        }
        state.eated[row as usize][col as usize] = true;
        state.set(col, row, None);
        if self.seed_bank.num_packets == 4 {
            self.seed_bank.num_packets = 5;
            self.seed_bank.packets[4].packet_type = None;
            self.seed_bank.packets[4].button = Some(BeghouledButton::Crater);
            self.seed_bank.packets[4].active = true;
            self.seed_bank.update_width(res);
            let message = res.translate("[ADVICE_BEGHOULED_USE_CRATER_1]");
            self.display_advice(res, &message, MessageStyle::HintFast, None);
        }
    }

    /// A purchase from the Beghouled seed bank: an upgrade, a shuffle or a
    /// crater clear.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn beghouled_packet_clicked(
        &mut self,
        index: usize,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) -> bool {
        if !self.is_beghouled() || self.has_level_award_dropped() {
            return false;
        }
        let Some(packet) = self.seed_bank.packets.get(index) else { return false };
        let (button, packet_type, active) = (packet.button, packet.packet_type, packet.active);
        let cost = match button {
            Some(BeghouledButton::Shuffle) => 100,
            Some(BeghouledButton::Crater) => 200,
            None => match packet_type {
                Some(SeedType::Repeater) => 1000,
                Some(SeedType::Fumeshroom) => 500,
                Some(SeedType::Tallnut) => 250,
                _ => return false,
            },
        };
        if !active {
            sounds.play_sample("SOUND_BUZZER");
            return true;
        }
        if !self.take_sun_money(cost, sounds) {
            return true;
        }
        if let Some(button) = button {
            match button {
                BeghouledButton::Shuffle => {
                    self.beghouled_shuffle(effects, res, images, sounds);
                    sounds.play_foley(FoleyType::Floop);
                }
                BeghouledButton::Crater => {
                    if let Some(state) = &mut self.beghouled {
                        state.eated = [[false; COLS]; ROWS];
                    }
                    if let Some(packet) = self.seed_bank.packets.get_mut(index) {
                        packet.active = false;
                    }
                    for _ in 0..(ROWS * COLS) {
                        self.beghouled_fall_and_refill(effects, res, images, sounds);
                    }
                }
            }
            return true;
        }
        let Some(seed) = packet_type else { return true };
        let (upgrade_index, from, to) = match seed {
            SeedType::Repeater => (0, SeedType::Peashooter, SeedType::Repeater),
            SeedType::Fumeshroom => (1, SeedType::Puffshroom, SeedType::Fumeshroom),
            SeedType::Tallnut => (2, SeedType::Wallnut, SeedType::Tallnut),
            _ => return false,
        };
        if let Some(state) = &mut self.beghouled {
            state.purchased[upgrade_index] = true;
            for row in 0..ROWS {
                for col in 0..COLS {
                    if state.grid[row][col] == Some(from) {
                        state.grid[row][col] = Some(to);
                    }
                }
            }
        }
        for (_, plant) in self.plants.iter_mut() {
            if !plant.dead && plant.seed == from {
                plant.seed = to;
            }
        }
        if let Some(packet) = self.seed_bank.packets.get_mut(index) {
            packet.active = false;
        }
        sounds.play_foley(FoleyType::Chime);
        true
    }

    /// `BeghouledTwistSquareFromMouse`: the 2×2 square whose top-left is
    /// 40 px up and left of the pointer.
    pub fn beghouled_twist_square(&self, p: IVec2) -> Option<(i32, i32)> {
        let col = self.pixel_to_grid_x(p.x - 40);
        let row = self.pixel_to_grid_y(p.x - 40, p.y - 40);
        (col <= 6 && row <= 3 && col >= 0 && row >= 0).then_some((col, row))
    }

    fn twist_valid(&self, col: i32, row: i32) -> bool {
        let Some(state) = &self.beghouled else { return false };
        (0..2).all(|dc| (0..2).all(|dr| state.seed_at(col + dc, row + dr).is_some()))
    }

    /// `BeghouledTwistMoveCausesMatch`: rotating this square makes a match.
    pub fn beghouled_twist_causes_match(&self, col: i32, row: i32) -> bool {
        if !self.twist_valid(col, row) {
            return false;
        }
        let Some(state) = &self.beghouled else { return false };
        let mut grid = state.grid;
        rotate_twist(&mut grid, col, row);
        beghouled_has_match(&grid)
    }

    /// `BeghouledTwistMouseDown`: rotates the square clockwise, if that
    /// makes a match.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn beghouled_twist_move(
        &mut self,
        col: i32,
        row: i32,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut Assets<Image>,
        sounds: &mut Sounds,
    ) {
        if self.has_level_award_dropped() || !self.beghouled_twist_causes_match(col, row) {
            sounds.play_foley(FoleyType::Floop);
            return;
        }
        let Some(state) = &mut self.beghouled else { return };
        let a = state.seed_at(col, row);
        let b = state.seed_at(col + 1, row);
        let c = state.seed_at(col, row + 1);
        let d = state.seed_at(col + 1, row + 1);
        // TL→TR, TR→BR, BR→BL, BL→TL.
        state.set(col + 1, row, a);
        state.set(col + 1, row + 1, b);
        state.set(col, row + 1, d);
        state.set(col, row, c);
        let targets = [
            ((col, row), (col + 1, row)),
            ((col + 1, row), (col + 1, row + 1)),
            ((col + 1, row + 1), (col, row + 1)),
            ((col, row + 1), (col, row)),
        ];
        let positions: Vec<TwistMove> = targets
            .iter()
            .map(|(from, to)| (*from, *to, self.grid_to_pixel_x(to.0), self.grid_to_pixel_y(to.0, to.1)))
            .collect();
        for (_, plant) in self.plants.iter_mut() {
            if plant.dead {
                continue;
            }
            if let Some((_, to, x, y)) = positions.iter().find(|(from, ..)| (plant.col, plant.row) == *from) {
                (plant.col, plant.row) = *to;
                plant.x = *x;
                plant.y = *y;
                plant.render_order = make_render_order(render_layer::PLANT, to.1, 0);
            }
        }
        self.resolve_beghouled(effects, res, images, sounds);
    }

}

/// `BeghouledBoardHasMatch`.
fn beghouled_has_match(grid: &[[Option<SeedType>; COLS]; ROWS]) -> bool {
    for row in 0..ROWS {
        for col in 0..COLS {
            let Some(seed) = grid[row][col] else { continue };
            let horizontal = (0..3).all(|i| col + i < COLS && grid[row][col + i] == Some(seed));
            let vertical = (0..3).all(|i| row + i < ROWS && grid[row + i][col] == Some(seed));
            if horizontal || vertical {
                return true;
            }
        }
    }
    false
}


/// Rotates a 2×2 square clockwise (`BeghouledTwistMoveCausesMatch`).
fn rotate_twist(grid: &mut [[Option<SeedType>; COLS]; ROWS], col: i32, row: i32) {
    let (c, r) = (col as usize, row as usize);
    let (a, b, c_seed, d) = (grid[r][c], grid[r][c + 1], grid[r + 1][c], grid[r + 1][c + 1]);
    grid[r][c + 1] = a;
    grid[r + 1][c + 1] = b;
    grid[r + 1][c] = d;
    grid[r][c] = c_seed;
}
