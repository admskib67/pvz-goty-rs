//! The award screen shown between adventure levels: the new plant (or the
//! shovel, a note, ...) the player just won, with the button to carry on.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Widget/AwardScreen.cpp`:
//! `AwardScreen`, `DrawBottom`, `DrawAwardSeed`, `Draw`, `Update`,
//! `StartButtonPressed`, `MouseDown` and `MouseUp`. Not yet: the trophy and
//! money-bag screens, the credits note, the almanac and store the original
//! opens from some levels.

use bevy::math::Affine2;
use bevy::prelude::*;

use crate::audio::{Music, MusicTune, Sounds};
use crate::graphics::{Graphics, Justify, SexyColor, SexyRect};
use crate::input::GameCursor;
use crate::plant::{SeedType, award_seed_for_level};
use crate::resources::GameResources;
use crate::{AppState, UiSet};

pub struct AwardPlugin;

impl Plugin for AwardPlugin {
    fn build(&self, app: &mut App) {
        let open = resource_exists::<AwardScreen>;
        app.add_systems(OnEnter(AppState::Award), start_award)
            .add_systems(OnExit(AppState::Award), |mut commands: Commands| commands.remove_resource::<AwardScreen>())
            .add_systems(FixedUpdate, update_award.run_if(open))
            .add_systems(Update, award_input.in_set(UiSet::ScreenInput).run_if(open))
            .add_systems(Update, draw_award.in_set(UiSet::ScreenDraw).run_if(open));
    }
}

/// What the screen shows, from the level the player is now on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AwardKind {
    Seed(SeedType),
    Shovel,
    /// A zombie's note (levels 10, 20, 30, 40, 50), numbered 1 to 5.
    Note(i32),
    Almanac,
    Keys,
    Taco,
    WateringCan,
}

impl AwardKind {
    /// `level` is the player's level after the one just won.
    fn for_level(level: i32) -> Option<Self> {
        Some(match level {
            5 => Self::Shovel,
            10 | 20 | 30 | 40 | 50 => Self::Note(level / 10),
            15 => Self::Almanac,
            25 => Self::Keys,
            35 => Self::Taco,
            45 => Self::WateringCan,
            _ => Self::Seed(SeedType::from_index(award_seed_for_level(level - 1))?),
        })
    }

    fn is_paper_note(self) -> bool {
        matches!(self, Self::Note(_))
    }
}

/// `AwardScreen`. Inserted by the board when a level is won.
#[derive(Resource)]
pub struct AwardScreen {
    /// The player's level now (`mPlayerInfo->GetLevel()`).
    pub level: i32,
    kind: AwardKind,
    /// `mFadeInCounter`: the white (or, for notes, black) fade-in.
    fade_in: i32,
    start_over: bool,
    menu_over: bool,
}

impl AwardScreen {
    /// `None` when there's nothing to show for this level.
    pub fn new(level: i32) -> Option<Self> {
        let kind = AwardKind::for_level(level)?;
        Some(Self { level, kind, fade_in: 180, start_over: false, menu_over: false })
    }

    fn menu_visible(&self) -> bool {
        // Hidden for the first few levels.
        self.level > 3
    }

    /// `mStartButton`: x, y, width, height.
    fn start_rect(&self) -> (f32, f32, f32, f32) {
        (324.0, if self.kind.is_paper_note() { 520.0 } else { 500.0 }, 156.0, 42.0)
    }

    const MENU_RECT: (f32, f32, f32, f32) = (677.0, 16.0, 111.0, 26.0);

    fn start_label(&self) -> &'static str {
        match self.level {
            15 => "[VIEW_ALMANAC_BUTTON]",
            25 | 35 | 45 => "[CONTINUE_BUTTON]",
            _ => "[NEXT_LEVEL_BUTTON]",
        }
    }
}

fn start_award(
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut music: ResMut<Music>,
    screen: Option<Res<AwardScreen>>,
    mut sounds: Sounds,
) {
    let Some(screen) = screen else { return };
    let mut groups = vec!["DelayLoad_AwardScreen".to_owned()];
    if let AwardKind::Note(n) = screen.kind {
        groups.push(if n % 2 == 1 { "DelayLoad_Background1" } else { "DelayLoad_Background2" }.to_owned());
        groups.push("DelayLoad_ZombieNote".to_owned());
        groups.push(if n == 5 { "DelayLoad_ZombieFinalNote".to_owned() } else { format!("DelayLoad_ZombieNote{n}") });
    }
    for group in groups {
        if let Err(e) = resources.load_group_now(&group, &mut images) {
            error!("{e}");
        }
    }
    if let AwardKind::Seed(seed) = screen.kind
        && let Err(e) = resources.reanim_definition(seed.definition().reanim.file(), &mut images)
    {
        error!("{e}");
    }
    if screen.kind.is_paper_note() {
        music.stop_all_music();
        sounds.play_sample("SOUND_PAPER");
    } else {
        music.make_sure_music_is_playing(MusicTune::ZenGarden);
    }
}

fn update_award(mut screen: ResMut<AwardScreen>) {
    if screen.fade_in > 0 {
        screen.fade_in -= 1;
    }
}

fn in_rect(p: Vec2, (x, y, w, h): (f32, f32, f32, f32)) -> bool {
    p.x >= x && p.x < x + w && p.y >= y && p.y < y + h
}

/// `AwardScreen::MouseDown`, `MouseUp` and `KeyChar`.
fn award_input(
    mut screen: ResMut<AwardScreen>,
    cursor: Res<GameCursor>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut sounds: Sounds,
) {
    let (start, menu_rect) = (screen.start_rect(), AwardScreen::MENU_RECT);
    screen.start_over = cursor.0.is_some_and(|p| in_rect(p, start));
    screen.menu_over = screen.menu_visible() && cursor.0.is_some_and(|p| in_rect(p, menu_rect));
    if mouse.just_pressed(MouseButton::Left) && (screen.start_over || screen.menu_over) {
        sounds.play_sample("SOUND_TAP");
    }
    if mouse.just_released(MouseButton::Left) {
        if screen.start_over {
            next_state.set(AppState::Board);
        } else if screen.menu_over {
            next_state.set(AppState::GameSelector);
        }
    }
    if keys.any_just_pressed([KeyCode::Space, KeyCode::Enter, KeyCode::Escape]) {
        next_state.set(AppState::Board);
    }
}

/// `DrawBottom`: the picture's frame, title, name and description.
fn draw_bottom(g: &mut Graphics, res: &GameResources, title: &str, award: &str, message: &str) {
    if let Some(back) = res.image("IMAGE_AWARDSCREEN_BACK") {
        g.draw_image(back, 0.0, 0.0);
    }
    if let Some(font) = res.font("FONT_DWARVENTODCRAFT24") {
        g.tod_draw_string(font, 400, 58, title, SexyColor::rgba(213, 159, 43, 255), Justify::Center);
    }
    if let Some(font) = res.font("FONT_DWARVENTODCRAFT18YELLOW") {
        g.tod_draw_string(font, 400, 326, award, SexyColor::WHITE, Justify::Center);
    }
    if let Some(font) = res.font("FONT_BRIANNETOD16") {
        g.tod_draw_string_wrapped(
            font,
            SexyRect::new(285.0, 360.0, 230.0, 90.0),
            message,
            SexyColor::rgba(40, 50, 90, 255),
            Justify::CenterMiddle,
        );
    }
}

/// An image stretched to a rectangle.
fn draw_stretched(g: &mut Graphics, res: &GameResources, id: &str, x: f32, y: f32, w: f32, h: f32) {
    let Some(image) = res.image(id) else { return };
    let (sx, sy) = (w / image.width as f32, h / image.height as f32);
    g.draw_image_matrix(image, Affine2::from_cols_array(&[sx, 0.0, 0.0, sy, x + w / 2.0, y + h / 2.0]));
}

fn draw_item(g: &mut Graphics, res: &GameResources, id: &str, y: f32) {
    if let Some(image) = res.image(id) {
        g.draw_image(image, 400.0 - image.width as f32 / 2.0, y);
    }
}

fn draw_award(screen: Res<AwardScreen>, resources: Res<GameResources>, mut g: ResMut<Graphics>) {
    let (res, g) = (&*resources, &mut *g);
    let tr = |key: &str| res.translate(key);
    match screen.kind {
        AwardKind::Seed(seed) => {
            let name = tr(&format!("[{}]", seed.definition().name));
            let message = tr(&format!("[{}_TOOLTIP]", seed.definition().name));
            draw_bottom(g, res, &tr("[NEW_PLANT]"), &name, &message);
            g.push_state();
            g.set_scale(2.0, 350.0, 129.0);
            crate::board::draw_seed_packet_at(g, res, 350.0, 129.0, seed);
            g.pop_state();
        }
        AwardKind::Shovel => {
            draw_bottom(g, res, &tr("[GOT_SHOVEL]"), &tr("[SHOVEL]"), &tr("[SHOVEL_DESCRIPTION]"));
            draw_item(g, res, "IMAGE_SHOVEL_HI_RES", 137.0);
        }
        AwardKind::Almanac => {
            draw_bottom(g, res, &tr("[FOUND_SUBURBAN_ALMANAC]"), &tr("[SUBURBAN_ALMANAC]"), &tr("[SUBURBAN_ALMANAC_DESCRIPTION]"));
            draw_item(g, res, "IMAGE_ALMANAC", 160.0);
        }
        AwardKind::Keys => {
            draw_bottom(g, res, &tr("[FOUND_KEYS]"), &tr("[KEYS]"), &tr("[KEYS_DESCRIPTION]"));
            draw_item(g, res, "IMAGE_CARKEYS", 160.0);
        }
        AwardKind::Taco => {
            draw_bottom(g, res, &tr("[FOUND_TACO]"), &tr("[TACO]"), &tr("[TACO_DESCRIPTION]"));
            draw_item(g, res, "IMAGE_TACO", 160.0);
        }
        AwardKind::WateringCan => {
            draw_bottom(g, res, &tr("[FOUND_WATERING_CAN]"), &tr("[WATERING_CAN]"), &tr("[WATERING_CAN_DESCRIPTION]"));
            draw_item(g, res, "IMAGE_WATERINGCAN", 160.0);
        }
        AwardKind::Note(n) => {
            let background = if n % 2 == 1 { "IMAGE_BACKGROUND1" } else { "IMAGE_BACKGROUND2" };
            draw_stretched(g, res, background, -700.0, -300.0, 2800.0, 1200.0);
            if let Some(note) = res.image("IMAGE_ZOMBIE_NOTE") {
                g.draw_image(note, 80.0, 80.0);
            }
            let (id, x, y) = match n {
                1 => ("IMAGE_ZOMBIE_NOTE1", 131.0, 132.0),
                2 => ("IMAGE_ZOMBIE_NOTE2", 133.0, 127.0),
                3 => ("IMAGE_ZOMBIE_NOTE3", 120.0, 117.0),
                4 => ("IMAGE_ZOMBIE_NOTE4", 102.0, 117.0),
                _ => ("IMAGE_ZOMBIE_FINAL_NOTE", 114.0, 138.0),
            };
            if let Some(image) = res.image(id) {
                g.draw_image(image, x, y);
            }
            if let Some(font) = res.font("FONT_DWARVENTODCRAFT24") {
                g.tod_draw_string(font, 400, 70, &tr("[FOUND_NOTE]"), SexyColor::rgba(255, 200, 0, 255), Justify::Center);
            }
        }
    }

    // The two buttons.
    let (x, y, w, h) = screen.start_rect();
    let image = "IMAGE_SEEDCHOOSER_BUTTON";
    if let Some(button) = res.image(image) {
        g.draw_image(button, x, y);
    }
    if screen.start_over
        && let Some(glow) = res.image("IMAGE_SEEDCHOOSER_BUTTON_GLOW")
    {
        g.draw_image(glow, x, y);
    }
    if let Some(font) = res.font("FONT_DWARVENTODCRAFT15") {
        let ascent = font.font.ascent;
        let baseline = y as i32 + (h as i32 - ascent / 6 - 1 + ascent) / 2 - 1;
        g.tod_draw_string(font, (x + w / 2.0) as i32, baseline, &tr(screen.start_label()), SexyColor::rgba(213, 159, 43, 255), Justify::Center);
    }
    if screen.menu_visible() {
        let (x, y, w, h) = AwardScreen::MENU_RECT;
        if let Some(button) = res.image("IMAGE_SEEDCHOOSER_BUTTON2") {
            g.draw_image(button, x, y);
        }
        if screen.menu_over
            && let Some(glow) = res.image("IMAGE_SEEDCHOOSER_BUTTON2_GLOW")
        {
            g.draw_image(glow, x, y);
        }
        if let Some(font) = res.font("FONT_BRIANNETOD12") {
            let ascent = font.font.ascent;
            let baseline = y as i32 + (h as i32 - ascent / 6 - 1 + ascent) / 2 + 1;
            g.tod_draw_string(font, (x + w / 2.0) as i32, baseline, &tr("[AWARD_MAIN_MENU_BUTTON]"), SexyColor::rgba(42, 42, 90, 255), Justify::Center);
        }
    }

    // The fade in.
    let alpha = sexy::tod::animate_curve(180, 0, screen.fade_in, 255, 0, sexy::tod::TodCurve::Linear).clamp(0, 255) as u8;
    g.push_state();
    g.set_color(if screen.kind.is_paper_note() { SexyColor::rgba(0, 0, 0, alpha) } else { SexyColor::rgba(255, 255, 255, alpha) });
    g.fill_rect(0.0, 0.0, crate::GAME_WIDTH, crate::GAME_HEIGHT);
    g.pop_state();
}
