//! Settings: a file written, in CSV, is read back the same, another's rows
//! left as they are; a machine with no file is given the default
//! settings, and one with a file keeps what it has.
//!
//! `cargo test`

use utilities::settings::{file, folder, world_in, world_name, Settings, FILE, FOLDER, WORLDS};

/// The file is the one file in Civil Egregore's one folder.
#[test]
fn the_file_is_in_civil_egregores_folder() {
    assert!(folder().ends_with(FOLDER) && file() == folder().join(FILE));
}

/// Settings set, changed and unset are written and read back as they
/// stand, a row each under the one naming the columns; a setting never set has none.
#[test]
fn settings_are_kept_a_line_each() {
    let path = utilities::transient_data::TransientData::of(env!("CARGO_MANIFEST_DIR")).under("tests/settings").join(FILE);
    let mut settings = Settings::default();
    settings.set("wall length", Some("0.75".to_string()));
    settings.set("another's", Some("kept as it is".to_string()));
    settings.set("ocean level", Some("300".to_string()));
    settings.set("ocean level", Some("412".to_string()));
    settings.set("wall length", None);
    settings.set("never set", None);
    settings.write_to(&path).expect("written");
    assert_eq!(std::fs::read_to_string(&path).expect("the file"), "setting,value\nanother's,kept as it is\nocean level,412\n");
    let read = Settings::read_from(&path);
    assert_eq!(read, settings);
    assert_eq!((read.number::<u32>("ocean level"), read.number::<f32>("another's"), read.get("wall length")), (Some(412), None, None));
    assert_eq!(Settings::read_from(&path.with_file_name("none")), Settings::default());
}

/// With no file, the default settings are copied to where it belongs;
/// with one, it is left as it is, and what it has stands before the
/// default settings, which give the rest.
#[test]
fn the_default_settings_never_replace_a_machines() {
    let folder = utilities::transient_data::TransientData::of(env!("CARGO_MANIFEST_DIR")).under("tests/settings");
    let (none, some) = (folder.join("started"), folder.join("kept"));
    _ = std::fs::remove_file(&none);
    let defaults = Settings::defaults();
    assert!(defaults.get(WORLDS).is_some(), "the default settings name the worlds' folder");
    assert_eq!(Settings::read_or_start(&none), defaults);
    assert_eq!(Settings::read_from(&none), defaults, "the default settings, copied");

    let own = "setting,value\nworlds,/somewhere/else\nanother's,7\n";
    std::fs::write(&some, own).expect("written");
    let read = Settings::read_or_start(&some);
    assert_eq!(std::fs::read_to_string(&some).expect("the file"), own, "the machine's file as it was");
    assert_eq!((read.get(WORLDS), read.get("another's")), (Some("/somewhere/else"), Some("7")));
    let (name, value) = ("ocean share", defaults.get("ocean share").expect("a default"));
    assert_eq!(read.get(name), Some(value), "what the file lacks, from the default settings");
}

/// Settings are found by name, whatever the order of their lines:
/// the default settings' lines turned round are the same settings;
/// and a file's columns are found by name, whatever their order, one
/// it lacks empty.
#[test]
fn settings_are_found_by_name_in_any_order() {
    let path = utilities::transient_data::TransientData::of(env!("CARGO_MANIFEST_DIR")).under("tests/settings").join("turned round");
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/default_settings.csv")).expect("the default settings");
    let mut lines: Vec<&str> = text.lines().collect();
    lines[1..].reverse();
    std::fs::write(&path, lines.join("\n")).expect("written");
    let (read, defaults) = (Settings::read_from(&path), Settings::defaults());
    for name in utilities::tuning::NAMES.into_iter().chain([WORLDS]) {
        assert_eq!(read.get(name), defaults.get(name), "{name}");
    }
    let rows = utilities::csv::rows_by_column("uses,seed\n3,0x1\n", &["seed", "uses", "never written"]);
    assert_eq!(rows, [["0x1", "3", ""]]);
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
