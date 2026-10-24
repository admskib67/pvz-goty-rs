//! The seed chooser: from level 1-8 on, the player picks which plants to take
//! into the level. It slides up during the intro, once the view has panned to
//! the street, and the intro's clock waits until the seeds are chosen.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Widget/SeedChooserScreen.cpp`:
//! `GetSeedPositionInChooser`, `GetSeedPositionInBank`, `Draw`,
//! `LandFlyingSeed`, `Update`, `ClickedSeedInBank`, `ClickedSeedInChooser`,
//! `MouseDown`, `CloseSeedChooser` and `EnableStartButton`; and
//! `CutScene::StartSeedChooser`/`EndSeedChooser`, `ShowToolTip`,
//! `SeedNotRecommendedToPick` and `OnStartButton`'s repick warnings; and
//! `Board::SeedNotRecommendedForLevel`. Not yet: the almanac, store and
//! imitater buttons (none of them are unlocked by then on a first playthrough).

use bevy::prelude::*;

use crate::audio::Sounds;
use crate::graphics::{Graphics, Justify, SexyColor};
use crate::plant::SeedType;
use crate::resources::GameResources;

use super::seed_bank::draw_seed_packet;
use super::tooltip::ToolTip;
use super::Board;

const SEED_PACKET_WIDTH: i32 = 50;
const SEED_PACKET_HEIGHT: i32 = 70;
/// The PLAY button (`mStartButton`): x, y, width, height.
const START_BUTTON: (i32, i32, i32, i32) = (154, 545, 156, 42);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChosenSeedState {
    InChooser,
    FlyingToBank,
    FlyingToChooser,
    InBank,
}

#[derive(Debug, Clone)]
struct ChosenSeed {
    x: i32,
    y: i32,
    start: (i32, i32),
    end: (i32, i32),
    time_start: i32,
    time_end: i32,
    state: ChosenSeedState,
    index_in_bank: usize,
}

/// `SeedChooserScreen`.
#[derive(Debug, Clone)]
pub struct SeedChooser {
    seeds: Vec<ChosenSeed>,
    age: i32,
    in_flight: i32,
    in_bank: usize,
    /// The widget's y: it slides in from below (`Move`).
    pub y: i32,
    /// Whether the seeds can be clicked (`mMouseVisible`).
    pub mouse_visible: bool,
    /// How many of the start button's warnings the player has said yes to.
    warnings_accepted: usize,
    /// The player's answer to the last warning ("Yes", or "Re-pick").
    pub warning_answer: Option<bool>,
}

/// `SEED_CHOOSER_OFFSET_Y`: where the chooser waits, just off the bottom of the screen.
pub fn offset_y(res: &GameResources) -> i32 {
    res.image("IMAGE_SEEDCHOOSER_BACKGROUND").map_or(513, |i| i.height as i32)
}

/// `GetSeedPositionInChooser`, for a lawn that hasn't unlocked a seventh row.
fn position_in_chooser(index: usize) -> (i32, i32) {
    let (row, col) = ((index / 8) as i32, (index % 8) as i32);
    (col * 53 + 22, row * 73 + 128)
}

impl SeedChooser {
    pub fn new(available: usize, y: i32) -> Self {
        let seeds = (0..available)
            .map(|i| {
                let (x, y) = position_in_chooser(i);
                ChosenSeed {
                    x,
                    y,
                    start: (x, y),
                    end: (x, y),
                    time_start: 0,
                    time_end: 0,
                    state: ChosenSeedState::InChooser,
                    index_in_bank: 0,
                }
            })
            .collect();
        Self { seeds, age: 0, in_flight: 0, in_bank: 0, y, mouse_visible: false, warnings_accepted: 0, warning_answer: None }
    }

    /// `LandFlyingSeed`.
    fn land(seed: &mut ChosenSeed, in_flight: &mut i32) {
        match seed.state {
            ChosenSeedState::FlyingToBank => seed.state = ChosenSeedState::InBank,
            ChosenSeedState::FlyingToChooser => seed.state = ChosenSeedState::InChooser,
            _ => return,
        }
        (seed.x, seed.y) = seed.end;
        (seed.time_start, seed.time_end) = (0, 0);
        *in_flight -= 1;
    }

    fn land_all(&mut self) {
        for seed in &mut self.seeds {
            Self::land(seed, &mut self.in_flight);
        }
    }

    /// The plant sitting in a bank slot (`FindSeedInBank`).
    fn seed_in_bank(&self, index: usize) -> Option<usize> {
        self.seeds.iter().position(|s| s.state == ChosenSeedState::InBank && s.index_in_bank == index)
    }

    /// `SeedHitTest`.
    fn seed_at(&self, p: IVec2) -> Option<usize> {
        self.seeds.iter().position(|s| {
            p.x >= s.x && p.x < s.x + SEED_PACKET_WIDTH && p.y >= s.y && p.y < s.y + SEED_PACKET_HEIGHT
        })
    }
}

impl Board {
    /// `GetSeedPositionInBank`.
    fn seed_position_in_bank(&self, chooser_y: i32, index: usize) -> (i32, i32) {
        (self.seed_bank.x + self.seed_bank.packet_x(index), self.seed_bank.y - chooser_y + 8)
    }

    /// `CutScene::StartSeedChooser`.
    pub(super) fn start_seed_choosing(&mut self) {
        if let Some(chooser) = &mut self.seed_chooser {
            chooser.mouse_visible = true;
        }
        self.cut_scene.seed_choosing = true;
    }

    /// `SeedChooserScreen::Update`: seeds fly between the chooser and the bank.
    pub(super) fn update_seed_chooser(&mut self) {
        let Some(chooser) = &mut self.seed_chooser else { return };
        chooser.age += 1;
        let age = chooser.age;
        for seed in &mut chooser.seeds {
            if matches!(seed.state, ChosenSeedState::FlyingToBank | ChosenSeedState::FlyingToChooser) {
                let curve = |from: i32, to: i32| {
                    sexy::tod::animate_curve(seed.time_start, seed.time_end, age, from, to, sexy::tod::TodCurve::EaseInOut)
                };
                (seed.x, seed.y) = (curve(seed.start.0, seed.end.0), curve(seed.start.1, seed.end.1));
                if age >= seed.time_end {
                    SeedChooser::land(seed, &mut chooser.in_flight);
                }
            }
        }
    }

    /// `Board::SeedNotRecommendedForLevel`: the tooltip's warning for a plant
    /// that won't do much here, if any (and it's drawn grayed).
    pub(super) fn seed_not_recommended(&self, seed: SeedType) -> Option<&'static str> {
        use SeedType::*;
        if seed.is_nocturnal() && !self.stage_is_night() {
            return Some("[NOCTURNAL_WARNING]");
        }
        let not_here = match seed {
            InstantCoffee => self.stage_is_night(),
            Gravebuster => !self.stage_has_grave_stones(),
            Plantern => !self.stage_has_fog(),
            Flowerpot => !self.stage_has_roof(),
            Spikeweed | Spikerock => self.stage_has_roof(),
            Lilypad | Tanglekelp | Seashroom | Cattail => !self.stage_has_pool(),
            _ => false,
        };
        not_here.then_some("[NOT_RECOMMENDED_FOR_LEVEL]")
    }

    /// `OnStartButton`'s questions before play starts: "are you sure, without ...?"
    pub(super) fn chooser_warnings(&self, picked: impl Fn(SeedType) -> bool) -> Vec<&'static str> {
        let mut warnings = Vec::new();
        let night_one = self.first_time_adventure && self.level == 11;
        if night_one && !picked(SeedType::Puffshroom) {
            warnings.push("[SEED_CHOOSER_PUFFSHROOM_WARNING]");
        }
        if ![SeedType::Sunflower, SeedType::Twinsunflower, SeedType::Sunshroom].into_iter().any(&picked) {
            warnings.push(if night_one { "[SEED_CHOOSER_NIGHT_SUN_WARNING]" } else { "[SEED_CHOOSER_SUN_WARNING]" });
        }
        // TODO: the pool, roof and flyer warnings, with those levels.
        warnings
    }

    /// `OnStartButton`: once the player's answered every warning with
    /// "yes", play starts. Returns the next warning to ask, if any.
    pub(super) fn seed_chooser_start(&mut self) -> Option<&'static str> {
        let chooser = self.seed_chooser.as_ref()?;
        if chooser.in_bank != self.seed_bank.num_packets {
            return None;
        }
        let picked = |seed: SeedType| {
            chooser.seeds.get(seed as usize).is_some_and(|s| matches!(s.state, ChosenSeedState::InBank | ChosenSeedState::FlyingToBank))
        };
        let warnings = self.chooser_warnings(picked);
        if let Some(&warning) = warnings.get(chooser.warnings_accepted) {
            return Some(warning);
        }
        self.close_seed_chooser();
        None
    }

    /// The player's answer to the chooser's warning dialog, for the next tick.
    pub fn answer_chooser_warning(&mut self, yes: bool) {
        if let Some(chooser) = &mut self.seed_chooser {
            chooser.warning_answer = Some(yes);
        }
    }

    /// The answer to a warning: "yes" moves on to the next one (or play),
    /// "Re-pick" goes back to choosing.
    pub(super) fn seed_chooser_warning_answered(&mut self) -> Option<&'static str> {
        let chooser = self.seed_chooser.as_mut()?;
        match chooser.warning_answer.take()? {
            true => {
                chooser.warnings_accepted += 1;
                self.seed_chooser_start()
            }
            false => {
                chooser.warnings_accepted = 0;
                None
            }
        }
    }

    /// `SeedChooserScreen::MouseDown`, at a point in screen coordinates.
    /// Returns a warning to ask about before play starts, if any.
    pub(super) fn seed_chooser_mouse_down(&mut self, p: IVec2, sounds: &mut Sounds) -> Option<&'static str> {
        let num_packets = self.seed_bank.num_packets;
        let chooser = self.seed_chooser.as_mut().filter(|c| c.mouse_visible)?;
        chooser.land_all();
        // The chooser's own coordinates.
        let p = p - IVec2::new(0, chooser.y);

        let (bx, by, bw, bh) = START_BUTTON;
        if p.x >= bx && p.x < bx + bw && p.y >= by && p.y < by + bh {
            sounds.play_sample("SOUND_TAP");
            chooser.warnings_accepted = 0;
            return self.seed_chooser_start();
        }

        let index = chooser.seed_at(p)?;
        match chooser.seeds[index].state {
            ChosenSeedState::InBank => {
                let (age, chooser_y) = (chooser.age, chooser.y);
                let from = chooser.seeds[index].index_in_bank;
                // The packets after it slide one place left.
                let moving: Vec<(usize, usize)> =
                    (from + 1..num_packets).filter_map(|slot| chooser.seed_in_bank(slot).map(|s| (s, slot))).collect();
                let moves: Vec<(usize, (i32, i32))> =
                    moving.into_iter().map(|(s, slot)| (s, self.seed_position_in_bank(chooser_y, slot - 1))).collect();
                let chooser = self.seed_chooser.as_mut().expect("chooser is open");
                for (seed_index, end) in moves {
                    let seed = &mut chooser.seeds[seed_index];
                    seed.time_start = age;
                    seed.time_end = age + 15;
                    seed.start = (seed.x, seed.y);
                    seed.end = end;
                    seed.state = ChosenSeedState::FlyingToBank;
                    seed.index_in_bank -= 1;
                    chooser.in_flight += 1;
                }
                let seed = &mut chooser.seeds[index];
                seed.time_start = age;
                seed.time_end = age + 25;
                seed.start = (seed.x, seed.y);
                seed.end = position_in_chooser(index);
                seed.state = ChosenSeedState::FlyingToChooser;
                seed.index_in_bank = 0;
                chooser.in_bank -= 1;
                chooser.in_flight += 1;
                sounds.play_sample("SOUND_TAP");
            }
            ChosenSeedState::InChooser => {
                if chooser.in_bank == num_packets {
                    return None;
                }
                let (age, chooser_y, slot) = (chooser.age, chooser.y, chooser.in_bank);
                let end = self.seed_position_in_bank(chooser_y, slot);
                let chooser = self.seed_chooser.as_mut().expect("chooser is open");
                let seed = &mut chooser.seeds[index];
                seed.time_start = age;
                seed.time_end = age + 25;
                seed.start = (seed.x, seed.y);
                seed.end = end;
                seed.state = ChosenSeedState::FlyingToBank;
                seed.index_in_bank = slot;
                chooser.in_flight += 1;
                chooser.in_bank += 1;
                sounds.play_sample("SOUND_TAP");
            }
            _ => {}
        }
        None
    }

    /// `SeedChooserScreen::CloseSeedChooser` + `CutScene::EndSeedChooser`.
    fn close_seed_chooser(&mut self) {
        let Some(chooser) = &mut self.seed_chooser else { return };
        chooser.land_all();
        for slot in 0..self.seed_bank.num_packets {
            let seed = chooser.seed_in_bank(slot).and_then(|i| SeedType::from_index(i as i32));
            self.seed_bank.packets[slot].set_packet_type(seed);
        }
        chooser.mouse_visible = false;
        self.cut_scene.end_seed_choosing();
    }

    /// `SeedChooserScreen::Draw`, in screen coordinates.
    pub(super) fn draw_seed_chooser(&self, res: &GameResources, g: &mut Graphics) {
        let Some(chooser) = &self.seed_chooser else { return };
        let Some(background) = res.image("IMAGE_SEEDCHOOSER_BACKGROUND") else { return };
        let choosing = self.cut_scene.seed_choosing;
        g.push_state();
        g.translate(-self.x as f32, -self.y as f32);
        g.translate(0.0, chooser.y as f32);
        g.draw_image(background, 0.0, 87.0);
        let title = res.translate("[CHOOSE_YOUR_SEEDS]");
        let title = if title.starts_with("<Missing") { res.translate("[CHOOSE_YOUR_PLANTS]") } else { title };
        if let Some(font) = res.font("FONT_DWARVENTODCRAFT18YELLOW") {
            g.tod_draw_string(font, 229, 110, &title, SexyColor::WHITE, Justify::Center);
        }

        // The slots: a dim copy of a plant that's been taken, a silhouette for the rest.
        for index in 0..40 {
            let (x, y) = position_in_chooser(index);
            match chooser.seeds.get(index) {
                Some(seed) if seed.state != ChosenSeedState::InChooser => {
                    if let Some(s) = SeedType::from_index(index as i32) {
                        draw_seed_packet(g, res, x as f32, y as f32, s, 0.0, 55, true);
                    }
                }
                Some(_) => {}
                None => {
                    if let Some(silhouette) = res.image("IMAGE_SEEDPACKETSILHOUETTE") {
                        g.draw_image(silhouette, x as f32, y as f32);
                    }
                }
            }
        }
        for slot in 0..self.seed_bank.num_packets {
            if chooser.seed_in_bank(slot).is_none()
                && let Some(silhouette) = res.image("IMAGE_SEEDPACKETSILHOUETTE")
            {
                let (x, y) = self.seed_position_in_bank(chooser.y, slot);
                g.draw_image(silhouette, x as f32, y as f32);
            }
        }

        let flying = |s: &ChosenSeed| matches!(s.state, ChosenSeedState::FlyingToBank | ChosenSeedState::FlyingToChooser);
        for pass_flying in [false, true] {
            for (index, seed) in chooser.seeds.iter().enumerate() {
                if flying(seed) != pass_flying || (!pass_flying && seed.state != ChosenSeedState::InChooser && !choosing) {
                    continue;
                }
                if let Some(ty) = SeedType::from_index(index as i32) {
                    let grayed = seed.state == ChosenSeedState::InChooser && self.seed_not_recommended(ty).is_some();
                    draw_seed_packet(g, res, seed.x as f32, seed.y as f32, ty, 0.0, if grayed { 115 } else { 255 }, true);
                }
            }
        }

        // The PLAY button.
        let enabled = chooser.in_bank == self.seed_bank.num_packets;
        let (bx, by, bw, bh) = START_BUTTON;
        let image = if enabled { "IMAGE_SEEDCHOOSER_BUTTON" } else { "IMAGE_SEEDCHOOSER_BUTTON_DISABLED" };
        if let Some(button) = res.image(image) {
            g.draw_image(button, bx as f32, by as f32);
        }
        let mouse = self.mouse.map(|m| (m.x + self.x as f32, m.y + self.y as f32 - chooser.y as f32));
        let over = mouse.is_some_and(|(mx, my)| {
            mx >= bx as f32 && mx < (bx + bw) as f32 && my >= by as f32 && my < (by + bh) as f32
        });
        if enabled
            && over
            && chooser.mouse_visible
            && let Some(glow) = res.image("IMAGE_SEEDCHOOSER_BUTTON_GLOW")
        {
            g.draw_image(glow, bx as f32, by as f32);
        }
        let label = res.translate("[PLAY_BUTTON]");
        let label = if label.starts_with("<Missing") { res.translate("[LETS_ROCK_BUTTON]") } else { label };
        if let Some(font) = res.font("FONT_DWARVENTODCRAFT15") {
            let color = if enabled { SexyColor::WHITE } else { SexyColor::rgba(64, 64, 64, 255) };
            let ascent = font.font.ascent;
            let y = by + (bh - ascent / 6 - 1 + ascent) / 2 - 1;
            g.tod_draw_string(font, bx + bw / 2, y, &label, color, Justify::Center);
        }
        // ShowToolTip: the plant under the mouse, when nothing's moving.
        if chooser.mouse_visible
            && chooser.in_flight <= 0
            && !self.paused
            && let Some((mx, my)) = mouse
            && let Some(index) = chooser.seed_at(IVec2::new(mx.floor() as i32, my.floor() as i32))
            && let Some(seed) = SeedType::from_index(index as i32)
        {
            let name = seed.definition().name;
            let warning = self.seed_not_recommended(seed).unwrap_or("");
            if let Some(mut tip) = ToolTip::new(res, &format!("[{name}]"), &format!("[{name}_TOOLTIP]"), warning) {
                let s = &chooser.seeds[index];
                tip.x = ((SEED_PACKET_WIDTH - tip.width) / 2 + s.x).clamp(0, super::BOARD_WIDTH - tip.width);
                tip.y = s.y + 70;
                tip.draw(g, res);
            }
        }
        g.pop_state();

        // The Menu button comes with the chooser (screen coordinates).
        g.push_state();
        g.translate(-self.x as f32, -self.y as f32);
        super::draw_menu_button(self, res, g);
        g.pop_state();
    }
}
