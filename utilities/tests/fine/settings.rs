//! Settings: a file written, in CSV, is read back the same, another's rows
//! left as they are; a machine with no file is given the default
//! settings, and one with a file keeps what it has.
//!
//! `cargo test`

use std::path::PathBuf;
use utilities::settings::{settings_file, folder, world_in, world_name, Settings, SETTINGS_FILE_NAME, FOLDER, WORLDS};

/// The tests' own folder, in the crate's `transient_data/`, kept out of git.
fn tests_folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("transient_data").join("tests").join("settings")
}

/// The file is the one file in Civil Egregore's one folder.
#[test]
fn the_file_is_in_civil_egregores_folder() {
    assert!(folder().ends_with(FOLDER) && settings_file() == folder().join(SETTINGS_FILE_NAME));
}

/// Settings set, changed and unset are written and read back as they
/// stand, a row each under the one naming the columns; a setting never set has none.
#[test]
fn settings_are_kept_a_line_each() {
    let path = tests_folder().join(SETTINGS_FILE_NAME);
    let mut settings = Settings::default();
    settings.set("wall length", Some("0.75".to_string()));
    settings.set("another's", Some("kept as it is".to_string()));
    settings.set("ocean level", Some("300".to_string()));
    settings.set("ocean level", Some("412".to_string()));
    settings.set("wall length", None);
    settings.set("never set", None);
    settings.write_to(&path).expect("written");
    assert_eq!(std::fs::read_to_string(&path).expect("the file"), "setting,value\nanother's,kept as it is\nocean level,412\n");
    let read_back = Settings::read_from(&path);
    assert_eq!(read_back, settings);
    assert_eq!((read_back.value_as::<u32>("ocean level"), read_back.value_as::<f32>("another's"), read_back.get("wall length")), (Some(412), None, None));
    assert_eq!(Settings::read_from(&path.with_file_name("none")), Settings::default());
}

/// With no file, the default settings are copied to where it belongs;
/// with one, it is left as it is, and what it has stands before the
/// default settings, which give the rest.
#[test]
fn the_default_settings_never_replace_a_machines() {
    let folder = tests_folder();
    let (missing_file, existing_file) = (folder.join("started"), folder.join("kept"));
    _ = std::fs::remove_file(&missing_file);
    let default_values = Settings::default_settings();
    assert!(default_values.get(WORLDS).is_some(), "the default settings name the worlds' folder");
    assert_eq!(Settings::read_or_start(&missing_file), default_values);
    assert_eq!(Settings::read_from(&missing_file), default_values, "the default settings, copied");

    let own_text = "setting,value\nworlds,/somewhere/else\nanother's,7\n";
    std::fs::write(&existing_file, own_text).expect("written");
    let read_settings = Settings::read_or_start(&existing_file);
    assert_eq!(std::fs::read_to_string(&existing_file).expect("the file"), own_text, "the machine's file as it was");
    assert_eq!((read_settings.get(WORLDS), read_settings.get("another's")), (Some("/somewhere/else"), Some("7")));
    // A file lacking it: the worlds' folder from the default settings.
    std::fs::write(&existing_file, "setting,value\nanother's,7\n").expect("written");
    assert_eq!(Settings::read_or_start(&existing_file).get(WORLDS), default_values.get(WORLDS), "what the file lacks, from the default settings");
}

/// Settings are found by name, whatever the order of their lines:
/// the default settings' lines turned round are the same settings;
/// and a file's columns are found by name, whatever their order, one
/// it lacks empty.
#[test]
fn settings_are_found_by_name_in_any_order() {
    let path = tests_folder().join("turned round");
    let default_text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/default_settings.csv")).expect("the default settings");
    let mut lines: Vec<&str> = default_text.lines().collect();
    lines[1..].reverse();
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("its folder made");
    std::fs::write(&path, lines.join("\n")).expect("written");
    let (read_settings, default_values) = (Settings::read_from(&path), Settings::default_settings());
    assert_eq!(read_settings.get(WORLDS), default_values.get(WORLDS));
    let found_rows = utilities::csv::rows_by_column("uses,seed\n3,0x1\n", &["seed", "uses", "never written"]);
    assert_eq!(found_rows, [["0x1", "3", ""]]);
}

/// A world named plainly is in the worlds' folder; a path is itself.
#[test]
fn a_world_named_plainly_is_in_the_worlds_folder() {
    let worlds = folder().join(WORLDS);
    assert_eq!(world_in(&worlds, "Meadow"), worlds.join("Meadow"));
    assert_eq!(world_in(&worlds, " Up: the <hills>? "), worlds.join("Up the hills"), "only what a folder may be named");
    assert_eq!((world_name("con.txt"), world_name("Lpt7"), world_name("Common"), world_name(" ./\\. ")), (Some("_con.txt".to_string()), Some("_Lpt7".to_string()), Some("Common".to_string()), None));
    let path = std::path::Path::new("some").join("where");
    assert_eq!(world_in(&worlds, path.to_str().expect("text")), path);
}
