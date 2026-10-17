//! Adversarial bitmaps kept as plain PBM images: the worst found so
//! far, replaced when beaten, and saved ones, never replaced
//! (`docs/lab.md`, "`diagnostics/adversarial/`").

use bitmap::{Bitmap, HEIGHT, WIDTH};
use std::fs;
use std::path::{Path, PathBuf};

/// Where the saved bitmaps are kept, under the crate's folder.
const SAVED_RELATIVE_PATH: &str = "external_benchmarks/adversarial/saved";
/// A plain PBM's first word.
const PBM_MAGIC: &str = "P1";

/// The worst bitmaps' folder.
fn worst_folder() -> PathBuf {
    crate::transient_data::worst()
}

/// The saved bitmaps' folder.
fn saved_folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SAVED_RELATIVE_PATH)
}

/// The worst bitmap named `bitmap_name`'s file.
pub fn path(bitmap_name: &str) -> PathBuf {
    worst_folder().join(format!("{bitmap_name}.pbm"))
}

/// The saved bitmap named `bitmap_name`'s file.
pub fn saved_path(bitmap_name: &str) -> PathBuf {
    saved_folder().join(format!("{bitmap_name}.pbm"))
}

/// Every PBM image in `folder` that reads as a bitmap, each named by its
/// file's stem, in name order.
fn every_in(folder: &Path) -> Vec<(String, Bitmap)> {
    let mut paths: Vec<PathBuf> = fs::read_dir(folder)
        .map(|folder_entries| folder_entries.filter_map(|folder_entry| Some(folder_entry.ok()?.path())).collect())
        .unwrap_or_default();
    paths.retain(|path| path.extension().is_some_and(|file_extension| file_extension == "pbm"));
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| Some((path.file_stem()?.to_str()?.to_string(), read_from(&path)?)))
        .collect()
}

/// Every worst bitmap there is, each named, in name order: the worst
/// found so far for each search.
pub fn every_worst() -> Vec<(String, Bitmap)> {
    every_in(&worst_folder())
}

/// Every saved bitmap, each named, in name order: the fixed hard cases
/// the fine tests check and the optimization benchmarks encode.
pub fn every_saved() -> Vec<(String, Bitmap)> {
    every_in(&saved_folder())
}

/// The worst bitmap named `bitmap_name`, if there is one and it reads as a 256x256
/// plain PBM.
pub fn read_worst(bitmap_name: &str) -> Option<Bitmap> {
    read_from(&path(bitmap_name))
}

/// A 256x256 plain PBM image, from anywhere.
pub fn read_from(path: &Path) -> Option<Bitmap> {
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

/// The comment lines of the PBM image at `path`, without their `#`.
pub fn notes_from(path: &Path) -> Vec<String> {
    let pbm_text = fs::read_to_string(path).unwrap_or_default();
    pbm_text.lines().filter_map(|line| line.strip_prefix('#')).map(|comment_line| comment_line.trim().to_string()).collect()
}

/// `bitmap` as a plain PBM image at `path`, each of `comment_notes` a comment
/// line, replacing any file there was.
fn write_to(path: &Path, bitmap: &Bitmap, comment_notes: &[String]) {
    let mut pbm_text = format!("{PBM_MAGIC}\n");
    for comment_note in comment_notes {
        pbm_text.push_str(&format!("# {comment_note}\n"));
    }
    pbm_text.push_str(&format!("{WIDTH} {HEIGHT}\n"));
    for y in 0..=u8::MAX {
        let row_text: String = (0..=u8::MAX).map(|x| if bitmap.get(x, y) { '1' } else { '0' }).collect();
        pbm_text.push_str(&row_text);
        pbm_text.push('\n');
    }
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, pbm_text).unwrap();
}

/// Keeps `bitmap` as the worst bitmap `bitmap_name`, `comment_note` in the file's
/// comment line, replacing any there was.
pub fn write(bitmap_name: &str, bitmap: &Bitmap, comment_note: &str) {
    write_to(&path(bitmap_name), bitmap, &[comment_note.to_string()]);
}

/// Saves `bitmap` as the bitmap `bitmap_name`, `comment_notes` in its comment lines,
/// replacing any saved bitmap of that name.
pub fn save(bitmap_name: &str, bitmap: &Bitmap, comment_notes: &[String]) {
    write_to(&saved_path(bitmap_name), bitmap, comment_notes);
}
