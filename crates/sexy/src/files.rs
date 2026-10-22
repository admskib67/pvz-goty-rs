//! Where game files are read from: the pak first, then loose files beside it
//! (e.g. `properties/partner_logo.jpg` ships outside `main.pak`).

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use pak::Pak;

pub trait Files: Send + Sync {
    fn read(&self, path: &str) -> Option<Cow<'_, [u8]>>;
}

impl Files for Pak {
    fn read(&self, path: &str) -> Option<Cow<'_, [u8]>> {
        self.get(path).map(Cow::Borrowed)
    }
}

/// A pak plus the directory it lives in.
#[derive(Clone)]
pub struct GameFiles {
    pub pak: Arc<Pak>,
    pub root: PathBuf,
}

impl Files for GameFiles {
    fn read(&self, path: &str) -> Option<Cow<'_, [u8]>> {
        self.pak.read(path).or_else(|| std::fs::read(self.root.join(path.replace('\\', "/"))).ok().map(Cow::Owned))
    }
}
