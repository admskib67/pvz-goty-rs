//! Reader for PopCap `.pak` archives, as used by `main.pak` in Plants vs. Zombies.
//!
//! Layout (every byte of the file is XOR'd with [`XOR_KEY`]):
//!
//! ```text
//! u32 magic   = 0xBAC04AC0
//! u32 version = 0
//! repeat {
//!     u8  flags          // 0x80 = end of directory
//!     u8  name_len
//!     [u8; name_len]     // path, '\' separated
//!     u32 size
//!     u64 filetime       // Windows FILETIME
//! }
//! file data, concatenated in directory order
//! ```
//!
//! Lookups are case-insensitive and accept either slash direction, matching
//! how SexyFramework resolves paths.

use std::collections::HashMap;
use std::path::Path;

pub const MAGIC: u32 = 0xBAC0_4AC0;
pub const XOR_KEY: u8 = 0xF7;
const FLAG_END: u8 = 0x80;

#[derive(Debug, thiserror::Error)]
pub enum PakError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a PopCap pak (magic {0:#010x})")]
    BadMagic(u32),
    #[error("unsupported pak version {0}")]
    BadVersion(u32),
    #[error("pak directory is truncated")]
    Truncated,
    #[error("pak entry {name:?} extends past end of file")]
    EntryOutOfBounds { name: String },
}

#[derive(Debug, Clone)]
pub struct PakEntry {
    /// Path as stored in the archive, with `/` separators.
    pub name: String,
    pub offset: usize,
    pub size: usize,
    pub filetime: u64,
}

/// A fully decrypted pak held in memory (main.pak is ~40 MB).
pub struct Pak {
    data: Vec<u8>,
    entries: Vec<PakEntry>,
    index: HashMap<String, usize>,
}

impl std::fmt::Debug for Pak {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pak").field("entries", &self.entries.len()).finish()
    }
}

/// Normalises a path to the form used as an index key.
pub fn normalize_path(path: &str) -> String {
    path.trim_start_matches(['/', '\\'])
        .replace('\\', "/")
        .to_ascii_lowercase()
}

impl Pak {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PakError> {
        Self::from_bytes(std::fs::read(path)?)
    }

    /// Parses an encrypted pak image.
    pub fn from_bytes(mut data: Vec<u8>) -> Result<Self, PakError> {
        for b in &mut data {
            *b ^= XOR_KEY;
        }

        let mut cur = Cursor { data: &data, pos: 0 };
        let magic = cur.u32()?;
        if magic != MAGIC {
            return Err(PakError::BadMagic(magic));
        }
        let version = cur.u32()?;
        if version != 0 {
            return Err(PakError::BadVersion(version));
        }

        let mut raw = Vec::new();
        loop {
            let flags = cur.u8()?;
            if flags & FLAG_END != 0 {
                break;
            }
            let len = cur.u8()? as usize;
            let name = String::from_utf8_lossy(cur.bytes(len)?).replace('\\', "/");
            let size = cur.u32()? as usize;
            let filetime = cur.u64()?;
            raw.push((name, size, filetime));
        }

        let mut offset = cur.pos;
        let mut entries = Vec::with_capacity(raw.len());
        let mut index = HashMap::with_capacity(raw.len());
        for (name, size, filetime) in raw {
            if offset + size > data.len() {
                return Err(PakError::EntryOutOfBounds { name });
            }
            index.insert(normalize_path(&name), entries.len());
            entries.push(PakEntry { name, offset, size, filetime });
            offset += size;
        }

        Ok(Self { data, entries, index })
    }

    pub fn entries(&self) -> &[PakEntry] {
        &self.entries
    }

    pub fn entry(&self, path: &str) -> Option<&PakEntry> {
        self.index.get(&normalize_path(path)).map(|&i| &self.entries[i])
    }

    pub fn contains(&self, path: &str) -> bool {
        self.entry(path).is_some()
    }

    /// Returns the decrypted contents of a file.
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.entry(path)
            .map(|e| &self.data[e.offset..e.offset + e.size])
    }

    /// Finds `base` with the first matching extension. Resource manifests
    /// name files without one (`images/titlescreen`), so callers try each.
    pub fn find_with_extensions(&self, base: &str, exts: &[&str]) -> Option<&PakEntry> {
        exts.iter().find_map(|ext| self.entry(&format!("{base}.{ext}")))
    }

    /// True if any file lives under `dir`.
    pub fn is_dir(&self, dir: &str) -> bool {
        let prefix = dir_prefix(dir);
        prefix.is_empty() || self.index.keys().any(|k| k.starts_with(&prefix))
    }

    /// Lists the immediate children (files and directories) of `dir`, as
    /// full archive paths in their stored case.
    pub fn read_dir(&self, dir: &str) -> Vec<String> {
        let prefix = dir_prefix(dir);
        let mut out: Vec<String> = Vec::new();
        for e in &self.entries {
            let lower = e.name.to_ascii_lowercase();
            let Some(rest) = lower.strip_prefix(&prefix) else { continue };
            let child_len = rest.find('/').unwrap_or(rest.len());
            let child = &e.name[..prefix.len() + child_len];
            if !out.iter().any(|c| c.eq_ignore_ascii_case(child)) {
                out.push(child.to_owned());
            }
        }
        out
    }
}

fn dir_prefix(dir: &str) -> String {
    let mut p = normalize_path(dir);
    if !p.is_empty() && !p.ends_with('/') {
        p.push('/');
    }
    p
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8], PakError> {
        let s = self.data.get(self.pos..self.pos + n).ok_or(PakError::Truncated)?;
        self.pos += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, PakError> {
        Ok(self.bytes(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, PakError> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, PakError> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }
}

/// Builds an encrypted pak image. Used by tests and tooling.
pub fn build(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for (name, data) in files {
        let name = name.replace('/', "\\");
        out.push(0);
        out.push(name.len() as u8);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&0u64.to_le_bytes());
    }
    out.push(FLAG_END);
    for (_, data) in files {
        out.extend_from_slice(data);
    }
    for b in &mut out {
        *b ^= XOR_KEY;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Pak {
        Pak::from_bytes(build(&[
            ("properties/resources.xml", b"<ResourceManifest/>"),
            ("images/Titlescreen.jpg", b"jpg"),
            ("images/sub/a.png", b"png"),
        ]))
        .unwrap()
    }

    #[test]
    fn roundtrip_and_case_insensitive_lookup() {
        let pak = sample();
        assert_eq!(pak.entries().len(), 3);
        assert_eq!(pak.get("properties/resources.xml"), Some(&b"<ResourceManifest/>"[..]));
        assert_eq!(pak.get("IMAGES\\titlescreen.JPG"), Some(&b"jpg"[..]));
        assert_eq!(pak.entry("images/titlescreen.jpg").unwrap().name, "images/Titlescreen.jpg");
        assert!(pak.get("images/missing.png").is_none());
    }

    #[test]
    fn extension_lookup() {
        let pak = sample();
        let e = pak.find_with_extensions("images/titlescreen", &["png", "jpg"]).unwrap();
        assert_eq!(e.name, "images/Titlescreen.jpg");
        assert!(pak.find_with_extensions("images/titlescreen", &["gif"]).is_none());
    }

    #[test]
    fn directories() {
        let pak = sample();
        assert!(pak.is_dir("images"));
        assert!(pak.is_dir("Images/Sub/"));
        assert!(!pak.is_dir("sounds"));
        assert_eq!(pak.read_dir("images"), vec!["images/Titlescreen.jpg", "images/sub"]);
        assert_eq!(pak.read_dir(""), vec!["properties", "images"]);
    }

    #[test]
    fn rejects_bad_magic() {
        assert!(matches!(Pak::from_bytes(vec![0; 16]), Err(PakError::BadMagic(_))));
    }

    #[test]
    fn rejects_truncated_entry() {
        let mut img = build(&[("a.txt", b"hello")]);
        img.truncate(img.len() - 2);
        assert!(matches!(Pak::from_bytes(img), Err(PakError::EntryOutOfBounds { .. })));
    }
}
