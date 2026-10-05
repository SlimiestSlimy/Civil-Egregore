//! Settings: only what was changed has a line; a file written is read
//! back the same; another's lines are left as they are.
//!
//! `cargo test`

use utilities::settings::{file, folder, Settings, FILE, FOLDER};

/// The file is the one file in TileSim's one folder.
#[test]
fn the_file_is_in_tilesims_folder() {
    assert!(folder().ends_with(FOLDER) && file() == folder().join(FILE));
}

/// Settings set, changed and unset are written and read back as they
/// stand, a line each; a setting never changed has none.
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
