//! The settings a person has changed on this machine, kept from one
//! run to the next: one file, in one folder of TileSim's own under the
//! place the system gives a user's programs for what they keep.
//!
//! | system | the folder |
//! |---|---|
//! | Linux and the like | `$XDG_DATA_HOME/tilesim`, or `~/.local/share/tilesim` |
//! | Windows | `%LOCALAPPDATA%\tilesim`, or `%APPDATA%\tilesim` |
//! | macOS | `~/Library/Application Support/tilesim` |
//!
//! The file ([`FILE`]) is text, a line a setting: its name, ` = `, its
//! value. It holds overrides only -- a setting left as the code has it
//! has no line -- so a default changed in the code reaches everyone
//! who never touched it. Whatever has settings shares the one file:
//! each reads the names it knows and, writing, leaves the others'
//! lines as they are.

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

/// The system's place for what a user's programs keep: where there is
/// none to be found, the folder the program runs in.
fn kept_by_programs() -> PathBuf {
    let named = |variable: &str| var_os(variable).filter(|path| !path.is_empty()).map(PathBuf::from);
    let found = if cfg!(windows) {
        named("LOCALAPPDATA").or_else(|| named("APPDATA"))
    } else if cfg!(target_os = "macos") {
        named("HOME").map(|home| home.join("Library").join("Application Support"))
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

/// The settings changed: each one's name and its value, as the file
/// has them, in its order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// A name and a value a line.
    lines: Vec<(String, String)>,
}

impl Settings {
    /// The settings in the machine's file ([`file`]): none, if there is
    /// no file.
    pub fn read() -> Self {
        Self::read_from(&file())
    }

    /// The settings in the file at `path`: none, if it cannot be read.
    /// A line that is not a name and a value is passed over.
    pub fn read_from(path: &Path) -> Self {
        let text = read_to_string(path).unwrap_or_default();
        Self { lines: text.lines().filter_map(|line| line.split_once(IS)).map(|(name, value)| (name.trim().to_string(), value.trim().to_string())).collect() }
    }

    /// The value of the setting named `name`, if it was changed.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.lines.iter().find(|line| line.0 == name).map(|line| line.1.as_str())
    }

    /// The value of the setting named `name`, as a `T`, if it was
    /// changed and is one.
    pub fn number<T: FromStr>(&self, name: &str) -> Option<T> {
        self.get(name)?.parse().ok()
    }

    /// Makes `value` the setting named `name`'s -- or, with none, the
    /// setting is as the code has it again, and has no line.
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
    /// is not there.
    pub fn write(&self) -> io::Result<()> {
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
