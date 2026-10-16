//! The commands: a world made, run and looked at, as the command line
//! would.

use std::fs;
use server::commands::{describe_saved_world, new, run_saved_world};
use server::transient_data::saves;

/// For worlds of ten sheep and of a thousand: made -- the origin and its
/// halo, nine superchunks -- run twice, the second run going on from
/// where the first saved, and looked at, each command saying what it
/// did, the world named as its folder is; a second world in the same
/// folder refused.
#[test]
fn a_world_is_made_run_and_looked_at() {
    for sheep_per_flock in [10, 1_000] {
        let world_name = format!("commands_{sheep_per_flock}");
        let folder = saves().join("tests").join(&world_name);
        let _ = fs::remove_dir_all(&folder);
        let folder = folder.as_path();
        let made_text = new(folder, &["5", &sheep_per_flock.to_string()]).expect("made");
        assert!(made_text.starts_with(&format!("{world_name}, seed 0x0000000000000005: 9 superchunks, {sheep_per_flock} entities, ")), "{made_text}");
        let first_run_text = run_saved_world(folder, &["40"]).expect("run");
        assert!(first_run_text.starts_with(&format!("{world_name}: tick 0 -> 40, ")), "{first_run_text}");
        let second_run_text = run_saved_world(folder, &["60"]).expect("run on");
        assert!(second_run_text.starts_with(&format!("{world_name}: tick 40 -> 100, ")), "{second_run_text}");
        assert!(describe_saved_world(folder).expect("looked at").starts_with(&format!("{world_name}: seed 0x0000000000000005, at tick 100, ")));
        assert!(new(folder, &[]).is_err(), "a world there already");
        fs::remove_dir_all(folder).expect("removed");
    }
}
