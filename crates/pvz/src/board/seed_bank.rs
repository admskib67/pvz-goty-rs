//! The seed bank along the top of the board and its seed packets.
//!
//! Behaviour per ruslan831 decomp, `Lawn/SeedPacket.cpp`: `SeedBank::Draw`,
//! `UpdateWidth`, `SeedPacket::Update`/`Draw`/`SetPacketType`,
//! `DrawSeedPacket` and `SeedPacketDrawSeed`; `Plant::DrawSeedType` and
//! `ReanimatorCache::MakeCachedPlantFrame` for the plant pictures; and
//! `Board::GetSeedPacketPositionX`/`GetSeedBankExtraWidth`.

use crate::graphics::{Graphics, Justify, SexyColor, SexyRect};
use crate::plant::SeedType;
use crate::reanim::draw_reanimator_frame;
use crate::resources::GameResources;

use super::{Board, GameScene};

pub const SEEDBANK_MAX: usize = 10;
const SEED_PACKET_WIDTH: f32 = 50.0;

/// What a Slot Machine reel shows (`SEED_SLOT_MACHINE_*` or a plant).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotSymbol {
    Plant(SeedType),
    Sun,
    Diamond,
}

/// One seed packet. Coordinates are relative to the seed bank.
#[derive(Debug, Clone)]
pub struct SeedPacket {
    pub x: i32,
    pub y: i32,
    pub packet_type: Option<SeedType>,
    pub refresh_counter: i32,
    pub refresh_time: i32,
    pub refreshing: bool,
    pub active: bool,
    pub offset_x: i32,
    /// An imitater packet: `packet_type` is the plant it copies (`mImitaterType`).
    pub imitater: bool,
    /// Slot Machine's rolling display (`mSlotMachiningPosition` and the seed
    /// it is about to show).
    pub slot_symbol: Option<SlotSymbol>,
    pub slot_next: Option<SlotSymbol>,
    pub slot_countdown: i32,
    pub slot_position: f32,
    /// Beghouled's Shuffle/Crater button packet (`SEED_BEGHOULED_BUTTON_*`).
    pub button: Option<super::beghouled::BeghouledButton>,
}

impl Default for SeedPacket {
    fn default() -> Self {
        Self {
            x: 0,
            y: 8,
            packet_type: None,
            refresh_counter: 0,
            refresh_time: 0,
            refreshing: false,
            active: true,
            offset_x: 0,
            imitater: false,
            slot_symbol: None,
            slot_next: None,
            slot_countdown: 0,
            slot_position: 0.0,
            button: None,
        }
    }
}

impl SeedPacket {
    /// `SetPacketType`: expensive plants start the level recharging.
    pub fn set_packet_type(&mut self, seed: Option<SeedType>) {
        self.packet_type = seed;
        self.imitater = false;
        self.refresh_counter = 0;
        self.refresh_time = 0;
        self.refreshing = false;
        self.active = true;
        let Some(seed) = seed else { return };
        let refresh = seed.definition().refresh_time;
        if seed.is_upgrade() || refresh == 5000 {
            (self.refresh_time, self.refreshing, self.active) = (3500, true, false);
        } else if refresh == 3000 {
            (self.refresh_time, self.refreshing, self.active) = (2000, true, false);
        }
    }

    /// `SeedPacket::Update`: recharging. Returns true when the packet should
    /// flash as ready (`FlashIfReady`): at the start and when recharged.
    /// The slot machine's reels: a packet that jumps up by its spin
    /// position, showing a plant, a sun or a diamond.
    fn draw_slot(&self, board: &Board, res: &GameResources, g: &mut Graphics) {
        let Some(seeds) = res.image("IMAGE_SEEDS") else { return };
        let Some(symbol) = self.slot_symbol else {
            if let Some(seed) = self.packet_type {
                super::seed_bank::draw_seed_packet(g, res, 0.0, 0.0, seed, 0.0, 255, false);
            }
            return;
        };
        let (x, y) = (0.0, -70.0 * self.slot_position);
        match symbol {
            SlotSymbol::Sun => g.draw_image_cel(seeds, x, y, 5, 0),
            SlotSymbol::Diamond => g.draw_image_cel(seeds, x, y, 6, 0),
            SlotSymbol::Plant(seed) => super::seed_bank::draw_seed_packet(g, res, x, y, seed, 0.0, 255, false),
        }
        let _ = board;
    }

    pub fn update(&mut self, scene: GameScene, main_counter: i32) -> bool {
        if scene != GameScene::Playing || self.packet_type.is_none() {
            return false;
        }
        let mut flash = main_counter == 0;
        if !self.active && self.refreshing {
            self.refresh_counter += 1;
            if self.refresh_counter > self.refresh_time {
                self.refresh_counter = 0;
                self.refreshing = false;
                self.active = true;
                flash = true;
            }
        }
        flash
    }

    /// `SeedPacket::Draw`.
    fn draw(&self, board: &Board, res: &GameResources, g: &mut Graphics) {
        if let Some(button) = self.button {
            let Some(seeds) = res.image("IMAGE_SEEDS") else { return };
            let cel = if button == super::beghouled::BeghouledButton::Shuffle { 4 } else { 3 };
            g.draw_image_cel(seeds, 0.0, 0.0, cel, 0);
            return;
        }
        if board.minigame() == Some(super::MiniGame::SlotMachine) {
            self.draw_slot(board, res, g);
            return;
        }
        let Some(seed) = self.packet_type else { return };
        let mut percent_dark = 0.0;
        if !self.active {
            percent_dark = if self.refresh_time == 0 {
                1.0
            } else {
                (self.refresh_time - self.refresh_counter) as f32 / self.refresh_time as f32
            };
        }
        let cost = board.plant_cost(seed);
        let conveyor = board.has_conveyor_belt_seed_bank();
        let mut grayness = 255;
        if board.scene != GameScene::Playing {
            grayness = board.seed_bank.cut_scene_darken;
            percent_dark = 0.0;
        } else if board.tutorial_state == super::TutorialState::WhackPickSeed
            || (board.tutorial_state == super::TutorialState::Level1PickUpPeashooter
                && board.tutorial_timer == -1
                && seed == SeedType::Peashooter)
            || (board.tutorial_state.is_sunflower_pick_up() && seed == SeedType::Sunflower)
        {
            grayness = flashing_gray(board.main_counter, 75);
        } else if (!board.can_take_sun_money(cost) && !conveyor) || percent_dark > 1.0 || !board.planting_requirements_met(seed) {
            grayness = 128;
        }
        draw_seed_packet_as(g, res, self.offset_x as f32, 0.0, seed, self.imitater, percent_dark, grayness, !conveyor);
    }
}

/// The seed bank. Its position is relative to the board.
#[derive(Debug, Clone)]
pub struct SeedBank {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub num_packets: usize,
    pub packets: [SeedPacket; SEEDBANK_MAX],
    /// How dark the packets are during the intro (255 is normal).
    pub cut_scene_darken: i32,
    /// A conveyor belt bank (`HasConveyorBeltSeedBank`): packets ride in.
    pub conveyor: bool,
    conveyor_belt_counter: i32,
}

impl SeedBank {
    /// `SeedBank::SeedBank`.
    pub fn new(res: &GameResources) -> Self {
        Self {
            x: 0,
            y: 0,
            width: res.image("IMAGE_SEEDBANK").map_or(0, |i| i.width as i32),
            num_packets: 0,
            packets: Default::default(),
            cut_scene_darken: 255,
            conveyor: false,
            conveyor_belt_counter: 0,
        }
    }

    /// `Board::GetSeedBankExtraWidth`: banks with more than six packets are wider.
    pub fn extra_width(&self) -> i32 {
        match self.num_packets {
            0..=6 => 0,
            7 => 60,
            8 => 76,
            9 => 112,
            _ => 153,
        }
    }

    /// `Board::GetSeedPacketPositionX`.
    pub fn packet_x(&self, index: usize) -> i32 {
        let i = index as i32;
        if self.conveyor {
            return i * 50 + 91;
        }
        match self.num_packets {
            0..=7 => i * 59 + 85,
            8 => i * 54 + 81,
            9 => i * 52 + 80,
            _ => i * 51 + 79,
        }
    }

    /// `SeedBank::UpdateWidth`.
    pub fn update_width(&mut self, res: &GameResources) {
        self.width = res.image("IMAGE_SEEDBANK").map_or(0, |i| i.width as i32) + self.extra_width();
        for i in 0..SEEDBANK_MAX {
            self.packets[i].x = self.packet_x(i);
        }
    }

    /// `SeedBank::Draw`: the bank, its packets and the sun count. During the
    /// intro it stays put on screen while the board pans.
    pub fn draw(&self, board: &Board, res: &GameResources, g: &mut Graphics) {
        let Some(bank) = res.image("IMAGE_SEEDBANK") else { return };
        g.push_state();
        g.translate(self.x as f32, self.y as f32);
        if board.scene != GameScene::Playing {
            g.translate(-board.x as f32, -board.y as f32);
        }

        let extra = self.extra_width();
        let (w, h) = (bank.width as f32, bank.height as f32);
        if self.conveyor {
            if let Some(backdrop) = res.image("IMAGE_CONVEYORBELT_BACKDROP") {
                g.draw_image(backdrop, 83.0, 0.0);
            }
            if let Some(belt) = res.image("IMAGE_CONVEYORBELT") {
                g.draw_image_cel(belt, 90.0, 63.0, 0, (self.conveyor_belt_counter / 4 % 6) as u32);
            }
            g.clip_rect(90.0, 0.0, 501.0, super::BOARD_HEIGHT as f32);
        } else {
            g.draw_image(bank, 0.0, 0.0);
            let src = SexyRect::new(w - extra as f32 - 12.0, 0.0, extra as f32 + 12.0, h);
            g.draw_image_src(bank, w - 12.0, 0.0, src);
        }

        for packet in &self.packets[..self.num_packets] {
            g.push_state();
            g.translate(packet.x as f32, packet.y as f32);
            packet.draw(board, res, g);
            g.pop_state();
        }

        if board.minigame() == Some(super::MiniGame::SlotMachine)
            && let Some(overlay) = res.image("IMAGE_SLOTMACHINE_OVERLAY")
        {
            g.draw_image(overlay, 189.0, -2.0);
        }
        if !self.conveyor
            && let Some(font) = res.font("FONT_CONTINUUMBOLD14")
        {
            let flash = board.out_of_money_counter > 0 && board.out_of_money_counter % 20 < 10;
            let color = if flash { SexyColor::rgba(255, 0, 0, 255) } else { SexyColor::BLACK };
            g.tod_draw_string(font, 34, 78, &board.sun_money.max(0).to_string(), color, Justify::Center);
        }
        g.pop_state();
    }
}

impl SeedBank {
    /// `SeedBank::GetNumSeedsOnConveyorBelt`.
    pub fn num_seeds_on_conveyor_belt(&self) -> usize {
        self.packets[..self.num_packets].iter().position(|p| p.packet_type.is_none()).unwrap_or(self.num_packets)
    }

    /// `SeedBank::CountOfTypeOnConveyorBelt`.
    pub fn count_of_type_on_conveyor_belt(&self, seed: SeedType) -> usize {
        self.packets[..self.num_packets].iter().filter(|p| p.packet_type == Some(seed)).count()
    }

    /// `SeedBank::AddSeed`: a new packet rides in from the right.
    pub fn add_seed(&mut self, seed: SeedType) {
        let n = self.num_seeds_on_conveyor_belt();
        if n == self.num_packets {
            return;
        }
        let prev_offset = (n > 0).then(|| self.packets[n - 1].offset_x);
        let packet = &mut self.packets[n];
        *packet = SeedPacket { x: packet.x, y: packet.y, packet_type: Some(seed), ..SeedPacket::default() };
        packet.offset_x = 515 - (SEED_PACKET_WIDTH as i32 + 1) * n as i32;
        if let Some(prev) = prev_offset
            && packet.offset_x < prev
        {
            packet.offset_x = prev + 40;
        }
    }

    /// `SeedBank::RemoveSeed`: the packets after it shift one place left.
    pub fn remove_seed(&mut self, index: usize) {
        for i in index..self.num_packets {
            if self.packets[i].packet_type.is_none() {
                break;
            }
            let next = (i + 1 < self.num_packets).then(|| (self.packets[i + 1].packet_type, self.packets[i + 1].offset_x));
            let packet = &mut self.packets[i];
            match next {
                Some((ty, offset)) => {
                    packet.packet_type = ty;
                    packet.offset_x = offset + SEED_PACKET_WIDTH as i32 + 1;
                }
                None => {
                    packet.packet_type = None;
                    packet.offset_x = 0;
                }
            }
            packet.refresh_counter = 0;
            packet.refresh_time = 0;
            packet.refreshing = false;
            packet.active = true;
        }
    }

    /// `SeedBank::UpdateConveyorBelt`: the packets creep left.
    pub fn update_conveyor_belt(&mut self) {
        self.conveyor_belt_counter += 1;
        if self.conveyor_belt_counter % 4 == 0 {
            for packet in &mut self.packets[..self.num_packets] {
                packet.offset_x = (packet.offset_x - 1).max(0);
            }
        }
    }
}

/// `GetFlashingColor`'s gray level: pulses between 55 and 255.
pub fn flashing_gray(counter: i32, flash_time: i32) -> i32 {
    let age = counter % flash_time;
    let half = flash_time / 2;
    (55 + 200 * (half - age).abs() / half).clamp(0, 255)
}

/// `DrawSeedPacket`: a packet's background, its plant, its recharge shading
/// and its cost, top-left at (x, y).
#[allow(clippy::too_many_arguments)]
pub fn draw_seed_packet(
    g: &mut Graphics,
    res: &GameResources,
    x: f32,
    y: f32,
    seed: SeedType,
    percent_dark: f32,
    grayness: i32,
    draw_cost: bool,
) {
    draw_seed_packet_as(g, res, x, y, seed, false, percent_dark, grayness, draw_cost);
}

/// `DrawSeedPacket` for an imitater packet when `imitater` is set: the
/// imitater's background with a washed-out copy of the plant.
#[allow(clippy::too_many_arguments)]
pub fn draw_seed_packet_as(
    g: &mut Graphics,
    res: &GameResources,
    x: f32,
    y: f32,
    seed: SeedType,
    imitater: bool,
    percent_dark: f32,
    grayness: i32,
    draw_cost: bool,
) {
    let Some(seeds) = res.image("IMAGE_SEEDS") else { return };
    g.push_state();
    if grayness != 255 {
        let v = grayness.clamp(0, 255) as u8;
        g.set_color(SexyColor::rgba(v, v, v, 255));
        g.set_colorize_images(true);
    } else if percent_dark > 0.0 {
        g.set_color(SexyColor::rgba(128, 128, 128, 255));
        g.set_colorize_images(true);
    }

    let background = if seed == SeedType::Imitater || imitater {
        0
    } else if seed.is_upgrade() {
        1
    } else {
        2
    };
    g.draw_image_cel(seeds, x, y, background, 0);

    let (scale, offset_x, offset_y) = packet_seed_placement(seed);
    let offset_y = offset_y + 1.0;
    let washed = imitater && grayness == 255 && percent_dark <= 0.0;
    if washed {
        g.push_state();
        g.set_color(SexyColor::rgba(185, 185, 185, 255));
        g.set_colorize_images(true);
    }
    seed_packet_draw_seed(g, res, x, y, seed, offset_x, offset_y, scale);
    if washed {
        g.pop_state();
    }

    if percent_dark > 0.0 {
        let dark_height = sexy::tod::float_round_to_int(68.0 * percent_dark) + 2;
        g.push_state();
        g.set_color(SexyColor::rgba(64, 64, 64, 255));
        g.set_colorize_images(true);
        g.clip_rect(x, y, SEED_PACKET_WIDTH, dark_height as f32);
        g.draw_image_cel(seeds, x, y, background, 0);
        seed_packet_draw_seed(g, res, x, y, seed, offset_x, offset_y, scale);
        g.pop_state();
    }

    if draw_cost && let Some(font) = res.font("FONT_PICO129") {
        let cost = seed.cost().to_string();
        let text_x = 32 - font.font.string_width(&cost);
        let text_y = font.font.ascent + 54;
        g.tod_draw_string(font, x as i32 + text_x, y as i32 + text_y, &cost, SexyColor::BLACK, Justify::Left);
    }
    g.pop_state();
}

/// Where `DrawSeedPacket` puts a plant in its packet: (scale, x, y).
fn packet_seed_placement(seed: SeedType) -> (f32, f32, f32) {
    use SeedType::*;
    match seed {
        Tallnut => (0.3, 12.0, 22.0),
        InstantCoffee => (0.55, 0.0, 9.0),
        Cobcannon => (0.26, 6.0, 22.0),
        Cactus => (0.5, 9.0, 13.0),
        Potatomine => (0.4, 8.0, 12.0),
        Magnetshroom => (0.5, 5.0, 12.0),
        Fumeshroom | Pumpkinshell | Chomper | Doomshroom | Squash | Hypnoshroom | Spikeweed | Spikerock | Plantern
        | Torchwood | Tanglekelp => (0.4, 8.0, 12.0),
        Twinsunflower | Gloomshroom => (0.45, 7.0, 14.0),
        Cattail => (0.45, 8.0, 13.0),
        Umbrella => (0.5, 5.0, 10.0),
        Kernelpult => (0.4, 13.0, 14.0),
        Cabbagepult => (0.4, 15.0, 14.0),
        Melonpult | Wintermelon => (0.35, 18.0, 19.0),
        Gravebuster => (0.4, 10.0, 15.0),
        Splitpea => (0.45, 12.0, 12.0),
        Blover => (0.4, 8.0, 17.0),
        Starfruit => (0.5, 6.0, 8.0),
        Threepeater => (0.5, 5.0, 10.0),
        Gatlingpea => (0.5, 2.0, 8.0),
        GiantWallnut => (0.5 * 0.75, 52.0, 58.0),
        _ => (0.5, 5.0, 8.0),
    }
}

/// `SeedPacketDrawSeed`: some plants have a hand-drawn packet picture in
/// `IMAGE_PACKET_PLANTS`; the rest are drawn from their reanimation.
#[allow(clippy::too_many_arguments)]
fn seed_packet_draw_seed(
    g: &mut Graphics,
    res: &GameResources,
    x: f32,
    y: f32,
    seed: SeedType,
    offset_x: f32,
    offset_y: f32,
    scale: f32,
) {
    use SeedType::*;
    let cel = match seed {
        Potatomine => Some(0),
        Chomper => Some(1),
        Hypnoshroom => Some(2),
        Tallnut => Some(3),
        Blover => Some(4),
        Pumpkinshell => Some(5),
        Twinsunflower => Some(6),
        Cobcannon => Some(7),
        Cabbagepult => Some(8),
        Kernelpult => Some(9),
        Melonpult => Some(10),
        Wintermelon => Some(11),
        Spikerock => Some(12),
        _ => None,
    };
    match (cel, res.image("IMAGE_PACKET_PLANTS")) {
        (Some(cel), Some(image)) => g.draw_image_cel(image, x, y, cel, 0),
        _ => draw_seed_type(g, res, seed, x + offset_x, y + offset_y, scale),
    }
}

/// `Plant::DrawSeedType` (through `ReanimatorCache::DrawCachedPlant`): a
/// plant's idle pose, its top-left-ish origin at (x, y), scaled.
pub fn draw_seed_type(g: &mut Graphics, res: &GameResources, seed: SeedType, x: f32, y: f32, scale: f32) {
    use SeedType::*;
    let reanim = seed.definition().reanim;
    let frame = |g: &mut Graphics, track: &str, extra_scale: f32, shift: f32| {
        draw_reanimator_frame(g, res, x + shift * scale, y + shift * scale, reanim, Some(track), scale * extra_scale);
    };
    match seed {
        // MakeCachedPlantFrame draws these smaller and nudged inwards.
        Potatomine => frame(g, "anim_armed", 0.85, 12.0),
        InstantCoffee => frame(g, "anim_idle", 0.8, 12.0),
        // `MakeCachedPlantFrame`: the explode-o-nut's art is tinted red.
        ExplodeONut => {
            g.push_state();
            g.set_colorize_images(true);
            g.set_color(crate::graphics::SexyColor::rgba(255, 64, 64, 255));
            frame(g, "anim_idle", 1.0, 0.0);
            g.pop_state();
        }
        // `DrawSeedType`: the left repeater's art is mirrored.
        Leftpeater => {
            let mirrored = |g: &mut Graphics, track: &str| {
                crate::reanim::draw_reanimator_frame_scaled(g, res, x + 80.0 * scale, y, reanim, Some(track), -scale, scale);
            };
            mirrored(g, "anim_idle");
            mirrored(g, "anim_head_idle");
        }
        _ => {
            frame(g, "anim_idle", 1.0, 0.0);
            match seed {
                Peashooter | Snowpea | Repeater | Leftpeater | Gatlingpea => frame(g, "anim_head_idle", 1.0, 0.0),
                Splitpea => {
                    frame(g, "anim_head_idle", 1.0, 0.0);
                    frame(g, "anim_splitpea_idle", 1.0, 0.0);
                }
                Threepeater => {
                    for head in ["anim_head_idle1", "anim_head_idle3", "anim_head_idle2"] {
                        frame(g, head, 1.0, 0.0);
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expensive_plants_start_recharging() {
        let mut packet = SeedPacket::default();
        packet.set_packet_type(Some(SeedType::Peashooter));
        assert!(packet.active);
        packet.set_packet_type(Some(SeedType::Cherrybomb));
        assert!(!packet.active && packet.refreshing && packet.refresh_time == 3500);
        packet.set_packet_type(Some(SeedType::Wallnut));
        assert_eq!(packet.refresh_time, 2000);
    }
}
