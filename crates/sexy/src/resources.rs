//! Parser for SexyFramework resource manifests (`properties/resources.xml`).
//!
//! Behaviour per ruslan831 decomp, `ResourceManager::ParseCommonResource`,
//! `ParseImageResource` and `ParseSetDefaults`.

use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("invalid XML: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("root element is <{0}>, expected <ResourceManifest>")]
    BadRoot(String),
    #[error("<{element}> in group {group:?} has no path")]
    MissingPath { element: String, group: String },
    #[error("resource id {0:?} is defined twice")]
    DuplicateId(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResourceKind {
    Image(ImageInfo),
    Sound,
    Font,
}

/// Image-specific attributes. Purely renderer hints (`a8r8g8b8`, `ddsurface`,
/// `minsubdivide`, `nopal`) are dropped.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageInfo {
    /// Grid used to split the image into cels.
    pub rows: u32,
    pub cols: u32,
    /// Explicit alpha mask covering the whole image.
    pub alpha_image: Option<String>,
    /// Alpha mask for a single cel, applied to every cel.
    pub alpha_grid: Option<String>,
    /// RGB used for images that only exist as an alpha mask.
    pub alpha_color: u32,
}

impl Default for ImageInfo {
    fn default() -> Self {
        Self { rows: 1, cols: 1, alpha_image: None, alpha_grid: None, alpha_color: 0xFF_FFFF }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resource {
    /// Full id including the group's prefix, e.g. `IMAGE_TITLESCREEN`.
    pub id: String,
    /// Path inside the pak, without extension, e.g. `images/titlescreen`.
    pub path: String,
    pub kind: ResourceKind,
}

impl Resource {
    pub fn image(&self) -> Option<&ImageInfo> {
        match &self.kind {
            ResourceKind::Image(info) => Some(info),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct Manifest {
    resources: Vec<Resource>,
    groups: Vec<(String, Vec<usize>)>,
    by_id: HashMap<String, usize>,
}

impl Manifest {
    pub fn parse(xml: &str) -> Result<Self, ManifestError> {
        let doc = roxmltree::Document::parse(xml.trim_start_matches('\u{feff}'))?;
        let root = doc.root_element();
        if root.tag_name().name() != "ResourceManifest" {
            return Err(ManifestError::BadRoot(root.tag_name().name().to_owned()));
        }

        let mut manifest = Self::default();
        for group in root.children().filter(|n| n.has_tag_name("Resources")) {
            let group_name = group.attribute("id").unwrap_or_default().to_owned();
            let mut default_path = String::new();
            let mut default_prefix = String::new();
            let mut members = Vec::new();

            for el in group.children().filter(|n| n.is_element()) {
                let element = el.tag_name().name();
                if element == "SetDefaults" {
                    if let Some(p) = el.attribute("path") {
                        default_path = format!("{}/", p.trim_end_matches(['/', '\\']));
                    }
                    if let Some(p) = el.attribute("idprefix") {
                        default_prefix = p.trim_end_matches(['/', '\\']).to_owned();
                    }
                    continue;
                }
                let kind = match element {
                    "Image" => ResourceKind::Image(parse_image(&el, &default_path)),
                    "Sound" => ResourceKind::Sound,
                    "Font" => ResourceKind::Font,
                    _ => continue,
                };

                let path = el.attribute("path").filter(|p| !p.is_empty()).ok_or_else(|| {
                    ManifestError::MissingPath { element: element.to_owned(), group: group_name.clone() }
                })?;
                // `!`-prefixed paths name program-generated resources and skip the default path.
                let path = if path.starts_with('!') { path.to_owned() } else { format!("{default_path}{path}") };
                let id = match el.attribute("id") {
                    Some(id) => format!("{default_prefix}{id}"),
                    None => format!("{default_prefix}{}", file_stem(&path)),
                };

                if manifest.by_id.contains_key(&id) {
                    return Err(ManifestError::DuplicateId(id));
                }
                manifest.by_id.insert(id.clone(), manifest.resources.len());
                members.push(manifest.resources.len());
                manifest.resources.push(Resource { id, path, kind });
            }
            manifest.groups.push((group_name, members));
        }
        Ok(manifest)
    }

    pub fn get(&self, id: &str) -> Option<&Resource> {
        self.by_id.get(id).map(|&i| &self.resources[i])
    }

    pub fn group_names(&self) -> impl Iterator<Item = &str> {
        self.groups.iter().map(|(name, _)| name.as_str())
    }

    /// Resources of a group in manifest order, or `None` if it doesn't exist.
    pub fn group(&self, name: &str) -> Option<impl Iterator<Item = &Resource>> {
        self.groups
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, members)| members.iter().map(|&i| &self.resources[i]))
    }
}

fn parse_image(el: &roxmltree::Node, default_path: &str) -> ImageInfo {
    let int = |name| el.attribute(name).and_then(|v: &str| v.trim().parse().ok());
    ImageInfo {
        rows: int("rows").unwrap_or(1),
        cols: int("cols").unwrap_or(1),
        alpha_image: el.attribute("alphaimage").map(|p| format!("{default_path}{p}")),
        alpha_grid: el.attribute("alphagrid").map(|p| format!("{default_path}{p}")),
        alpha_color: el
            .attribute("alphacolor")
            .and_then(|v| u32::from_str_radix(v.trim(), 16).ok())
            .unwrap_or(0xFF_FFFF),
    }
}

/// File name without directory or extension (SexyFramework `GetFileName(path, true)`).
fn file_stem(path: &str) -> &str {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = "\u{feff}<?xml version=\"1.0\"?>
<ResourceManifest>
<Resources id=\"Init\">
  <SetDefaults path=\"images\" idprefix=\"IMAGE_\" />
  <Image id=\"BLANK\" path=\"blank\" />
  <Image id=\"SEEDS\" path=\"seeds\" cols=\"9\" />
  <Image id=\"POOL_NIGHT\" path=\"pool_night\" alphagrid=\"pool_\" rows=\"2\" alphacolor=\"ff0000\" />
  <SetDefaults path=\"sounds/\" idprefix=\"SOUND_\" />
  <Sound path=\"buttonclick\" />
</Resources>
<Resources id=\"Fonts\">
  <SetDefaults path=\"data\" idprefix=\"FONT_\" />
  <Font id=\"PICO129\" path=\"Pico129.txt\" />
</Resources>
</ResourceManifest>";

    #[test]
    fn parses_groups_defaults_and_attributes() {
        let m = Manifest::parse(XML).unwrap();
        assert_eq!(m.group_names().collect::<Vec<_>>(), ["Init", "Fonts"]);

        let ids: Vec<_> = m.group("Init").unwrap().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["IMAGE_BLANK", "IMAGE_SEEDS", "IMAGE_POOL_NIGHT", "SOUND_buttonclick"]);

        let seeds = m.get("IMAGE_SEEDS").unwrap();
        assert_eq!(seeds.path, "images/seeds");
        assert_eq!(seeds.image().unwrap().cols, 9);
        assert_eq!(seeds.image().unwrap().rows, 1);

        let pool = m.get("IMAGE_POOL_NIGHT").unwrap().image().unwrap();
        assert_eq!(pool.alpha_grid.as_deref(), Some("images/pool_"));
        assert_eq!(pool.alpha_color, 0xFF0000);

        let sound = m.get("SOUND_buttonclick").unwrap();
        assert_eq!(sound.path, "sounds/buttonclick");
        assert_eq!(sound.kind, ResourceKind::Sound);

        assert_eq!(m.get("FONT_PICO129").unwrap().path, "data/Pico129.txt");
        assert!(m.group("Missing").is_none());
    }

    #[test]
    fn rejects_duplicate_ids() {
        let xml = r#"<ResourceManifest><Resources id="A">
            <Image id="X" path="a"/><Image id="X" path="b"/></Resources></ResourceManifest>"#;
        assert!(matches!(Manifest::parse(xml), Err(ManifestError::DuplicateId(id)) if id == "X"));
    }
}
