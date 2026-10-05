//! TileSim, from the command line: the one program beside the
//! renderer. It holds nothing but which crates there are: a crate's
//! name is the first word, and the crate is handed the rest
//! (`utilities::commands`).
//!
//! `cargo run --release -- world new <folder> [name] [seed] [sheep]`
//! `cargo run --release -- world run <folder> [ticks]`
//! `cargo run --release -- world pasture [...]`
//! `cargo run --release -- tessera <tool> [...]`
//!
//! Run with nothing, or with a crate's name alone, and what there is to
//! run is listed.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use std::process::ExitCode;
use utilities::commands::{dispatch, Command, Parameter};

/// What a crate takes: one of its commands, and what that takes, handed on as given.
const REST: &[Parameter] = &[Parameter::new("command, and what it takes", "")];

/// The crates with commands, each by its name.
const CRATES: [Command; 2] = [
    Command { name: "world", does: "a world made, run and looked at, and the world's diagnostics tools; none named, they are listed", parameters: REST, run: |given| world::commands::dispatch(given.arguments()) },
    Command { name: "tessera", does: "Tessera's diagnostics tools; none named, they are listed", parameters: REST, run: |given| tessera::diagnostics::tool::dispatch(given.arguments()) },
];

/// Hands the command line to the crate its first word names, or says why not.
fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match dispatch("tilesim", &CRATES, &arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}
