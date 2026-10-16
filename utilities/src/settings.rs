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
pub const SETTINGS_FILE_NAME: &str = "settings.csv";
/// The file's columns.
const SETTINGS_COLUMNS: [&str; 2] = ["setting", "value"];
/// The default settings: every setting there is, and what it is
/// unless changed.
const DEFAULT_SETTINGS_TEXT: &str = include_str!("../default_settings.csv");
/// The setting naming the folder worlds are kept in: a path, taken
/// from Civil Egregore's folder unless it is a whole one.
pub const WORLDS: &str = "worlds";
/// Whether the machine's file is left alone: the settings are the
/// default ones.
const FORCE_DEFAULTS: bool = cfg!(feature = "force_default_settings");

/// The system's place for what a user's programs keep: where there is
/// none to be found, the folder the program runs in.
fn kept_by_programs() -> PathBuf {
    let path_from_variable = |variable_name: &str| var_os(variable_name).filter(|path| !path.is_empty()).map(PathBuf::from);
    let found_folder = if cfg!(windows) {
        path_from_variable("LOCALAPPDATA").or_else(|| path_from_variable("APPDATA"))
    } else {
        path_from_variable("XDG_DATA_HOME").or_else(|| path_from_variable("HOME").map(|home_folder| home_folder.join(".local").join("share")))
    };
    found_folder.unwrap_or_else(|| PathBuf::from("."))
}

/// Civil Egregore's folder on this machine.
pub fn folder() -> PathBuf {
    kept_by_programs().join(FOLDER)
}

/// The settings' file.
pub fn settings_file() -> PathBuf {
    folder().join(SETTINGS_FILE_NAME)
}

/// The folder worlds are kept in, as the settings have it: `worlds`
/// in Civil Egregore's folder, unless set.
pub fn worlds() -> PathBuf {
    folder().join(Settings::read_machine().get(WORLDS).unwrap_or(WORLDS))
}

/// Where the world named `name_or_path` is kept: a plain name is a folder of
/// the worlds' folder, anything more a path as it stands.
pub fn world(name_or_path: &str) -> PathBuf {
    world_in(&worlds(), name_or_path)
}

/// Where the world named `name_or_path` is kept, the worlds' folder being
/// `worlds`: a plain name as a folder may have it ([`world_name`]).
pub fn world_in(worlds: &Path, name_or_path: &str) -> PathBuf {
    let path = Path::new(name_or_path);
    let plain = path.components().count() == 1 && path.is_relative() && name_or_path != "." && name_or_path != "..";
    match world_name(name_or_path).filter(|_| plain) {
        Some(folder_name) => worlds.join(folder_name),
        None => path.to_path_buf(),
    }
}

/// `given_name` as a world's name, which is its folder's: only what a
/// folder may be named on Windows and on Linux alike -- letters,
/// digits, spaces, `-`, `_` and `.`, no space or `.` at either end,
/// and not a name Windows keeps for a device (`CON`, `COM1`, ...),
/// which is given a `_` before it. `None` if nothing is left.
pub fn world_name(given_name: &str) -> Option<String> {
    let kept_letters: String = given_name.chars().filter(|&given_char| given_char.is_alphanumeric() || " -_.".contains(given_char)).collect();
    let trimmed_name = kept_letters.trim_matches([' ', '.']);
    let upper_stem = trimmed_name.split('.').next().unwrap_or(trimmed_name).trim_end().to_ascii_uppercase();
    let is_numbered_device = |device_prefix: &str| upper_stem.strip_prefix(device_prefix).is_some_and(|device_number| matches!(device_number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"));
    let is_device_name = matches!(upper_stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") || is_numbered_device("COM") || is_numbered_device("LPT");
    (!trimmed_name.is_empty()).then(|| if is_device_name { format!("_{trimmed_name}") } else { trimmed_name.to_string() })
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
    pub fn default_settings() -> Self {
        Self::parse_text(DEFAULT_SETTINGS_TEXT)
    }

    /// The settings of this machine: its file's ([`settings_file`]), given a
    /// copy of the default settings first if it has none, and the
    /// default ones for what its file lacks.
    pub fn read_machine() -> Self {
        if FORCE_DEFAULTS { Self::default_settings() } else { Self::read_or_start(&settings_file()) }
    }

    /// The settings of the file at `path`, as [`Self::read_machine`] has the
    /// machine's: a file that is there is never written over, and what
    /// it has stands before the default settings.
    pub fn read_or_start(path: &Path) -> Self {
        let mut settings = Self::default_settings();
        if !path.exists() {
            // Not copied, the settings are the default ones all the same.
            _ = settings.write_to(path);
        }
        for (setting_name, value) in Self::read_from(path).lines {
            settings.set(&setting_name, Some(value));
        }
        settings
    }

    /// The settings in the file at `path`: none, if it cannot be read.
    pub fn read_from(path: &Path) -> Self {
        Self::parse_text(&read_to_string(path).unwrap_or_default())
    }

    /// The settings `settings_text` has, a row each after the one naming the
    /// columns: a row that is not a name and a value is passed over.
    fn parse_text(settings_text: &str) -> Self {
        let name_and_value = |settings_row: Vec<String>| <[String; 2]>::try_from(settings_row).ok().map(|[row_name, value]| (row_name.trim().to_string(), value.trim().to_string()));
        Self { lines: crate::csv::rows_named(settings_text).into_iter().filter_map(name_and_value).collect() }
    }

    /// The value of the setting named `setting_name`, if it has a line.
    pub fn get(&self, setting_name: &str) -> Option<&str> {
        self.lines.iter().find(|line| line.0 == setting_name).map(|line| line.1.as_str())
    }

    /// The value of the setting named `setting_name`, as a `T`, if it has a
    /// line and is one.
    pub fn value_as<T: FromStr>(&self, setting_name: &str) -> Option<T> {
        self.get(setting_name)?.parse().ok()
    }

    /// Makes `value` the setting named `setting_name`'s -- or, with none, takes
    /// its line out.
    pub fn set(&mut self, setting_name: &str, value: Option<String>) {
        let line_position = self.lines.iter().position(|line| line.0 == setting_name);
        match (line_position, value) {
            (Some(found_position), Some(value)) => self.lines[found_position].1 = value,
            (Some(removed_position), None) => drop(self.lines.remove(removed_position)),
            (None, Some(value)) => self.lines.push((setting_name.to_string(), value)),
            (None, None) => {}
        }
    }

    /// Writes the settings to the machine's file, its folder made if it
    /// is not there -- unless the machine's file is left alone.
    pub fn write(&self) -> io::Result<()> {
        if FORCE_DEFAULTS {
            return Ok(());
        }
        self.write_to(&settings_file())
    }

    /// Writes the settings to the file at `path`, its folder made if it
    /// is not there.
    pub fn write_to(&self, path: &Path) -> io::Result<()> {
        if let Some(folder) = path.parent() {
            create_dir_all(folder)?;
        }
        write(path, crate::csv::row_text(&SETTINGS_COLUMNS) + &self.lines.iter().map(|(setting_name, value)| crate::csv::row_text(&[setting_name, value])).collect::<String>())
    }
}
