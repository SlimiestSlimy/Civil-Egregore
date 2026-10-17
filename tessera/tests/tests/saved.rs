//! The saved adversarial bitmaps: fixed hard cases, kept as plain PBM
//! images in `tests/saved/`, that every tier checks.

use bitmap::{Bitmap, HEIGHT, WIDTH};
use std::fs;
use std::path::{Path, PathBuf};

/// A plain PBM's first word.
const PBM_MAGIC: &str = "P1";

/// Every saved bitmap, each named by its file's stem, in name order.
pub fn every_saved() -> Vec<(String, Bitmap)> {
    let saved_folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/saved");
    let mut paths: Vec<PathBuf> = fs::read_dir(saved_folder).expect("the saved bitmaps' folder").filter_map(|folder_entry| Some(folder_entry.ok()?.path())).collect();
    paths.retain(|path| path.extension().is_some_and(|file_extension| file_extension == "pbm"));
    paths.sort();
    paths.into_iter().map(|path| (path.file_stem().expect("a file").to_string_lossy().into_owned(), read_from(&path).unwrap_or_else(|| panic!("{} is no 256x256 plain PBM", path.display())))).collect()
}

/// A 256x256 plain PBM image.
fn read_from(path: &Path) -> Option<Bitmap> {
    let pbm_text = fs::read_to_string(path).ok()?;
    let mut words = pbm_text.lines().filter(|line| !line.starts_with('#')).flat_map(str::split_whitespace);
    if words.next()? != PBM_MAGIC || words.next()?.parse::<usize>().ok()? != WIDTH || words.next()?.parse::<usize>().ok()? != HEIGHT {
        return None;
    }
    let cells: Vec<bool> = words.flat_map(str::chars).map(|cell_character| cell_character == '1').collect();
    if cells.len() != WIDTH * HEIGHT {
        return None;
    }
    let mut bitmap = Bitmap::new();
    for (reading_index, &value) in cells.iter().enumerate() {
        if value {
            bitmap.set((reading_index % WIDTH) as u8, (reading_index / WIDTH) as u8);
        }
    }
    Some(bitmap)
}
