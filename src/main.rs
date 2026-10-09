//! Civil Egregore, from the command line: the one program, built with the
//! renderer or without it (the `renderer` feature, on by default). It
//! holds nothing but which crates have commands; the rest is
//! `utilities::commands`.
//!
//! No arguments, built with the renderer: the window, on its main menu ([`renderer::run`]).
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

#[cfg(all(windows, feature = "renderer"))]
unsafe extern "system" {
    /// Windows' own: lets go of the console the program was given.
    safe fn FreeConsole() -> i32;
}

/// On Windows, lets go of the console before the window opens: the one
/// program is a console program, so that its commands print where they
/// are typed, and Windows opens a console beside one started by a
/// click, which the window has no use for. Elsewhere, nothing.
#[cfg(feature = "renderer")]
fn leave_the_console() {
    #[cfg(windows)]
    FreeConsole();
}

/// No arguments, built with the renderer: the window, and nothing else
/// run once it closes. Otherwise, hands the command line to the crate
/// its first word names, or says why not.
fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();

    #[cfg(feature = "renderer")]
    if arguments.is_empty() {
        leave_the_console();
        renderer::run();
        return ExitCode::SUCCESS;
    }

    match program("Civil_Egregore", &CRATES, &arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}
