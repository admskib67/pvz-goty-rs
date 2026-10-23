//! Image loading with SexyFramework's alpha-mask conventions.
//!
//! Many PvZ images are a JPEG plus a separate greyscale mask. Behaviour per
//! ruslan831 decomp, `ImageLib::GetImage` and `ResourceManager::DoLoadImage`,
//! `LoadAlphaImage` and `LoadAlphaGridImage`.

use crate::files::Files;
use crate::resources::ImageInfo;

/// Extensions tried, in order, for a path given without one.
const EXTENSIONS: [&str; 4] = ["tga", "jpg", "png", "gif"];

/// A decoded image as straight-alpha RGBA8 rows.
#[derive(Clone, PartialEq, Eq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for DecodedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DecodedImage({}x{})", self.width, self.height)
    }
}

impl DecodedImage {
    /// Replaces this image's alpha with the mask's blue channel (mask images are greyscale).
    fn apply_alpha(&mut self, mask: &DecodedImage) {
        for (px, m) in self.rgba.as_chunks_mut::<4>().0.iter_mut().zip(mask.rgba.as_chunks::<4>().0) {
            px[3] = m[2];
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("image {0:?} not found")]
    NotFound(String),
    #[error("alpha image {mask:?} is {mask_size:?} but {image:?} needs {expected:?}")]
    SizeMismatch { image: String, mask: String, mask_size: (u32, u32), expected: (u32, u32) },
}

fn decode(bytes: &[u8]) -> Option<DecodedImage> {
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    Some(DecodedImage { width: img.width(), height: img.height(), rgba: img.into_raw() })
}

fn split_ext(path: &str) -> (&str, Option<&str>) {
    let slash = path.rfind(['/', '\\']).map_or(0, |i| i + 1);
    match path[slash..].rfind('.') {
        Some(dot) => (&path[..slash + dot], Some(&path[slash + dot + 1..])),
        None => (path, None),
    }
}

/// Loads an image, then looks for a `_name` or `name_` mask beside it.
/// A mask with no base image becomes a solid image of `alpha_color`.
pub fn load_image(files: &dyn Files, path: &str, look_for_alpha: bool, alpha_color: u32) -> Option<DecodedImage> {
    let (base, ext) = split_ext(path);
    let image = EXTENSIONS
        .iter()
        .filter(|e| ext.is_none_or(|x| x.eq_ignore_ascii_case(e)))
        .find_map(|e| files.read(&format!("{base}.{e}")))
        .and_then(|bytes| decode(&bytes));

    let mask = look_for_alpha.then(|| {
        let slash = path.rfind(['/', '\\']).map_or(0, |i| i + 1);
        let prefixed = format!("{}_{}", &path[..slash], &path[slash..]);
        load_image(files, &prefixed, false, alpha_color)
            .or_else(|| load_image(files, &format!("{path}_"), false, alpha_color))
    });

    match (image, mask.flatten()) {
        (Some(mut image), Some(mask)) => {
            // ImageLib silently ignores a mask of the wrong size.
            if (image.width, image.height) == (mask.width, mask.height) {
                image.apply_alpha(&mask);
            }
            Some(image)
        }
        (image, None) => image,
        (None, Some(mut mask)) => {
            let [_, r, g, b] = alpha_color.to_be_bytes();
            for px in mask.rgba.as_chunks_mut::<4>().0 {
                *px = [r, g, b, px[2]];
            }
            Some(mask)
        }
    }
}

/// Loads an image resource, applying its explicit or per-cel alpha mask.
pub fn load_image_resource(files: &dyn Files, path: &str, info: &ImageInfo) -> Result<DecodedImage, ImageError> {
    // The original computes "look for alpha" from these attributes but then
    // always searches anyway (SexyAppBase::GetImage ignores it); we match that.
    let mut image =
        load_image(files, path, true, info.alpha_color).ok_or_else(|| ImageError::NotFound(path.to_owned()))?;

    if let Some(mask_path) = &info.alpha_image {
        let mask = load_image(files, mask_path, true, 0xFF_FFFF)
            .ok_or_else(|| ImageError::NotFound(mask_path.clone()))?;
        check_size(path, mask_path, &mask, (image.width, image.height))?;
        image.apply_alpha(&mask);
    }

    if let Some(mask_path) = &info.alpha_grid {
        let mask = load_image(files, mask_path, true, 0xFF_FFFF)
            .ok_or_else(|| ImageError::NotFound(mask_path.clone()))?;
        let cel = (image.width / info.cols.max(1), image.height / info.rows.max(1));
        check_size(path, mask_path, &mask, cel)?;
        for row in 0..info.rows {
            for col in 0..info.cols {
                for y in 0..cel.1 {
                    for x in 0..cel.0 {
                        let src = ((y * cel.0 + x) * 4 + 2) as usize;
                        let dst = (((row * cel.1 + y) * image.width + col * cel.0 + x) * 4 + 3) as usize;
                        image.rgba[dst] = mask.rgba[src];
                    }
                }
            }
        }
    }
    Ok(image)
}

fn check_size(path: &str, mask_path: &str, mask: &DecodedImage, expected: (u32, u32)) -> Result<(), ImageError> {
    if (mask.width, mask.height) == expected {
        Ok(())
    } else {
        Err(ImageError::SizeMismatch {
            image: path.to_owned(),
            mask: mask_path.to_owned(),
            mask_size: (mask.width, mask.height),
            expected,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};
    use pak::Pak;

    fn png(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let img = RgbaImage::from_fn(w, h, |x, y| Rgba(f(x, y)));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn pak(files: &[(&str, Vec<u8>)]) -> Pak {
        let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, d)| (*n, d.as_slice())).collect();
        Pak::from_bytes(pak::build(&refs)).unwrap()
    }

    fn alpha(img: &DecodedImage) -> Vec<u8> {
        img.rgba.as_chunks::<4>().0.iter().map(|p| p[3]).collect()
    }

    #[test]
    fn suffix_mask_sets_alpha_from_blue() {
        let pak = pak(&[
            ("images/logo.png", png(2, 1, |_, _| [10, 20, 30, 255])),
            ("images/logo_.png", png(2, 1, |x, _| [0, 0, if x == 0 { 0 } else { 200 }, 255])),
        ]);
        let img = load_image(&pak, "images/logo", true, 0xFF_FFFF).unwrap();
        assert_eq!(alpha(&img), [0, 200]);
        assert_eq!(&img.rgba[..3], &[10, 20, 30]);
    }

    #[test]
    fn prefix_mask_alone_becomes_alpha_color() {
        let pak = pak(&[("data/_Font.png", png(1, 1, |_, _| [0, 0, 128, 255]))]);
        let img = load_image(&pak, "data/Font", true, 0x11_2233).unwrap();
        assert_eq!(img.rgba, [0x11, 0x22, 0x33, 128]);
    }

    #[test]
    fn tries_extensions_in_order() {
        let pak = pak(&[("a.png", png(1, 1, |_, _| [1, 1, 1, 255])), ("a.gif", Vec::new())]);
        // No tga or jpg present, so png wins over the (invalid) gif.
        assert_eq!(load_image(&pak, "a", false, 0xFF_FFFF).unwrap().rgba, [1, 1, 1, 255]);
        assert!(load_image(&pak, "missing", true, 0xFF_FFFF).is_none());
    }

    #[test]
    fn alpha_grid_repeats_mask_per_cel() {
        let pak = pak(&[
            ("pool.png", png(4, 1, |_, _| [9, 9, 9, 255])),
            ("grid.png", png(2, 1, |x, _| [0, 0, x as u8 * 100, 255])),
        ]);
        let info = ImageInfo { cols: 2, alpha_grid: Some("grid".into()), ..Default::default() };
        let img = load_image_resource(&pak, "pool", &info).unwrap();
        assert_eq!(alpha(&img), [0, 100, 0, 100]);
    }

    #[test]
    fn alpha_image_size_mismatch_is_an_error() {
        let pak = pak(&[("a.png", png(2, 2, |_, _| [0; 4])), ("m.png", png(1, 1, |_, _| [0; 4]))]);
        let info = ImageInfo { alpha_image: Some("m".into()), ..Default::default() };
        assert!(matches!(load_image_resource(&pak, "a", &info), Err(ImageError::SizeMismatch { .. })));
    }
}
