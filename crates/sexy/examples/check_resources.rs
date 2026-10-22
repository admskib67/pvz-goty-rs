//! Parses `resources.xml` from a pak and decodes every image resource,
//! reporting any that fail.
//!
//!     cargo run -p sexy --release --example check_resources -- <main.pak>

use std::path::PathBuf;
use std::sync::Arc;

use sexy::files::GameFiles;
use sexy::image::load_image_resource;
use sexy::resources::Manifest;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(std::env::args_os().nth(1).ok_or("usage: check_resources <main.pak>")?);
    let pak = Arc::new(pak::Pak::open(&path)?);
    let files = GameFiles { pak: pak.clone(), root: path.parent().unwrap().to_path_buf() };
    let xml = pak.get("properties/resources.xml").ok_or("no properties/resources.xml")?;
    let manifest = Manifest::parse(std::str::from_utf8(xml)?)?;

    let (mut ok, mut failed) = (0, 0);
    for group in manifest.group_names() {
        for res in manifest.group(group).unwrap() {
            let Some(info) = res.image() else { continue };
            match load_image_resource(&files, &res.path, info) {
                Ok(_) => ok += 1,
                Err(e) => {
                    failed += 1;
                    println!("{group}/{}: {e}", res.id);
                }
            }
        }
    }
    println!("{ok} images decoded, {failed} failed");
    Ok(())
}
