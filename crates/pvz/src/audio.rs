//! Music and sound effects.
//!
//! Music: PvZ's songs all live in one tracker module, `mainmusic.mo3`. The
//! original plays it as two instances, "main" and "drums", each with some
//! channels muted, plus `mainmusic_hihats.mo3`, kept in step and faded in
//! and out. Here the three libopenmpt modules are rendered side by side into
//! one stream, so they stay sample-locked and need no resyncing.
//!
//! Sounds: `PlaySample` plays a sound once; foley (`TodFoley`) adds random
//! variations, pitch and limits on how often a sound may overlap.
//!
//! Behaviour per ruslan831 decomp, `Lawn/System/Music.cpp` and
//! `Sexy.TodLib/TodFoley.cpp`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::audio::{AddAudioSource, AudioSource, ChannelCount, Decodable, SampleRate, Source, Volume};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use sexy::rand::MtRand;
use sexy::tod::{TodCurve, animate_curve_float};

use crate::player::GameRng;

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<MusicStream>()
            .init_resource::<SoundSystem>()
            .init_resource::<UpdateCount>()
            .add_systems(Startup, start_music_stream)
            .add_systems(FixedFirst, |mut count: ResMut<UpdateCount>| count.0 += 1)
            .add_systems(FixedUpdate, |mut music: ResMut<Music>| music.update());
    }
}

/// Game ticks since startup (`SexyAppBase::mUpdateCount`).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct UpdateCount(pub u64);

/// The original's default volumes (`LawnApp::LawnApp`).
const MUSIC_VOLUME: f32 = 0.85;
const SFX_VOLUME: f32 = 0.5525;

const SAMPLE_RATE: u32 = 44_100;
/// Frames rendered per lock of the music mixer.
const BLOCK_FRAMES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MusicFile {
    Main,
    Drums,
    Hihats,
    // TODO: the credits song (ZombiesOnYourLawn.ogg) once the credits exist.
}

impl MusicFile {
    pub fn path(self) -> &'static str {
        match self {
            Self::Main | Self::Drums => "sounds/mainmusic.mo3",
            Self::Hihats => "sounds/mainmusic_hihats.mo3",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

#[allow(dead_code, reason = "tunes are listed in full; screens that use them come later")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicTune {
    DayGrasswalk,
    NightMoongrains,
    PoolWaterygraves,
    FogRigormormist,
    RoofGrazetheroof,
    ChooseYourSeeds,
    TitleCrazyDaveMainTheme,
    ZenGarden,
    PuzzleCerebrawl,
    MinigameLoonboon,
    Conveyer,
    FinalBossBrainiacManiac,
}

/// One module being rendered.
struct Track {
    module: openmpt::Module,
    volume: f32,
    playing: bool,
}

/// Renders the playing tracks into one stereo stream.
#[derive(Default)]
struct Mixer {
    tracks: [Option<Track>; 3],
    scratch: Vec<f32>,
}

impl Mixer {
    fn render(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        self.scratch.resize(out.len(), 0.0);
        for track in self.tracks.iter_mut().flatten().filter(|t| t.playing) {
            // Silent tracks still advance, so the drums stay in step with the main tune.
            let frames = track.module.read_stereo(SAMPLE_RATE as i32, &mut self.scratch);
            for (o, s) in out.iter_mut().zip(&self.scratch[..frames * 2]) {
                *o += s * track.volume;
            }
        }
    }
}

/// The endless music stream Bevy plays; the game controls it through [`Music`].
#[derive(Asset, TypePath, Clone)]
pub struct MusicStream(Arc<Mutex<Mixer>>);

pub struct MusicDecoder {
    mixer: Arc<Mutex<Mixer>>,
    buffer: Vec<f32>,
    pos: usize,
}

impl Iterator for MusicDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.pos >= self.buffer.len() {
            self.mixer.lock().unwrap().render(&mut self.buffer);
            self.pos = 0;
        }
        self.pos += 1;
        Some(self.buffer[self.pos - 1])
    }
}

impl Source for MusicDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).unwrap()
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(SAMPLE_RATE).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Decodable for MusicStream {
    type Decoder = MusicDecoder;

    fn decoder(&self) -> MusicDecoder {
        MusicDecoder { mixer: self.0.clone(), buffer: vec![0.0; BLOCK_FRAMES * 2], pos: BLOCK_FRAMES * 2 }
    }
}

fn start_music_stream(mut commands: Commands, mut streams: ResMut<Assets<MusicStream>>) {
    let mixer = Arc::new(Mutex::new(Mixer::default()));
    commands.spawn(AudioPlayer(streams.add(MusicStream(mixer.clone()))));
    commands.insert_resource(Music::new(mixer));
}

/// Loads a music module (e.g. on the loading thread) for [`Music::set_song`].
pub fn load_song(bytes: &[u8]) -> Result<openmpt::Module, String> {
    let mut module = openmpt::Module::from_memory(bytes)?;
    module.set_repeat_count(-1);
    Ok(module)
}

/// The original's `Music`: which tune is playing and how its parts are mixed.
#[derive(Resource)]
pub struct Music {
    mixer: Arc<Mutex<Mixer>>,
    cur_tune: Option<MusicTune>,
    cur_main: Option<MusicFile>,
    cur_drums: Option<MusicFile>,
    cur_hihats: Option<MusicFile>,
    music_disabled: bool,
    fade_out_counter: i32,
    fade_out_duration: i32,
    volume_cap: f32,
}

impl Music {
    fn new(mixer: Arc<Mutex<Mixer>>) -> Self {
        Self {
            mixer,
            cur_tune: None,
            cur_main: None,
            cur_drums: None,
            cur_hihats: None,
            music_disabled: false,
            fade_out_counter: 0,
            fade_out_duration: 0,
            volume_cap: MUSIC_VOLUME,
        }
    }

    /// `LoadSong`. A failed load disables music, like the original.
    pub fn set_song(&mut self, file: MusicFile, song: Result<openmpt::Module, String>) {
        match song {
            Ok(module) => {
                self.mixer.lock().unwrap().tracks[file.index()] = Some(Track { module, volume: 0.0, playing: false })
            }
            Err(e) => {
                error!("music failed to load: {e}");
                self.music_disabled = true;
            }
        }
    }

    /// `PlayFromOffset`: restarts a file at an order with its channels set up
    /// for the current tune.
    fn play_from_offset(&mut self, file: MusicFile, order: i32, volume: f32) {
        let tune = self.cur_tune;
        let mut mixer = self.mixer.lock().unwrap();
        let Some(track) = &mut mixer.tracks[file.index()] else { return };
        setup_music_file_for_tune(&mut track.module, file, tune);
        track.module.set_position_order_row(order, 0);
        track.volume = self.volume_cap * volume;
        track.playing = true;
    }

    /// `PlayMusic`, always from the tune's start.
    pub fn play_music(&mut self, tune: MusicTune) {
        if self.music_disabled {
            return;
        }
        self.cur_tune = Some(tune);
        (self.cur_main, self.cur_drums, self.cur_hihats) = (None, None, None);
        use MusicFile::*;
        use MusicTune::*;
        let layered = |order| (Some(Main), Some(Drums), Some(Hihats), order, order);
        let (main, drums, hihats, order, drums_order) = match tune {
            DayGrasswalk => layered(0),
            NightMoongrains => (Some(Main), Some(Drums), None, 0x30, 0x5C),
            PoolWaterygraves => layered(0x5E),
            FogRigormormist => layered(0x7D),
            RoofGrazetheroof => layered(0xB8),
            ChooseYourSeeds => (Some(Main), None, None, 0x7A, 0),
            TitleCrazyDaveMainTheme => (Some(Main), None, None, 0x98, 0),
            ZenGarden => (Some(Main), None, None, 0xDD, 0),
            PuzzleCerebrawl => (Some(Main), None, None, 0xB1, 0),
            MinigameLoonboon => (Some(Main), None, None, 0xA6, 0),
            Conveyer => (Some(Main), None, None, 0xD4, 0),
            FinalBossBrainiacManiac => (Some(Main), None, None, 0x9E, 0),
        };
        (self.cur_main, self.cur_drums, self.cur_hihats) = (main, drums, hihats);
        if let Some(f) = main {
            self.play_from_offset(f, order, 1.0);
        }
        if let Some(f) = drums {
            self.play_from_offset(f, drums_order, 0.0);
        }
        if let Some(f) = hihats {
            self.play_from_offset(f, order, 0.0);
        }
    }

    /// `StopAllMusic`.
    pub fn stop_all_music(&mut self) {
        let mut mixer = self.mixer.lock().unwrap();
        for file in [self.cur_main, self.cur_drums, self.cur_hihats].into_iter().flatten() {
            if let Some(track) = &mut mixer.tracks[file.index()] {
                track.playing = false;
            }
        }
        drop(mixer);
        self.cur_tune = None;
        (self.cur_main, self.cur_drums, self.cur_hihats) = (None, None, None);
        self.fade_out_counter = 0;
    }

    /// `MakeSureMusicIsPlaying`.
    pub fn make_sure_music_is_playing(&mut self, tune: MusicTune) {
        if self.cur_tune != Some(tune) {
            self.stop_all_music();
            self.play_music(tune);
        }
    }

    /// `FadeOut`: fades the main part out over `duration` ticks, then stops.
    pub fn fade_out(&mut self, duration: i32) {
        if self.cur_tune.is_some() {
            self.fade_out_counter = duration;
            self.fade_out_duration = duration;
        }
    }

    /// `MusicUpdate`, every tick. TODO: drum/hi-hat bursts once the board exists.
    fn update(&mut self) {
        if self.fade_out_counter > 0 {
            self.fade_out_counter -= 1;
            if self.fade_out_counter == 0 {
                self.stop_all_music();
            } else if let Some(main) = self.cur_main {
                let level =
                    animate_curve_float(self.fade_out_duration, 0, self.fade_out_counter, 1.0, 0.0, TodCurve::Linear);
                if let Some(track) = &mut self.mixer.lock().unwrap().tracks[main.index()] {
                    track.volume = self.volume_cap * level;
                }
            }
        }
    }
}

/// `SetupMusicFileForTune`: which of the module's channels this part plays.
fn setup_music_file_for_tune(module: &mut openmpt::Module, file: MusicFile, tune: Option<MusicTune>) {
    use MusicFile::*;
    use MusicTune::*;
    // (channel count, first range, optional second range)
    let (count, first, second): (i32, (i32, i32), (i32, i32)) = match (tune, file) {
        (Some(DayGrasswalk), Main) => (29, (0, 23), (-1, -1)),
        (Some(DayGrasswalk), Hihats) => (29, (27, 27), (-1, -1)),
        (Some(DayGrasswalk), Drums) => (29, (24, 26), (-1, -1)),
        (Some(PoolWaterygraves), Main) => (29, (0, 17), (-1, -1)),
        (Some(PoolWaterygraves), Hihats) => (29, (18, 24), (29, 29)),
        (Some(PoolWaterygraves), Drums) => (29, (25, 28), (-1, -1)),
        (Some(FogRigormormist), Main) => (29, (0, 15), (-1, -1)),
        (Some(FogRigormormist), Hihats) => (29, (23, 23), (-1, -1)),
        (Some(FogRigormormist), Drums) => (29, (16, 22), (-1, -1)),
        (Some(RoofGrazetheroof), Main) => (29, (0, 17), (-1, -1)),
        (Some(RoofGrazetheroof), Hihats) => (29, (21, 21), (-1, -1)),
        (Some(RoofGrazetheroof), Drums) => (29, (18, 20), (-1, -1)),
        (_, Main | Drums) => (29, (0, 29), (-1, -1)),
        (_, Hihats) => (0, (-1, -1), (-1, -1)),
    };
    for channel in 0..count {
        let on = (first.0..=first.1).contains(&channel) || (second.0..=second.1).contains(&channel);
        module.set_channel_volume(channel, if on { 1.0 } else { 0.0 });
    }
}

/// `FoleyFlags`.
mod foley_flags {
    pub const LOOP: u32 = 1 << 0;
    pub const ONE_AT_A_TIME: u32 = 1 << 1;
    pub const MUTE_ON_PAUSE: u32 = 1 << 2;
    pub const USES_MUSIC_VOLUME: u32 = 1 << 3;
    pub const DONT_REPEAT: u32 = 1 << 4;
}

/// Foley types (`FoleyType`). Only the ones in use so far; the original has ~100.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FoleyType {
    Bleep,
    LimbsPop,
    DirtRise,
    /// The level intro's sod rolling out.
    Digger,
    /// Collecting sun.
    Sun,
    /// Planting.
    Plant,
    /// Putting a carried plant back.
    Drop,
    Splat,
    Throw,
    Chomp,
    ChompSoft,
    Groan,
    ZombieFalling,
    ShieldHit,
    PlasticHit,
    SpawnSun,
    Lawnmower,
    Scream,
    WinMusic,
    BigChomp,
    Shovel,
    UseShovel,
    Cherrybomb,
    Juicy,
    ReverseExplosion,
    Frozen,
    SnowPeaSparkles,
}

struct FoleyParams {
    /// Random pitch up to this many semitones.
    pitch_range: f32,
    sounds: &'static [&'static str],
    flags: u32,
}

impl FoleyType {
    /// Entries of `gLawnFoleyParamArray`.
    fn params(self) -> FoleyParams {
        match self {
            Self::Bleep => FoleyParams { pitch_range: 0.0, sounds: &["SOUND_BLEEP"], flags: 0 },
            Self::LimbsPop => FoleyParams { pitch_range: 10.0, sounds: &["SOUND_LIMBS_POP"], flags: 0 },
            Self::DirtRise => FoleyParams { pitch_range: 5.0, sounds: &["SOUND_DIRT_RISE"], flags: 0 },
            Self::Sun => FoleyParams { pitch_range: 10.0, sounds: &["SOUND_POINTS"], flags: 0 },
            Self::Plant => FoleyParams { pitch_range: 0.0, sounds: &["SOUND_PLANT", "SOUND_PLANT2"], flags: 0 },
            Self::Drop => FoleyParams { pitch_range: 0.0, sounds: &["SOUND_TAP2"], flags: 0 },
            Self::Splat => FoleyParams {
                pitch_range: 10.0,
                sounds: &["SOUND_SPLAT", "SOUND_SPLAT2", "SOUND_SPLAT3"],
                flags: 0,
            },
            Self::Throw => FoleyParams {
                pitch_range: 10.0,
                sounds: &["SOUND_THROW", "SOUND_THROW", "SOUND_THROW", "SOUND_THROW2"],
                flags: 0,
            },
            Self::Chomp => FoleyParams { pitch_range: 0.0, sounds: &["SOUND_CHOMP", "SOUND_CHOMP2"], flags: 0 },
            Self::ChompSoft => FoleyParams { pitch_range: 4.0, sounds: &["SOUND_CHOMPSOFT"], flags: 0 },
            Self::Groan => FoleyParams {
                pitch_range: 0.0,
                sounds: &["SOUND_GROAN", "SOUND_GROAN2", "SOUND_GROAN3", "SOUND_GROAN4", "SOUND_GROAN5", "SOUND_GROAN6"],
                flags: 0,
            },
            Self::ZombieFalling => FoleyParams {
                pitch_range: 10.0,
                sounds: &["SOUND_ZOMBIE_FALLING_1", "SOUND_ZOMBIE_FALLING_2"],
                flags: 0,
            },
            Self::ShieldHit => FoleyParams {
                pitch_range: 10.0,
                sounds: &["SOUND_SHIELDHIT", "SOUND_SHIELDHIT2"],
                flags: 0,
            },
            Self::Lawnmower => FoleyParams { pitch_range: 10.0, sounds: &["SOUND_LAWNMOWER"], flags: 0 },
            Self::WinMusic => FoleyParams {
                pitch_range: 0.0,
                sounds: &["SOUND_WINMUSIC"],
                flags: foley_flags::USES_MUSIC_VOLUME,
            },
            Self::Cherrybomb => FoleyParams { pitch_range: 0.0, sounds: &["SOUND_CHERRYBOMB"], flags: 0 },
            Self::Juicy => FoleyParams { pitch_range: 2.0, sounds: &["SOUND_JUICY"], flags: 0 },
            Self::ReverseExplosion => FoleyParams { pitch_range: 0.0, sounds: &["SOUND_REVERSE_EXPLOSION"], flags: 0 },
            Self::Frozen => FoleyParams { pitch_range: 0.0, sounds: &["SOUND_FROZEN"], flags: 0 },
            Self::SnowPeaSparkles => FoleyParams { pitch_range: 10.0, sounds: &["SOUND_SNOW_PEA_SPARKLES"], flags: 0 },
            Self::Shovel => FoleyParams { pitch_range: 5.0, sounds: &["SOUND_SHOVEL"], flags: 0 },
            Self::UseShovel => FoleyParams { pitch_range: 0.0, sounds: &["SOUND_PLANT2"], flags: 0 },
            Self::BigChomp => FoleyParams { pitch_range: -2.0, sounds: &["SOUND_BIGCHOMP"], flags: 0 },
            Self::Scream => FoleyParams { pitch_range: 0.0, sounds: &["SOUND_SCREAM"], flags: 0 },
            Self::SpawnSun => FoleyParams { pitch_range: 10.0, sounds: &["SOUND_THROW"], flags: 0 },
            Self::PlasticHit => FoleyParams {
                pitch_range: 5.0,
                sounds: &["SOUND_PLASTICHIT", "SOUND_PLASTICHIT2"],
                flags: 0,
            },
            Self::Digger => FoleyParams {
                pitch_range: 0.0,
                sounds: &["SOUND_DIGGER_ZOMBIE"],
                flags: foley_flags::LOOP | foley_flags::ONE_AT_A_TIME | foley_flags::MUTE_ON_PAUSE,
            },
        }
    }
}

const MAX_FOLEY_INSTANCES: usize = 8;
/// A foley type won't restart within this many ticks of its last start.
const FOLEY_MIN_INTERVAL: u64 = 10;

#[derive(Clone, Copy)]
struct FoleyInstance {
    entity: Entity,
    ref_count: u32,
    start_time: u64,
}

#[derive(Default)]
struct FoleyTypeData {
    instances: [Option<FoleyInstance>; MAX_FOLEY_INSTANCES],
    last_variation: Option<usize>,
}

/// Loaded sound effects and the foley bookkeeping.
#[derive(Resource, Default)]
pub struct SoundSystem {
    sounds: HashMap<String, Handle<AudioSource>>,
    foley: HashMap<FoleyType, FoleyTypeData>,
}

/// Formats Bevy can decode here; anything else would panic the audio thread.
fn playable(bytes: &[u8]) -> bool {
    bytes.starts_with(b"OggS") || bytes.starts_with(b"RIFF")
}

impl SoundSystem {
    /// Adds a loaded sound (`SOUND_*`). Unsupported formats are skipped.
    pub fn insert(&mut self, id: String, bytes: Vec<u8>, assets: &mut Assets<AudioSource>) {
        if playable(&bytes) {
            self.sounds.insert(id, assets.add(AudioSource { bytes: bytes.into() }));
        } else {
            // TODO: the Windows pak has `.au` sounds, which need their own decoder.
            warn!("{id}: sound format not supported yet, it will be silent");
        }
    }
}

/// Everything needed to play sounds from a system.
#[derive(SystemParam)]
pub struct Sounds<'w, 's> {
    commands: Commands<'w, 's>,
    system: ResMut<'w, SoundSystem>,
    rng: ResMut<'w, GameRng>,
    tick: Res<'w, UpdateCount>,
    playing: Query<'w, 's, (), With<AudioPlayer<AudioSource>>>,
}

impl Sounds<'_, '_> {
    /// The game's random number generator, for systems that also play sounds.
    pub fn rng(&mut self) -> &mut MtRand {
        &mut self.rng.0
    }

    fn spawn(&mut self, id: &str, speed: f32, looping: bool) -> Option<Entity> {
        let handle = self.system.sounds.get(id)?.clone();
        let mode = if looping { PlaybackSettings::LOOP } else { PlaybackSettings::DESPAWN };
        let settings = mode.with_volume(Volume::Linear(SFX_VOLUME)).with_speed(speed);
        Some(self.commands.spawn((AudioPlayer(handle), settings)).id())
    }

    /// `PlaySample`: plays a sound once.
    pub fn play_sample(&mut self, id: &str) {
        self.spawn(id, 1.0, false);
    }

    /// `TodFoley::PlayFoley`: a random variation with random pitch.
    pub fn play_foley(&mut self, foley: FoleyType) {
        let params = foley.params();
        let pitch = if params.pitch_range != 0.0 { self.rng.0.next_f32(params.pitch_range) } else { 0.0 };
        self.play_foley_pitch(foley, pitch);
    }

    /// `TodFoley::PlayFoleyPitch`, `pitch` in semitones.
    pub fn play_foley_pitch(&mut self, foley: FoleyType, pitch: f32) {
        let params = foley.params();
        let now = self.tick.0;
        let playing = &self.playing;
        let data = self.system.foley.entry(foley).or_default();

        // SoundSystemReleaseFinishedInstances.
        for slot in &mut data.instances {
            if slot.is_some_and(|i| !playing.contains(i.entity)) {
                *slot = None;
            }
        }
        let too_recent = data.instances.iter().flatten().any(|i| now - i.start_time < FOLEY_MIN_INTERVAL);
        if too_recent && params.flags & foley_flags::LOOP == 0 {
            return;
        }
        if params.flags & foley_flags::ONE_AT_A_TIME != 0
            && let Some(instance) = data.instances.iter_mut().flatten().next()
        {
            instance.ref_count += 1;
            instance.start_time = now;
            return;
        }
        let Some(free) = data.instances.iter().position(Option::is_none) else { return };

        let choices: Vec<usize> = (0..params.sounds.len())
            .filter(|&i| params.flags & foley_flags::DONT_REPEAT == 0 || data.last_variation != Some(i))
            .collect();
        // TodPickFromArray.
        let variation = choices[self.rng.0.next_range(choices.len() as u32) as usize];
        self.system.foley.get_mut(&foley).unwrap().last_variation = Some(variation);

        // TODO: pausing MUTE_ON_PAUSE foley while the game is paused.
        let speed = 2f32.powf(pitch / 12.0);
        let looping = params.flags & foley_flags::LOOP != 0;
        if let Some(entity) = self.spawn(params.sounds[variation], speed, looping) {
            self.system.foley.get_mut(&foley).unwrap().instances[free] =
                Some(FoleyInstance { entity, ref_count: 1, start_time: now });
        }
    }

    /// `TodFoley::StopFoley`: releases one play of a foley type, stopping it
    /// once nothing holds it (one-at-a-time types count their plays).
    pub fn stop_foley(&mut self, foley: FoleyType) {
        let Some(data) = self.system.foley.get_mut(&foley) else { return };
        let Some(slot) = data.instances.iter_mut().find(|i| i.is_some()) else { return };
        let instance = slot.as_mut().expect("found above");
        instance.ref_count -= 1;
        if instance.ref_count == 0 {
            if let Ok(mut entity) = self.commands.get_entity(instance.entity) {
                entity.despawn();
            }
            *slot = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_formats_are_playable() {
        assert!(playable(b"OggS\0\x02rest"));
        assert!(playable(b"RIFF....WAVE"));
        assert!(!playable(b".snd\0\0\0\x18"));
    }

    #[test]
    fn mixer_sums_playing_tracks_only() {
        let mut mixer = Mixer::default();
        let mut out = vec![1.0; 8];
        mixer.render(&mut out);
        assert!(out.iter().all(|&s| s == 0.0));
    }
}
