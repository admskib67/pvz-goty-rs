//! Exposes the game's `main.pak` to Bevy as the `pak://` asset source.
//!
//! The source has to be registered before `AssetPlugin` is built, but the pak
//! location is only known after the setup screen runs. So the reader holds a
//! shared slot that starts empty and is filled once an install is chosen;
//! until then every read reports `NotFound`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use bevy::asset::io::{
    AssetReader, AssetReaderError, AssetSourceBuilder, PathStream, Reader, VecReader,
};
use bevy::prelude::*;
use bevy::tasks::futures_lite::stream;
use pak::Pak;

pub const SOURCE: &str = "pak";

/// The loaded pak, shared between the ECS and the asset reader.
#[derive(Resource, Clone, Default)]
pub struct GamePak(Arc<RwLock<Option<Arc<Pak>>>>);

impl GamePak {
    pub fn set(&self, pak: Pak) {
        *self.0.write().unwrap() = Some(Arc::new(pak));
    }

    pub fn get(&self) -> Option<Arc<Pak>> {
        self.0.read().unwrap().clone()
    }
}

pub struct PakSourcePlugin;

impl Plugin for PakSourcePlugin {
    fn build(&self, app: &mut App) {
        let pak = GamePak::default();
        let reader_pak = pak.clone();
        app.insert_resource(pak).register_asset_source(
            SOURCE,
            AssetSourceBuilder::new(move || Box::new(PakAssetReader(reader_pak.clone()))),
        );
    }
}

struct PakAssetReader(GamePak);

fn path_str(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

impl AssetReader for PakAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let not_found = || AssetReaderError::NotFound(path.to_path_buf());
        let pak = self.0.get().ok_or_else(not_found)?;
        let bytes = pak.get(&path_str(path)).ok_or_else(not_found)?;
        Ok(VecReader::new(bytes.to_vec()))
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        // The pak never carries .meta files; Bevy falls back to default settings.
        Err::<VecReader, _>(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        let pak = self
            .0
            .get()
            .ok_or_else(|| AssetReaderError::NotFound(path.to_path_buf()))?;
        let children: Vec<PathBuf> = pak.read_dir(&path_str(path)).into_iter().map(PathBuf::from).collect();
        Ok(Box::new(stream::iter(children)))
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(self.0.get().is_some_and(|pak| pak.is_dir(&path_str(path))))
    }
}
