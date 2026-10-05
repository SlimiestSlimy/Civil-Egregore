//! TileSim, from the command line: its worlds, and every crate's
//! diagnostics tools (`tilesim::commands`). No crate but this one and
//! the renderer is a program.
//!
//! `cargo run --release -- new <folder> [name] [seed] [sheep]`
//! `cargo run --release -- run <folder> [ticks]`
//! `cargo run --release -- info <folder>`
//! `cargo run --release -- world <tool> [...]`
//! `cargo run --release -- tessera <tool> [...]`
//!
//! Run with nothing, or with a crate's name alone, and what there is to
//! run is listed.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use std::process::ExitCode;

/// Does what the command line asks, or says why not.
fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match tilesim::commands::dispatch(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}
