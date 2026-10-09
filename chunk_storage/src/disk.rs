//! The world on disk: a folder. Its world file (`world.csv`) says what the world
//! is; its hot file (`hot.csv`) which superchunks were hot, which of them
//! cooling, and which warming -- both CSV (`utilities::csv`); `superchunks/` holds two files a superchunk, each named
//! by its superchunk index -- 44 bits, in hexadecimal: `.image`, its cells,
//! the image as the cold pool holds it, and `.state`, words that are whoever
//! ticks the world's to make sense of -- its random numbers, its
//! entities. Design: `../docs/chunk_storage.md`, "On disk".

use crate::layer_codec::LayerType;
use crate::superchunk_image::SuperchunkImage;
use coordinates::SuperchunkIndex;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use utilities::csv;

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
    Io(PathBuf, io::Error),
    /// A file is not what a save writes: the file, and what is wrong.
    Invalid(PathBuf, String),
}

impl fmt::Display for DiskError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Io(path, error) => write!(formatter, "{}: {error}", path.display()),
            Self::Invalid(path, what) => write!(formatter, "{}: {what}", path.display()),
        }
    }
}

impl std::error::Error for DiskError {}

/// A superchunk's file in `folder`: its index in hexadecimal,
/// and `extension`.
fn superchunk_file(folder: &Path, superchunk: SuperchunkIndex, extension: &str) -> PathBuf {
    folder.join(SUPERCHUNKS).join(format!("{:011x}.{extension}", superchunk.0))
}

/// Writes what the world is as `folder`'s world file, the folder
/// made if not there: how many bytes.
pub fn write_world(folder: &Path, info: &WorldInfo) -> Result<u64, DiskError> {
    make_folder(&folder.join(SUPERCHUNKS))?;
    write(&folder.join(WORLD_FILE), info.to_text().as_bytes())
}

/// What the world in `folder` is: its world file read.
pub fn read_world(folder: &Path) -> Result<WorldInfo, DiskError> {
    let path = folder.join(WORLD_FILE);
    let text = String::from_utf8(read(&path)?).map_err(|_| DiskError::Invalid(path.clone(), "not text".to_string()))?;
    WorldInfo::from_text(&text).map_err(|what| DiskError::Invalid(path, what))
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
    let text = String::from_utf8(read(&path)?).map_err(|_| DiskError::Invalid(path.clone(), "not text".to_string()))?;
    HotSuperchunks::from_text(&text).map_err(|what| DiskError::Invalid(path, what))
}

/// Writes `image` as `superchunk`'s in `folder`: how many bytes.
pub fn write_image(folder: &Path, superchunk: SuperchunkIndex, image: &SuperchunkImage) -> Result<u64, DiskError> {
    make_folder(&folder.join(SUPERCHUNKS))?;
    write_words(&superchunk_file(folder, superchunk, "image"), image.words())
}

/// The image of `superchunk` in `folder`, checked.
pub fn read_image(folder: &Path, superchunk: SuperchunkIndex) -> Result<SuperchunkImage, DiskError> {
    let path = superchunk_file(folder, superchunk, "image");
    SuperchunkImage::from_words(read_words(&path)?.into_boxed_slice()).map_err(|invalid| DiskError::Invalid(path, invalid.0.to_string()))
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
    fs::create_dir_all(folder).map_err(|error| DiskError::Io(folder.to_path_buf(), error))
}

/// Writes `bytes` as `path`: how many.
fn write(path: &Path, bytes: &[u8]) -> Result<u64, DiskError> {
    let beside = path.with_extension("writing");
    fs::write(&beside, bytes).and_then(|()| fs::rename(&beside, path)).map_err(|error| DiskError::Io(path.to_path_buf(), error))?;
    Ok(bytes.len() as u64)
}

/// Writes `words` as `path`: how many bytes.
fn write_words(path: &Path, words: &[u64]) -> Result<u64, DiskError> {
    let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    write(path, &bytes)
}

/// The bytes of `path`.
fn read(path: &Path) -> Result<Vec<u8>, DiskError> {
    fs::read(path).map_err(|error| DiskError::Io(path.to_path_buf(), error))
}

/// The words of `path`.
fn read_words(path: &Path) -> Result<Vec<u64>, DiskError> {
    let bytes = read(path)?;
    if bytes.len() % 8 != 0 {
        return Err(DiskError::Invalid(path.to_path_buf(), "not a whole number of words".to_string()));
    }
    Ok(bytes.as_chunks::<8>().0.iter().map(|&word| u64::from_le_bytes(word)).collect())
}

/// Every superchunk with an image in `folder`.
fn images_in(folder: &Path) -> Result<Vec<SuperchunkIndex>, DiskError> {
    let io = |error| DiskError::Io(folder.to_path_buf(), error);
    let mut superchunks = Vec::new();
    for entry in fs::read_dir(folder).map_err(io)? {
        let path = entry.map_err(io)?.path();
        if path.extension().is_some_and(|extension| extension == "image") {
            let name = path.file_stem().and_then(|name| name.to_str()).unwrap_or_default();
            let index = u64::from_str_radix(name, 16).map_err(|_| DiskError::Invalid(path.clone(), "not named by a superchunk index".to_string()))?;
            superchunks.push(SuperchunkIndex(index));
        }
    }
    Ok(superchunks)
}

/// The world file's columns.
const WORLD_COLUMNS: [&str; 2] = ["world", "is"];
/// The world file's format, and the number of the one written.
const FORMAT: (&str, &str) = ("format", "2");

/// The name of the kind of entity a world is hot about, in its file.
const HOT_ENTITY: &str = "hot entity";

/// What starts the name of a number the world is generated by, in its file.
const GENERATION: &str = "generation ";

/// What a world is. Its name is its folder's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldInfo {
    /// Its seed: what everything generated and drawn comes from.
    pub seed: u64,
    /// The tick it is at: the next to run.
    pub tick: u64,
    /// Its layer types: made hot on every chunk when it is loaded.
    pub layers: Vec<LayerType>,
    /// Superchunks along a side of it, if it has a size: a square
    /// about its origin, nothing ever made outside it.
    pub side: Option<u32>,
    /// Whether every superchunk of it is hot throughout, whatever its
    /// entities do: only one with a size can be.
    pub forced: bool,
    /// The kind of entity it is hot about, if it is not forced hot,
    /// as a number: whoever runs it knows the kinds. None if its file
    /// says none: its runner's own then.
    pub hot_entity: Option<u64>,
    /// How it is generated: numbers, each with its name, kept for
    /// whoever generates it, who knows what they mean.
    pub generation: Vec<(String, u64)>,
}

impl WorldInfo {
    /// As the world's file holds it: a row a thing the world is, its
    /// name and what it is -- its seed in hexadecimal, its layer types
    /// with spaces between.
    fn to_text(&self) -> String {
        let layers: Vec<String> = self.layers.iter().map(|layer| layer.0.to_string()).collect();
        let mut rows = vec![
            (FORMAT.0.to_string(), FORMAT.1.to_string()),
            ("seed".to_string(), utilities::seed::hex(self.seed)),
            ("tick".to_string(), self.tick.to_string()),
            ("layers".to_string(), layers.join(" ")),
        ];
        rows.extend(self.side.map(|side| ("side".to_string(), side.to_string())));
        rows.extend(self.forced.then(|| ("forced".to_string(), "1".to_string())));
        rows.extend(self.hot_entity.map(|kind| (HOT_ENTITY.to_string(), kind.to_string())));
        rows.extend(self.generation.iter().map(|(name, value)| (format!("{GENERATION}{name}"), value.to_string())));
        csv::row(&WORLD_COLUMNS) + &rows.iter().map(|(name, is)| csv::row(&[name, is])).collect::<String>()
    }

    /// From the world's file's text, or what is wrong with it.
    fn from_text(text: &str) -> Result<Self, String> {
        let mut rows = csv::rows(text).into_iter();
        if rows.next().is_none_or(|columns| columns != WORLD_COLUMNS) || rows.next().is_none_or(|format| format != [FORMAT.0, FORMAT.1]) {
            return Err(format!("does not start with `{}` and `{},{}`", WORLD_COLUMNS.join(","), FORMAT.0, FORMAT.1));
        }
        let (mut seed, mut tick, mut layers, mut side, mut forced, mut hot_entity, mut generation) = (None, None, None, None, false, None, Vec::new());
        for row in rows {
            let [key, value] = &row[..] else {
                return Err(format!("`{}` is not a name and what it is", row.join(",")));
            };
            let (key, value) = (key.trim(), value.trim());
            let number = || value.parse::<u64>().map_err(|_| format!("`{value}` is not a number"));
            match key {
                "seed" => seed = Some(utilities::seed::of_hex(value).ok_or_else(|| format!("`{value}` is not a seed"))?),
                "tick" => tick = Some(number()?),
                "layers" => layers = Some(value.split_whitespace().map(|layer| layer.parse().map(LayerType).map_err(|_| format!("`{layer}` is not a layer type"))).collect::<Result<Vec<_>, _>>()?),
                "side" => side = Some(value.parse().map_err(|_| format!("`{value}` is not a side"))?),
                "forced" => forced = number()? != 0,
                HOT_ENTITY => hot_entity = Some(number()?),
                _ if key.starts_with(GENERATION) => generation.push((key[GENERATION.len()..].to_string(), number()?)),
                // A name of another format: passed over.
                _ => {}
            }
        }
        if forced && side.is_none() {
            return Err("forced hot, with no side to be hot to".to_string());
        }
        Ok(Self { seed: seed.ok_or("no seed")?, tick: tick.ok_or("no tick")?, layers: layers.ok_or("no layers")?, side, forced, hot_entity, generation })
    }
}

/// The hot file's columns.
const HOT_COLUMNS: [&str; 3] = ["superchunk", "is", "until"];
/// What a superchunk hot and not cooling is, in the hot file.
const HOT: &str = "hot";
/// What a hot superchunk cooling is, in the hot file.
const COOLING: &str = "cooling";
/// What a superchunk warming is, in the hot file.
const WARMING: &str = "warming";

/// Which superchunks a world had hot when saved: made hot again as it is
/// loaded, before it ticks. A row a superchunk: its index in
/// hexadecimal, then `hot`, or `cooling` and the tick it goes cold at
/// if hot and cooling, or `warming` and the tick it turns hot at.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HotSuperchunks {
    /// The hot superchunks, sorted.
    pub hot: Vec<SuperchunkIndex>,
    /// The hot superchunks cooling, sorted, each with the tick it goes
    /// cold at.
    pub cooling: Vec<(SuperchunkIndex, u64)>,
    /// The superchunks warming, sorted, each with the tick it turns hot
    /// at.
    pub warming: Vec<(SuperchunkIndex, u64)>,
}

impl HotSuperchunks {
    /// As the hot file holds it.
    fn to_text(&self) -> String {
        let cools = |superchunk: &SuperchunkIndex| self.cooling.binary_search_by_key(superchunk, |&(cooling, _)| cooling).ok().map(|at| self.cooling[at].1);
        let index = |superchunk: &SuperchunkIndex| format!("{:011x}", superchunk.0);
        let hot = self.hot.iter().map(|superchunk| match cools(superchunk) {
            Some(tick) => csv::row(&[index(superchunk), COOLING.to_string(), tick.to_string()]),
            None => csv::row(&[index(superchunk), HOT.to_string(), String::new()]),
        });
        let warming = self.warming.iter().map(|(superchunk, tick)| csv::row(&[index(superchunk), WARMING.to_string(), tick.to_string()]));
        std::iter::once(csv::row(&HOT_COLUMNS)).chain(hot).chain(warming).collect()
    }

    /// From the hot file's text, or what is wrong with it.
    fn from_text(text: &str) -> Result<Self, String> {
        let mut rows = csv::rows(text).into_iter();
        if rows.next().is_none_or(|columns| columns != HOT_COLUMNS) {
            return Err(format!("does not start with `{}`", HOT_COLUMNS.join(",")));
        }
        let mut read = Self::default();
        for row in rows {
            let wrong = || format!("`{}` is not a superchunk index, `{HOT}`, `{COOLING}` or `{WARMING}`, and a tick unless hot", row.join(","));
            let [superchunk, is, until] = &row[..] else {
                return Err(wrong());
            };
            let superchunk = SuperchunkIndex(u64::from_str_radix(superchunk.trim(), 16).map_err(|_| wrong())?);
            let tick = || until.trim().parse().map_err(|_| wrong());
            match is.trim() {
                HOT if until.trim().is_empty() => read.hot.push(superchunk),
                COOLING => {
                    read.hot.push(superchunk);
                    read.cooling.push((superchunk, tick()?));
                }
                WARMING => read.warming.push((superchunk, tick()?)),
                _ => return Err(wrong()),
            }
        }
        read.hot.sort_unstable();
        read.cooling.sort_unstable();
        read.warming.sort_unstable();
        Ok(read)
    }
}
