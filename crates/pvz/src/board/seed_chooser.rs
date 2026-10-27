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
use crate::zombie::ZombieType;
use crate::resources::GameResources;

use super::seed_bank::{draw_seed_packet, draw_seed_packet_as};
use super::tooltip::ToolTip;
use super::Board;

const SEED_PACKET_WIDTH: i32 = 50;
const SEED_PACKET_HEIGHT: i32 = 70;
/// The PLAY button (`mStartButton`): x, y, width, height.
const START_BUTTON: (i32, i32, i32, i32) = (154, 545, 156, 42);
/// `mAlmanacButton`: `[ALMANAC_BUTTON]`, at the bottom right of the chooser.
const ALMANAC_BUTTON: (i32, i32) = (560, 572);
/// `IMAGE_SEEDCHOOSER_BUTTON2`'s size.
const ALMANAC_BUTTON_SIZE: (i32, i32) = (111, 26);
/// What a click in the chooser asked for besides picking seeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChooserClick {
    /// `OnStartButton` has a question before play starts.
    Warning(ChooserWarning),
    /// The Almanac button was pressed.
    Almanac,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChosenSeedState {
    InChooser,
    FlyingToBank,
    FlyingToChooser,
    InBank,
    /// Not in the chooser: a plant the player doesn't have, or the imitater's own spot until it's picked.
    Hidden,
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
    /// The plant an imitater copies (`mImitaterType`).
    imitating: Option<SeedType>,
    /// This is the imitater's slot: back in the chooser it's hidden again.
    imitater_slot: bool,
}

/// One of `OnStartButton`'s questions: a string key, or the upgrade
/// warning naming the two plants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChooserWarning {
    Key(&'static str),
    Upgrade { to: SeedType, from: SeedType },
}

impl From<&'static str> for ChooserWarning {
    fn from(key: &'static str) -> Self {
        Self::Key(key)
    }
}

impl PartialEq<&str> for ChooserWarning {
    fn eq(&self, other: &&str) -> bool {
        matches!(self, Self::Key(key) if key == other)
    }
}

impl ChooserWarning {
    /// The dialog's text.
    pub fn text(&self, res: &GameResources) -> String {
        match *self {
            Self::Key(key) => res.translate(key),
            Self::Upgrade { to, from } => res
                .translate("[SEED_CHOOSER_UPGRADE_WARNING]")
                .replace("{UPGRADE_TO}", &res.translate(&format!("[{}]", to.definition().name)))
                .replace("{UPGRADE_FROM}", &res.translate(&format!("[{}]", from.definition().name))),
        }
    }
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
    /// Whether the chooser has seven rows (`Has7Rows`), which packs them closer.
    seven_rows: bool,
    /// The imitater's panel is up, asking which plant to copy.
    imitater_panel: bool,
}

/// `SEED_CHOOSER_OFFSET_Y`: where the chooser waits, just off the bottom of the screen.
pub fn offset_y(res: &GameResources) -> i32 {
    res.image("IMAGE_SEEDCHOOSER_BACKGROUND").map_or(513, |i| i.height as i32)
}

/// The imitater's button (`mImitaterButton`): x, y.
const IMITATER_BUTTON: (i32, i32) = (464, 515);

/// `ImitaterDialog::GetSeedPosition`, in chooser coordinates (the panel sits in the middle).
fn imitater_panel_position(index: usize) -> (i32, i32) {
    ((index % 8) as i32 * (SEED_PACKET_WIDTH + 1) + 400 - 210, (index / 8) as i32 * (SEED_PACKET_HEIGHT + 1) + 190)
}

/// `GetSeedPositionInChooser`.
fn position_in_chooser(index: usize, seven_rows: bool) -> (i32, i32) {
    if index == SeedType::Imitater as usize {
        return IMITATER_BUTTON;
    }
    let (row, col) = ((index / 8) as i32, (index % 8) as i32);
    (col * 53 + 22, if seven_rows { row * 70 + 123 } else { row * 73 + 128 })
}

impl SeedChooser {
    pub fn new(owned: &[bool], seven_rows: bool, y: i32) -> Self {
        let seeds = (0..owned.len())
            .map(|i| {
                let (x, y) = position_in_chooser(i, seven_rows);
                let usable = owned[i] && i != SeedType::Imitater as usize;
                ChosenSeed {
                    x,
                    y,
                    start: (x, y),
                    end: (x, y),
                    time_start: 0,
                    time_end: 0,
                    state: if usable { ChosenSeedState::InChooser } else { ChosenSeedState::Hidden },
                    index_in_bank: 0,
                    imitating: None,
                    imitater_slot: i == SeedType::Imitater as usize,
                }
            })
            .collect();
        Self { seeds, age: 0, in_flight: 0, in_bank: 0, y, mouse_visible: false, warnings_accepted: 0, warning_answer: None, seven_rows, imitater_panel: false }
    }

    /// What's been picked: each seed's index and the plant it copies, if an imitater.
    #[cfg(test)]
    pub fn picked(&self) -> Vec<(usize, Option<SeedType>)> {
        self.seeds
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s.state, ChosenSeedState::InBank | ChosenSeedState::FlyingToBank))
            .map(|(i, s)| (i, s.imitating))
            .collect()
    }

    /// Whether the imitater's panel is up.
    #[cfg(test)]
    pub fn imitater_panel_open(&self) -> bool {
        self.imitater_panel
    }

    /// Fills the bank with the first `count` choosable seeds, for tests that
    /// need the chooser's Close path without clicking every packet.
    #[cfg(test)]
    pub fn fill_bank(&mut self, count: usize) {
        let mut filled = 0;
        for (index, seed) in self.seeds.iter_mut().enumerate() {
            if filled == count {
                break;
            }
            if seed.state != ChosenSeedState::InChooser || seed.imitater_slot {
                continue;
            }
            seed.state = ChosenSeedState::InBank;
            seed.index_in_bank = filled;
            filled += 1;
            let _ = index;
        }
        self.in_bank = filled;
    }

    /// `LandFlyingSeed`.
    fn land(seed: &mut ChosenSeed, in_flight: &mut i32) {
        match seed.state {
            ChosenSeedState::FlyingToBank => seed.state = ChosenSeedState::InBank,
            ChosenSeedState::FlyingToChooser if seed.imitater_slot => {
                // The imitater's packet goes back to its button.
                seed.state = ChosenSeedState::Hidden;
                seed.imitating = None;
            }
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
            s.state != ChosenSeedState::Hidden && p.x >= s.x && p.x < s.x + SEED_PACKET_WIDTH && p.y >= s.y && p.y < s.y + SEED_PACKET_HEIGHT
        })
    }
}

impl Board {
    /// `SeedChooserScreen::Has7Rows`: with the store's upgrades there are 48 plants, not 40.
    pub(super) fn has_seven_chooser_rows(&self) -> bool {
        let owned = |seed: SeedType| self.owned_seeds.get(seed as usize).copied().unwrap_or(false);
        self.finished_adventure
            || owned(SeedType::Gatlingpea)
            || (SeedType::Twinsunflower as usize..SeedType::Cobcannon as usize).any(|i| i != SeedType::Spikerock as usize && self.owned_seeds.get(i).copied().unwrap_or(false))
    }

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
    pub(super) fn chooser_warnings(&self, picked: impl Fn(SeedType) -> bool) -> Vec<ChooserWarning> {
        let mut warnings: Vec<ChooserWarning> = Vec::new();
        // `SeedChooserScreen::OnStartButton`: Seeing Stars needs a starfruit.
        if self.minigame() == Some(super::MiniGame::SeeingStars) && !picked(SeedType::Starfruit) {
            warnings.push("[SEED_CHOOSER_SEEING_STARS_WARNING]".into());
        }
        let night_one = self.first_time_adventure && self.level == 11;
        if night_one && !picked(SeedType::Puffshroom) {
            warnings.push("[SEED_CHOOSER_PUFFSHROOM_WARNING]".into());
        }
        if ![SeedType::Sunflower, SeedType::Twinsunflower, SeedType::Sunshroom].into_iter().any(&picked) {
            warnings.push((if night_one { "[SEED_CHOOSER_NIGHT_SUN_WARNING]" } else { "[SEED_CHOOSER_SUN_WARNING]" }).into());
        }
        if self.stage_has_pool() && ![SeedType::Lilypad, SeedType::Seashroom, SeedType::Tanglekelp].into_iter().any(&picked) {
            warnings.push((if self.first_time_adventure && self.level == 21 {
                "[SEED_CHOOSER_LILY_WARNING]"
            } else {
                "[SEED_CHOOSER_POOL_WARNING]"
            }).into());
        }
        if self.stage_has_roof() && !picked(SeedType::Flowerpot) && self.owned_seeds[SeedType::Flowerpot as usize] {
            warnings.push("[SEED_CHOOSER_ROOF_WARNING]".into());
        }
        // Behaviour per ruslan831 decomp, SeedChooserScreen::FlyersAreComming
        // and FlyProtectionCurrentlyPlanted.
        let flyers = self.waves.zombies_in_wave.iter().flatten().any(|&z| z == ZombieType::Balloon);
        let protected = |seed: SeedType| picked(seed) || self.plants.iter().any(|(_, p)| p.seed == seed);
        if flyers && ![SeedType::Cattail, SeedType::Cactus].into_iter().any(protected) && !picked(SeedType::Blover) {
            warnings.push("[SEED_CHOOSER_FLYER_WARNING]".into());
        }
        // `CheckSeedUpgrade`, in the original's order.
        use SeedType::*;
        for to in [Gatlingpea, Wintermelon, Twinsunflower, Spikerock, Cobcannon, GoldMagnet, Gloomshroom, Cattail] {
            if let Some(from) = to.upgrade_base()
                && picked(to)
                && !picked(from)
            {
                warnings.push(ChooserWarning::Upgrade { to, from });
            }
        }
        warnings
    }

    /// `OnStartButton`: once the player's answered every warning with
    /// "yes", play starts. Returns the next warning to ask, if any.
    pub(super) fn seed_chooser_start(&mut self) -> Option<ChooserWarning> {
        let chooser = self.seed_chooser.as_ref()?;
        if chooser.in_bank != self.seed_bank.num_packets {
            return None;
        }
        let in_bank = |s: &ChosenSeed| matches!(s.state, ChosenSeedState::InBank | ChosenSeedState::FlyingToBank);
        // A plant copied by an imitater counts as picked.
        let picked = |seed: SeedType| {
            chooser.seeds.get(seed as usize).is_some_and(in_bank) || chooser.seeds.iter().any(|s| in_bank(s) && s.imitating == Some(seed))
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
    pub(super) fn seed_chooser_warning_answered(&mut self) -> Option<ChooserWarning> {
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
    pub(super) fn seed_chooser_mouse_down(&mut self, p: IVec2, sounds: &mut Sounds) -> Option<ChooserClick> {
        let num_packets = self.seed_bank.num_packets;
        let chooser = self.seed_chooser.as_mut().filter(|c| c.mouse_visible)?;
        chooser.land_all();
        // The chooser's own coordinates.
        let p = p - IVec2::new(0, chooser.y);

        let seven_rows = chooser.seven_rows;
        // The imitater's panel: pick a plant to copy, or click away.
        if chooser.imitater_panel {
            chooser.imitater_panel = false;
            let picked = (0..SeedType::Gatlingpea as usize).find(|&i| {
                let (x, y) = imitater_panel_position(i);
                self.owned_seeds[i] && p.x >= x && p.x < x + SEED_PACKET_WIDTH && p.y >= y && p.y < y + SEED_PACKET_HEIGHT
            });
            let chooser = self.seed_chooser.as_mut().expect("chooser is open");
            let slot = SeedType::Imitater as usize;
            let seed = picked.and_then(|i| SeedType::from_index(i as i32))?;
            if chooser.in_bank == num_packets {
                return None;
            }
            // `ImitaterDialog::MouseDown`: the imitater packet flies into the bank.
            let (age, chooser_y, bank_slot) = (chooser.age, chooser.y, chooser.in_bank);
            let end = self.seed_position_in_bank(chooser_y, bank_slot);
            let chooser = self.seed_chooser.as_mut().expect("chooser is open");
            let imitater = &mut chooser.seeds[slot];
            imitater.imitating = Some(seed);
            (imitater.x, imitater.y) = IMITATER_BUTTON;
            imitater.time_start = age;
            imitater.time_end = age + 25;
            imitater.start = (imitater.x, imitater.y);
            imitater.end = end;
            imitater.state = ChosenSeedState::FlyingToBank;
            imitater.index_in_bank = bank_slot;
            chooser.in_flight += 1;
            chooser.in_bank += 1;
            sounds.play_sample("SOUND_TAP");
            return None;
        }

        // `CanShowAlmanac`: the Almanac button.
        if (!self.first_time_adventure || self.level >= 15)
            && p.x >= ALMANAC_BUTTON.0
            && p.x < ALMANAC_BUTTON.0 + ALMANAC_BUTTON_SIZE.0
            && p.y >= ALMANAC_BUTTON.1
            && p.y < ALMANAC_BUTTON.1 + ALMANAC_BUTTON_SIZE.1
        {
            sounds.play_sample("SOUND_TAP");
            return Some(ChooserClick::Almanac);
        }
        let (bx, by, bw, bh) = START_BUTTON;
        if p.x >= bx && p.x < bx + bw && p.y >= by && p.y < by + bh {
            sounds.play_sample("SOUND_TAP");
            chooser.warnings_accepted = 0;
            return self.seed_chooser_start().map(ChooserClick::Warning);
        }
        // The imitater's button opens its panel, once it has been bought and isn't in use.
        let (ix, iy) = IMITATER_BUTTON;
        if p.x >= ix
            && p.x < ix + SEED_PACKET_WIDTH
            && p.y >= iy
            && p.y < iy + SEED_PACKET_HEIGHT
            && self.owned_seeds[SeedType::Imitater as usize]
            && chooser.seeds[SeedType::Imitater as usize].state == ChosenSeedState::Hidden
        {
            chooser.imitater_panel = true;
            sounds.play_sample("SOUND_TAP");
            return None;
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
                seed.end = position_in_chooser(index, seven_rows);
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
            let index = chooser.seed_in_bank(slot);
            let copied = index.and_then(|i| chooser.seeds[i].imitating);
            // An imitater's packet is the copied plant's, marked as a copy.
            let seed = copied.or_else(|| index.and_then(|i| SeedType::from_index(i as i32)));
            self.seed_bank.packets[slot].set_packet_type(seed);
            self.seed_bank.packets[slot].imitater = copied.is_some();
        }
        chooser.mouse_visible = false;
        self.cut_scene.end_seed_choosing();
        // `CutScene::IsSurvivalRepick` ends here; the board starts the stage
        // on its next tick (it has the assets for the flash).
        if self.survival_repick {
            self.survival_repick = false;
            self.seed_chooser = None;
            self.cut_scene.seed_choosing = false;
            self.pending_survival_repick_start = true;
        }
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

        // The imitater's add-on, once it has been bought.
        let imitater_owned = self.owned_seeds[SeedType::Imitater as usize];
        if imitater_owned && let Some(addon) = res.image("IMAGE_SEEDCHOOSER_IMITATERADDON") {
            g.draw_image(addon, 459.0, 503.0);
        }
        // The slots: a dim copy of a plant that's been taken, a silhouette for the rest.
        let num_slots = if chooser.seven_rows { 48 } else { 40 };
        for index in 0..num_slots {
            let (x, y) = position_in_chooser(index, chooser.seven_rows);
            let Some(s) = SeedType::from_index(index as i32) else { continue };
            if self.owned_seeds[index] {
                if chooser.seeds[index].state != ChosenSeedState::InChooser {
                    draw_seed_packet(g, res, x as f32, y as f32, s, 0.0, 55, true);
                }
            } else if let Some(silhouette) = res.image("IMAGE_SEEDPACKETSILHOUETTE") {
                g.draw_image(silhouette, x as f32, y as f32);
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
                if seed.state == ChosenSeedState::Hidden || flying(seed) != pass_flying || (!pass_flying && seed.state != ChosenSeedState::InChooser && !choosing) {
                    continue;
                }
                if let Some(ty) = SeedType::from_index(index as i32) {
                    // An imitater's packet shows the plant it copies.
                    let (shown, copy) = match seed.imitating {
                        Some(copied) => (copied, true),
                        None => (ty, false),
                    };
                    let grayed = seed.state == ChosenSeedState::InChooser && self.seed_not_recommended(ty).is_some();
                    draw_seed_packet_as(g, res, seed.x as f32, seed.y as f32, shown, copy, 0.0, if grayed { 115 } else { 255 }, true);
                }
            }
        }
        // The imitater's button: it grays out while its packet is in use.
        if imitater_owned {
            let in_use = chooser.seeds[SeedType::Imitater as usize].state != ChosenSeedState::Hidden;
            let image = if in_use { "IMAGE_IMITATERSEEDDISABLED" } else { "IMAGE_IMITATERSEED" };
            if let Some(button) = res.image(image) {
                g.draw_image(button, IMITATER_BUTTON.0 as f32, IMITATER_BUTTON.1 as f32);
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
        // `CanShowAlmanac`: the Almanac button, bottom right.
        if (!self.first_time_adventure || self.level >= 15)
            && let Some(button) = res.image("IMAGE_SEEDCHOOSER_BUTTON2")
        {
            let (ax, ay) = ALMANAC_BUTTON;
            g.draw_image(button, ax as f32, ay as f32);
            let over = mouse.is_some_and(|(mx, my)| {
                mx >= ax as f32
                    && mx < (ax + ALMANAC_BUTTON_SIZE.0) as f32
                    && my >= ay as f32
                    && my < (ay + ALMANAC_BUTTON_SIZE.1) as f32
            });
            if over
                && chooser.mouse_visible
                && let Some(glow) = res.image("IMAGE_SEEDCHOOSER_BUTTON2_GLOW")
            {
                g.draw_image(glow, ax as f32, ay as f32);
            }
            let label = res.translate("[ALMANAC_BUTTON]");
            if let Some(font) = res.font("FONT_BRIANNETOD12") {
                let ascent = font.font.ascent;
                let y = ay + (ALMANAC_BUTTON_SIZE.1 - ascent / 6 - 1 + ascent) / 2 - 1;
                g.tod_draw_string(font, ax + ALMANAC_BUTTON_SIZE.0 / 2, y, &label, SexyColor::rgba(42, 42, 90, 255), Justify::Center);
            }
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
        // The imitater's panel: every plant it can copy.
        if chooser.imitater_panel {
            g.set_color(SexyColor::rgba(0, 0, 0, 140));
            g.fill_rect(0.0, 87.0, 800.0, 513.0);
            g.set_color(SexyColor::rgba(70, 45, 20, 255));
            g.fill_rect(150.0, 140.0, 500.0, 330.0);
            g.set_color(SexyColor::rgba(230, 200, 120, 255));
            g.draw_rect(150.0, 140.0, 500.0, 330.0);
            let title = res.translate("[CHOOSE_SEED_TO_COPY]");
            if let Some(font) = res.font("FONT_DWARVENTODCRAFT18YELLOW") {
                g.tod_draw_string(font, 400, 170, &title, SexyColor::WHITE, Justify::Center);
            }
            for index in 0..SeedType::Gatlingpea as usize {
                if self.owned_seeds[index]
                    && let Some(seed) = SeedType::from_index(index as i32)
                {
                    let (x, y) = imitater_panel_position(index);
                    let grayed = self.seed_not_recommended(seed).is_some();
                    draw_seed_packet_as(g, res, x as f32, y as f32, seed, true, 0.0, if grayed { 115 } else { 255 }, true);
                }
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
