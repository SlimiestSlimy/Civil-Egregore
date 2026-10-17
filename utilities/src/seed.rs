//! Seeds as the binary has them: written and read in hexadecimal, and
//! drawn fresh where none is given. The tests' seed, kept in the
//! workspace's file, is not here: it is test infrastructure,
//! `utilities/test_seed.rs` (`docs/utilities.md`, "The seed").

use std::io::Write;

/// `seed` as it is written everywhere: `0x` and its 16 hexadecimal
/// digits.
pub fn seed_as_hex(seed: u64) -> String {
    format!("{seed:#018x}")
}

/// The seed `hex_text` is, in hexadecimal, with or without `0x` before it.
pub fn of_hex(hex_text: &str) -> Option<u64> {
    let trimmed_text = hex_text.trim();
    u64::from_str_radix(trimmed_text.strip_prefix("0x").or_else(|| trimmed_text.strip_prefix("0X")).unwrap_or(trimmed_text), 16).ok()
}

/// A seed drawn from the operating system's randomness: std's hasher
/// keys are taken from it (`getrandom`, `ProcessPrng`), and hashing any
/// word with them gives 64 bits of it. `std::random` would say it
/// plainly, but is not stable yet.
pub fn fresh_seed() -> u64 {
    use std::hash::{BuildHasher, RandomState};
    RandomState::new().hash_one(std::process::id())
}

/// `given_seed`, or, if none is given, one drawn fresh for this run and
/// said on standard error, to be given back to run the same again: what
/// a command of the binary runs on, never the tests' file
/// (`docs/utilities.md`, "The seed").
pub fn given_or_fresh_seed(given_seed: Option<u64>) -> u64 {
    given_seed.unwrap_or_else(|| {
        let drawn_seed = fresh_seed();
        let _ = writeln!(std::io::stderr(), "seed {} drawn for this run: give it back to run the same again", seed_as_hex(drawn_seed));
        drawn_seed
    })
}
