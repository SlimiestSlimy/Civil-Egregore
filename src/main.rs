//! Civil Egregore, from the command line: the one program, built with the
//! renderer or without it (the `renderer` feature, on by default). It
//! holds nothing but which crates have commands; the rest is
//! `utilities::crate_commands`.
//!
//! No arguments, built with the renderer: the window, on its main menu ([`renderer::run_window`]).
//! `cargo run --release -- help`: every command of every crate, what
//! each takes and what that is if not given.
//! `cargo run --release -- server run <folder> [ticks]`
//! `cargo run --release -- tessera <tool> [...]`
//! `cargo run --release -- docs stale [from commit]`

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use std::process::ExitCode;
use utilities::commands::{run_program, CommandCrate};

/// The crates with commands.
const CRATES_WITH_COMMANDS: &[CommandCrate] = &[
    CommandCrate { crate_name: "server", what_it_does: "a world made, run and looked at, and the server's diagnostics tools", crate_commands: &server::commands::COMMANDS },
    CommandCrate { crate_name: "tessera", what_it_does: "Tessera's diagnostics tools", crate_commands: &tessera::diagnostics::tool::COMMANDS },
    CommandCrate { crate_name: "docs", what_it_does: "the docs read against a change", crate_commands: &utilities::stale_docs::COMMANDS },
    #[cfg(feature = "renderer")]
    CommandCrate { crate_name: "renderer", what_it_does: "the renderer's diagnostics tools: what the window draws, with no window", crate_commands: &renderer::diagnostics::tool::COMMANDS },
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
    let owned_arguments: Vec<String> = std::env::args().skip(1).collect();
    let command_line_arguments: Vec<&str> = owned_arguments.iter().map(String::as_str).collect();

    #[cfg(feature = "renderer")]
    if command_line_arguments.is_empty() {
        leave_the_console();
        renderer::run_window();
        return ExitCode::SUCCESS;
    }

    match run_program("Civil_Egregore", CRATES_WITH_COMMANDS, &command_line_arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure_reason) => {
            eprintln!("{failure_reason}");
            ExitCode::FAILURE
        }
    }
}
