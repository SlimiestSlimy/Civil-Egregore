//! Settings: a file written is read back the same, another's lines
//! left as they are; a machine with no file is given the default
//! settings, and one with a file keeps what it has.
//!
//! `cargo test`

use utilities::settings::{file, folder, world_in, Settings, FILE, FOLDER, WORLDS};

/// The file is the one file in TileSim's one folder.
#[test]
fn the_file_is_in_tilesims_folder() {
    assert!(folder().ends_with(FOLDER) && file() == folder().join(FILE));
}

/// Settings set, changed and unset are written and read back as they
/// stand, a line each; a setting never set has none.
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
    assert_eq!(std::fs::read_to_string(&path).expect("the file"), "another's = kept as it is\nocean level = 412\n");
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

    let own = "worlds = /somewhere/else\nanother's = 7\n";
    std::fs::write(&some, own).expect("written");
    let read = Settings::read_or_start(&some);
    assert_eq!(std::fs::read_to_string(&some).expect("the file"), own, "the machine's file as it was");
    assert_eq!((read.get(WORLDS), read.get("another's")), (Some("/somewhere/else"), Some("7")));
    let (name, value) = ("ocean share", defaults.get("ocean share").expect("a default"));
    assert_eq!(read.get(name), Some(value), "what the file lacks, from the default settings");
}

/// A world named plainly is in the worlds' folder; a path is itself.
#[test]
fn a_world_named_plainly_is_in_the_worlds_folder() {
    let worlds = folder().join(WORLDS);
    assert_eq!(world_in(&worlds, "Meadow"), worlds.join("Meadow"));
    let path = std::path::Path::new("some").join("where");
    assert_eq!(world_in(&worlds, path.to_str().expect("text")), path);
}
