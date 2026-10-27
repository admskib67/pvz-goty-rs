//! Mini-games: the challenge screen's `Challenge` page, one entry per
//! `gChallengeDefs` row, and the board rules each one changes.
//!
//! Behaviour per ruslan831 decomp, `Lawn/Widget/ChallengeScreen.cpp`
//! (`gChallengeDefs`), `Lawn/Challenge.cpp` (`InitLevel`, `StartLevel`,
//! `InitZombieWaves`) and `Lawn/Board.cpp` (`PickBackground`, the
//! `mNumWaves` table, `PickZombieWaves`' per-mode points and fixed waves).
//!
//! Ported so far (`MiniGame::is_ported`): War and Peas 1 and 2, Wall-nut
//! Bowling 1 and 2, Big Trouble Little Zombie, Bobsled Bonanza,
//! Whack a Zombie, Pogo Party, Column Like You See 'Em, Invisighoul and
//! Dr. Zomboss's Revenge. The rest of the page shows as Coming Soon.
//!
//! Not yet: the per-game extras for the rest of the page (Slot Machine,
//! Raining Seeds, Beghouled 1 and 2, Seeing Stars, Zombiquarium, Portal
//! Combat, Zombies on Speed, Last Stand) and War and Peas' debug-only head
//! spawns.

use sexy::rand::MtRand;

use crate::audio::Sounds;
use crate::plant::SeedType;
use crate::reanim::EffectSystem;
use crate::resources::GameResources;
use crate::zombie::ZombieType;

use super::waves::{WaveState, ZOMBIE_COUNTDOWN_FIRST_WAVE};
use super::{BackgroundType, Board, MAX_GRID_SIZE_Y, PlantRowType};

/// `gChallengeDefs`' entries on `CHALLENGE_PAGE_CHALLENGE`, in page order
/// (`mRow * 5 + mCol`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MiniGame {
    WarAndPeas,
    WallnutBowling,
    SlotMachine,
    RainingSeeds,
    Beghouled,
    Invisighoul,
    SeeingStars,
    Zombiquarium,
    BeghouledTwist,
    LittleTrouble,
    PortalCombat,
    Column,
    BobsledBonanza,
    Speed,
    WhackAZombie,
    LastStand,
    WarAndPeas2,
    WallnutBowling2,
    PogoParty,
    FinalBoss,
}

impl MiniGame {
    pub const ALL: [Self; 20] = [
        Self::WarAndPeas,
        Self::WallnutBowling,
        Self::SlotMachine,
        Self::RainingSeeds,
        Self::Beghouled,
        Self::Invisighoul,
        Self::SeeingStars,
        Self::Zombiquarium,
        Self::BeghouledTwist,
        Self::LittleTrouble,
        Self::PortalCombat,
        Self::Column,
        Self::BobsledBonanza,
        Self::Speed,
        Self::WhackAZombie,
        Self::LastStand,
        Self::WarAndPeas2,
        Self::WallnutBowling2,
        Self::PogoParty,
        Self::FinalBoss,
    ];

    /// `mChallengeMode` on the challenge page: the tile's order.
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&m| m == self).unwrap_or(0)
    }

    /// `mRow`.
    pub fn row(self) -> i32 {
        (self.index() / 5) as i32
    }

    /// `mCol`.
    pub fn col(self) -> i32 {
        (self.index() % 5) as i32
    }

    /// `mChallengeIconIndex`: the cel in `Challenge_Thumbnails`.
    pub fn icon(self) -> u32 {
        match self {
            Self::WarAndPeas | Self::WarAndPeas2 => 0,
            Self::WallnutBowling | Self::WallnutBowling2 => 6,
            Self::SlotMachine => 2,
            Self::RainingSeeds => 3,
            Self::Beghouled => 1,
            Self::Invisighoul => 8,
            Self::SeeingStars => 5,
            Self::Zombiquarium => 7,
            Self::BeghouledTwist => 20,
            Self::LittleTrouble => 12,
            Self::PortalCombat => 15,
            Self::Column => 4,
            Self::BobsledBonanza => 17,
            Self::Speed => 18,
            Self::WhackAZombie => 16,
            Self::LastStand => 21,
            Self::PogoParty => 14,
            Self::FinalBoss => 19,
        }
    }

    /// `mChallengeName`, the `LawnStrings` key.
    pub fn name(self) -> &'static str {
        match self {
            Self::WarAndPeas => "[WAR_AND_PEAS]",
            Self::WallnutBowling => "[WALL_NUT_BOWLING]",
            Self::SlotMachine => "[SLOT_MACHINE]",
            Self::RainingSeeds => "[ITS_RAINING_SEEDS]",
            Self::Beghouled => "[BEGHOULED]",
            Self::Invisighoul => "[INVISIGHOUL]",
            Self::SeeingStars => "[SEEING_STARS]",
            Self::Zombiquarium => "[ZOMBIQUARIUM]",
            Self::BeghouledTwist => "[BEGHOULED_TWIST]",
            Self::LittleTrouble => "[LITTLE_TROUBLE]",
            Self::PortalCombat => "[PORTAL_COMBAT]",
            Self::Column => "[COLUMN_AS_YOU_SEE_EM]",
            Self::BobsledBonanza => "[BOBSLED_BONANZA]",
            Self::Speed => "[ZOMBIES_ON_SPEED]",
            Self::WhackAZombie => "[WHACK_A_ZOMBIE]",
            Self::LastStand => "[LAST_STAND]",
            Self::WarAndPeas2 => "[WAR_AND_PEAS_2]",
            Self::WallnutBowling2 => "[WALL_NUT_BOWLING_EXTREME]",
            Self::PogoParty => "[POGO_PARTY]",
            Self::FinalBoss => "[FINAL_BOSS]",
        }
    }

    /// Whether this port plays the game; the rest of the page says so.
    pub fn is_ported(self) -> bool {
        matches!(
            self,
            Self::WarAndPeas
                | Self::WallnutBowling
                | Self::LittleTrouble
                | Self::BobsledBonanza
                | Self::WhackAZombie
                | Self::WarAndPeas2
                | Self::WallnutBowling2
                | Self::PogoParty
                | Self::Column
                | Self::Invisighoul
                | Self::FinalBoss
                | Self::Speed
                | Self::SeeingStars
                | Self::RainingSeeds
                | Self::PortalCombat
        )
    }

    /// `PickBackground`'s per-mode lawn.
    pub fn background(self) -> BackgroundType {
        match self {
            Self::WarAndPeas | Self::WallnutBowling | Self::SeeingStars | Self::WallnutBowling2 => BackgroundType::Day,
            Self::SlotMachine => BackgroundType::Day,
            Self::Beghouled | Self::BeghouledTwist | Self::PortalCombat | Self::WhackAZombie => BackgroundType::Night,
            Self::LittleTrouble | Self::BobsledBonanza | Self::Speed | Self::LastStand | Self::WarAndPeas2 => BackgroundType::Pool,
            Self::RainingSeeds | Self::Invisighoul => BackgroundType::Fog,
            Self::Column | Self::PogoParty => BackgroundType::Roof,
            Self::Zombiquarium => BackgroundType::Day,
            Self::FinalBoss => BackgroundType::Boss,
        }
    }

    /// `Board`'s `mNumWaves` table for non-adventure modes.
    pub fn num_waves(self) -> i32 {
        match self {
            Self::WhackAZombie => 12,
            Self::WallnutBowling
            | Self::WarAndPeas
            | Self::Invisighoul
            | Self::PortalCombat => 20,
            Self::LittleTrouble
            | Self::Column
            | Self::WarAndPeas2
            | Self::WallnutBowling2
            | Self::PogoParty => 30,
            _ => 40,
        }
    }

    /// `Challenge::InitZombieWaves`'s list for the mode.
    pub fn allowed(self) -> &'static [ZombieType] {
        use ZombieType as Z;
        match self {
            Self::WarAndPeas => &[Z::PeaHead, Z::WallnutHead],
            Self::WarAndPeas2 => &[Z::PeaHead, Z::WallnutHead, Z::JalapenoHead, Z::GatlingHead, Z::SquashHead, Z::TallnutHead],
            Self::WallnutBowling => &[Z::Normal, Z::TrafficCone, Z::Pail, Z::Polevaulter, Z::Newspaper],
            Self::WallnutBowling2 => &[Z::Normal, Z::TrafficCone, Z::Pail, Z::Polevaulter, Z::Newspaper, Z::Dancer, Z::Door],
            Self::LittleTrouble => &[Z::Normal, Z::TrafficCone, Z::Football, Z::Snorkel],
            Self::BobsledBonanza => &[Z::Bobsled, Z::Zamboni],
            Self::WhackAZombie => &[Z::Normal, Z::TrafficCone, Z::Pail],
            Self::PogoParty => &[Z::Pogo],
            Self::Speed => &[Z::Normal, Z::TrafficCone, Z::DolphinRider, Z::Polevaulter],
            Self::PortalCombat => &[Z::Normal, Z::Pail, Z::Football, Z::Balloon],
            Self::SeeingStars => &[Z::Normal, Z::TrafficCone, Z::Pail],
            Self::RainingSeeds => &[
                Z::Normal,
                Z::TrafficCone,
                Z::Pail,
                Z::Door,
                Z::Football,
                Z::Newspaper,
                Z::JackInTheBox,
                Z::Bungee,
            ],
            Self::Column => &[Z::Normal, Z::TrafficCone, Z::Pail, Z::Football],
            Self::Invisighoul => &[Z::Normal, Z::TrafficCone, Z::Pail, Z::DolphinRider, Z::Zamboni, Z::JackInTheBox],
            // The boss level's own waves never run (`is_final_boss_level`);
            // the table is the reference's fallback.
            Self::FinalBoss => &[Z::Normal, Z::TrafficCone, Z::Pail],
            // Not ported yet: their tables are ignored until they are.
            _ => &[],
        }
    }

    /// `PickZombieWaves`' per-mode zombie-point multiplier.
    fn points_multiplier(self) -> i32 {
        match self {
            Self::LittleTrouble | Self::WallnutBowling | Self::WallnutBowling2 => 4,
            Self::Column => 6,
            Self::Invisighoul | Self::PortalCombat => 2,
            _ => 1,
        }
    }

    /// `Challenge::UpdateConveyorBelt`'s per-mode speed multiplier.
    pub fn conveyor_speed_multiplier(self) -> f32 {
        match self {
            Self::FinalBoss => 0.875,
            Self::Invisighoul => 2.0,
            Self::Column => 3.0,
            Self::PortalCombat => 1.5,
            _ => 1.0,
        }
    }

    /// War and Peas' flag waves take no plain or flag zombies: there are no
    /// ordinary zombies in the mode.
    pub fn plain_flag_waves(self) -> bool {
        !matches!(self, Self::WarAndPeas | Self::WarAndPeas2)
    }

    /// `Challenge::StartLevel`'s first-wave delay.
    pub fn first_wave_countdown(self) -> i32 {
        match self {
            Self::WallnutBowling | Self::WallnutBowling2 | Self::LittleTrouble | Self::WhackAZombie | Self::Invisighoul => 200,
            Self::Column => 2400,
            Self::PortalCombat => 200,
            Self::BobsledBonanza => 4500,
            Self::PogoParty => 5500,
            _ => ZOMBIE_COUNTDOWN_FIRST_WAVE,
        }
    }

    /// `PickZombieType` skips `mFirstAllowedWave` in these modes.
    pub fn ignores_first_allowed_wave(self) -> bool {
        matches!(self, Self::PogoParty | Self::BobsledBonanza)
    }
}

impl Board {
    /// The mini-game being played, if any.
    pub fn minigame(&self) -> Option<MiniGame> {
        match self.mode {
            super::GameMode::MiniGame(game) => Some(game),
            _ => None,
        }
    }

    pub fn is_minigame(&self) -> bool {
        self.minigame().is_some()
    }

    /// Challenge modes (survival and mini-games): level 50's adventure rules
    /// don't apply to them.
    pub fn is_challenge(&self) -> bool {
        self.is_survival() || self.is_minigame()
    }

    /// `Board::InitZombieWaves` for a mini-game.
    pub(super) fn init_minigame_waves(&mut self, rng: &mut MtRand) {
        let Some(game) = self.minigame() else { return };
        self.waves = WaveState::default();
        self.waves.num_waves = game.num_waves();
        self.waves.zombies_in_wave = vec![Vec::new(); self.waves.num_waves as usize];
        for &ty in game.allowed() {
            self.waves.zombie_allowed[ty as usize] = true;
        }
        self.pick_zombie_waves(rng);
        self.waves.zombie_count_down = game.first_wave_countdown();
        self.waves.zombie_count_down_start = self.waves.zombie_count_down;
        // `Challenge::StartLevel`: Column Like You See 'Em starts at wave 9.
        if game == MiniGame::Column {
            self.waves.current_wave = 9;
        }
        // `Challenge::StartLevel`: Bobsled Bonanza starts with the lawn frozen.
        if game == MiniGame::BobsledBonanza {
            for row in 0..MAX_GRID_SIZE_Y {
                if self.plant_row[row] != PlantRowType::Pool {
                    self.ice_min_x[row] = 400;
                    self.ice_timer[row] = i32::MAX;
                }
            }
        }
    }

    /// `PickZombieWaves`'s multiplier for the current mini-game.
    pub(super) fn minigame_points_multiplier(&self) -> i32 {
        self.minigame().map_or(1, MiniGame::points_multiplier)
    }

    /// `Challenge::UpdateConveyorBelt`'s speed for the current mode.
    pub(super) fn conveyor_speed_multiplier(&self) -> f32 {
        self.minigame().map_or(1.0, MiniGame::conveyor_speed_multiplier)
    }

    /// `Zombie::IsVisible`: Invisighoul's zombies aren't drawn (their bites
    /// still land). The level intro's street zombies are.
    pub fn zombie_hidden(&self, zombie: &super::zombie::Zombie) -> bool {
        self.minigame() == Some(MiniGame::Invisighoul) && zombie.from_wave != super::zombie::ZOMBIE_WAVE_CUTSCENE
    }

    /// `LawnApp::IsContinuousChallenge`: these modes don't end when the
    /// waves run out (an art challenge ends on its picture, the boss on the
    /// boss).
    pub fn is_continuous_challenge(&self) -> bool {
        matches!(self.minigame(), Some(MiniGame::FinalBoss | MiniGame::SeeingStars))
    }

    /// `gArtChallengeStarFruit`: Seeing Stars' ghost picture. A row is a
    /// grid row, a column a grid column.
    pub fn art_challenge_seed(&self, col: i32, row: i32) -> Option<SeedType> {
        if self.minigame() != Some(MiniGame::SeeingStars) || !(0..9).contains(&col) || !(0..6).contains(&row) {
            return None;
        }
        use SeedType::Starfruit;
        const N: Option<SeedType> = None;
        const S: Option<SeedType> = Some(Starfruit);
        const SHAPE: [[Option<SeedType>; 9]; 6] = [
            [N, N, N, S, N, N, N, N, N],
            [N, N, N, S, S, N, N, N, N],
            [N, S, S, S, S, S, S, N, N],
            [N, N, N, S, S, S, N, N, N],
            [N, N, N, S, N, N, S, N, N],
            [N, N, N, N, N, N, N, N, N],
        ];
        SHAPE[row as usize][col as usize]
    }

    /// `LawnApp::IsChallengeWithoutSeedBank`: It's Raining Seeds plants
    /// straight from the packets that fall.
    pub fn is_challenge_without_seed_bank(&self) -> bool {
        self.minigame() == Some(MiniGame::RainingSeeds)
    }

    /// `Challenge::UpdateRainingSeeds` + `Challenge::UpdateRain`.
    pub(super) fn update_raining_seeds(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut bevy::asset::Assets<bevy::image::Image>,
        sounds: &mut Sounds,
    ) {
        if self.minigame() != Some(MiniGame::RainingSeeds) || self.scene != super::GameScene::Playing || self.has_level_award_dropped() {
            return;
        }
        // The rain itself: splashes at three heights, like UpdateRain.
        self.rain_splash_counter -= 1;
        if self.rain_splash_counter < 0 {
            for (low, high, circle) in [(90.0, 240.0, false), (290.0, 410.0, true), (450.0, 580.0, false)] {
                let x = sounds.rng().range_f32(40.0, 740.0);
                let y = sounds.rng().range_f32(low, high);
                let ty = if circle { crate::reanim::ReanimationType::RainCircle } else { crate::reanim::ReanimationType::RainSplash };
                if let Ok((_, reanim)) = effects.add_reanimation(x, y, ty, res, images) {
                    let alpha = sounds.rng().range_int(100, 200) as u8;
                    reanim.color_override = [255, 255, 255, alpha];
                    let scale = sounds.rng().range_f32(0.7, 1.2);
                    reanim.override_scale(scale, scale);
                }
            }
            self.rain_splash_counter = sounds.rng().range_int(10, 20);
        }
        if self.raining_seeds_counter > 0 {
            self.raining_seeds_counter -= 1;
            return;
        }
        self.raining_seeds_counter = sounds.rng().range_int(500, 999);
        let seed = self.random_raining_seed(sounds.rng());
        let x = sounds.rng().range_int(100, 649) as f32;
        let id = self.add_coin(x, 60.0, super::coin::CoinType::UsableSeedPacket, super::coin::CoinMotion::FromSkySlow, effects, res, images, sounds);
        if let Some(coin) = self.coins.get_mut(id) {
            coin.usable_seed = Some(seed);
        }
    }

    /// `UpdateRainingSeeds`' pick: a plant the player owns that isn't a sun
    /// producer, an upgrade or one of the unusable kinds; lily pads get
    /// likelier as more are planted.
    fn random_raining_seed(&mut self, rng: &mut sexy::rand::MtRand) -> SeedType {
        use SeedType::*;
        let excluded = [Sunflower, Twinsunflower, Sunshroom, InstantCoffee, Umbrella, Imitater];
        let mut seed = None;
        for _ in 0..200 {
            let index = rng.range_int(0, 53);
            let Some(candidate) = SeedType::from_index(index) else { continue };
            if !self.owned_seeds.get(index as usize).copied().unwrap_or(false)
                || excluded.contains(&candidate)
                || candidate.upgrade_base().is_some()
            {
                continue;
            }
            seed = Some(candidate);
            break;
        }
        let seed = seed.unwrap_or(Peashooter);
        let lilies = self.plants.iter().filter(|(_, p)| !p.dead && p.seed == Lilypad).count() as i32;
        if rng.range_int(0, 100) < sexy::tod::animate_curve(0, 18, lilies, 30, 1, sexy::tod::TodCurve::Linear) {
            return Lilypad;
        }
        seed
    }

    /// The paired portal's cell (`GetOtherPortal`).
    fn other_portal_cell(&self, portal_type: super::grid_item::GridItemType, x: i32, y: i32) -> Option<(i32, i32)> {
        self.grid_items
            .iter()
            .find(|(_, g)| g.is_open_portal() && g.item_type == portal_type && (g.grid_x, g.grid_y) != (x, y))
            .map(|(_, g)| (g.grid_x, g.grid_y))
    }

    /// `Challenge::GetPortalToLeft`.
    fn portal_to_left(&self, grid_x: i32, grid_y: i32) -> Option<(super::grid_item::GridItemType, i32, i32)> {
        self.grid_items
            .iter()
            .filter(|(_, g)| g.is_open_portal() && g.grid_y == grid_y && g.grid_x < grid_x)
            .max_by_key(|(_, g)| g.grid_x)
            .map(|(_, g)| (g.item_type, g.grid_x, g.grid_y))
    }

    /// `Challenge::GetPortalDistanceToMower`.
    fn portal_distance_to_mower(&self, row: i32) -> i32 {
        let (mut grid_x, mut grid_y, mut distance) = (10, row, 0);
        let mut steps = 0;
        while distance < 40 && steps < 20 {
            steps += 1;
            let Some((ty, px, py)) = self.portal_to_left(grid_x, grid_y) else {
                distance += grid_x;
                break;
            };
            let Some((ox, oy)) = self.other_portal_cell(ty, px, py) else {
                distance += grid_x;
                break;
            };
            distance += grid_x - px;
            grid_x = ox;
            grid_y = oy;
        }
        distance
    }

    /// `Challenge::PortalCombatRowSpawnWeight`: the zombies favour the rows
    /// the portals lead into.
    pub(super) fn portal_row_weight(&self, row: i32) -> f32 {
        if self.portal_distance_to_mower(row) < 5 {
            return 0.01;
        }
        if self.grid_items.iter().any(|(_, g)| g.is_open_portal() && g.grid_y == row) { 1.0 } else { 0.2 }
    }

    /// `Challenge::PortalStart`: the two pairs of portals and the onslaught
    /// timer.
    pub(super) fn start_portal_combat(&mut self, effects: &mut EffectSystem, res: &mut GameResources, images: &mut bevy::asset::Assets<bevy::image::Image>) {
        if self.minigame() != Some(MiniGame::PortalCombat) {
            return;
        }
        self.portal_relocate_counter = 9000;
        self.conveyor_belt_counter = 200;
        for (ty, x, y) in [
            (super::grid_item::GridItemType::PortalSquare, 2, 0),
            (super::grid_item::GridItemType::PortalSquare, 9, 1),
            (super::grid_item::GridItemType::PortalCircle, 9, 3),
            (super::grid_item::GridItemType::PortalCircle, 2, 4),
        ] {
            let item = super::grid_item::GridItem::portal(ty, x, y);
            let id = self.grid_items.alloc(item);
            self.open_portal(id, effects, res, images);
        }
    }

    /// `Challenge::UpdatePortalCombat` + `Challenge::UpdatePortal`: portals
    /// pulse, move their zombies, shots and running mowers to the paired
    /// portal, and relocate every 6000 ticks.
    pub(super) fn update_portal_combat(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut bevy::asset::Assets<bevy::image::Image>,
        sounds: &mut Sounds,
    ) {
        use super::grid_item::{GridItemState, GridItemType};
        if self.minigame() != Some(MiniGame::PortalCombat) {
            return;
        }
        // Animation state: appear once, then pulse forever; disappear, then die.
        let portal_ids: Vec<_> = self.grid_items.iter().filter(|(_, g)| g.is_portal()).map(|(id, _)| id).collect();
        for id in portal_ids {
            let Some(item) = self.grid_items.get(id) else { continue };
            let (state, reanim) = (item.state, item.portal_reanim());
            let Some(reanim_id) = reanim else { continue };
            let Some(reanim) = effects.get_mut(reanim_id) else { continue };
            if reanim.loop_count == 0 {
                continue;
            }
            if state == GridItemState::PortalClosed {
                if let Some(item) = self.grid_items.get_mut(id) {
                    item.dead = true;
                }
            } else if reanim.loop_type == sexy::reanim::LoopType::PlayOnceAndHold {
                reanim.play("anim_pulse", sexy::reanim::LoopType::Loop, 0, 12.0);
            }
        }
        // The pairs, by type.
        let pairs: Vec<((i32, i32), (i32, i32))> = [
            GridItemType::PortalSquare,
            GridItemType::PortalCircle,
        ]
        .into_iter()
        .filter_map(|ty| {
            let mut cells = self.grid_items.iter().filter(|(_, g)| g.item_type == ty && g.is_open_portal()).map(|(_, g)| (g.grid_x, g.grid_y));
            let first = cells.next()?;
            let second = cells.next()?;
            Some((first, second))
        })
        .collect();
        let env = self.zombie_env();
        for (portal, other) in pairs {
            // Zombies.
            for (_, zombie) in self.zombies.iter_mut() {
                if zombie.dead || !zombie.is_on_board() || zombie.row != portal.1 || zombie.last_portal_x == portal.0 {
                    continue;
                }
                let rect = zombie.zombie_rect();
                let center = rect.x + rect.w / 2;
                let portal_x = portal.0 * 80 + 25;
                if (center - portal_x).abs() <= 45 {
                    let mut diff = (center - zombie.pos_x as i32) as f32;
                    if zombie.is_walking_backwards() {
                        diff -= 60.0;
                    }
                    zombie.portal_teleport(other.0 as f32 * 80.0 - diff, other.1, &env);
                    zombie.last_portal_x = other.0;
                }
            }
            // Straight shots.
            for (_, projectile) in self.projectiles.iter_mut() {
                if !projectile.is_straight_motion() || projectile.row != portal.1 {
                    continue;
                }
                if projectile.last_portal_x == portal.0 {
                    continue;
                }
                let rect = projectile.rect();
                let center = rect.x + rect.w / 2;
                let portal_x = portal.0 * 80 + 55;
                if (center - portal_x).abs() <= 40 {
                    let new_x = projectile.pos_x() + (other.0 * 80 - center + 60) as f32;
                    let delta_y = ((other.1 - portal.1) * 100) as f32;
                    projectile.portal_teleport(new_x, delta_y, other.1);
                    projectile.last_portal_x = other.0;
                }
            }
            // A running mower carries on through.
            for mower in self.lawn_mowers.iter_mut() {
                if mower.state != super::lawn_mower::LawnMowerState::Triggered || mower.row != portal.1 || mower.last_portal_x == portal.0 {
                    continue;
                }
                let mower_x = mower.pos_x as i32 + 45;
                let portal_x = portal.0 * 80 + 25;
                if (mower_x - portal_x).abs() <= 20 {
                    mower.pos_x = (other.0 * 80) as f32 + 25.0;
                    mower.row = other.1;
                    mower.pos_y = ((other.1 - portal.1) * 100) as f32;
                    mower.last_portal_x = other.0;
                    mower.render_order = super::make_render_order(super::render_layer::LAWN_MOWER, other.1, 0);
                }
            }
        }
        // Relocation.
        if self.has_level_award_dropped() {
            self.clear_advice(None);
            return;
        }
        self.portal_relocate_counter -= 1;
        if self.portal_relocate_counter == 500 {
            self.display_advice(res, "[ADVICE_PORTAL_RELOCATING]", super::message::MessageStyle::HintFast, None);
        } else if self.portal_relocate_counter <= 0 {
            self.clear_advice(None);
            self.portal_relocate_counter = 6000;
            self.move_a_portal(effects, res, images);
        }
        let _ = sounds;
    }

    /// `Challenge::MoveAPortal`: one portal jumps somewhere new.
    fn move_a_portal(&mut self, effects: &mut EffectSystem, res: &mut GameResources, images: &mut bevy::asset::Assets<bevy::image::Image>) {
        let open: Vec<_> = self.grid_items.iter().filter(|(_, g)| g.is_open_portal()).map(|(id, _)| id).collect();
        if open.is_empty() {
            return;
        }
        let Some(portal_id) = open.get(self.grid_items.iter().count() % open.len()).copied() else { return };
        let Some(portal) = self.grid_items.get(portal_id) else { return };
        let (item_type, px, py) = (portal.item_type, portal.grid_x, portal.grid_y);
        let Some(other) = self
            .grid_items
            .iter()
            .find(|(id, g)| *id != portal_id && g.item_type == item_type)
            .map(|(_, g)| (g.grid_x, g.grid_y))
        else {
            return;
        };
        let candidates: Vec<(i32, i32)> = (0..10)
            .flat_map(|x| (0..5).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                self.grid_items.iter().all(|(_, g)| !g.is_open_portal() || (g.grid_x, g.grid_y) != (x, y))
                    && other.0 != x
                    && other.1 != y
            })
            .collect();
        if candidates.is_empty() {
            return;
        }
        let pick = candidates[(px as usize + py as usize) % candidates.len()];
        self.close_portal(portal_id, effects);
        let item = super::grid_item::GridItem::portal(item_type, pick.0, pick.1);
        let id = self.grid_items.alloc(item);
        self.open_portal(id, effects, res, images);
    }

    /// `Challenge::CheckForCompleteArtChallenge` + `SpawnLevelAward`: the
    /// finished picture drops the level's award coin.
    pub(super) fn check_art_challenge_complete(
        &mut self,
        effects: &mut EffectSystem,
        res: &mut GameResources,
        images: &mut bevy::asset::Assets<bevy::image::Image>,
        sounds: &mut Sounds,
    ) {
        if self.minigame() != Some(MiniGame::SeeingStars) || self.has_level_award_dropped() || !self.art_challenge_complete() {
            return;
        }
        let Some((col, row)) = (0..6)
            .flat_map(|row| (0..9).map(move |col| (col, row)))
            .find(|&(col, row)| self.art_challenge_seed(col, row).is_some())
        else {
            return;
        };
        let (x, y) = (self.grid_to_pixel_x(col) + 40, self.grid_to_pixel_y(col, row) + 40);
        self.level_award_spawned = true;
        sounds.play_foley(crate::audio::FoleyType::SpawnSun);
        self.add_coin(
            x as f32,
            y as f32,
            super::coin::CoinType::SilverSunflower,
            super::coin::CoinMotion::Coin,
            effects,
            res,
            images,
            sounds,
        );
    }

    /// `Challenge::CheckForCompleteArtChallenge`: every ghost cell carries
    /// its plant.
    pub fn art_challenge_complete(&self) -> bool {
        for row in 0..6 {
            for col in 0..9 {
                if let Some(seed) = self.art_challenge_seed(col, row)
                    && self.normal_plant_at(col, row).is_none_or(|p| p.seed != seed)
                {
                    return false;
                }
            }
        }
        true
    }
}
