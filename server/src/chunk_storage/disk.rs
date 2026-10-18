//! The world on disk: a folder -- its world file, its hot file, and two
//! files a superchunk, its image and its state (`docs/chunk_storage.md`,
//! "On disk").

mod world_files;
pub use world_files::{HotSuperchunks, WorldInfo};

use crate::chunk_storage::superchunk_image::SuperchunkImage;
use utilities::coordinates::SuperchunkIndex;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The world's file, in its folder.
pub const WORLD_FILE: &str = "world.csv";
/// The hot superchunks' file, in the world's folder.
pub const HOT_FILE: &str = "hot.csv";
/// The superchunks' folder, in the world's folder.
const SUPERCHUNKS: &str = "superchunks";

/// Why a world was not written, or not read.
#[derive(Debug)]
pub enum DiskError {
    /// The system refused: the file, and what it said.
    IoFailed(PathBuf, io::Error),
    /// A file is not what a save writes: the file, and what is wrong.
    InvalidFile(PathBuf, String),
}

impl fmt::Display for DiskError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::IoFailed(path, io_error) => write!(formatter, "{}: {io_error}", path.display()),
            Self::InvalidFile(path, what_is_wrong) => write!(formatter, "{}: {what_is_wrong}", path.display()),
        }
    }
}

impl std::error::Error for DiskError {}

/// A superchunk's file in `folder`: its index in hexadecimal,
/// and `file_extension`.
fn superchunk_file(folder: &Path, superchunk: SuperchunkIndex, file_extension: &str) -> PathBuf {
    folder.join(SUPERCHUNKS).join(format!("{:011x}.{file_extension}", superchunk.0))
}

/// Writes what the world is as `folder`'s world file, the folder
/// made if not there: how many bytes.
pub fn write_world(folder: &Path, world_info: &WorldInfo) -> Result<u64, DiskError> {
    make_folder(&folder.join(SUPERCHUNKS))?;
    write(&folder.join(WORLD_FILE), world_info.to_text().as_bytes())
}

/// What the world in `folder` is: its world file read.
pub fn read_world(folder: &Path) -> Result<WorldInfo, DiskError> {
    let path = folder.join(WORLD_FILE);
    let world_text = String::from_utf8(read_bytes(&path)?).map_err(|_| DiskError::InvalidFile(path.clone(), "not text".to_string()))?;
    WorldInfo::from_text(&world_text).map_err(|what_is_wrong| DiskError::InvalidFile(path, what_is_wrong))
}

/// Writes `hot` as `folder`'s hot file, the folder made if not
/// there: how many bytes.
pub fn write_hot(folder: &Path, hot: &HotSuperchunks) -> Result<u64, DiskError> {
    make_folder(folder)?;
    write(&folder.join(HOT_FILE), hot.to_text().as_bytes())
}

/// Which superchunks were hot in `folder`'s world: its hot file read.
pub fn read_hot(folder: &Path) -> Result<HotSuperchunks, DiskError> {
    let path = folder.join(HOT_FILE);
    let hot_text = String::from_utf8(read_bytes(&path)?).map_err(|_| DiskError::InvalidFile(path.clone(), "not text".to_string()))?;
    HotSuperchunks::from_text(&hot_text).map_err(|what_is_wrong| DiskError::InvalidFile(path, what_is_wrong))
}

/// Writes `image` as `superchunk`'s in `folder`: how many bytes.
pub fn write_image(folder: &Path, superchunk: SuperchunkIndex, image: &SuperchunkImage) -> Result<u64, DiskError> {
    make_folder(&folder.join(SUPERCHUNKS))?;
    write_words(&superchunk_file(folder, superchunk, "image"), image.words())
}

/// Copies the image of `superchunk` from its file of the folder `from_folder`
/// to its file of `folder`: how many bytes. The same file -- `folder`
/// is `from_folder` -- is left as it is.
pub fn copy_image(from_folder: &Path, folder: &Path, superchunk: SuperchunkIndex) -> Result<u64, DiskError> {
    let (source_path, path) = (superchunk_file(from_folder, superchunk, "image"), superchunk_file(folder, superchunk, "image"));
    let io_failed_at = |path: &Path| { let path = path.to_path_buf(); move |io_error| DiskError::IoFailed(path, io_error) };
    let same_folder = matches!((from_folder.canonicalize(), folder.canonicalize()), (Ok(canonical_from), Ok(folder)) if canonical_from == folder);
    if same_folder {
        return Ok(fs::metadata(&source_path).map_err(io_failed_at(&source_path))?.len());
    }
    make_folder(&folder.join(SUPERCHUNKS))?;
    let writing_path = path.with_extension("writing");
    let bytes_copied = fs::copy(&source_path, &writing_path).map_err(io_failed_at(&source_path))?;
    fs::rename(&writing_path, &path).map_err(io_failed_at(&path))?;
    Ok(bytes_copied)
}

/// The image of `superchunk` in `folder`, checked.
pub fn read_image(folder: &Path, superchunk: SuperchunkIndex) -> Result<SuperchunkImage, DiskError> {
    let path = superchunk_file(folder, superchunk, "image");
    SuperchunkImage::from_words(read_words(&path)?.into_boxed_slice()).map_err(|invalid_image| DiskError::InvalidFile(path, invalid_image.0.to_string()))
}

/// Writes `words` as `superchunk`'s state in `folder`: how many
/// bytes.
pub fn write_state(folder: &Path, superchunk: SuperchunkIndex, words: &[u64]) -> Result<u64, DiskError> {
    make_folder(&folder.join(SUPERCHUNKS))?;
    write_words(&superchunk_file(folder, superchunk, "state"), words)
}

/// The state of `superchunk` in `folder`: its words, and its file,
/// to say what is wrong with them.
pub fn read_state(folder: &Path, superchunk: SuperchunkIndex) -> Result<(Vec<u64>, PathBuf), DiskError> {
    let path = superchunk_file(folder, superchunk, "state");
    Ok((read_words(&path)?, path))
}

/// Every superchunk with an image in `folder`, in Morton order.
pub fn saved_superchunks(folder: &Path) -> Result<Vec<SuperchunkIndex>, DiskError> {
    let mut superchunks = images_in(&folder.join(SUPERCHUNKS))?;
    superchunks.sort_unstable();
    Ok(superchunks)
}

/// Makes `folder`, and those above it, if not there.
fn make_folder(folder: &Path) -> Result<(), DiskError> {
    fs::create_dir_all(folder).map_err(|io_error| DiskError::IoFailed(folder.to_path_buf(), io_error))
}

/// Writes `file_bytes` as `path`: how many.
fn write(path: &Path, file_bytes: &[u8]) -> Result<u64, DiskError> {
    let writing_path = path.with_extension("writing");
    fs::write(&writing_path, file_bytes).and_then(|()| fs::rename(&writing_path, path)).map_err(|io_error| DiskError::IoFailed(path.to_path_buf(), io_error))?;
    Ok(file_bytes.len() as u64)
}

/// Writes `words` as `path`: how many bytes.
fn write_words(path: &Path, words: &[u64]) -> Result<u64, DiskError> {
    let word_bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    write(path, &word_bytes)
}

/// The bytes of `path`.
fn read_bytes(path: &Path) -> Result<Vec<u8>, DiskError> {
    fs::read(path).map_err(|io_error| DiskError::IoFailed(path.to_path_buf(), io_error))
}

/// The words of `path`.
fn read_words(path: &Path) -> Result<Vec<u64>, DiskError> {
    let file_bytes = read_bytes(path)?;
    if file_bytes.len() % 8 != 0 {
        return Err(DiskError::InvalidFile(path.to_path_buf(), "not a whole number of words".to_string()));
    }
    Ok(file_bytes.as_chunks::<8>().0.iter().map(|&word| u64::from_le_bytes(word)).collect())
}

/// Every superchunk with an image in `folder`.
fn images_in(folder: &Path) -> Result<Vec<SuperchunkIndex>, DiskError> {
    let io_failed = |io_error| DiskError::IoFailed(folder.to_path_buf(), io_error);
    let mut superchunks = Vec::new();
    for folder_entry in fs::read_dir(folder).map_err(io_failed)? {
        let path = folder_entry.map_err(io_failed)?.path();
        if path.extension().is_some_and(|file_extension| file_extension == "image") {
            let stem_text = path.file_stem().and_then(|os_stem| os_stem.to_str()).unwrap_or_default();
            let superchunk_number = u64::from_str_radix(stem_text, 16).map_err(|_| DiskError::InvalidFile(path.clone(), "not named by a superchunk index".to_string()))?;
            superchunks.push(SuperchunkIndex(superchunk_number));
        }
    }
    Ok(superchunks)
}
