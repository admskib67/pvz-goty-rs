//! Crazy Dave's Twiddydinkies: the store opened from the main menu or the Zen
//! Garden. Dave sells plant upgrades, seed slots and garden tools out of his
//! car; the player pages through them with the arrows and pays with coins.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Widget/StoreScreen.cpp`:
//! `gStoreItemSpots`, `GetStoreItemType`, `IsPageShown`, `IsComingSoon`,
//! `IsItemSoldOut`, `IsItemUnavailable`, `GetItemCost`, `PurchaseItem`,
//! `DrawItemIcon`, `DrawItem`, `Draw`, `UpdateMouse`, `Update`,
//! `ButtonDepress`, `MouseDown`, `AdvanceCrazyDaveDialog` and
//! `EnableButtons`. The logic (what is for sale, at what price, what buying
//! does) is plain functions of the `PlayerInfo`, tested without the game.
//!
//! Not yet: the bonus lawn mower and chocolate (not on any page), the trial
//! and "buy the game" items, the store intro talks from the award screen, the
//! diamond Dave hands out, the hand cursor, and opening the store from the
//! seed chooser.

use bevy::math::Affine2;
use bevy::prelude::*;
use sexy::tod::{TodCurve, animate_curve};

use crate::audio::{FoleyType, Music, MusicTune, Sounds};
use crate::board::draw_seed_packet_free;
use crate::board::figures::{CrazyDave, DaveState, DaveVars, money_string};
use crate::dialog::{ButtonMode, DialogId, Dialogs};
use crate::graphics::{Graphics, Justify, SexyColor, SexyRect};
use crate::input::GameCursor;
use crate::lawn_app::lawn_message_box;
use crate::player::{PlayerInfo, ProfileMgr};
use crate::plant::SeedType;
use crate::reanim::EffectSystem;
use crate::resources::GameResources;
use crate::widgets::{ButtonEvent, GameButton, LabelButton, update_buttons};
use crate::{AppState, UiSet};

/// `PURCHASE_COUNT_OFFSET`: consumables count from here, so that "none
/// bought yet" (0) differs from "bought and used up" (1000).
pub const PURCHASE_COUNT_OFFSET: i32 = 1000;

/// Where the store's back button goes. The Zen Garden opens the store over
/// itself (`ZenGarden::OpenStore`) and expects to come back; the menu does not.
#[derive(Resource, Clone, Copy, Default)]
pub struct StoreReturn(pub Option<AppState>);

/// The store was opened on top of a live board (the seed chooser's Shop
/// button); closing it returns to that board instead of changing state.
#[derive(Resource, Default)]
pub struct StoreOverlay;

/// `StoreScreen::SetupForIntro`: Dave's first message when the store opens
/// after an award, before the pages unlock.
#[derive(Resource, Clone, Copy)]
pub struct StoreIntro(pub i32);
/// `MAX_PAGE_SPOTS`.
const MAX_PAGE_SPOTS: usize = 8;
/// `GAMEMODE_TREE_OF_WISDOM`, the index of its challenge record.
const GAMEMODE_TREE_OF_WISDOM: usize = 50;
/// The most coins a player can hold.
const MAX_COINS: i32 = 99_999;

pub struct StorePlugin;

impl Plugin for StorePlugin {
    fn build(&self, app: &mut App) {
        let open = resource_exists::<StoreScreen>;
        app.add_systems(OnEnter(AppState::Store), start_store)
            .add_systems(OnExit(AppState::Store), end_store)
            .add_systems(FixedUpdate, update_store.run_if(open))
            .add_systems(Update, store_input.in_set(UiSet::ScreenInput).run_if(open))
            .add_systems(
                Update,
                draw_store.in_set(UiSet::ScreenDraw).run_if(open).after(crate::board::draw_board),
            );
    }
}

/// `StoreItem`: also the item's slot in `PlayerInfo::purchases`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreItem {
    GatlingPea,
    TwinSunflower,
    GloomShroom,
    Cattail,
    WinterMelon,
    GoldMagnet,
    SpikeRock,
    CobCannon,
    Imitater,
    #[allow(dead_code, reason = "in the original's list but on no page")]
    BonusLawnMower,
    PottedMarigold1,
    PottedMarigold2,
    PottedMarigold3,
    GoldWateringCan,
    Fertilizer,
    BugSpray,
    Phonograph,
    GardeningGlove,
    MushroomGarden,
    WheelBarrow,
    StinkyTheSnail,
    PacketUpgrade,
    PoolCleaner,
    RoofCleaner,
    Rake,
    AquariumGarden,
    #[allow(dead_code, reason = "in the original's list but on no page")]
    Chocolate,
    TreeOfWisdom,
    TreeFood,
    FirstAid,
}

impl StoreItem {
    /// Its entry in `PlayerInfo::purchases`.
    pub fn slot(self) -> usize {
        self as usize
    }

    fn is_potted_plant(self) -> bool {
        matches!(self, Self::PottedMarigold1 | Self::PottedMarigold2 | Self::PottedMarigold3)
    }

    /// The plant packet this upgrade is (`theItemType + 40`).
    fn seed(self) -> Option<SeedType> {
        (self.slot() <= Self::Imitater.slot()).then(|| SeedType::from_index(self.slot() as i32 + 40)).flatten()
    }
}

/// `StorePages`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorePage {
    SlotUpgrades,
    PlantUpgrades,
    Zen1,
    Zen2,
}

const PAGES: [StorePage; 4] = [StorePage::SlotUpgrades, StorePage::PlantUpgrades, StorePage::Zen1, StorePage::Zen2];

impl StorePage {
    fn index(self) -> usize {
        self as usize
    }

    /// `gStoreItemSpots[self][spot]`; the first four spots are the top row.
    pub fn item(self, spot: usize) -> Option<StoreItem> {
        use StoreItem::*;
        const SPOTS: [[Option<StoreItem>; MAX_PAGE_SPOTS]; 4] = [
            [
                Some(PacketUpgrade),
                Some(PoolCleaner),
                Some(Rake),
                Some(RoofCleaner),
                Some(GatlingPea),
                Some(TwinSunflower),
                Some(GloomShroom),
                Some(Cattail),
            ],
            [Some(SpikeRock), Some(GoldMagnet), Some(WinterMelon), Some(CobCannon), Some(Imitater), Some(FirstAid), None, None],
            [
                Some(PottedMarigold1),
                Some(PottedMarigold2),
                Some(PottedMarigold3),
                Some(GoldWateringCan),
                Some(Fertilizer),
                Some(BugSpray),
                Some(Phonograph),
                Some(GardeningGlove),
            ],
            [
                Some(MushroomGarden),
                Some(AquariumGarden),
                Some(WheelBarrow),
                Some(StinkyTheSnail),
                Some(TreeOfWisdom),
                Some(TreeFood),
                None,
                None,
            ],
        ];
        SPOTS[self.index()].get(spot).copied().flatten()
    }

    /// `IsPageShown`: pages open up as the adventure goes on.
    pub fn is_shown(self, player: &PlayerInfo) -> bool {
        if player.has_finished_adventure() {
            return true;
        }
        match self {
            Self::PlantUpgrades => player.level >= 42,
            Self::Zen1 => player.level >= 45,
            Self::Zen2 => false,
            Self::SlotUpgrades => true,
        }
    }
}

/// `StoreScreen::IsComingSoon`.
pub fn is_coming_soon(player: &PlayerInfo, item: StoreItem) -> bool {
    let bought = |i: StoreItem| player.purchases[i.slot()] != 0;
    match item {
        StoreItem::WheelBarrow => !bought(StoreItem::MushroomGarden) && !bought(StoreItem::AquariumGarden),
        i if i.is_potted_plant() => false,
        StoreItem::TreeFood => {
            !bought(StoreItem::TreeOfWisdom) || player.purchases[StoreItem::TreeFood.slot()] < PURCHASE_COUNT_OFFSET
        }
        _ => false,
    }
}

/// `StoreScreen::IsItemSoldOut`.
pub fn is_sold_out(player: &PlayerInfo, item: StoreItem) -> bool {
    let count = player.purchases[item.slot()];
    match item {
        StoreItem::PacketUpgrade => count >= 4,
        StoreItem::Fertilizer | StoreItem::BugSpray => count - PURCHASE_COUNT_OFFSET > 15,
        StoreItem::TreeFood => count - PURCHASE_COUNT_OFFSET >= 10,
        StoreItem::BonusLawnMower => count >= 2,
        // One potted plant of each kind a day, while the garden has room.
        i if i.is_potted_plant() => {
            crate::zen_garden::main_garden_full(player) || count == days_since_2000()
        }
        _ => count != 0,
    }
}

/// `StoreScreen::IsItemUnavailable`: not for sale yet, so not shown at all.
pub fn is_unavailable(player: &PlayerInfo, item: StoreItem) -> bool {
    let finished = player.has_finished_adventure();
    let before = |level| !finished && player.level < level;
    match item {
        StoreItem::RoofCleaner => before(42),
        StoreItem::GloomShroom | StoreItem::Cattail => before(35),
        StoreItem::SpikeRock | StoreItem::GoldMagnet => before(41),
        StoreItem::WinterMelon | StoreItem::CobCannon | StoreItem::Imitater | StoreItem::FirstAid => !finished,
        _ => false,
    }
}

/// Whether clicking the item buys it.
pub fn can_buy(player: &PlayerInfo, item: StoreItem) -> bool {
    !is_sold_out(player, item) && !is_unavailable(player, item) && !is_coming_soon(player, item)
}

/// `StoreScreen::GetItemCost`, in coins (shown times ten).
pub fn item_cost(player: &PlayerInfo, item: StoreItem) -> i32 {
    use StoreItem::*;
    match item {
        BonusLawnMower => {
            if player.purchases[BonusLawnMower.slot()] != 0 {
                500
            } else {
                200
            }
        }
        GatlingPea | TwinSunflower => 500,
        GloomShroom | SpikeRock => 750,
        Cattail | WinterMelon => 1000,
        GoldMagnet => 300,
        CobCannon => 2000,
        Imitater => 3000,
        PottedMarigold1 | PottedMarigold2 | PottedMarigold3 => 250,
        GoldWateringCan => 1000,
        Fertilizer => 75,
        BugSpray => 100,
        Phonograph => 1500,
        GardeningGlove => 100,
        MushroomGarden | AquariumGarden => 3000,
        WheelBarrow | Rake => 20,
        StinkyTheSnail => 300,
        PacketUpgrade => crate::board::packet_upgrade_cost(player.purchases[PacketUpgrade.slot()]),
        PoolCleaner => 100,
        RoofCleaner => 300,
        TreeOfWisdom => 1000,
        TreeFood => 250,
        FirstAid => 200,
        Chocolate => 0,
    }
}

/// What buying something leads to, beyond the purchase itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bought {
    Done,
    /// A seed slot: the player now has this many.
    Slots(i32),
    /// The tree of wisdom: offer to visit it.
    VisitTree,
    /// The first aid wall-nut: Dave has something to say.
    FirstAid,
}

/// `GetCurrentDaysSince2000`, the original's one-a-day marker.
fn days_since_2000() -> i32 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    ((now - 946_684_800) / 86_400) as i32
}

/// The money part of `StoreScreen::PurchaseItem` after the player said yes:
/// pays and records the purchase. `now` is the time in seconds (Stinky
/// remembers when he was bought). `None` when the player can't afford it.
pub fn buy(player: &mut PlayerInfo, item: StoreItem, now: i32) -> Option<Bought> {
    if item.is_potted_plant() && crate::zen_garden::main_garden_full(player) {
        return None;
    }
    let cost = item_cost(player, item);
    if player.coins < cost {
        return None;
    }
    player.coins = (player.coins - cost).clamp(0, MAX_COINS);
    let slot = item.slot();
    let mut result = Bought::Done;
    match item {
        StoreItem::PacketUpgrade => {
            player.purchases[slot] += 1;
            result = Bought::Slots(6 + player.purchases[slot]);
        }
        StoreItem::BonusLawnMower => player.purchases[slot] += 1,
        StoreItem::Rake => player.purchases[slot] = 3,
        StoreItem::StinkyTheSnail => player.purchases[slot] = now,
        StoreItem::Fertilizer | StoreItem::BugSpray => {
            player.purchases[slot] = player.purchases[slot].max(PURCHASE_COUNT_OFFSET) + 5;
        }
        StoreItem::TreeFood => player.purchases[slot] = player.purchases[slot].max(PURCHASE_COUNT_OFFSET) + 1,
        StoreItem::TreeOfWisdom => {
            player.purchases[slot] = 1;
            player.challenge_records[GAMEMODE_TREE_OF_WISDOM] = 1;
            // Dave's first-visit gift (`CutScene::AdvanceCrazyDaveDialog`,
            // message 3200): five tree foods so the tree can be fed.
            if player.purchases[StoreItem::TreeFood.slot()] < PURCHASE_COUNT_OFFSET {
                player.purchases[StoreItem::TreeFood.slot()] = PURCHASE_COUNT_OFFSET + 5;
            }
            result = Bought::VisitTree;
        }
        i if i.is_potted_plant() => {
            // `ZenGarden::AddPottedPlant`: the marigold lands in the first
            // empty main-garden spot.
            if !crate::zen_garden::add_potted_plant(player, SeedType::Marigold) {
                return None;
            }
            player.purchases[slot] = days_since_2000();
        }
        _ => player.purchases[slot] = 1,
    }
    if item == StoreItem::FirstAid {
        result = Bought::FirstAid;
    }
    Some(result)
}

/// The message Dave says about an item the mouse is over (`UpdateMouse`).
fn item_message(player: &PlayerInfo, item: StoreItem) -> i32 {
    use StoreItem::*;
    match item {
        GatlingPea => 2000,
        TwinSunflower => 2001,
        GloomShroom => 2002,
        Cattail => 2003,
        WinterMelon => 2004,
        GoldMagnet => 2005,
        SpikeRock => 2006,
        CobCannon => 2007,
        Imitater => 2008,
        BonusLawnMower => 2009,
        PottedMarigold1 | PottedMarigold2 | PottedMarigold3 => 2010,
        GoldWateringCan => 2019,
        Fertilizer => 2020,
        BugSpray => 2022,
        Phonograph => 2021,
        GardeningGlove => 2023,
        MushroomGarden => 2032,
        WheelBarrow => 2024,
        StinkyTheSnail => 2025,
        PacketUpgrade => player.purchases[PacketUpgrade.slot()] + 2011,
        PoolCleaner => 2026,
        RoofCleaner => 2027,
        Rake => 2028,
        AquariumGarden => 2029,
        Chocolate => -1,
        TreeOfWisdom => 2030,
        TreeFood => 2031,
        FirstAid => 2033,
    }
}

/// Where an item's spot is on screen (`GetStorePosition`).
fn spot_position(spot: usize) -> (i32, i32) {
    const ITEM_SIZE: i32 = 74;
    if spot <= 3 {
        (422 + ITEM_SIZE * spot as i32, 206)
    } else {
        (372 + ITEM_SIZE * (spot as i32 - 4), 310)
    }
}

/// `TodPickFromWeightedArray`.
fn pick_weighted(items: &[(i32, i32)], rng: &mut sexy::rand::MtRand) -> i32 {
    let total: i32 = items.iter().map(|&(_, w)| w).sum();
    if total <= 0 {
        return items[0].0;
    }
    let mut roll = rng.range_int(0, total - 1);
    for &(item, weight) in items {
        if roll < weight {
            return item;
        }
        roll -= weight;
    }
    items[items.len() - 1].0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arrow {
    Prev,
    Next,
}

/// `StoreScreen`.
#[derive(Resource)]
pub struct StoreScreen {
    /// `mStoreTime`: ticks since Dave arrived; the sign drops in over the first 110.
    store_time: i32,
    bubble_countdown: i32,
    bubble_click_to_continue: bool,
    /// `mStartDialog`: an intro message to say before the buttons unlock.
    start_dialog: i32,
    ambient_countdown: i32,
    previous_ambient: i32,
    page: StorePage,
    mouse_over: Option<StoreItem>,
    hatch_timer: i32,
    hatch_open: bool,
    shake_y: i32,
    dave: CrazyDave,
    back: LabelButton,
    prev: GameButton<Arrow>,
    next: GameButton<Arrow>,
    /// The arrows are greyed out when there's only one page.
    arrows_disabled: bool,
    /// The back button's `mDisabled` is false (it's off while the hatch moves).
    back_enabled: bool,
    /// The item the player is being asked about.
    pending: Option<StoreItem>,
    /// What the player answered in one of the store's dialogs this frame.
    pub dialog_answer: Option<(DialogId, bool)>,
}

impl StoreScreen {
    fn new(res: &GameResources, player: &PlayerInfo) -> Self {
        let mut back = LabelButton::new(
            res.translate("[STORE_MAIN_MENU_BUTTON]"),
            "FONT_HOUSEOFTERROR20",
            SexyColor::rgba(98, 153, 235, 255),
            "IMAGE_STORE_MAINMENUBUTTON",
        );
        back.hilite_color = SexyColor::rgba(167, 192, 235, 255);
        back.over_image = Some("IMAGE_STORE_MAINMENUBUTTONHIGHLIGHT");
        back.down_image = Some("IMAGE_STORE_MAINMENUBUTTONDOWN");
        back.text_offset = (-7, 1);
        let size = |id: &str| res.image(id).map_or((0, 0), |i| (i.width as i32, i.height as i32));
        let (w, h) = size("IMAGE_STORE_MAINMENUBUTTON");
        back.resize(Self::BACK_POS.0, Self::BACK_POS.1, w, h);
        let arrow = |id, normal: &str, over: &str, pos: (i32, i32)| {
            let mut button = GameButton::new(id, res, normal, over, over);
            button.resize(pos.0, pos.1, size(normal).0, size(normal).1);
            button.translate = (0, 0);
            button
        };
        let prev = arrow(Arrow::Prev, "IMAGE_STORE_PREVBUTTON", "IMAGE_STORE_PREVBUTTONHIGHLIGHT", Self::PREV_POS);
        let next = arrow(Arrow::Next, "IMAGE_STORE_NEXTBUTTON", "IMAGE_STORE_NEXTBUTTONHIGHLIGHT", Self::NEXT_POS);
        let mut store = Self {
            store_time: 0,
            bubble_countdown: 0,
            bubble_click_to_continue: false,
            start_dialog: -1,
            ambient_countdown: 200,
            previous_ambient: -1,
            page: StorePage::SlotUpgrades,
            mouse_over: None,
            hatch_timer: 0,
            hatch_open: true,
            shake_y: 0,
            dave: CrazyDave::new(),
            back,
            prev,
            next,
            arrows_disabled: !StorePage::PlantUpgrades.is_shown(player),
            back_enabled: true,
            pending: None,
            dialog_answer: None,
        };
        if store.arrows_disabled {
            store.prev.disabled = true;
            store.next.disabled = true;
        }
        store
    }

    const BACK_POS: (i32, i32) = (366, 512);
    const PREV_POS: (i32, i32) = (252, 402);
    const NEXT_POS: (i32, i32) = (596, 402);

    /// `CanInteractWithButtons`.
    fn can_interact(&self) -> bool {
        self.store_time >= 120 && !self.bubble_click_to_continue && self.hatch_timer <= 0
    }

    /// `EnableButtons`.
    fn enable_buttons(&mut self, player: &PlayerInfo, enable: bool) {
        if StorePage::PlantUpgrades.is_shown(player) || !enable {
            for button in [&mut self.prev, &mut self.next] {
                button.mouse_visible = enable;
                button.disabled = !enable;
            }
        }
        self.back_enabled = enable;
    }
}

/// What Dave's messages fill in.
fn dave_vars(player: &PlayerInfo) -> DaveVars {
    DaveVars {
        name: player.name.clone(),
        money: player.coins,
        upgrade_cost: item_cost(player, StoreItem::PacketUpgrade),
    }
}

/// What speaking and playing sounds need.
struct Ctx<'a, 'w, 's> {
    effects: &'a mut EffectSystem,
    res: &'a mut GameResources,
    images: &'a mut Assets<Image>,
    sounds: &'a mut Sounds<'w, 's>,
}

impl StoreScreen {
    /// `StoreScreen::SetupForIntro`: a fixed opening talk, hatch shut, buttons
    /// locked, and a Next Level back button.
    fn setup_for_intro(&mut self, message: i32, player: &PlayerInfo, res: &GameResources) {
        self.start_dialog = message;
        self.hatch_open = false;
        self.back.label = res.translate("[STORE_NEXT_LEVEL_BUTTON]");
        self.enable_buttons(player, false);
    }

    /// `SetBubbleText`.
    fn set_bubble_text(&mut self, message: i32, time: i32, click_to_continue: bool, player: &PlayerInfo, ctx: &mut Ctx) {
        let vars = dave_vars(player);
        self.dave.talk_index(message, &vars, ctx.effects, ctx.res, ctx.images, ctx.sounds);
        self.bubble_countdown = time;
        self.bubble_click_to_continue = click_to_continue;
    }

    /// `UpdateMouse`: Dave talks about the item under the mouse.
    fn update_mouse(&mut self, player: &PlayerInfo, mouse: Option<Vec2>, ctx: &mut Ctx) {
        self.mouse_over = None;
        if self.store_time < 120 || self.bubble_click_to_continue || self.hatch_timer > 0 {
            return;
        }
        let Some(mouse) = mouse else { return };
        for spot in 0..MAX_PAGE_SPOTS {
            let Some(item) = self.page.item(spot) else { continue };
            if is_unavailable(player, item) || !item_rect(spot).contains(mouse) {
                continue;
            }
            self.mouse_over = Some(item);
            let message = item_message(player, item);
            if self.dave.message_index != message {
                self.set_bubble_text(message, 100, false, player, ctx);
            } else {
                self.bubble_countdown = 100;
            }
            break;
        }
    }

    /// `AdvanceCrazyDaveDialog`, as far as the store's own talks go.
    fn advance_dialog(&mut self, player: &PlayerInfo, ctx: &mut Ctx) {
        if !self.bubble_click_to_continue {
            return;
        }
        let vars = dave_vars(player);
        if !self.dave.advance_text(&vars, ctx.effects, ctx.res, ctx.images, ctx.sounds) {
            self.dave.stop_talking(ctx.effects, ctx.sounds);
            self.bubble_click_to_continue = false;
            self.bubble_countdown = 500;
            if self.hatch_timer == 0 {
                self.enable_buttons(player, true);
            }
        } else {
            let message = self.dave.message_index;
            self.set_bubble_text(message, 0, true, player, ctx);
        }
    }

    /// The start of `PurchaseItem`: Dave goes quiet and the player is asked.
    fn begin_purchase(&mut self, item: StoreItem, player: &PlayerInfo, dialogs: &mut Dialogs, ctx: &mut Ctx) {
        self.bubble_countdown = 0;
        self.dave.stop_talking(ctx.effects, ctx.sounds);
        if player.coins < item_cost(player, item) {
            let (header, lines, ok) = ("[NOT_ENOUGH_MONEY]", "[CANNOT_AFFORD_ITEM]", "[DIALOG_BUTTON_OK]");
            lawn_message_box(dialogs, ctx.res, DialogId::NotEnoughMoney, header, lines, ok, "", ButtonMode::Footer);
        } else {
            let (header, lines) = ("[BUY_ITEM_HEADER]", "[BUY_ITEM]");
            let (yes, no) = ("[DIALOG_BUTTON_YES]", "[DIALOG_BUTTON_NO]");
            lawn_message_box(dialogs, ctx.res, DialogId::StorePurchase, header, lines, yes, no, ButtonMode::YesNo);
            self.pending = Some(item);
        }
    }

    /// The rest of `PurchaseItem` once the player answered the question.
    fn finish_purchase(&mut self, yes: bool, profiles: &mut ProfileMgr, dialogs: &mut Dialogs, ctx: &mut Ctx) {
        let Some(item) = self.pending.take().filter(|_| yes) else { return };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i32::try_from(d.as_secs()).unwrap_or(i32::MAX));
        let Some(bought) = profiles.player_mut().and_then(|p| buy(p, item, now)) else { return };
        profiles.save_current_details();
        let ok = "[DIALOG_BUTTON_OK]";
        match bought {
            Bought::Slots(slots) => {
                let lines = format!("You can now choose {slots} seeds on each level!");
                lawn_message_box(dialogs, ctx.res, DialogId::Upgraded, "[MORE_SLOTS]", &lines, ok, "", ButtonMode::Footer);
            }
            Bought::VisitTree => {
                let (header, body) = ("[VISIT_TREE_HEADER]", "[VISIT_TREE_BODY]");
                let (yes, no) = ("[DIALOG_BUTTON_YES]", "[DIALOG_BUTTON_NO]");
                lawn_message_box(dialogs, ctx.res, DialogId::VisitTree, header, body, yes, no, ButtonMode::YesNo);
            }
            Bought::FirstAid => {
                if let Some(player) = profiles.player() {
                    self.set_bubble_text(3400, 800, false, player, ctx);
                }
            }
            Bought::Done => {}
        }
    }

    /// `ButtonDepress` for the arrows: the hatch closes, the page turns.
    fn turn_page(&mut self, arrow: Arrow, player: &PlayerInfo, ctx: &mut Ctx) {
        self.hatch_timer = 50;
        ctx.sounds.play_sample("SOUND_HATCHBACK_CLOSE");
        self.bubble_countdown = 0;
        self.dave.stop_talking(ctx.effects, ctx.sounds);
        self.enable_buttons(player, false);
        let mut index = self.page.index();
        loop {
            index = match arrow {
                Arrow::Prev => (index + PAGES.len() - 1) % PAGES.len(),
                Arrow::Next => (index + 1) % PAGES.len(),
            };
            if PAGES[index].is_shown(player) {
                break;
            }
        }
        self.page = PAGES[index];
    }
}

/// The area of an item's spot that can be clicked.
fn item_rect(spot: usize) -> Rect {
    let (x, y) = spot_position(spot);
    Rect::new(x as f32, y as f32, (x + 50) as f32, (y + 87) as f32)
}

#[allow(clippy::too_many_arguments)]
fn preload_potted_plant(resources: &mut GameResources, images: &mut Assets<Image>) {
    for seed in [SeedType::Flowerpot, SeedType::Marigold] {
        let _ = resources.reanim_definition(seed.definition().reanim.file(), images);
    }
}

#[allow(clippy::too_many_arguments)]
fn start_store(
    mut commands: Commands,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut effects: ResMut<EffectSystem>,
    mut sounds: Sounds,
    profiles: Res<ProfileMgr>,
    return_to: Option<Res<StoreReturn>>,
    intro: Option<Res<StoreIntro>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let return_to = return_to.map(|r| *r).unwrap_or_default();
    let intro = intro.map(|i| *i);
    let Some(player) = profiles.player() else {
        next_state.set(AppState::GameSelector);
        return;
    };
    if let Err(e) = resources.load_group_now("DelayLoad_Store", &mut images) {
        error!("{e}");
    }
    preload_potted_plant(&mut resources, &mut images);
    let mut store = StoreScreen::new(&resources, player);
    if let Some(return_to) = return_to.0 {
        store.page = if matches!(return_to, AppState::ZenGarden) { StorePage::Zen1 } else { store.page };
        store.back.label = resources.translate("[STORE_BACK_TO_GAME]");
    }
    if let Some(intro) = intro {
        store.setup_for_intro(intro.0, player, &resources);
        commands.remove_resource::<StoreIntro>();
    }
    store.dave.enter(&mut effects, &mut resources, &mut images, sounds.rng());
    commands.insert_resource(store);
}

/// `RemovedFromManager`: Dave goes away with the store.
fn end_store(mut commands: Commands, store: Option<ResMut<StoreScreen>>, mut effects: ResMut<EffectSystem>, mut sounds: Sounds) {
    if let Some(mut store) = store {
        store.dave.die(&mut effects, &mut sounds);
    }
    commands.remove_resource::<StoreScreen>();
    commands.remove_resource::<StoreReturn>();
    commands.remove_resource::<StoreOverlay>();
    commands.remove_resource::<StoreIntro>();
}

/// Opens the store as an overlay on the live board (the seed chooser's Shop
/// button). The board stays as a resource and is paused; closing the store
/// removes it again.
pub fn open_store_over_board(
    commands: &mut Commands,
    resources: &mut GameResources,
    images: &mut Assets<Image>,
    effects: &mut EffectSystem,
    sounds: &mut Sounds,
    player: &PlayerInfo,
) {
    if let Err(e) = resources.load_group_now("DelayLoad_Store", images) {
        error!("{e}");
    }
    preload_potted_plant(resources, images);
    let mut store = StoreScreen::new(resources, player);
    store.back.label = resources.translate("[STORE_BACK_TO_GAME]");
    store.dave.enter(effects, resources, images, sounds.rng());
    commands.insert_resource(store);
    commands.insert_resource(StoreReturn(Some(AppState::Board)));
    commands.insert_resource(StoreOverlay);
}

/// `StoreScreen::Update`, once a tick.
#[allow(clippy::too_many_arguments)]
fn update_store(
    mut store: ResMut<StoreScreen>,
    mut effects: ResMut<EffectSystem>,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut sounds: Sounds,
    mut music: ResMut<Music>,
    dialogs: Res<Dialogs>,
    cursor: Res<GameCursor>,
    profiles: Res<ProfileMgr>,
) {
    let Some(player) = profiles.player() else { return };
    let store = &mut *store;
    music.make_sure_music_is_playing(MusicTune::TitleCrazyDaveMainTheme);
    let mut ctx = Ctx { effects: &mut effects, res: &mut resources, images: &mut images, sounds: &mut sounds };
    store.dave.update(ctx.effects, ctx.res, ctx.images, ctx.sounds);
    // A dialog over the store holds it still.
    if dialogs.is_modal() || store.dave.state == DaveState::Off {
        return;
    }
    store.store_time += 1;
    if store.start_dialog != -1 && store.dave.state != DaveState::Entering {
        let message = store.start_dialog;
        store.start_dialog = -1;
        store.set_bubble_text(message, 0, true, player, &mut ctx);
        store.enable_buttons(player, false);
    }
    if store.dave.state != DaveState::Entering {
        if store.hatch_timer > 0 {
            store.hatch_timer -= 1;
            if store.hatch_timer == 0 {
                store.enable_buttons(player, true);
                store.shake_y = 0;
            } else {
                // The car rattles as the hatch closes.
                store.shake_y = if store.hatch_timer > 35 { ctx.sounds.rng().range_int(1, 3) } else { 0 };
            }
            let y = store.shake_y;
            store.back.y = StoreScreen::BACK_POS.1 + y;
            store.prev.y = StoreScreen::PREV_POS.1 + y;
            store.next.y = StoreScreen::NEXT_POS.1 + y;
        } else if !store.bubble_click_to_continue {
            if store.bubble_countdown > 0 {
                store.bubble_countdown -= 1;
                if store.bubble_countdown == 0 {
                    let talking = [FoleyType::CrazyDaveShort, FoleyType::CrazyDaveLong, FoleyType::CrazyDaveExtraLong]
                        .into_iter()
                        .any(|f| ctx.sounds.is_foley_playing(f));
                    if talking {
                        store.bubble_countdown = 1;
                    } else {
                        store.dave.stop_talking(ctx.effects, ctx.sounds);
                    }
                }
            } else {
                store.ambient_countdown -= 1;
                if store.ambient_countdown <= 0 {
                    // The last remark needs the adventure finished and isn't repeated.
                    let choices: Vec<(i32, i32)> = (0..4)
                        .map(|i| {
                            let message = 2015 + i;
                            let weight = if store.previous_ambient == message {
                                0
                            } else if i == 3 {
                                if player.has_finished_adventure() { 20 } else { 0 }
                            } else {
                                100
                            };
                            (message, weight)
                        })
                        .collect();
                    let message = pick_weighted(&choices, ctx.sounds.rng());
                    store.previous_ambient = message;
                    store.set_bubble_text(message, 800, false, player, &mut ctx);
                    store.ambient_countdown = ctx.sounds.rng().range_int(500, 1000);
                }
            }
        }
    }
    store.update_mouse(player, cursor.0, &mut ctx);
}

/// `StoreScreen::MouseDown`, `MouseUp`, `KeyChar` and the buttons.
#[allow(clippy::too_many_arguments)]
fn store_input(
    mut commands: Commands,
    mut store: ResMut<StoreScreen>,
    cursor: Res<GameCursor>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut dialogs: ResMut<Dialogs>,
    mut effects: ResMut<EffectSystem>,
    mut resources: ResMut<GameResources>,
    mut images: ResMut<Assets<Image>>,
    mut sounds: Sounds,
    mut profiles: ResMut<ProfileMgr>,
    return_to: Option<Res<StoreReturn>>,
    overlay: Option<Res<StoreOverlay>>,
    mut music: ResMut<Music>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let back_state = return_to.as_ref().and_then(|r| r.0).unwrap_or(AppState::GameSelector);
    let store = &mut *store;
    let mut ctx = Ctx { effects: &mut effects, res: &mut resources, images: &mut images, sounds: &mut sounds };
    if let Some((id, yes)) = store.dialog_answer.take() {
        match id {
            DialogId::StorePurchase => store.finish_purchase(yes, &mut profiles, &mut dialogs, &mut ctx),
            DialogId::VisitTree if yes => {
                if back_state == AppState::Board && overlay.is_some() {
                    store.dave.die(ctx.effects, ctx.sounds);
                    commands.remove_resource::<StoreScreen>();
                    commands.remove_resource::<StoreReturn>();
                    commands.remove_resource::<StoreOverlay>();
                }
                next_state.set(AppState::TreeOfWisdom);
            }
            _ => {}
        }
    }
    let Some(player) = profiles.player() else { return };

    // A dialog over the store takes the mouse and keys.
    if dialogs.is_modal() || dialogs.took_input() {
        for button in [&mut store.prev, &mut store.next] {
            button.is_over = false;
            button.is_down = false;
        }
        store.back.is_over = false;
        store.back.is_down = false;
        return;
    }
    if keys.any_just_pressed([KeyCode::Space, KeyCode::Enter]) {
        store.advance_dialog(player, &mut ctx);
    }

    let mut result = update_buttons(&mut [&mut store.prev, &mut store.next], &cursor, &mouse);

    // The Main Menu button.
    let over_back = store.back_enabled && cursor.0.is_some_and(|p| store.back.contains(p.as_ivec2()));
    store.back.is_over = over_back;
    if mouse.just_pressed(MouseButton::Left) && over_back {
        store.back.is_down = true;
        ctx.sounds.play_sample("SOUND_BUTTONCLICK");
        result.background_press = None;
    }
    if mouse.just_released(MouseButton::Left) {
        if store.back.is_down && over_back {
            store.back.is_down = false;
            if back_state == AppState::Board && overlay.is_some() {
                // Overlay store: back to the live board, Dave leaves with it.
                store.dave.die(&mut effects, &mut sounds);
                music.make_sure_music_is_playing(MusicTune::ChooseYourSeeds);
                commands.remove_resource::<StoreScreen>();
                commands.remove_resource::<StoreReturn>();
                commands.remove_resource::<StoreOverlay>();
                return;
            }
            next_state.set(back_state);
        }
        store.back.is_down = false;
    }

    for event in result.events {
        if let ButtonEvent::Depress(arrow) = event {
            store.turn_page(arrow, player, &mut ctx);
        }
    }

    if let Some(at) = result.background_press {
        if store.bubble_click_to_continue {
            store.advance_dialog(player, &mut ctx);
        } else if store.can_interact() {
            let clicked = (0..MAX_PAGE_SPOTS).find_map(|spot| store.page.item(spot).filter(|_| item_rect(spot).contains(at)));
            if let Some(item) = clicked.filter(|&item| can_buy(player, item)) {
                store.begin_purchase(item, player, &mut dialogs, &mut ctx);
            }
        }
    }
}

/// An image at (x, y), or glowing over what's already drawn.
fn draw_icon_image(g: &mut Graphics, res: &GameResources, id: &str, x: i32, y: i32, glow_alpha: Option<u8>) {
    let Some(image) = res.image(id) else { return };
    let (x, y) = (x as f32, y as f32);
    match glow_alpha {
        None => g.draw_image(image, x, y),
        Some(alpha) => {
            g.push_state();
            g.set_additive(true);
            g.set_colorize_images(true);
            g.set_color(SexyColor::rgba(255, 255, 255, alpha));
            let center = Vec2::new(x + image.width as f32 / 2.0, y + image.height as f32 / 2.0);
            g.draw_image_matrix(image, Affine2::from_translation(center));
            g.pop_state();
        }
    }
}

/// Replaces `{KEY}` placeholders with numbers (`TodReplaceNumberString`).
fn replace_number(text: &str, key: &str, value: i32) -> String {
    text.replace(&format!("{{{key}}}"), &value.to_string())
}

/// `DrawItemIcon`; `highlight` draws the glow for the item under the mouse.
fn draw_item_icon(g: &mut Graphics, res: &GameResources, player: &PlayerInfo, spot: usize, item: StoreItem, highlight: bool) {
    use StoreItem::*;
    let (x, y) = spot_position(spot);
    let glow = highlight.then_some(96);
    let image = |g: &mut Graphics, id: &str, dx: i32, dy: i32| draw_icon_image(g, res, id, x + dx, y + dy, glow);
    let white = SexyColor::WHITE;
    match item {
        PacketUpgrade => {
            draw_icon_image(g, res, "IMAGE_STORE_PACKETUPGRADE", x - 7, y + 7, highlight.then_some(32));
            let slots = replace_number(&res.translate("[STORE_UPGRADE_SLOTS]"), "SLOTS", player.purchases[item.slot()] + 7);
            if let Some(font) = res.font("FONT_HOUSEOFTERROR16") {
                let rect = SexyRect::new(x as f32, (y + 6) as f32, 55.0, 70.0);
                g.tod_draw_string_wrapped(font, rect, &slots, white, Justify::CenterMiddle);
            }
        }
        PoolCleaner => image(g, "IMAGE_ICON_POOLCLEANER", 1, 7),
        Rake => image(g, "IMAGE_ICON_RAKE", -5, 10),
        RoofCleaner => image(g, "IMAGE_ICON_ROOFCLEANER", 0, 28),
        Imitater => image(g, "IMAGE_IMITATERSEED", 0, 0),
        MushroomGarden => image(g, "IMAGE_STORE_MUSHROOMGARDENICON", -8, 2),
        AquariumGarden => image(g, "IMAGE_STORE_AQUARIUMGARDENICON", -8, 2),
        TreeOfWisdom => image(g, "IMAGE_STORE_TREEOFWISDOMICON", -8, 2),
        FirstAid => image(g, "IMAGE_STORE_FIRSTAIDWALLNUTICON", -1, 13),
        TreeFood => image(g, "IMAGE_TREEFOOD", -8, -2),
        StinkyTheSnail => image(g, "IMAGE_REANIM_STINKY_TURN3", -24, 14),
        GoldWateringCan => image(g, "IMAGE_WATERINGCANGOLD", -14, -4),
        Fertilizer | BugSpray => {
            let (id, dx, dy) = if item == Fertilizer { ("IMAGE_FERTILIZER", -11, -2) } else { ("IMAGE_BUG_SPRAY", -12, 3) };
            image(g, id, dx, dy);
            if let Some(font) = res.font("FONT_HOUSEOFTERROR16") {
                g.tod_draw_string(font, x + 56, y + 62, "x5", white, Justify::Right);
            }
        }
        Phonograph => image(g, "IMAGE_PHONOGRAPH", -12, 3),
        GardeningGlove => image(g, "IMAGE_ZEN_GARDENGLOVE", -12, 3),
        WheelBarrow => image(g, "IMAGE_ZEN_WHEELBARROW", -12, 3),
        PottedMarigold1 | PottedMarigold2 | PottedMarigold3 => {
            crate::zen_garden::draw_potted_plant_icon(g, res, x as f32 - 6.0, y as f32 + 4.0)
        }
        _ => {
            if let Some(seed) = item.seed() {
                draw_seed_packet_free(g, res, x as f32, y as f32, seed);
            }
        }
    }
}

/// `DrawItem`: the icon, its price and what stops it being bought.
fn draw_item(g: &mut Graphics, res: &GameResources, player: &PlayerInfo, spot: usize, item: StoreItem, hovered: bool) {
    if is_unavailable(player, item) {
        return;
    }
    draw_item_icon(g, res, player, spot, item, false);
    let (x, y) = spot_position(spot);
    if let Some(tag) = res.image("IMAGE_STORE_PRICETAG") {
        g.draw_image(tag, (x - 3) as f32, (y + 70) as f32);
    }
    if let Some(font) = res.font("FONT_BRIANNETOD12") {
        let cost = money_string(item_cost(player, item));
        g.tod_draw_string(font, x + 23, y + 85, &cost, SexyColor::BLACK, Justify::Center);
    }
    let red = SexyColor::rgba(255, 0, 0, 255);
    let message = |g: &mut Graphics, key: &str, width: f32| {
        if let Some(font) = res.font("FONT_HOUSEOFTERROR16") {
            let rect = SexyRect::new(x as f32, y as f32, width, 70.0);
            g.tod_draw_string_wrapped(font, rect, &res.translate(key), red, Justify::CenterMiddle);
        }
    };
    if is_coming_soon(player, item) {
        message(g, "[COMING_SOON]", 60.0);
    } else if is_sold_out(player, item) {
        message(g, "[SOLD_OUT]", 50.0);
    } else if hovered {
        // The packets flash; everything else glows.
        if item.seed().is_some() {
            if let Some(flash) = res.image("IMAGE_SEEDPACKETFLASH") {
                g.draw_image(flash, x as f32, y as f32);
            }
        } else {
            draw_item_icon(g, res, player, spot, item, true);
        }
    }
}

/// `StoreScreen::Draw`.
fn draw_store(
    store: Res<StoreScreen>,
    resources: Res<GameResources>,
    effects: Res<EffectSystem>,
    profiles: Res<ProfileMgr>,
    mut g: ResMut<Graphics>,
) {
    let (res, g) = (&*resources, &mut *g);
    let Some(player) = profiles.player() else { return };
    let image = |g: &mut Graphics, id: &str, x: i32, y: i32| {
        if let Some(image) = res.image(id) {
            g.draw_image(image, x as f32, y as f32);
        }
    };
    let sign_y = animate_curve(50, 110, store.store_time, -150, 0, TodCurve::EaseInOut);
    // `LawnApp::IsNight`: the store is drawn at night for the night worlds.
    let night = matches!(player.level, 11..=20 | 31..=40 | 50);
    image(g, if night { "IMAGE_STORE_BACKGROUNDNIGHT" } else { "IMAGE_STORE_BACKGROUND" }, 0, 0);
    let hatch_up = store.hatch_timer == 0 && store.hatch_open;
    if hatch_up {
        image(g, "IMAGE_STORE_CAR", 196, 138);
        image(g, "IMAGE_STORE_HATCHBACKOPEN", 299, 0);
    } else {
        image(g, "IMAGE_STORE_CARCLOSED", 196, 138);
    }
    image(g, "IMAGE_STORE_SIGN", 285, sign_y);

    g.push_state();
    g.translate(-42.0, 68.0);
    store.dave.draw(&effects, res, g);
    g.pop_state();

    if hatch_up {
        for spot in 0..MAX_PAGE_SPOTS {
            if let Some(item) = store.page.item(spot) {
                draw_item(g, res, player, spot, item, store.mouse_over == Some(item));
            }
        }
    }

    // The coin bank.
    const COINBANK: (i32, i32) = (650, 559);
    image(g, "IMAGE_COINBANK", COINBANK.0, COINBANK.1);
    if let Some(font) = res.font("FONT_CONTINUUMBOLD14") {
        let label = money_string(player.coins);
        g.push_state();
        g.set_color(SexyColor::rgba(180, 255, 90, 255));
        g.draw_string(font, COINBANK.0 + 111 - font.font.string_width(&label), COINBANK.1 + 24, &label);
        g.pop_state();
    }

    // Which page this is, when there is more than one.
    if !store.prev.disabled
        && let Some(font) = res.font("FONT_BRIANNETOD12")
    {
        let shown = PAGES.iter().filter(|p| p.is_shown(player)).count() as i32;
        let text = replace_number(&res.translate("[STORE_PAGE]"), "PAGE", store.page.index() as i32 + 1);
        let text = replace_number(&text, "NUM_PAGES", shown);
        g.tod_draw_string(font, 470, COINBANK.1, &text, SexyColor::rgba(80, 80, 80, 255), Justify::Center);
    }

    store.back.draw(g, res);
    for (button, disabled_image) in
        [(&store.prev, "IMAGE_STORE_PREVBUTTONDISABLED"), (&store.next, "IMAGE_STORE_NEXTBUTTONDISABLED")]
    {
        if store.arrows_disabled {
            image(g, disabled_image, button.x, button.y);
        } else {
            button.draw(g, res);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use StoreItem::*;

    fn player(level: i32, coins: i32) -> PlayerInfo {
        PlayerInfo { level, coins, ..PlayerInfo::default() }
    }

    fn finished() -> PlayerInfo {
        PlayerInfo { finished_adventure: 1, coins: 100_000, ..PlayerInfo::default() }
    }

    #[test]
    fn items_sit_where_the_original_puts_them() {
        assert_eq!(StorePage::SlotUpgrades.item(0), Some(PacketUpgrade));
        assert_eq!(StorePage::SlotUpgrades.item(7), Some(Cattail));
        assert_eq!(StorePage::PlantUpgrades.item(5), Some(FirstAid));
        assert_eq!(StorePage::PlantUpgrades.item(6), None);
        assert_eq!(StorePage::Zen2.item(4), Some(TreeOfWisdom));
        // The enum's order is the purchases array's.
        assert_eq!(PacketUpgrade.slot(), crate::board::STORE_ITEM_PACKET_UPGRADE);
        assert_eq!(Imitater.seed(), Some(SeedType::Imitater));
        assert_eq!(GatlingPea.seed(), Some(SeedType::Gatlingpea));
        assert_eq!(Rake.seed(), None);
    }

    #[test]
    fn pages_open_up_with_the_adventure() {
        let early = player(20, 0);
        assert!(StorePage::SlotUpgrades.is_shown(&early));
        assert!(!StorePage::PlantUpgrades.is_shown(&early));
        assert!(StorePage::PlantUpgrades.is_shown(&player(42, 0)));
        assert!(!StorePage::Zen1.is_shown(&player(44, 0)));
        assert!(StorePage::Zen1.is_shown(&player(45, 0)));
        assert!(!StorePage::Zen2.is_shown(&player(50, 0)));
        assert!(PAGES.iter().all(|p| p.is_shown(&finished())));
    }

    #[test]
    fn costs_follow_the_price_list() {
        let mut p = player(1, 0);
        assert_eq!(item_cost(&p, GatlingPea), 500);
        assert_eq!(item_cost(&p, Imitater), 3000);
        assert_eq!(item_cost(&p, Rake), 20);
        assert_eq!(item_cost(&p, PacketUpgrade), 75);
        for (bought, cost) in [(1, 500), (2, 2000), (3, 8000)] {
            p.purchases[PacketUpgrade.slot()] = bought;
            assert_eq!(item_cost(&p, PacketUpgrade), cost);
        }
        assert_eq!(item_cost(&p, BonusLawnMower), 200);
        p.purchases[BonusLawnMower.slot()] = 1;
        assert_eq!(item_cost(&p, BonusLawnMower), 500);
    }

    #[test]
    fn late_items_wait_for_their_level() {
        let p = player(30, 0);
        assert!(is_unavailable(&p, GloomShroom));
        assert!(is_unavailable(&p, RoofCleaner));
        assert!(!is_unavailable(&player(35, 0), Cattail));
        assert!(is_unavailable(&player(41, 0), RoofCleaner));
        assert!(!is_unavailable(&player(42, 0), RoofCleaner));
        assert!(is_unavailable(&player(50, 0), Imitater));
        assert!(!is_unavailable(&finished(), Imitater));
        assert!(!is_unavailable(&p, GatlingPea));
    }

    #[test]
    fn coming_soon_and_sold_out() {
        let mut p = finished();
        assert!(is_coming_soon(&p, WheelBarrow));
        p.purchases[MushroomGarden.slot()] = 1;
        assert!(!is_coming_soon(&p, WheelBarrow));
        // Tree food needs the tree and one bought already (the first comes with it).
        assert!(is_coming_soon(&p, TreeFood));
        p.purchases[TreeOfWisdom.slot()] = 1;
        assert!(is_coming_soon(&p, TreeFood));
        p.purchases[TreeFood.slot()] = PURCHASE_COUNT_OFFSET;
        assert!(!is_coming_soon(&p, TreeFood));

        assert!(!is_sold_out(&p, GatlingPea));
        p.purchases[GatlingPea.slot()] = 1;
        assert!(is_sold_out(&p, GatlingPea));
        p.purchases[PacketUpgrade.slot()] = 3;
        assert!(!is_sold_out(&p, PacketUpgrade));
        p.purchases[PacketUpgrade.slot()] = 4;
        assert!(is_sold_out(&p, PacketUpgrade));
        p.purchases[Fertilizer.slot()] = PURCHASE_COUNT_OFFSET + 15;
        assert!(!is_sold_out(&p, Fertilizer));
        p.purchases[Fertilizer.slot()] = PURCHASE_COUNT_OFFSET + 16;
        assert!(is_sold_out(&p, Fertilizer));
    }

    #[test]
    fn buying_a_potted_plant_puts_it_in_the_garden() {
        let mut p = finished();
        p.coins = 300;
        assert_eq!(buy(&mut p, PottedMarigold1, 0), Some(Bought::Done));
        assert_eq!(p.coins, 50);
        assert_eq!(p.potted_plants.len(), 1);
        assert!(p.purchases[PottedMarigold1.slot()] > 1);
        assert!(!can_buy(&p, PottedMarigold1));
    }

    #[test]
    fn buying_pays_and_records() {
        let mut p = finished();
        p.coins = 1000;
        assert_eq!(buy(&mut p, GatlingPea, 0), Some(Bought::Done));
        assert_eq!((p.coins, p.purchases[GatlingPea.slot()]), (500, 1));
        // The new plant is now in the player's hands.
        assert!(crate::plant::has_seed_type(&p, SeedType::Gatlingpea));
        assert!(!can_buy(&p, GatlingPea));

        assert_eq!(buy(&mut p, Imitater, 0), None);
        assert_eq!((p.coins, p.purchases[Imitater.slot()]), (500, 0));

        assert_eq!(buy(&mut p, PacketUpgrade, 0), Some(Bought::Slots(7)));
        assert_eq!((p.coins, p.purchases[PacketUpgrade.slot()]), (425, 1));

        assert_eq!(buy(&mut p, Rake, 0), Some(Bought::Done));
        assert_eq!(p.purchases[Rake.slot()], 3);
        assert_eq!(buy(&mut p, StinkyTheSnail, 12345), Some(Bought::Done));
        assert_eq!(p.purchases[StinkyTheSnail.slot()], 12345);
    }

    #[test]
    fn consumables_count_from_the_offset() {
        let mut p = finished();
        buy(&mut p, Fertilizer, 0);
        assert_eq!(p.purchases[Fertilizer.slot()], PURCHASE_COUNT_OFFSET + 5);
        buy(&mut p, Fertilizer, 0);
        assert_eq!(p.purchases[Fertilizer.slot()], PURCHASE_COUNT_OFFSET + 10);
        buy(&mut p, BugSpray, 0);
        assert_eq!(p.purchases[BugSpray.slot()], PURCHASE_COUNT_OFFSET + 5);
        buy(&mut p, TreeFood, 0);
        assert_eq!(p.purchases[TreeFood.slot()], PURCHASE_COUNT_OFFSET + 1);
        assert_eq!(buy(&mut p, TreeOfWisdom, 0), Some(Bought::VisitTree));
        assert_eq!(p.challenge_records[GAMEMODE_TREE_OF_WISDOM], 1);
        assert_eq!(buy(&mut p, FirstAid, 0), Some(Bought::FirstAid));
    }

    #[test]
    fn dave_talks_about_what_the_mouse_is_over() {
        let mut p = player(1, 0);
        assert_eq!(item_message(&p, GatlingPea), 2000);
        assert_eq!(item_message(&p, PacketUpgrade), 2011);
        p.purchases[PacketUpgrade.slot()] = 2;
        assert_eq!(item_message(&p, PacketUpgrade), 2013);
    }

    #[test]
    fn ambient_remarks_follow_their_weights() {
        let mut rng = sexy::rand::MtRand::new(1);
        // The only message with weight is the one picked.
        for _ in 0..20 {
            assert_eq!(pick_weighted(&[(2015, 0), (2016, 100), (2017, 0)], &mut rng), 2016);
        }
    }

    #[test]
    fn spots_are_laid_out_in_two_rows() {
        assert_eq!(spot_position(0), (422, 206));
        assert_eq!(spot_position(3), (644, 206));
        assert_eq!(spot_position(4), (372, 310));
        assert_eq!(spot_position(7), (594, 310));
        assert!(item_rect(0).contains(Vec2::new(430.0, 250.0)));
        assert!(!item_rect(0).contains(Vec2::new(480.0, 250.0)));
    }
}
