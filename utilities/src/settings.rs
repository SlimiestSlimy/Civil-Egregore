//! The settings of this machine, kept from one run to the next: one CSV
//! file in Civil Egregore's own folder, which also holds the worlds
//! (`docs/utilities.md`, "Settings").

use std::env::var_os;
use std::fs::{create_dir_all, read_to_string, write};
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// The folder's name, under the system's place for what programs keep.
pub const FOLDER: &str = "Civil Egregore";
/// The file's name, in the folder.
pub const FILE: &str = "settings.csv";
/// The file's columns.
const COLUMNS: [&str; 2] = ["setting", "value"];
/// The default settings: every setting there is, and what it is
/// unless changed.
const DEFAULTS: &str = include_str!("../default_settings.csv");
/// The setting naming the folder worlds are kept in: a path, taken
/// from Civil Egregore's folder unless it is a whole one.
pub const WORLDS: &str = "worlds";
/// Whether the machine's file is left alone: the settings are the
/// default ones.
const FORCE_DEFAULTS: bool = cfg!(feature = "force_default_settings");

/// The system's place for what a user's programs keep: where there is
/// none to be found, the folder the program runs in.
fn kept_by_programs() -> PathBuf {
    let named = |variable: &str| var_os(variable).filter(|path| !path.is_empty()).map(PathBuf::from);
    let found = if cfg!(windows) {
        named("LOCALAPPDATA").or_else(|| named("APPDATA"))
    } else {
        named("XDG_DATA_HOME").or_else(|| named("HOME").map(|home| home.join(".local").join("share")))
    };
    found.unwrap_or_else(|| PathBuf::from("."))
}

/// Civil Egregore's folder on this machine.
pub fn folder() -> PathBuf {
    kept_by_programs().join(FOLDER)
}

/// The settings' file.
pub fn file() -> PathBuf {
    folder().join(FILE)
}

/// The folder worlds are kept in, as the settings have it: `worlds`
/// in Civil Egregore's folder, unless set.
pub fn worlds() -> PathBuf {
    folder().join(Settings::read().get(WORLDS).unwrap_or(WORLDS))
}

/// Where the world named `named` is kept: a plain name is a folder of
/// the worlds' folder, anything more a path as it stands.
pub fn world(named: &str) -> PathBuf {
    world_in(&worlds(), named)
}

/// Where the world named `named` is kept, the worlds' folder being
/// `worlds`: a plain name as a folder may have it ([`world_name`]).
pub fn world_in(worlds: &Path, named: &str) -> PathBuf {
    let path = Path::new(named);
    let plain = path.components().count() == 1 && path.is_relative() && named != "." && named != "..";
    match world_name(named).filter(|_| plain) {
        Some(name) => worlds.join(name),
        None => path.to_path_buf(),
    }
}

/// `given` as a world's name, which is its folder's: only what a
/// folder may be named on Windows and on Linux alike -- letters,
/// digits, spaces, `-`, `_` and `.`, no space or `.` at either end,
/// and not a name Windows keeps for a device (`CON`, `COM1`, ...),
/// which is given a `_` before it. `None` if nothing is left.
pub fn world_name(given: &str) -> Option<String> {
    let kept: String = given.chars().filter(|&letter| letter.is_alphanumeric() || " -_.".contains(letter)).collect();
    let name = kept.trim_matches([' ', '.']);
    let stem = name.split('.').next().unwrap_or(name).trim_end().to_ascii_uppercase();
    let numbered = |device: &str| stem.strip_prefix(device).is_some_and(|number| matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"));
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") || numbered("COM") || numbered("LPT");
    (!name.is_empty()).then(|| if device { format!("_{name}") } else { name.to_string() })
}

/// Settings: each one's name and its value, as a file has them, in
/// its order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// A name and a value a line.
    lines: Vec<(String, String)>,
}

impl Settings {
    /// The default settings: every setting there is.
    pub fn defaults() -> Self {
        Self::of(DEFAULTS)
    }

    /// The settings of this machine: its file's ([`file`]), given a
    /// copy of the default settings first if it has none, and the
    /// default ones for what its file lacks.
    pub fn read() -> Self {
        if FORCE_DEFAULTS { Self::defaults() } else { Self::read_or_start(&file()) }
    }

    /// The settings of the file at `path`, as [`Self::read`] has the
    /// machine's: a file that is there is never written over, and what
    /// it has stands before the default settings.
    pub fn read_or_start(path: &Path) -> Self {
        let mut settings = Self::defaults();
        if !path.exists() {
            // Not copied, the settings are the default ones all the same.
            _ = settings.write_to(path);
        }
        for (name, value) in Self::read_from(path).lines {
            settings.set(&name, Some(value));
        }
        settings
    }

    /// The settings in the file at `path`: none, if it cannot be read.
    pub fn read_from(path: &Path) -> Self {
        Self::of(&read_to_string(path).unwrap_or_default())
    }

    /// The settings `text` has, a row each after the one naming the
    /// columns: a row that is not a name and a value is passed over.
    fn of(text: &str) -> Self {
        let named = |row: Vec<String>| <[String; 2]>::try_from(row).ok().map(|[name, value]| (name.trim().to_string(), value.trim().to_string()));
        Self { lines: crate::csv::rows_named(text).into_iter().filter_map(named).collect() }
    }

    /// The value of the setting named `name`, if it has a line.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.lines.iter().find(|line| line.0 == name).map(|line| line.1.as_str())
    }

    /// The value of the setting named `name`, as a `T`, if it has a
    /// line and is one.
    pub fn number<T: FromStr>(&self, name: &str) -> Option<T> {
        self.get(name)?.parse().ok()
    }

    /// Makes `value` the setting named `name`'s -- or, with none, takes
    /// its line out.
    pub fn set(&mut self, name: &str, value: Option<String>) {
        let at = self.lines.iter().position(|line| line.0 == name);
        match (at, value) {
            (Some(at), Some(value)) => self.lines[at].1 = value,
            (Some(at), None) => drop(self.lines.remove(at)),
            (None, Some(value)) => self.lines.push((name.to_string(), value)),
            (None, None) => {}
        }
    }

    /// Writes the settings to the machine's file, its folder made if it
    /// is not there -- unless the machine's file is left alone.
    pub fn write(&self) -> io::Result<()> {
        if FORCE_DEFAULTS {
            return Ok(());
        }
        self.write_to(&file())
    }

    /// Writes the settings to the file at `path`, its folder made if it
    /// is not there.
    pub fn write_to(&self, path: &Path) -> io::Result<()> {
        if let Some(folder) = path.parent() {
            create_dir_all(folder)?;
        }
        write(path, crate::csv::row(&COLUMNS) + &self.lines.iter().map(|(name, value)| crate::csv::row(&[name, value])).collect::<String>())
    }
}
