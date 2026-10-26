//! Resource groups from `resources.xml`, turned into Bevy assets.
//!
//! Mirrors the original's split: the tiny `Init` group is loaded up front,
//! everything else on a background "loading thread" whose weighted progress
//! drives the title screen's loading bar.
//! Behaviour per ruslan831 decomp, `LawnApp::LoadingThreadProc`/`LoadGroup`
//! and `SexyAppBase::GetLoadingThreadProgress`.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::asset::RenderAssetUsages;
use bevy::audio::AudioSource;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::{AsyncComputeTaskPool, Task};
use sexy::files::{Files, GameFiles};
use sexy::font::{Font, LoadedFont, latin1_to_string, load_font};
use sexy::image::{DecodedImage, load_image, load_image_resource};
use sexy::reanim::{ReanimDefinition, image_search_paths};
use sexy::particle::ParticleDefinition;
use sexy::resources::{Manifest, ResourceKind};
use sexy::strings::StringTable;

use crate::audio::{Music, MusicFile, SoundSystem, load_song};

/// Group loaded synchronously at startup (PopCap logo).
pub const INIT_GROUP: &str = "Init";
/// Group the title screen needs before it can show the loading bar.
pub const LOADER_BAR_GROUP: &str = "LoaderBar";
const STRINGS_PATH: &str = "properties/LawnStrings.txt";

/// Groups the loading thread loads after `LoaderBar`, with the original's
/// per-resource progress weights ("average ms to load").
const LOADING_GROUPS: [(&str, u32); 3] = [("LoadingImages", 9), ("LoadingFonts", 54), ("LoadingSounds", 54)];
/// Progress weight of each music file loaded by the thread (`Music::GetNumLoadingTasks`).
const MUSIC_TASK_WEIGHT: u32 = 3500;
/// Music the loading thread loads (`Music::MusicInit`); `Main` is loaded by the title screen.
const THREAD_MUSIC: [MusicFile; 2] = [MusicFile::Drums, MusicFile::Hihats];

pub struct ResourcesPlugin;

impl Plugin for ResourcesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, receive_loaded_groups.run_if(resource_exists::<GameResources>));
    }
}

/// An image resource uploaded to the GPU.
#[derive(Clone, Debug)]
pub struct ImageRes {
    pub handle: Handle<Image>,
    pub width: u32,
    pub height: u32,
    /// Cel grid (`rows`/`cols` in the manifest).
    pub rows: u32,
    pub cols: u32,
}

impl ImageRes {
    pub fn cel_width(&self) -> u32 {
        self.width / self.cols.max(1)
    }

    pub fn cel_height(&self) -> u32 {
        self.height / self.rows.max(1)
    }
}

/// An image font with each layer's glyph sheet uploaded.
#[derive(Clone, Debug)]
pub struct FontRes {
    pub font: Arc<Font>,
    /// One per `font.layers` entry.
    pub layer_images: Vec<ImageRes>,
}

/// The parsed manifest plus every resource loaded so far.
#[derive(Resource)]
pub struct GameResources {
    files: GameFiles,
    manifest: Arc<Manifest>,
    images: HashMap<String, ImageRes>,
    fonts: HashMap<String, FontRes>,
    strings: StringTable,
    reanim_definitions: HashMap<String, Arc<ReanimDefinition>>,
    particle_definitions: HashMap<String, Arc<ParticleDefinition>>,
    loaded_groups: HashSet<String>,
}

impl GameResources {
    pub fn new(files: GameFiles) -> Result<Self, String> {
        let xml = files.read("properties/resources.xml").ok_or("properties/resources.xml is missing")?;
        let xml = std::str::from_utf8(&xml).map_err(|e| format!("resources.xml isn't UTF-8: {e}"))?;
        let manifest = Manifest::parse(xml).map_err(|e| format!("resources.xml: {e}"))?;
        Ok(Self {
            files,
            manifest: Arc::new(manifest),
            images: HashMap::new(),
            fonts: HashMap::new(),
            strings: StringTable::default(),
            reanim_definitions: HashMap::new(),
            particle_definitions: HashMap::new(),
            loaded_groups: HashSet::new(),
        })
    }

    /// Raw bytes of a game file (pak first, then loose files).
    pub fn read_file(&self, path: &str) -> Option<Vec<u8>> {
        self.files.read(path).map(|b| b.into_owned())
    }

    pub fn image(&self, id: &str) -> Option<&ImageRes> {
        self.images.get(id)
    }

    pub fn font(&self, id: &str) -> Option<&FontRes> {
        self.fonts.get(id)
    }

    /// `TodStringTranslate`: `[NAME]` → text from `LawnStrings.txt`.
    pub fn translate(&self, s: &str) -> String {
        self.strings.translate(s)
    }

    pub fn is_group_loaded(&self, group: &str) -> bool {
        self.loaded_groups.contains(group)
    }

    /// `ReanimatorEnsureDefinitionLoaded`: parses a reanimation file and loads
    /// every image it uses, on first use. Images missing from the manifest
    /// are found by path (`DefinitionLoadImage`).
    pub fn reanim_definition(&mut self, path: &str, assets: &mut Assets<Image>) -> Result<Arc<ReanimDefinition>, String> {
        if let Some(def) = self.reanim_definitions.get(path) {
            return Ok(def.clone());
        }
        let def = Arc::new(ReanimDefinition::load(&self.files, path).map_err(|e| format!("{path}: {e}"))?);

        for id in def.image_ids() {
            self.load_image(id, assets).map_err(|e| format!("{path}: {e}"))?;
        }

        self.reanim_definitions.insert(path.to_owned(), def.clone());
        Ok(def)
    }

    /// `TodParticleLoadADef`: parses a particle effect and loads its images,
    /// on first use. (The original loads every effect at startup.)
    pub fn particle_definition(&mut self, path: &str, assets: &mut Assets<Image>) -> Result<Arc<ParticleDefinition>, String> {
        if let Some(def) = self.particle_definitions.get(path) {
            return Ok(def.clone());
        }
        let def = Arc::new(ParticleDefinition::load(&self.files, path).map_err(|e| format!("{path}: {e}"))?);
        for id in def.image_ids() {
            self.load_image(id, assets).map_err(|e| format!("{path}: {e}"))?;
        }
        self.particle_definitions.insert(path.to_owned(), def.clone());
        Ok(def)
    }

    /// A reanimation definition already loaded by [`Self::reanim_definition`].
    pub fn loaded_reanim_definition(&self, path: &str) -> Option<Arc<ReanimDefinition>> {
        self.reanim_definitions.get(path).cloned()
    }

    /// Returns an image, loading it now if it hasn't been: from the manifest,
    /// or by path for ids that aren't in it (`DefinitionLoadImage`).
    pub fn load_image(&mut self, id: &str, assets: &mut Assets<Image>) -> Result<&ImageRes, String> {
        if !self.images.contains_key(id) {
            let (image, rows, cols) = match self.manifest.get(id).and_then(|r| r.image().map(|info| (r, info))) {
                Some((res, info)) => load_image_resource(&self.files, &res.path, info)
                    .map(|i| (i, info.rows, info.cols))
                    .map_err(|e| format!("{id}: {e}"))?,
                None => image_search_paths(id)
                    .iter()
                    .find_map(|p| load_image(&self.files, p, true, 0xFF_FFFF))
                    .map(|i| (i, 1, 1))
                    .ok_or_else(|| format!("image {id} not found"))?,
            };
            self.images.insert(id.to_owned(), upload(image, rows, cols, assets));
        }
        Ok(&self.images[id])
    }

    /// The pixels of a manifest image, decoded again from the game's files.
    /// For effects that compute from a picture: Bevy's copy isn't kept in
    /// the main world once it's on the GPU.
    pub fn decode_image(&self, id: &str) -> Option<DecodedImage> {
        let res = self.manifest.get(id)?;
        let info = res.image()?;
        load_image_resource(&self.files, &res.path, info).ok()
    }

    /// Loads a group on the calling thread (`TodLoadResources`), e.g. `Init`
    /// or a level's background. Does nothing if it's already loaded.
    pub fn load_group_now(&mut self, group: &str, assets: &mut Assets<Image>) -> Result<(), String> {
        if self.is_group_loaded(group) {
            return Ok(());
        }
        let loaded = load_group(&self.files, &self.manifest, group, &|| {})?;
        self.insert(loaded, assets);
        Ok(())
    }

    /// Starts the background loading thread.
    pub fn start_loading_thread(&self) -> LoadingThread {
        let shared = Arc::new(LoadingShared::default());
        let (files, manifest, thread_shared) = (self.files.clone(), self.manifest.clone(), shared.clone());
        let task = AsyncComputeTaskPool::get().spawn(async move {
            if let Err(e) = loading_thread_proc(&files, &manifest, &thread_shared) {
                error!("loading failed: {e}");
                *thread_shared.error.lock().unwrap() = Some(e);
            }
            thread_shared.finished.store(true, Ordering::Release);
        });
        LoadingThread { shared, _task: task }
    }

    /// Uploads a group's images and fonts; returns its sounds for the [`SoundSystem`].
    fn insert(&mut self, group: LoadedGroup, assets: &mut Assets<Image>) -> Vec<(String, Vec<u8>)> {
        for (id, image, rows, cols) in group.images {
            self.images.insert(id, upload(image, rows, cols, assets));
        }
        for (id, loaded) in group.fonts {
            let layer_images = loaded.images.into_iter().map(|i| upload(i, 1, 1, assets)).collect();
            self.fonts.insert(id, FontRes { font: Arc::new(loaded.font), layer_images });
        }
        self.loaded_groups.insert(group.name);
        group.sounds
    }
}

fn upload(image: DecodedImage, rows: u32, cols: u32, assets: &mut Assets<Image>) -> ImageRes {
    let (width, height) = (image.width, image.height);
    ImageRes { handle: assets.add(to_bevy_image(image)), width, height, rows, cols }
}

fn to_bevy_image(mut image: DecodedImage) -> Image {
    image.bleed_alpha();
    Image::new(
        Extent3d { width: image.width, height: image.height, depth_or_array_layers: 1 },
        TextureDimension::D2,
        image.rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Decoded resources of one group, waiting to be turned into assets.
struct LoadedGroup {
    name: String,
    images: Vec<(String, DecodedImage, u32, u32)>,
    fonts: Vec<(String, LoadedFont)>,
    sounds: Vec<(String, Vec<u8>)>,
}

/// What the loading thread hands to the main thread.
enum Loaded {
    Group(LoadedGroup),
    Strings(StringTable),
    Music(MusicFile, Result<openmpt::Module, String>),
}

/// Loads every resource of a group, calling `tick` after each one.
fn load_group(files: &GameFiles, manifest: &Manifest, group: &str, tick: &dyn Fn()) -> Result<LoadedGroup, String> {
    let resources = manifest.group(group).ok_or_else(|| format!("resource group {group:?} doesn't exist"))?;
    let (mut images, mut fonts, mut sounds) = (Vec::new(), Vec::new(), Vec::new());
    for res in resources {
        match &res.kind {
            ResourceKind::Image(info) => {
                let image = load_image_resource(files, &res.path, info).map_err(|e| format!("{}: {e}", res.id))?;
                images.push((res.id.clone(), image, info.rows, info.cols));
            }
            ResourceKind::Font => {
                fonts.push((res.id.clone(), load_font(files, &res.path).map_err(|e| format!("{}: {e}", res.id))?));
            }
            // Extensions in the order DSoundManager::LoadSound tries them;
            // the Windows pak has `.au` sounds.
            ResourceKind::Sound => {
                let bytes = ["wav", "mp3", "ogg", "au"]
                    .iter()
                    .find_map(|e| files.read(&format!("{}.{e}", res.path)))
                    .ok_or_else(|| format!("{}: sound {:?} not found", res.id, res.path))?;
                sounds.push((res.id.clone(), bytes.into_owned()));
            }
        }
        tick();
    }
    Ok(LoadedGroup { name: group.to_owned(), images, fonts, sounds })
}

#[derive(Default)]
struct LoadingShared {
    completed: AtomicU32,
    total: AtomicU32,
    finished: AtomicBool,
    error: Mutex<Option<String>>,
    inbox: Mutex<Vec<Loaded>>,
}

/// Handle to the background loader. Groups it finishes are moved into
/// [`GameResources`] by `receive_loaded_groups`.
#[derive(Resource)]
pub struct LoadingThread {
    shared: Arc<LoadingShared>,
    _task: Task<()>,
}

impl LoadingThread {
    /// Weighted fraction of loading work done, 0..=1.
    pub fn progress(&self) -> f32 {
        let total = self.shared.total.load(Ordering::Acquire);
        if total == 0 {
            return 0.0;
        }
        (self.shared.completed.load(Ordering::Acquire) as f32 / total as f32).min(1.0)
    }

    /// True once the thread has finished and its last group has been received.
    pub fn is_complete(&self) -> bool {
        self.shared.finished.load(Ordering::Acquire) && self.shared.inbox.lock().unwrap().is_empty()
    }

    pub fn error(&self) -> Option<String> {
        self.shared.error.lock().unwrap().clone()
    }
}

fn loading_thread_proc(files: &GameFiles, manifest: &Manifest, shared: &LoadingShared) -> Result<(), String> {
    let loader_bar = load_group(files, manifest, LOADER_BAR_GROUP, &|| {})?;
    let strings = files.read(STRINGS_PATH).ok_or(format!("{STRINGS_PATH} is missing"))?;
    let strings = StringTable::parse(&latin1_to_string(&strings)).map_err(|e| format!("{STRINGS_PATH}: {e}"))?;
    // Strings first: the title screen shows "LOADING..." as soon as LoaderBar is in.
    shared.inbox.lock().unwrap().extend([Loaded::Strings(strings), Loaded::Group(loader_bar)]);

    let mut total = 0;
    for (group, weight) in LOADING_GROUPS {
        total += manifest.group(group).map_or(0, |g| g.count() as u32) * weight;
    }
    total += THREAD_MUSIC.len() as u32 * MUSIC_TASK_WEIGHT;
    // TODO: the original also counts reanimation, particle and preload tasks
    // here; add them as those systems get ported.
    shared.total.store(total, Ordering::Release);

    for (group, weight) in LOADING_GROUPS {
        let loaded = load_group(files, manifest, group, &|| {
            shared.completed.fetch_add(weight, Ordering::AcqRel);
        })?;
        shared.inbox.lock().unwrap().push(Loaded::Group(loaded));

        // LoadingThreadProc runs MusicInit between the fonts and the sounds.
        if group == "LoadingFonts" {
            for file in THREAD_MUSIC {
                let song = files.read(file.path()).ok_or(format!("{} is missing", file.path())).and_then(|b| load_song(&b));
                shared.inbox.lock().unwrap().push(Loaded::Music(file, song));
                shared.completed.fetch_add(MUSIC_TASK_WEIGHT, Ordering::AcqRel);
            }
        }
    }
    Ok(())
}

fn receive_loaded_groups(
    thread: Option<Res<LoadingThread>>,
    mut resources: ResMut<GameResources>,
    mut assets: ResMut<Assets<Image>>,
    mut sounds: ResMut<SoundSystem>,
    mut audio: ResMut<Assets<AudioSource>>,
    mut music: Option<ResMut<Music>>,
) {
    let Some(thread) = thread else { return };
    let loaded = std::mem::take(&mut *thread.shared.inbox.lock().unwrap());
    for item in loaded {
        match item {
            Loaded::Strings(strings) => resources.strings = strings,
            Loaded::Music(file, song) => {
                if let Some(music) = &mut music {
                    music.set_song(file, song);
                }
            }
            Loaded::Group(group) => {
                info!(
                    "loaded resource group {} ({} images, {} fonts)",
                    group.name,
                    group.images.len(),
                    group.fonts.len()
                );
                for (id, bytes) in resources.insert(group, &mut assets) {
                    sounds.insert(id, bytes, &mut audio);
                }
            }
        }
    }
}
