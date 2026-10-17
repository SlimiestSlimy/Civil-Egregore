//! The bitmaps a diagnostic looks at by name: every adversarial worst bitmap
//! and saved bitmap, and the PBM image named in `TESSERA_DIAGNOSE`, if any.

use super::adversarial::worst;
use bitmap::Bitmap;
use std::path::Path;

/// The environment variable naming one more PBM image to look at.
pub const EXTRA_IMAGE_VARIABLE: &str = "TESSERA_DIAGNOSE";

/// Every adversarial worst bitmap and saved bitmap, then `TESSERA_DIAGNOSE`'s
/// image, each named.
pub fn looked_at() -> Vec<(String, Bitmap)> {
    let mut bitmaps = worst::every_worst();
    bitmaps.extend(worst::every_saved());
    if let Ok(path) = std::env::var(EXTRA_IMAGE_VARIABLE) {
        let image_stem = Path::new(&path).file_stem().and_then(|os_stem| os_stem.to_str()).unwrap_or("extra").to_string();
        let bitmap = worst::read_from(Path::new(&path)).unwrap_or_else(|| panic!("{path} is not a 256x256 PBM image"));
        bitmaps.push((image_stem, bitmap));
    }
    bitmaps
}
