//! Civil Egregore, from the command line: the one program beside the
//! renderer. It holds nothing but which crates have commands; the rest
//! is `utilities::commands`.
//!
//! `cargo run --release -- help`: every command of every crate, what
//! each takes and what that is if not given.
//! `cargo run --release -- server run <folder> [ticks]`
//! `cargo run --release -- tessera <tool> [...]`

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use std::process::ExitCode;
use utilities::commands::{program, Crate};

/// The crates with commands.
const CRATES: [Crate; 2] = [
    Crate { name: "server", does: "a world made, run and looked at, and the server's diagnostics tools", commands: &server::commands::COMMANDS },
    Crate { name: "tessera", does: "Tessera's diagnostics tools", commands: &tessera::diagnostics::tool::COMMANDS },
];

/// Hands the command line to the crate its first word names, or says why not.
/// needs updating for lab and renderer as the default changes.
fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    
    #[cfg(feature = "renderer")]
    if arguments.len() == 0 {
        renderer::render_main_lab()
    }
    
    match program("Civil_Egregore", &CRATES, &arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}




