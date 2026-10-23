//! Finding the player's PvZ install and remembering it between runs.

use std::path::{Path, PathBuf};

use pak::Pak;
use serde::{Deserialize, Serialize};

/// Overrides the configured install, mainly for development.
pub const ENV_GAME_PATH: &str = "PVZ_GAME_PATH";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// What the player picked: the install folder, the `.app`, or `main.pak` itself.
    pub game_path: Option<PathBuf>,
}

fn project_dirs() -> Option<directories::ProjectDirs> {
    directories::ProjectDirs::from("", "", "pvz-goty-rs")
}

fn config_file() -> Option<PathBuf> {
    project_dirs().map(|d| d.config_dir().join("config.toml"))
}

/// Deletes everything this app stores on disk (config, and later saves and
/// caches), returning the directories that were removed.
pub fn clean_user_data() -> std::io::Result<Vec<PathBuf>> {
    let Some(dirs) = project_dirs() else { return Ok(Vec::new()) };
    let mut candidates = vec![dirs.config_dir(), dirs.data_dir(), dirs.data_local_dir(), dirs.cache_dir()];
    candidates.sort();
    candidates.dedup();
    let mut removed = Vec::new();
    for dir in candidates {
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
            removed.push(dir.to_path_buf());
        }
    }
    Ok(removed)
}

impl Config {
    pub fn load() -> Self {
        config_file()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(file) = config_file() else { return Ok(()) };
        std::fs::create_dir_all(file.parent().unwrap())?;
        std::fs::write(file, toml::to_string_pretty(self).expect("config serializes"))
    }
}

/// Finds `main.pak` given any of: the pak itself, a Windows/Linux install
/// folder, a macOS `.app` bundle, or the folder containing the `.app`.
pub fn locate_pak(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    let mut candidates = vec![path.join("main.pak"), path.join("Contents/Resources/main.pak")];
    if let Ok(dir) = std::fs::read_dir(path) {
        for entry in dir.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("app")) {
                candidates.push(p.join("Contents/Resources/main.pak"));
            }
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// Opens and sanity-checks a pak so a wrong pick gets a useful message.
pub fn open_install(path: &Path) -> Result<(PathBuf, Pak), String> {
    let pak_path = locate_pak(path)
        .ok_or_else(|| format!("No main.pak found in {}", path.display()))?;
    let pak = Pak::open(&pak_path).map_err(|e| format!("Couldn't read {}: {e}", pak_path.display()))?;
    if !pak.contains("properties/resources.xml") {
        return Err(format!("{} isn't a Plants vs. Zombies pak", pak_path.display()));
    }
    Ok((pak_path, pak))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locates_pak_in_each_layout() {
        let tmp = std::env::temp_dir().join(format!("pvz-locate-{}", std::process::id()));
        let win = tmp.join("win");
        let mac = tmp.join("mac/Plants vs. Zombies.app/Contents/Resources");
        std::fs::create_dir_all(&win).unwrap();
        std::fs::create_dir_all(&mac).unwrap();
        std::fs::write(win.join("main.pak"), b"").unwrap();
        std::fs::write(mac.join("main.pak"), b"").unwrap();

        assert_eq!(locate_pak(&win), Some(win.join("main.pak")));
        assert_eq!(locate_pak(&win.join("main.pak")), Some(win.join("main.pak")));
        assert_eq!(locate_pak(&tmp.join("mac")), Some(mac.join("main.pak")));
        assert_eq!(locate_pak(&tmp.join("mac/Plants vs. Zombies.app")), Some(mac.join("main.pak")));
        assert_eq!(locate_pak(&tmp), None);
        std::fs::remove_dir_all(tmp).unwrap();
    }
}
