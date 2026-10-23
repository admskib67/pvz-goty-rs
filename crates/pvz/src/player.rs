//! Player profiles: `PlayerInfo` (one player's progress), `ProfileMgr` (the
//! roster) and the unlock checks built on them (`LawnApp::CanShowAlmanac`
//! and friends).
//!
//! Profiles are saved in the original's formats, in a `userdata` folder in
//! the app's data folder: `users.dat` lists every profile's name, id and
//! last-use order, and `user<id>.dat` holds one player's details. So a
//! `userdata` folder copied from the original game should load as-is.
//!
//! Behaviour per ruslan831 decomp, `Lawn/System/PlayerInfo.cpp`,
//! `ProfileMgr.cpp` and `DataSync.cpp`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use sexy::rand::MtRand;

/// `gProfileVersion`, the `users.dat` format.
const PROFILE_VERSION: u32 = 14;
/// `gUserVersion`, the `user<id>.dat` format.
const USER_VERSION: u32 = 12;
/// `sizeof(PottedPlant)` in the 32-bit game.
const POTTED_PLANT_SIZE: usize = 0x58;
const MAX_POTTED_PLANTS: usize = 200;
/// `DeleteOldProfiles` keeps at most this many profiles.
const MAX_PROFILES: usize = 200;

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerInfo {
    pub name: String,
    /// When the profile was last used, as a counter: the oldest goes first.
    pub use_seq: u32,
    /// Names the profile's `user<id>.dat`.
    pub id: u32,
    /// Next adventure level, 1-based (level 1-1 is 1, 2-1 is 11, ...).
    pub level: i32,
    pub coins: i32,
    /// Times adventure mode has been completed.
    pub finished_adventure: i32,
    pub challenge_records: [i32; 100],
    pub purchases: [i32; 80],
    pub play_time_active_player: i32,
    pub play_time_inactive_player: i32,
    pub has_used_cheat_keys: i32,
    pub has_woken_stinky: i32,
    pub didnt_purchase_packet_upgrade: i32,
    pub last_stinky_chocolate_time: i32,
    pub stinky_pos_x: i32,
    pub stinky_pos_y: i32,
    pub has_unlocked_minigames: bool,
    pub has_unlocked_puzzle_mode: bool,
    pub has_new_mini_game: i32,
    pub has_new_scary_potter: i32,
    pub has_new_i_zombie: i32,
    pub has_new_survival: i32,
    pub has_unlocked_survival_mode: bool,
    pub needs_message_on_game_selector: i32,
    pub needs_magic_taco_reward: i32,
    pub has_seen_stinky: i32,
    pub has_seen_upsell: bool,
    pub place_holder_player_stats: i32,
    /// Zen garden plants, kept as the original's raw structs until the zen
    /// garden is ported.
    pub potted_plants: Vec<[u8; POTTED_PLANT_SIZE]>,
}

impl Default for PlayerInfo {
    /// `PlayerInfo::Reset`.
    fn default() -> Self {
        Self {
            name: String::new(),
            use_seq: 0,
            id: 0,
            level: 1,
            coins: 0,
            finished_adventure: 0,
            challenge_records: [0; 100],
            purchases: [0; 80],
            play_time_active_player: 0,
            play_time_inactive_player: 0,
            has_used_cheat_keys: 0,
            has_woken_stinky: 0,
            didnt_purchase_packet_upgrade: 0,
            last_stinky_chocolate_time: 0,
            stinky_pos_x: 0,
            stinky_pos_y: 0,
            has_unlocked_minigames: false,
            has_unlocked_puzzle_mode: false,
            has_new_mini_game: 0,
            has_new_scary_potter: 0,
            has_new_i_zombie: 0,
            has_new_survival: 0,
            has_unlocked_survival_mode: false,
            needs_message_on_game_selector: 0,
            needs_magic_taco_reward: 0,
            has_seen_stinky: 0,
            has_seen_upsell: false,
            place_holder_player_stats: 0,
            potted_plants: Vec::new(),
        }
    }
}

impl PlayerInfo {
    pub fn has_finished_adventure(&self) -> bool {
        self.finished_adventure > 0
    }

    pub fn can_show_almanac(&self) -> bool {
        self.has_finished_adventure() || self.level >= 15
    }

    pub fn can_show_store(&self) -> bool {
        self.has_finished_adventure() || self.has_seen_upsell || self.level >= 25
    }

    pub fn can_show_zen_garden(&self) -> bool {
        self.has_finished_adventure() || self.level >= 45
    }

    /// Clears progress but keeps the summary (`Reset`, which only touches details).
    fn reset_details(&mut self) {
        *self = Self { name: std::mem::take(&mut self.name), use_seq: self.use_seq, id: self.id, ..Self::default() };
    }

    /// `SyncDetails` when writing.
    fn write_details(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.long(USER_VERSION);
        for v in [self.level, self.coins, self.finished_adventure] {
            w.int(v);
        }
        for &v in self.challenge_records.iter().chain(&self.purchases) {
            w.int(v);
        }
        for v in [
            self.play_time_active_player,
            self.play_time_inactive_player,
            self.has_used_cheat_keys,
            self.has_woken_stinky,
            self.didnt_purchase_packet_upgrade,
            self.last_stinky_chocolate_time,
            self.stinky_pos_x,
            self.stinky_pos_y,
            self.has_unlocked_minigames as i32,
            self.has_unlocked_puzzle_mode as i32,
            self.has_new_mini_game,
            self.has_new_scary_potter,
            self.has_new_i_zombie,
            self.has_new_survival,
            self.has_unlocked_survival_mode as i32,
            self.needs_message_on_game_selector,
            self.needs_magic_taco_reward,
            self.has_seen_stinky,
            self.has_seen_upsell as i32,
            self.place_holder_player_stats,
            self.potted_plants.len() as i32,
        ] {
            w.int(v);
        }
        for plant in &self.potted_plants {
            w.0.extend_from_slice(plant);
        }
        w.0
    }

    /// `SyncDetails` when reading. A file from another version leaves the
    /// details reset, like the original.
    fn read_details(&mut self, data: &[u8]) -> Result<(), ReadError> {
        self.reset_details();
        let mut r = Reader(data);
        if r.long()? != USER_VERSION {
            return Ok(());
        }
        self.level = r.int()?;
        self.coins = r.int()?;
        self.finished_adventure = r.int()?;
        for v in self.challenge_records.iter_mut().chain(self.purchases.iter_mut()) {
            *v = r.int()?;
        }
        self.play_time_active_player = r.int()?;
        self.play_time_inactive_player = r.int()?;
        self.has_used_cheat_keys = r.int()?;
        self.has_woken_stinky = r.int()?;
        self.didnt_purchase_packet_upgrade = r.int()?;
        self.last_stinky_chocolate_time = r.int()?;
        self.stinky_pos_x = r.int()?;
        self.stinky_pos_y = r.int()?;
        self.has_unlocked_minigames = r.int()? != 0;
        self.has_unlocked_puzzle_mode = r.int()? != 0;
        self.has_new_mini_game = r.int()?;
        self.has_new_scary_potter = r.int()?;
        self.has_new_i_zombie = r.int()?;
        self.has_new_survival = r.int()?;
        self.has_unlocked_survival_mode = r.int()? != 0;
        self.needs_message_on_game_selector = r.int()?;
        self.needs_magic_taco_reward = r.int()?;
        self.has_seen_stinky = r.int()?;
        self.has_seen_upsell = r.int()? != 0;
        self.place_holder_player_stats = r.int()?;
        let count = r.int()?;
        if !(0..=MAX_POTTED_PLANTS as i32).contains(&count) {
            return Err(ReadError);
        }
        for _ in 0..count {
            self.potted_plants.push(r.bytes::<POTTED_PLANT_SIZE>()?);
        }
        Ok(())
    }
}

/// The data ran out (`DataReaderException`).
#[derive(Debug)]
struct ReadError;

/// `DataReader`: little-endian, strings as a 16-bit length then Latin-1 bytes.
struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], ReadError> {
        let (head, rest) = self.0.split_first_chunk::<N>().ok_or(ReadError)?;
        self.0 = rest;
        Ok(*head)
    }

    fn short(&mut self) -> Result<u16, ReadError> {
        self.bytes().map(u16::from_le_bytes)
    }

    fn long(&mut self) -> Result<u32, ReadError> {
        self.bytes().map(u32::from_le_bytes)
    }

    fn int(&mut self) -> Result<i32, ReadError> {
        self.bytes().map(i32::from_le_bytes)
    }

    fn string(&mut self) -> Result<String, ReadError> {
        let len = self.short()? as usize;
        if self.0.len() < len {
            return Err(ReadError);
        }
        let (s, rest) = self.0.split_at(len);
        self.0 = rest;
        Ok(sexy::font::latin1_to_string(s))
    }
}

/// `DataWriter`.
#[derive(Default)]
struct Writer(Vec<u8>);

impl Writer {
    fn short(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    fn long(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    fn int(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    fn string(&mut self, s: &str) {
        let bytes: Vec<u8> = s.chars().map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?')).collect();
        self.short(bytes.len() as u16);
        self.0.extend_from_slice(&bytes);
    }
}

/// The roster of player profiles and which one is playing (`ProfileMgr`
/// plus `LawnApp::mPlayerInfo`).
#[derive(Resource, Debug)]
pub struct ProfileMgr {
    /// Keyed by lowercased name: names are unique ignoring case and listed
    /// in that order (`StringLessNoCase`).
    profiles: BTreeMap<String, PlayerInfo>,
    next_profile_id: u32,
    next_profile_use_seq: u32,
    /// The current player's id (`mPlayerInfo`), if any.
    current: Option<u32>,
    /// The `userdata` folder, or `None` to keep everything in memory.
    dir: Option<PathBuf>,
}

fn key(name: &str) -> String {
    name.to_ascii_lowercase()
}

impl ProfileMgr {
    /// An empty roster saved in `dir`.
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self { profiles: BTreeMap::new(), next_profile_id: 1, next_profile_use_seq: 1, current: None, dir }
    }

    /// `ProfileMgr::Load` from `dir`. A damaged file gives an empty roster.
    pub fn load(dir: Option<PathBuf>) -> Self {
        let mut mgr = Self::new(dir);
        let Some(data) = mgr.dir.as_ref().and_then(|d| std::fs::read(d.join("users.dat")).ok()) else {
            return mgr;
        };
        if mgr.read_users(&data).is_err() {
            warn!("users.dat is damaged; starting with no profiles");
            mgr.profiles.clear();
        }
        mgr
    }

    fn read_users(&mut self, data: &[u8]) -> Result<(), ReadError> {
        let mut r = Reader(data);
        if r.long()? != PROFILE_VERSION {
            return Ok(());
        }
        let (mut max_id, mut max_seq) = (0, 0);
        for _ in 0..r.short()? {
            let profile = PlayerInfo { name: r.string()?, use_seq: r.long()?, id: r.long()?, ..PlayerInfo::default() };
            max_id = max_id.max(profile.id);
            max_seq = max_seq.max(profile.use_seq);
            self.profiles.insert(key(&profile.name), profile);
        }
        self.next_profile_id = max_id + 1;
        self.next_profile_use_seq = max_seq + 1;
        Ok(())
    }

    fn write_users(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.long(PROFILE_VERSION);
        w.short(self.profiles.len() as u16);
        for p in self.profiles.values() {
            w.string(&p.name);
            w.long(p.use_seq);
            w.long(p.id);
        }
        w.0
    }

    fn write_file(&self, name: &str, data: &[u8]) {
        let Some(dir) = &self.dir else { return };
        if let Err(e) = std::fs::create_dir_all(dir).and_then(|_| std::fs::write(dir.join(name), data)) {
            error!("couldn't save {}: {e}", dir.join(name).display());
        }
    }

    /// `ProfileMgr::Save`: writes the roster (not the players' details).
    pub fn save(&self) {
        self.write_file("users.dat", &self.write_users());
    }

    fn details_file(id: u32) -> String {
        format!("user{id}.dat")
    }

    /// `PlayerInfo::LoadDetails`.
    fn load_details(dir: Option<&Path>, profile: &mut PlayerInfo) {
        let Some(data) = dir.and_then(|d| std::fs::read(d.join(Self::details_file(profile.id))).ok()) else {
            return;
        };
        if profile.read_details(&data).is_err() {
            warn!("{} is damaged; resetting that player's progress", Self::details_file(profile.id));
            profile.reset_details();
        }
    }

    /// `PlayerInfo::SaveDetails` for the current player.
    pub fn save_current_details(&self) {
        if let Some(p) = self.player() {
            self.write_file(&Self::details_file(p.id), &p.write_details());
        }
    }

    /// The profile names in roster order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.profiles.values().map(|p| p.name.as_str())
    }

    /// The current player (`mPlayerInfo`).
    pub fn player(&self) -> Option<&PlayerInfo> {
        let id = self.current?;
        self.profiles.values().find(|p| p.id == id)
    }

    /// Switches the current player (`mPlayerInfo = ...`), or clears it.
    pub fn set_current(&mut self, name: Option<&str>) {
        self.current = name.and_then(|n| self.profiles.get(&key(n))).map(|p| p.id);
    }

    /// `ProfileMgr::GetProfile`: loads the profile's details and marks it as
    /// just used. Returns its exact name.
    pub fn get_profile(&mut self, name: &str) -> Option<String> {
        let profile = self.profiles.get_mut(&key(name))?;
        Self::load_details(self.dir.as_deref(), profile);
        profile.use_seq = self.next_profile_use_seq;
        self.next_profile_use_seq += 1;
        Some(profile.name.clone())
    }

    /// `ProfileMgr::GetAnyProfile`: the first profile in roster order.
    pub fn get_any_profile(&mut self) -> Option<String> {
        let name = self.profiles.values().next()?.name.clone();
        self.get_profile(&name)
    }

    /// `ProfileMgr::AddProfile`. Returns false if the name is taken.
    pub fn add_profile(&mut self, name: &str) -> bool {
        if self.profiles.contains_key(&key(name)) {
            return false;
        }
        let profile = PlayerInfo {
            name: name.to_owned(),
            id: self.next_profile_id,
            use_seq: self.next_profile_use_seq,
            ..PlayerInfo::default()
        };
        self.next_profile_id += 1;
        self.next_profile_use_seq += 1;
        self.profiles.insert(key(name), profile);
        // DeleteOldProfiles.
        while self.profiles.len() > MAX_PROFILES {
            let oldest = self.profiles.values().min_by_key(|p| p.use_seq).map(|p| p.name.clone());
            match oldest {
                Some(oldest) => self.delete_profile(&oldest),
                None => break,
            };
        }
        true
    }

    /// `ProfileMgr::DeleteProfile`, including the player's files.
    pub fn delete_profile(&mut self, name: &str) -> bool {
        let Some(profile) = self.profiles.remove(&key(name)) else { return false };
        if self.current == Some(profile.id) {
            self.current = None;
        }
        // DeleteUserFiles. TODO: also the player's saved games once they exist.
        if let Some(dir) = &self.dir {
            let _ = std::fs::remove_file(dir.join(Self::details_file(profile.id)));
        }
        true
    }

    /// `ProfileMgr::RenameProfile`. Returns false if the new name is taken
    /// (changing only the case of a name is allowed).
    pub fn rename_profile(&mut self, old: &str, new: &str) -> bool {
        if !self.profiles.contains_key(&key(old)) {
            return false;
        }
        if key(old) != key(new) && self.profiles.contains_key(&key(new)) {
            return false;
        }
        let mut profile = self.profiles.remove(&key(old)).expect("checked above");
        profile.name = new.to_owned();
        self.profiles.insert(key(new), profile);
        true
    }
}

/// The game's random number generator (the original's global `MTRand`).
#[derive(Resource, Debug)]
pub struct GameRng(pub MtRand);

impl Default for GameRng {
    fn default() -> Self {
        // The original seeds from the tick count at startup.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(4357, |d| d.as_millis() as u32);
        Self(MtRand::new(seed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn details_round_trip_in_the_original_layout() {
        let mut p = PlayerInfo { name: "Ann".into(), id: 3, level: 14, coins: 250, ..PlayerInfo::default() };
        p.has_unlocked_minigames = true;
        p.challenge_records[5] = 7;
        p.potted_plants.push([9; POTTED_PLANT_SIZE]);
        let data = p.write_details();
        // Version, 3 ints, 180 records/purchases, 21 more ints, one plant.
        assert_eq!(data.len(), 4 + 4 * (3 + 180 + 21) + POTTED_PLANT_SIZE);
        assert_eq!(&data[..8], &[12, 0, 0, 0, 14, 0, 0, 0]);

        let mut q = PlayerInfo { name: "Ann".into(), id: 3, ..PlayerInfo::default() };
        q.read_details(&data).unwrap();
        assert_eq!(q, p);
        assert!(q.read_details(&data[..20]).is_err());
    }

    #[test]
    fn roster_round_trips_and_keeps_names_unique_ignoring_case() {
        let mut mgr = ProfileMgr::new(None);
        assert!(mgr.add_profile("bob"));
        assert!(mgr.add_profile("Alice"));
        assert!(!mgr.add_profile("BOB"));
        assert_eq!(mgr.names().collect::<Vec<_>>(), ["Alice", "bob"]);

        assert!(mgr.rename_profile("bob", "Bob"));
        assert!(!mgr.rename_profile("Bob", "alice"));
        mgr.set_current(Some("bob"));
        assert_eq!(mgr.player().map(|p| p.id), Some(1));

        let data = mgr.write_users();
        let mut loaded = ProfileMgr::new(None);
        loaded.read_users(&data).unwrap();
        assert_eq!(loaded.names().collect::<Vec<_>>(), ["Alice", "Bob"]);
        assert_eq!(loaded.next_profile_id, 3);
        assert!(loaded.add_profile("Carl"));
        assert_eq!(loaded.profiles["carl"].id, 3);

        assert!(mgr.delete_profile("BOB"));
        assert!(mgr.player().is_none());
        assert_eq!(mgr.get_any_profile().as_deref(), Some("Alice"));
    }
}
