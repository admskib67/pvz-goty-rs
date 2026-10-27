//! The Tree of Wisdom: a grown tree that brags, tells jokes and gives advice
//! when fed tree food.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Challenge.cpp`
//! (`TreeOfWisdomInit`, `TreeOfWisdomDraw`, `TreeOfWisdomGrow`,
//! `TreeOfWisdomFertilize`, `TreeOfWisdomGiveWisdom`, `TreeOfWisdomBabble`,
//! `TreeOfWisdomLeave`): the tree's height is challenge record 50, each
//! `TREE_FOOD` grows it once (to 50), and clicking it retells a piece of
//! advice from `[TREE_OF_WISDOM_N]`. The original's tool cursor and render
//! groups are simplified to a click on the tree.

use bevy::prelude::*;
use sexy::reanim::LoopType;

use crate::audio::{FoleyType, Sounds};
use crate::graphics::{Graphics, Justify, SexyColor, SexyRect};
use crate::input::GameCursor;
use crate::player::ProfileMgr;
use crate::reanim::{EffectSystem, ReanimationId, ReanimationType};
use crate::resources::GameResources;
use crate::store::{PURCHASE_COUNT_OFFSET, StoreItem};
use crate::widgets::draw_stone_button;
use crate::{AppState, UiSet};

/// `GAMEMODE_TREE_OF_WISDOM`: its challenge record holds the tree's height.
const TREE_RECORD: usize = 50;
const MAX_HEIGHT: i32 = 50;
/// The highest `[TREE_OF_WISDOM_N]` advice line; higher sizes repeat these.
const LAST_ADVICE: i32 = 28;
const TREE_POS: Vec2 = Vec2::new(0.0, 0.0);

pub struct TreeOfWisdomPlugin;

impl Plugin for TreeOfWisdomPlugin {
    fn build(&self, app: &mut App) {
        let open = resource_exists::<TreeOfWisdom>;
        app.add_systems(OnEnter(AppState::TreeOfWisdom), start_tree)
            .add_systems(OnExit(AppState::TreeOfWisdom), leave_tree)
            .add_systems(FixedUpdate, (tick_effects, update_tree).chain().run_if(open))
            .add_systems(Update, tree_input.in_set(UiSet::ScreenInput).run_if(open))
            .add_systems(Update, draw_tree.in_set(UiSet::ScreenDraw).run_if(open));
    }
}

#[derive(Resource)]
struct TreeOfWisdom {
    tree: Option<ReanimationId>,
    clouds: Vec<ReanimationId>,
    /// What the speech bubble says, and for how many ticks.
    speech: Option<(i32, i32)>,
    size: i32,
}

impl TreeOfWisdom {
    fn play_grow(&mut self, effects: &mut EffectSystem) {
        let Some(tree) = self.tree else { return };
        if let Some(reanim) = effects.get_mut(tree) {
            reanim.play(&format!("anim_grow{}", self.size.clamp(1, MAX_HEIGHT)), LoopType::PlayOnceAndHold, 0, 12.0);
        }
    }
}

fn tick_effects(mut effects: ResMut<EffectSystem>, mut sounds: Sounds) {
    effects.update(sounds.rng());
}

fn start_tree(
    mut commands: Commands,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut effects: ResMut<EffectSystem>,
    mut music: ResMut<crate::audio::Music>,
    profiles: Res<ProfileMgr>,
) {
    if let Err(e) = resources.load_image("IMAGE_STORE_SPEECHBUBBLE2", &mut images) {
        debug!("{e}");
    }
    music.make_sure_music_is_playing(crate::audio::MusicTune::ZenGarden);
    effects.clear();
    let size = profiles.player().map_or(1, |p| p.challenge_records[TREE_RECORD].clamp(1, MAX_HEIGHT));
    let tree = effects
        .add_reanimation(TREE_POS.x, TREE_POS.y, ReanimationType::Treeofwisdom, &mut resources, &mut images)
        .ok()
        .map(|(id, reanim)| {
            reanim.play(&format!("anim_grow{size}"), LoopType::PlayOnceAndHold, 0, 18.0);
            id
        });
    let mut clouds = Vec::new();
    for i in 0..6 {
        if let Ok((id, reanim)) = effects.add_reanimation(0.0, 0.0, ReanimationType::TreeofwisdomClouds, &mut resources, &mut images) {
            reanim.play(&format!("Cloud{}", i + 1), LoopType::PlayOnceAndHold, 0, 0.2);
            clouds.push(id);
        }
    }
    commands.insert_resource(TreeOfWisdom { tree, clouds, speech: Some((1, 500)), size });
}

fn leave_tree(mut commands: Commands, mut effects: ResMut<EffectSystem>, mut profiles: ResMut<ProfileMgr>, tree: Option<Res<TreeOfWisdom>>) {
    if let Some(tree) = tree
        && let Some(player) = profiles.player_mut()
    {
        player.challenge_records[TREE_RECORD] = tree.size;
        profiles.save_current_details();
    }
    effects.clear();
    commands.remove_resource::<TreeOfWisdom>();
}

fn update_tree(mut tree: ResMut<TreeOfWisdom>, mut effects: ResMut<EffectSystem>) {
    let tree = &mut *tree;
    if let Some((_, ticks)) = &mut tree.speech {
        *ticks -= 1;
        if *ticks <= 0 {
            tree.speech = None;
            if let Some(id) = tree.tree
                && let Some(reanim) = effects.get_mut(id)
            {
                reanim.play(&format!("anim_grow{}", tree.size.clamp(1, MAX_HEIGHT)), LoopType::PlayOnceAndHold, 0, 18.0);
            }
        }
    } else if tree.size > 1 && rand_tick(tree.size) {
        tree.speech = Some((advice_line(tree.size), 400));
    }
}

/// A cheap deterministic "sometimes" without pulling the game RNG into a
/// draw-only screen.
fn rand_tick(size: i32) -> bool {
    (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_millis() as i32) + size * 13) % 997 == 0
}

/// `TreeOfWisdomFertilize` + `TreeOfWisdomGrow`: spend one tree food and
/// grow the tree by one, up to 50.
pub fn feed_tree(player: &mut crate::player::PlayerInfo) -> bool {
    if player.purchases[StoreItem::TreeFood.slot()] <= PURCHASE_COUNT_OFFSET {
        return false;
    }
    if player.challenge_records[TREE_RECORD] >= MAX_HEIGHT {
        return false;
    }
    player.purchases[StoreItem::TreeFood.slot()] -= 1;
    player.challenge_records[TREE_RECORD] = (player.challenge_records[TREE_RECORD] + 1).min(MAX_HEIGHT);
    true
}

fn advice_line(size: i32) -> i32 {
    // `TreeOfWisdomGiveWisdom` says the previous height's line.
    (size - 1).clamp(1, LAST_ADVICE)
}

#[allow(clippy::too_many_arguments)]
fn tree_input(
    mut tree: ResMut<TreeOfWisdom>,
    cursor: Res<GameCursor>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut profiles: ResMut<ProfileMgr>,
    mut effects: ResMut<EffectSystem>,
    mut next_state: ResMut<NextState<AppState>>,
    mut sounds: Sounds,
) {
    let tree = &mut *tree;
    let Some(point) = cursor.0 else { return };
    if mouse.just_released(MouseButton::Left) && point.x < 140.0 && point.y > 550.0 {
        next_state.set(AppState::ZenGarden);
        return;
    }
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    // The tree's trunk and leaves.
    if point.x < 230.0 || point.x > 570.0 || point.y < 20.0 || point.y > 520.0 {
        return;
    }
    let Some(player) = profiles.player_mut() else { return };
    if !feed_tree(player) {
        tree.speech = Some((advice_line(tree.size.max(2)), 600));
        sounds.play_sample("SOUND_TAP");
        return;
    }
    tree.size = player.challenge_records[TREE_RECORD];
    profiles.save_current_details();
    tree.play_grow(&mut effects);
    tree.speech = Some((advice_line(tree.size), 700));
    sounds.play_foley(FoleyType::PlantGrow);
}

fn draw_tree(
    tree: Res<TreeOfWisdom>,
    resources: Res<GameResources>,
    effects: Res<EffectSystem>,
    profiles: Res<ProfileMgr>,
    mut g: ResMut<Graphics>,
) {
    let res = &*resources;
    let g = &mut *g;
    // The tree reanimation includes the sky and clouds; the separate cloud
    // reanimations sit over the trunk.
    if let Some(id) = tree.tree {
        effects.draw(g, id, res);
    }
    for id in &tree.clouds {
        effects.draw(g, *id, res);
    }
    if let Some((line, _)) = tree.speech
        && let Some(bubble) = res.image("IMAGE_STORE_SPEECHBUBBLE2")
        && let Some(font) = res.font("FONT_BRIANNETOD16")
    {
        let (x, y) = if tree.size < 7 { (400.0, 152.0) } else if tree.size < 12 { (395.0, 60.0) } else { (390.0, 52.0) };
        g.draw_image(bubble, x, y);
        let text = res.translate(&format!("[TREE_OF_WISDOM_{line}]"));
        g.tod_draw_string_wrapped(
            font,
            SexyRect::new(x + 25.0, y + 6.0, 233.0, 144.0),
            &text,
            SexyColor::rgba(0, 0, 0, 255),
            Justify::Center,
        );
    }
    if let Some(font) = res.font("FONT_BRIANNETOD12") {
        let size = profiles.player().map_or(0, |p| p.challenge_records[TREE_RECORD]);
        g.tod_draw_string(font, 400, 540, &format!("Height: {} / {MAX_HEIGHT}", size.max(tree.size)), SexyColor::rgba(255, 255, 255, 255), Justify::Center);
            let _ = size;
        if size >= MAX_HEIGHT {
            g.tod_draw_string(font, 400, 560, "The Tree of Wisdom has nothing left to teach.", SexyColor::rgba(255, 240, 0, 255), Justify::Center);
        } else {
            g.tod_draw_string(font, 400, 560, "Click the tree to feed it tree food.", SexyColor::rgba(255, 240, 0, 255), Justify::Center);
        }
        draw_stone_button(g, res, 10, 555, 110, 36, false, false, "");
        g.tod_draw_string(font, 65, 573, "Back", SexyColor::rgba(42, 42, 90, 255), Justify::Center);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::PlayerInfo;

    #[test]
    fn tree_food_grows_the_tree_and_is_spent() {
        let mut player = PlayerInfo::default();
        player.purchases[StoreItem::TreeFood.slot()] = PURCHASE_COUNT_OFFSET + 2;
        player.challenge_records[TREE_RECORD] = 1;
        assert!(feed_tree(&mut player));
        assert_eq!(player.challenge_records[TREE_RECORD], 2);
        assert_eq!(player.purchases[StoreItem::TreeFood.slot()], PURCHASE_COUNT_OFFSET + 1);
        assert!(feed_tree(&mut player));
        assert_eq!(player.challenge_records[TREE_RECORD], 3);
        assert_eq!(player.purchases[StoreItem::TreeFood.slot()], PURCHASE_COUNT_OFFSET);
        assert!(!feed_tree(&mut player));
        player.challenge_records[TREE_RECORD] = MAX_HEIGHT;
        player.purchases[StoreItem::TreeFood.slot()] = PURCHASE_COUNT_OFFSET + 1;
        player.purchases[StoreItem::TreeFood.slot()] = PURCHASE_COUNT_OFFSET + 1;
        assert!(!feed_tree(&mut player));
        assert_eq!(player.purchases[StoreItem::TreeFood.slot()], PURCHASE_COUNT_OFFSET + 1);
    }
}
