//! Parses `resources.xml` from a pak and loads every image and font
//! resource, reporting any that fail.
//!
//!     cargo run -p sexy --release --example check_resources -- <main.pak>

use std::path::PathBuf;
use std::sync::Arc;

use sexy::files::GameFiles;
use sexy::font::load_font;
use sexy::image::load_image_resource;
use sexy::resources::{Manifest, ResourceKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(std::env::args_os().nth(1).ok_or("usage: check_resources <main.pak>")?);
    let pak = Arc::new(pak::Pak::open(&path)?);
    let files = GameFiles { pak: pak.clone(), root: path.parent().unwrap().to_path_buf() };
    let xml = pak.get("properties/resources.xml").ok_or("no properties/resources.xml")?;
    let manifest = Manifest::parse(std::str::from_utf8(xml)?)?;

    let (mut ok, mut failed) = (0, 0);
    let mut report = |group: &str, id: &str, result: Result<(), String>| match result {
        Ok(()) => ok += 1,
        Err(e) => {
            failed += 1;
            println!("{group}/{id}: {e}");
        }
    };
    for group in manifest.group_names() {
        for res in manifest.group(group).unwrap() {
            match &res.kind {
                ResourceKind::Image(info) => report(
                    group,
                    &res.id,
                    load_image_resource(&files, &res.path, info).map(|_| ()).map_err(|e| e.to_string()),
                ),
                ResourceKind::Font => report(
                    group,
                    &res.id,
                    load_font(&files, &res.path).map(|f| {
                        let w = f.font.string_width("Plants vs. Zombies");
                        println!("{}: {} layer(s), ascent {}, sample width {w}", res.id, f.font.layers.len(), f.font.ascent);
                    }).map_err(|e| e.to_string()),
                ),
                ResourceKind::Sound => {}
            }
        }
    }
    println!("{ok} images and fonts loaded, {failed} failed");
    Ok(())
}
