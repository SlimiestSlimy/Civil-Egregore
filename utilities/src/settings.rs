//! The settings of this machine, kept from one run to the next: one
//! file, in one folder of TileSim's own under the place the system
//! gives a user's programs for what they keep.
//!
//! | system | the folder |
//! |---|---|
//! | Linux and the like | `$XDG_DATA_HOME/tilesim`, or `~/.local/share/tilesim` |
//! | Windows | `%LOCALAPPDATA%\tilesim`, or `%APPDATA%\tilesim` |
//!
//! The file ([`FILE`]) is text, a line a setting: its name, ` = `, its
//! value. The default settings (`default_settings.txt`, at the crate's root)
//! are such a file, with every setting there is, built into the
//! program: a machine with no file of its own is given a copy of it
//! the first time the settings are read, and a setting the machine's
//! file lacks is as the default settings have it. Whatever has
//! settings shares the one file: each reads the names it knows and,
//! writing, leaves the others' lines as they are.
//!
//! Built with the feature `default_settings`, the machine's file is
//! neither read nor written: the settings are the default ones.
//!
//! The folder also holds the worlds, in a folder of their own
//! ([`worlds`]), unless the setting [`WORLDS`] names another.

use std::env::var_os;
use std::fs::{create_dir_all, read_to_string, write};
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// The folder's name, under the system's place for what programs keep.
pub const FOLDER: &str = "tilesim";
/// The file's name, in the folder.
pub const FILE: &str = "settings.txt";
/// What stands between a setting's name and its value.
const IS: &str = " = ";
/// The default settings: every setting there is, and what it is
/// unless changed.
const DEFAULTS: &str = include_str!("../default_settings.txt");
/// The setting naming the folder worlds are kept in: a path, taken
/// from TileSim's folder unless it is a whole one.
pub const WORLDS: &str = "worlds";
/// Whether the machine's file is left alone: the settings are the
/// default ones.
const DEFAULTS_ONLY: bool = cfg!(feature = "default_settings");

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

/// TileSim's folder on this machine.
pub fn folder() -> PathBuf {
    kept_by_programs().join(FOLDER)
}

/// The settings' file.
pub fn file() -> PathBuf {
    folder().join(FILE)
}

/// The folder worlds are kept in, as the settings have it: `worlds`
/// in TileSim's folder, unless set.
pub fn worlds() -> PathBuf {
    folder().join(Settings::read().get(WORLDS).unwrap_or(WORLDS))
}

/// Where the world named `named` is kept: a plain name is a folder of
/// the worlds' folder, anything more a path as it stands.
pub fn world(named: &str) -> PathBuf {
    world_in(&worlds(), named)
}

/// Where the world named `named` is kept, the worlds' folder being
/// `worlds`.
pub fn world_in(worlds: &Path, named: &str) -> PathBuf {
    let path = Path::new(named);
    if path.components().count() == 1 && path.is_relative() && named != "." && named != ".." { worlds.join(path) } else { path.to_path_buf() }
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
        if DEFAULTS_ONLY { Self::defaults() } else { Self::read_or_start(&file()) }
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

    /// The settings `text` has, a line each: a line that is not a name
    /// and a value is passed over.
    fn of(text: &str) -> Self {
        Self { lines: text.lines().filter_map(|line| line.split_once(IS)).map(|(name, value)| (name.trim().to_string(), value.trim().to_string())).collect() }
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
        if DEFAULTS_ONLY {
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
        write(path, self.lines.iter().map(|(name, value)| format!("{name}{IS}{value}\n")).collect::<String>())
    }
}
