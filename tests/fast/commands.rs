//! The commands: a world made, run and looked at, from command lines
//! read by the server's flags, as the program's would be.

use protocol::server_parameters::ServerParameters;
use std::fs;

/// What the server says to `command_line`, read by its flags.
fn said_to(command_line: &str) -> Result<String, String> {
    let server_parameters = ServerParameters { shell_words: command_line.split_whitespace().map(str::to_string).collect() };
    server::start(&server_parameters.cleaned(server::params::server_flags()).map_err(|flag_error| format!("{flag_error:?}"))?)
}

/// For worlds of ten sheep and of a thousand: made -- the origin and its
/// halo, nine superchunks -- run twice, the second run going on from
/// where the first saved, and looked at, each command saying what it
/// did, the world named as its folder is; a second world in the same
/// folder refused.
#[test]
fn a_world_is_made_run_and_looked_at() {
    let seed_hex = utilities::seed::seed_as_hex(crate::test_seed::TestSeedHandle::open().seed());
    for sheep_per_flock in [10, 1_000] {
        let world_name = format!("commands_{sheep_per_flock}");
        let folder = crate::worlds::folder(&world_name);
        let folder_text = folder.display();
        let made_text = said_to(&format!("--seed {seed_hex} --new [{folder_text} --sheep {sheep_per_flock}]")).expect("made");
        assert!(made_text.starts_with(&format!("{world_name}, seed {seed_hex}: 9 superchunks, {sheep_per_flock} entities, ")), "{made_text}");
        let first_run_text = said_to(&format!("--run [{folder_text} --ticks 40]")).expect("run");
        assert!(first_run_text.starts_with(&format!("{world_name}: tick 0 -> 40, ")), "{first_run_text}");
        let second_run_text = said_to(&format!("--run [{folder_text} --ticks 60]")).expect("run on");
        assert!(second_run_text.starts_with(&format!("{world_name}: tick 40 -> 100, ")), "{second_run_text}");
        assert!(said_to(&format!("--info [{folder_text}]")).expect("looked at").starts_with(&format!("{world_name}: seed {seed_hex}, at tick 100, ")));
        assert!(said_to(&format!("--new [{folder_text}]")).is_err(), "a world there already");
        fs::remove_dir_all(&folder).expect("removed");
    }
}
