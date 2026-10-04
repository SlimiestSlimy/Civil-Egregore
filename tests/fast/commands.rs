//! The commands: a world made, run and looked at, as the command line
//! would.

use std::fs;
use tilesim::commands::{info, new, run};
use tilesim::transient_data::worlds;

/// For worlds of one superchunk and of four: made, run twice -- the
/// second run going on from where the first saved -- and looked at,
/// each command saying what it did; a second world in the same folder
/// refused.
#[test]
fn a_world_is_made_run_and_looked_at() {
    for superchunks in [1, 4] {
        let folder = worlds().join(format!("commands_{superchunks}"));
        let _ = fs::remove_dir_all(&folder);
        let folder = folder.as_path();
        let made = new(folder, &["Test", "5", &superchunks.to_string()]).expect("made");
        assert!(made.starts_with(&format!("Test, seed 5: {superchunks} superchunks, ")), "{made}");
        let first = run(folder, &["40"]).expect("run");
        assert!(first.starts_with("Test: tick 0 -> 40, "), "{first}");
        let second = run(folder, &["60"]).expect("run on");
        assert!(second.starts_with("Test: tick 40 -> 100, "), "{second}");
        assert_eq!(info(folder).expect("looked at"), format!("Test: seed 5, at tick 100, {superchunks} superchunks, layer types [1, 2, 8, 9]"));
        assert!(new(folder, &[]).is_err(), "a world there already");
        fs::remove_dir_all(folder).expect("removed");
    }
}
