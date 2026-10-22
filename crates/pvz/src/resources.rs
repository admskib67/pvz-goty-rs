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
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::{AsyncComputeTaskPool, Task};
use sexy::files::{Files, GameFiles};
use sexy::image::{DecodedImage, load_image_resource};
use sexy::resources::{Manifest, ResourceKind};

/// Group loaded synchronously at startup (PopCap logo).
pub const INIT_GROUP: &str = "Init";
/// Group the title screen needs before it can show the loading bar.
pub const LOADER_BAR_GROUP: &str = "LoaderBar";

/// Groups the loading thread loads after `LoaderBar`, with the original's
/// per-resource progress weights ("average ms to load").
const LOADING_GROUPS: [(&str, u32); 3] = [("LoadingImages", 9), ("LoadingFonts", 54), ("LoadingSounds", 54)];

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
    /// Cel grid, for `DrawImageCel` once a screen needs it.
    #[expect(dead_code, reason = "no ported screen draws cels yet")]
    pub rows: u32,
    #[expect(dead_code, reason = "no ported screen draws cels yet")]
    pub cols: u32,
}

/// The parsed manifest plus every resource loaded so far.
#[derive(Resource)]
pub struct GameResources {
    files: GameFiles,
    manifest: Arc<Manifest>,
    images: HashMap<String, ImageRes>,
    loaded_groups: HashSet<String>,
}

impl GameResources {
    pub fn new(files: GameFiles) -> Result<Self, String> {
        let xml = files.read("properties/resources.xml").ok_or("properties/resources.xml is missing")?;
        let xml = std::str::from_utf8(&xml).map_err(|e| format!("resources.xml isn't UTF-8: {e}"))?;
        let manifest = Manifest::parse(xml).map_err(|e| format!("resources.xml: {e}"))?;
        Ok(Self { files, manifest: Arc::new(manifest), images: HashMap::new(), loaded_groups: HashSet::new() })
    }

    pub fn image(&self, id: &str) -> Option<&ImageRes> {
        self.images.get(id)
    }

    pub fn is_group_loaded(&self, group: &str) -> bool {
        self.loaded_groups.contains(group)
    }

    /// Loads a group on the calling thread. Only for tiny groups like `Init`.
    pub fn load_group_now(&mut self, group: &str, assets: &mut Assets<Image>) -> Result<(), String> {
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

    fn insert(&mut self, group: LoadedGroup, assets: &mut Assets<Image>) {
        for (id, image, rows, cols) in group.images {
            let (width, height) = (image.width, image.height);
            let handle = assets.add(to_bevy_image(image));
            self.images.insert(id, ImageRes { handle, width, height, rows, cols });
        }
        self.loaded_groups.insert(group.name);
    }
}

fn to_bevy_image(image: DecodedImage) -> Image {
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
}

/// Loads every resource of a group, calling `tick` after each one.
fn load_group(files: &GameFiles, manifest: &Manifest, group: &str, tick: &dyn Fn()) -> Result<LoadedGroup, String> {
    let resources = manifest.group(group).ok_or_else(|| format!("resource group {group:?} doesn't exist"))?;
    let mut images = Vec::new();
    for res in resources {
        match &res.kind {
            ResourceKind::Image(info) => {
                let image = load_image_resource(files, &res.path, info).map_err(|e| format!("{}: {e}", res.id))?;
                images.push((res.id.clone(), image, info.rows, info.cols));
            }
            // TODO: decode once audio and image fonts exist. For now just
            // check the files are present so missing data fails early.
            ResourceKind::Sound => {
                if !["ogg", "wav", "mp3"].iter().any(|e| files.read(&format!("{}.{e}", res.path)).is_some()) {
                    return Err(format!("{}: sound {:?} not found", res.id, res.path));
                }
            }
            ResourceKind::Font => {
                files.read(&res.path).ok_or_else(|| format!("{}: font {:?} not found", res.id, res.path))?;
            }
        }
        tick();
    }
    Ok(LoadedGroup { name: group.to_owned(), images })
}

#[derive(Default)]
struct LoadingShared {
    completed: AtomicU32,
    total: AtomicU32,
    finished: AtomicBool,
    error: Mutex<Option<String>>,
    inbox: Mutex<Vec<LoadedGroup>>,
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
    shared.inbox.lock().unwrap().push(loader_bar);

    let mut total = 0;
    for (group, weight) in LOADING_GROUPS {
        total += manifest.group(group).map_or(0, |g| g.count() as u32) * weight;
    }
    // TODO: the original also counts music, reanimation, particle and preload
    // tasks here; add them as those systems get ported.
    shared.total.store(total, Ordering::Release);

    for (group, weight) in LOADING_GROUPS {
        let loaded = load_group(files, manifest, group, &|| {
            shared.completed.fetch_add(weight, Ordering::AcqRel);
        })?;
        shared.inbox.lock().unwrap().push(loaded);
    }
    Ok(())
}

fn receive_loaded_groups(
    thread: Option<Res<LoadingThread>>,
    mut resources: ResMut<GameResources>,
    mut assets: ResMut<Assets<Image>>,
) {
    let Some(thread) = thread else { return };
    let groups = std::mem::take(&mut *thread.shared.inbox.lock().unwrap());
    for group in groups {
        info!("loaded resource group {} ({} images)", group.name, group.images.len());
        resources.insert(group, &mut assets);
    }
}
