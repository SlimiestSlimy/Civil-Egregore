//! The commands: a world made, run and looked at, as the command line
//! would.

use std::fs;
use server::commands::{info, new, run};
use server::transient_data::saves;

/// For worlds of ten sheep and of a thousand: made -- the origin and its
/// halo, nine superchunks -- run twice, the second run going on from
/// where the first saved, and looked at, each command saying what it
/// did, the world named as its folder is; a second world in the same
/// folder refused.
#[test]
fn a_world_is_made_run_and_looked_at() {
    for sheep in [10, 1_000] {
        let name = format!("commands_{sheep}");
        let folder = saves().join("tests").join(&name);
        let _ = fs::remove_dir_all(&folder);
        let folder = folder.as_path();
        let made = new(folder, &["5", &sheep.to_string()]).expect("made");
        assert!(made.starts_with(&format!("{name}, seed 5: 9 superchunks, {sheep} entities, ")), "{made}");
        let first = run(folder, &["40"]).expect("run");
        assert!(first.starts_with(&format!("{name}: tick 0 -> 40, ")), "{first}");
        let second = run(folder, &["60"]).expect("run on");
        assert!(second.starts_with(&format!("{name}: tick 40 -> 100, ")), "{second}");
        assert!(info(folder).expect("looked at").starts_with(&format!("{name}: seed 5, at tick 100, ")));
        assert!(new(folder, &[]).is_err(), "a world there already");
        fs::remove_dir_all(folder).expect("removed");
    }
}
