//! TileSim's worlds, from the command line (`tilesim::commands`).
//!
//! `cargo run --release -- new <folder> [name] [seed] [superchunks]`
//! `cargo run --release -- run <folder> [ticks]`
//! `cargo run --release -- info <folder>`

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use std::path::Path;
use std::process::ExitCode;
use tilesim::commands::{info, new, run, USAGE};

/// Does what the command line asks and prints what came of it, or says
/// why not.
fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let done = match arguments.as_slice() {
        ["new", folder, rest @ ..] => new(Path::new(folder), rest),
        ["run", folder, rest @ ..] => run(Path::new(folder), rest),
        ["info", folder] => info(Path::new(folder)),
        _ => Err(USAGE.to_string()),
    };
    match done {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}
