//! The command line: each word read by the server's flags as the
//! program's would be.

use protocol::server_parameters::ServerParameters;

/// A seed is read in hexadecimal however it is written -- with `0x` or
/// without, all digits, or a word another number would take, `5eed` --
/// and a count is a count: each word read as its flag wants it.
#[test]
fn every_word_is_read_as_its_flag_wants_it() {
    use server::params::{SEED, SIDE};
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let seed_read = |command_line: &str| ServerParameters { shell_words: command_line.split_whitespace().map(str::to_string).collect() }.cleaned(server::params::server_flags()).map(|clean_parameters| (clean_parameters.hex(SEED), clean_parameters.integer(SIDE)));
    for (seed_written, seed_meant) in [(format!("{seed:#x}"), seed), (format!("{seed:x}"), seed), ("5eed".to_string(), 0x5eed), ("1234".to_string(), 0x1234)] {
        assert_eq!(seed_read(&format!("--seed {seed_written} --side 12")).ok(), Some((Some(seed_meant), Some(12))), "--seed {seed_written}");
    }
    assert!(seed_read("--seed seed").is_err(), "no hexadecimal");
    assert!(seed_read("--side 5eed").is_err(), "no whole number");
    let world_read = ServerParameters { shell_words: vec!["--world".to_string(), "saves/first".to_string()] }.cleaned(server::params::server_flags()).expect("read");
    assert_eq!(world_read.text(server::params::WORLD), Some("saves/first"), "a world's folder, as written");
}
