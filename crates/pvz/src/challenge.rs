//! `ChallengeScreen`'s survival page, opened from the main menu's Survival
//! button, and the survival records that move with the player's profile.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Widget/ChallengeScreen.cpp`
//! (`gChallengeDefs`' survival entries, `AccomplishmentsNeeded`,
//! `MoreTrophiesNeeded`, `DrawButton`, `Draw`) and `LawnApp.cpp`
//! (`ShowChallengeScreen`, `HasBeatenChallenge`, `GetNumTrophies`).
//!
//! Not yet: the mini-game, puzzle and Limbo pages, the page buttons, the
//! unlock shake/fade animation, tooltips, the challenge records used by the
//! other pages, and the endless "longest streak" label's exact wrapping.
//! The endless tiles live on this page instead of Limbo so that all 15
//! survival modes are reachable.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::audio::{Music, MusicTune, Sounds};
use crate::board::{GameMode, SelectedMode, SurvivalDifficulty, SurvivalLawn};
use crate::graphics::{Graphics, Justify, SexyColor, SexyRect};
use crate::input::GameCursor;
use crate::install::userdata_dir;
use crate::player::ProfileMgr;
use crate::resources::GameResources;
use crate::widgets::LabelButton;
use crate::{AppState, UiSet};

/// A survival tile's frame, from `ChallengeScreen`'s button layout (the
/// survival page sits one row lower than the challenge page).
const TILE: (f32, f32, f32, f32) = (38.0, 125.0, 104.0, 115.0);
const COLUMN_STEP: f32 = 155.0;
const ROW_STEP: f32 = 145.0;
const COLUMNS: i32 = 5;
/// The survival page's trophies: the normal and hard lawns.
const TROPHIES_TOTAL: i32 = 10;

pub struct ChallengePlugin;

impl Plugin for ChallengePlugin {
    fn build(&self, app: &mut App) {
        let open = resource_exists::<ChallengeScreen>;
        app.add_systems(OnEnter(AppState::Challenges), start_challenge_screen)
            .add_systems(OnExit(AppState::Challenges), end_challenge_screen)
            .add_systems(Update, challenge_input.in_set(UiSet::ScreenInput).run_if(open))
            .add_systems(Update, draw_challenge_screen.in_set(UiSet::ScreenDraw).run_if(open));
    }
}

/// The survival page of the challenge screen.
#[derive(Resource)]
pub struct ChallengeScreen {
    progress: ChallengeProgress,
    back: LabelButton,
}

/// What the player has done in survival (`mChallengeRecords` for these
/// modes, kept in `userdata/challenge{id}.dat`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ChallengeProgress {
    /// The normal and hard lawns won: `[difficulty][lawn]`.
    beaten: [[bool; 5]; 2],
    /// The most flags reached on each endless lawn.
    endless_best: [i32; 5],
}

impl ChallengeProgress {
    fn path(dir: &Path, id: u32) -> PathBuf {
        dir.join(format!("challenge{id}.dat"))
    }

    /// Reads the player's records; a missing or broken file is a blank slate.
    pub fn load(dir: &Path, id: u32) -> Self {
        std::fs::read_to_string(Self::path(dir, id)).ok().and_then(|text| toml::from_str(&text).ok()).unwrap_or_default()
    }

    pub fn save(&self, dir: &Path, id: u32) {
        let Ok(text) = toml::to_string(self) else { return };
        if let Err(e) = std::fs::create_dir_all(dir) {
            error!("couldn't make the userdata directory: {e}");
            return;
        }
        if let Err(e) = std::fs::write(Self::path(dir, id), text) {
            error!("couldn't save the survival records: {e}");
        }
    }

    /// `LawnApp::GetNumTrophies(CHALLENGE_PAGE_SURVIVAL)`.
    pub fn trophies(&self) -> i32 {
        self.beaten.iter().flatten().filter(|&&won| won).count() as i32
    }

    pub fn has_beaten(&self, lawn: SurvivalLawn, difficulty: SurvivalDifficulty) -> bool {
        match difficulty {
            SurvivalDifficulty::Normal | SurvivalDifficulty::Hard => self.beaten[difficulty.index() as usize][lawn.index() as usize],
            SurvivalDifficulty::Endless => false,
        }
    }

    /// A won stage: normal and hard runs earn their lawn's trophy; endless
    /// runs remember their best flags.
    pub fn record(&mut self, lawn: SurvivalLawn, difficulty: SurvivalDifficulty, flags: i32) {
        let index = lawn.index() as usize;
        match difficulty {
            SurvivalDifficulty::Normal | SurvivalDifficulty::Hard => {
                self.beaten[difficulty.index() as usize][index] = true;
            }
            SurvivalDifficulty::Endless => {
                self.endless_best[index] = self.endless_best[index].max(flags);
            }
        }
    }

    pub fn endless_best(&self, lawn: SurvivalLawn) -> i32 {
        self.endless_best[lawn.index() as usize]
    }

    /// `ChallengeScreen::AccomplishmentsNeeded` for a survival tile: how
    /// many more trophies it wants (0 means playable).
    fn accomplishments_needed(&self, index: i32, finished_adventure: bool) -> i32 {
        let endless = index >= 2 * COLUMNS;
        if endless {
            return (TROPHIES_TOTAL - self.trophies()).max(0);
        }
        if !finished_adventure {
            return match index {
                ..3 => 0,
                3 => 1,
                _ => 2,
            };
        }
        // Each trophy moves the free window one tile along.
        let owed = self.trophies() + 3;
        if index >= owed { index - owed + 1 } else { 0 }
    }
}

/// The survival mode a tile stands for.
fn mode_for(index: i32) -> Option<(SurvivalLawn, SurvivalDifficulty)> {
    let difficulty = SurvivalDifficulty::from_index(index / COLUMNS)?;
    let lawn = SurvivalLawn::from_index(index % COLUMNS)?;
    Some((lawn, difficulty))
}

fn tile_pos(index: i32) -> (f32, f32) {
    (TILE.0 + (index % COLUMNS) as f32 * COLUMN_STEP, TILE.1 + (index / COLUMNS) as f32 * ROW_STEP)
}

/// Records a won survival run for the current player.
pub(crate) fn record_survival_win(profiles: &ProfileMgr, board: &crate::board::Board) {
    let (Some((lawn, difficulty)), Some(player), Some(dir)) = (board.survival(), profiles.player(), userdata_dir()) else { return };
    let mut progress = ChallengeProgress::load(&dir, player.id);
    progress.record(lawn, difficulty, board.survival_flags_completed());
    progress.save(&dir, player.id);
}

/// Records where an endless run got to, when it's over.
pub(crate) fn record_endless_run(profiles: &ProfileMgr, board: &crate::board::Board) {
    let (Some((lawn, difficulty)), Some(player), Some(dir)) = (board.survival(), profiles.player(), userdata_dir()) else { return };
    if difficulty != SurvivalDifficulty::Endless {
        return;
    }
    let mut progress = ChallengeProgress::load(&dir, player.id);
    progress.record(lawn, difficulty, board.survival_flags_completed());
    progress.save(&dir, player.id);
}

/// `LawnApp::ShowChallengeScreen(CHALLENGE_PAGE_SURVIVAL)`.
fn start_challenge_screen(
    mut commands: Commands,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut music: ResMut<Music>,
    profiles: Res<ProfileMgr>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let Some(player) = profiles.player() else {
        next_state.set(AppState::GameSelector);
        return;
    };
    for id in [
        "IMAGE_CHALLENGE_BACKGROUND",
        "IMAGE_SURVIVAL_THUMBNAILS",
        "IMAGE_CHALLENGE_WINDOW",
        "IMAGE_CHALLENGE_WINDOW_HIGHLIGHT",
        "IMAGE_CHALLENGE_BLANK",
        "IMAGE_MINIGAME_TROPHY",
        "IMAGE_LOCK",
        "IMAGE_TROPHY",
        "IMAGE_SEEDCHOOSER_BUTTON2",
        "IMAGE_SEEDCHOOSER_BUTTON2_GLOW",
    ] {
        if let Err(e) = resources.load_image(id, &mut images) {
            error!("{e}");
        }
    }
    music.make_sure_music_is_playing(MusicTune::ChooseYourSeeds);
    let mut back = LabelButton::new(resources.translate("[BACK_TO_MENU]"), "FONT_BRIANNETOD12", SexyColor::rgba(42, 42, 90, 255), "IMAGE_SEEDCHOOSER_BUTTON2");
    back.hilite_color = SexyColor::rgba(42, 42, 90, 255);
    back.over_image = Some("IMAGE_SEEDCHOOSER_BUTTON2_GLOW");
    back.text_offset = (1, 1);
    let (w, h) = resources.image("IMAGE_SEEDCHOOSER_BUTTON2").map_or((111, 26), |i| (i.width as i32, i.height as i32));
    back.resize(18, 568, w, h);
    let progress = userdata_dir().map_or_else(ChallengeProgress::default, |dir| ChallengeProgress::load(&dir, player.id));
    commands.insert_resource(ChallengeScreen { progress, back });
}

fn end_challenge_screen(mut commands: Commands) {
    commands.remove_resource::<ChallengeScreen>();
}

fn challenge_input(
    mut commands: Commands,
    mut screen: ResMut<ChallengeScreen>,
    cursor: Res<GameCursor>,
    mouse: Res<ButtonInput<MouseButton>>,
    profiles: Res<ProfileMgr>,
    mut next_state: ResMut<NextState<AppState>>,
    mut sounds: Sounds,
) {
    let Some(player) = profiles.player() else { return };
    let screen = &mut *screen;
    let finished = player.has_finished_adventure();
    let over_back = screen.back.contains(cursor.0.unwrap_or_default().as_ivec2());
    screen.back.is_over = over_back;
    if mouse.just_pressed(MouseButton::Left) && over_back {
        screen.back.is_down = true;
        sounds.play_sample("SOUND_BUTTONCLICK");
    }
    if mouse.just_released(MouseButton::Left) {
        if screen.back.is_down && over_back {
            next_state.set(AppState::GameSelector);
        }
        screen.back.is_down = false;
    }
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(point) = cursor.0 else { return };
    for index in 0..(3 * COLUMNS) {
        let (x, y) = tile_pos(index);
        if point.x < x || point.y < y || point.x >= x + TILE.2 || point.y >= y + TILE.3 {
            continue;
        }
        if screen.progress.accomplishments_needed(index, finished) > 0 {
            return;
        }
        let Some((lawn, difficulty)) = mode_for(index) else { return };
        sounds.play_sample("SOUND_GRAVEBUTTON");
        commands.insert_resource(SelectedMode(Some(GameMode::Survival { lawn, difficulty })));
        next_state.set(AppState::Board);
        return;
    }
}

fn draw_challenge_screen(
    screen: Res<ChallengeScreen>,
    resources: Res<GameResources>,
    cursor: Res<GameCursor>,
    profiles: Res<ProfileMgr>,
    mut g: ResMut<Graphics>,
) {
    let res = &*resources;
    let g = &mut *g;
    let Some(player) = profiles.player() else { return };
    let finished = player.has_finished_adventure();
    let image = |g: &mut Graphics, id: &str, x: f32, y: f32| {
        if let Some(image) = res.image(id) {
            g.draw_image(image, x, y);
        }
    };
    image(g, "IMAGE_CHALLENGE_BACKGROUND", 0.0, 0.0);
    // `ChallengeScreen::Draw`: the survival page's title and trophy tally.
    if let Some(font) = res.font("FONT_HOUSEOFTERROR28") {
        g.tod_draw_string(font, 400, 58, &res.translate("[PICK_AREA]"), SexyColor::rgba(220, 220, 220, 255), Justify::Center);
    }
    if let Some(font) = res.font("FONT_DWARVENTODCRAFT12") {
        let tally = format!("{}/{}", screen.progress.trophies(), TROPHIES_TOTAL);
        g.tod_draw_string(font, 739, 73, &tally, SexyColor::rgba(255, 240, 0, 255), Justify::Center);
    }
    if let Some(trophy) = res.image("IMAGE_TROPHY") {
        g.draw_image_scaled(trophy, 718.0, 26.0, 0.5, 0.5);
    }

    let mouse = cursor.0;
    for index in 0..(3 * COLUMNS) {
        let Some((lawn, difficulty)) = mode_for(index) else { continue };
        let (x, y) = tile_pos(index);
        let needed = screen.progress.accomplishments_needed(index, finished);
        if needed > 1 {
            // Too far away to show yet: a blank frame.
            image(g, "IMAGE_CHALLENGE_BLANK", x, y);
            continue;
        }
        let over = mouse.is_some_and(|p| p.x >= x && p.y >= y && p.x < x + TILE.2 && p.y < y + TILE.3);
        let locked = needed == 1;
        // The thumbnail, greyed out while the tile is locked.
        if let Some(thumbs) = res.image("IMAGE_SURVIVAL_THUMBNAILS") {
            if locked || needed > 0 {
                g.push_state();
                g.set_colorize_images(true);
                g.set_color(SexyColor::rgba(92, 92, 92, 255));
            }
            g.draw_image_cel(thumbs, x + 13.0, y + 4.0, lawn.index() as u32, 0);
            if locked || needed > 0 {
                g.pop_state();
            }
        }
        image(g, if over { "IMAGE_CHALLENGE_WINDOW" } else { "IMAGE_CHALLENGE_WINDOW_HIGHLIGHT" }, x - 6.0, y - 2.0);
        if let Some(font) = res.font("FONT_BRIANNETOD12") {
            let name = if locked { "?".to_owned() } else { res.translate(lawn.name(difficulty)) };
            draw_tile_name(g, font, &name, x, y);
        }
        let beaten = screen.progress.has_beaten(lawn, difficulty);
        if beaten {
            image(g, "IMAGE_MINIGAME_TROPHY", x - 6.0, y - 2.0);
        } else if locked {
            if let Some(lock) = res.image("IMAGE_LOCK") {
                g.draw_image_scaled(lock, x + 24.0, y + 9.0, 0.7, 0.7);
            }
        } else if difficulty == SurvivalDifficulty::Endless {
            let best = screen.progress.endless_best(lawn);
            if best > 0
                && let Some(font) = res.font("FONT_CONTINUUMBOLD14")
            {
                let streak = res.translate("[LONGEST_STREAK]").replace("{STREAK}", &best.to_string());
                let rect = SexyRect::new(x, y + 15.0, 96.0, 200.0);
                g.tod_draw_string_wrapped(font, rect, &streak, SexyColor::WHITE, Justify::CenterMiddle);
            }
        }
    }
    screen.back.draw(g, res);
}

/// `ChallengeScreen::DrawButton`'s name: one line when short, otherwise
/// two lines split at the first space past the middle.
fn draw_tile_name(g: &mut Graphics, font: &crate::resources::FontRes, name: &str, x: f32, y: f32) {
    let color = SexyColor::rgba(42, 42, 90, 255);
    if name.chars().count() < 13 {
        g.tod_draw_string(font, (x + 52.0) as i32, (y + 96.0) as i32, name, color, Justify::Center);
        return;
    }
    let chars: Vec<char> = name.chars().collect();
    let half = if chars.len() > 7 { 7 } else { chars.len() / 2 };
    let split = chars[half..].iter().position(|c| *c == ' ').map(|i| half + i).or_else(|| chars.iter().position(|c| *c == ' '));
    let Some(split) = split else {
        g.tod_draw_string(font, (x + 52.0) as i32, (y + 96.0) as i32, name, color, Justify::Center);
        return;
    };
    let (first, second): (String, String) = (chars[..split].iter().collect(), chars[split + 1..].iter().collect());
    g.tod_draw_string(font, (x + 52.0) as i32, (y + 88.0) as i32, &first, color, Justify::Center);
    if !second.is_empty() {
        g.tod_draw_string(font, (x + 52.0) as i32, (y + 102.0) as i32, &second, color, Justify::Center);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_three_tiles_are_free_and_each_trophy_unlocks_one_more() {
        let mut progress = ChallengeProgress::default();
        assert_eq!(progress.accomplishments_needed(0, true), 0);
        assert_eq!(progress.accomplishments_needed(2, true), 0);
        assert_eq!(progress.accomplishments_needed(3, true), 1);
        assert_eq!(progress.accomplishments_needed(4, true), 2);
        progress.record(SurvivalLawn::Day, SurvivalDifficulty::Normal, 5);
        assert_eq!(progress.accomplishments_needed(3, true), 0);
        assert_eq!(progress.accomplishments_needed(4, true), 1);
        // Endless waits for all ten trophies; the one won shaves a trophy
        // off what it still needs.
        assert_eq!(progress.accomplishments_needed(10, true), 9);
    }

    #[test]
    fn a_won_lawn_keeps_its_trophy_and_the_records_round_trip() {
        let dir = std::env::temp_dir().join(format!("pvz-challenge-{}", std::process::id()));
        let mut progress = ChallengeProgress::default();
        progress.record(SurvivalLawn::Night, SurvivalDifficulty::Hard, 10);
        progress.record(SurvivalLawn::Pool, SurvivalDifficulty::Endless, 7);
        progress.save(&dir, 3);
        let read = ChallengeProgress::load(&dir, 3);
        assert!(read.has_beaten(SurvivalLawn::Night, SurvivalDifficulty::Hard));
        assert!(!read.has_beaten(SurvivalLawn::Night, SurvivalDifficulty::Normal));
        assert_eq!(read.endless_best(SurvivalLawn::Pool), 7);
        assert_eq!(read.trophies(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
