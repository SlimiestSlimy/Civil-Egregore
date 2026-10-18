//! Civil Egregore, from the command line: the one program, built with the
//! client or without it (the `client` feature, on by default) -- and its
//! initialization: what registers the base game's types
//! ([`basegame::default_types`]), handed to the server, which makes and
//! freezes each world's registry with it before the world. It hands its
//! command line to the server's flags ([`server::params`]) and the
//! server ([`server::start`]).
//!
//! No arguments, built with the client: the window, on its main menu ([`client::run_window`]).
//! `cargo run --release -- --help`: every flag, and what it takes.
//! `cargo run --release -- --seed 0x5 --new [folder --side 4 --forced_hot []]`
//! `cargo run --release -- --run [folder --ticks 2000]`

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use protocol::flags::FlagError;
use protocol::server_parameters::ServerParameters;
use std::process::ExitCode;

#[cfg(all(windows, feature = "client"))]
unsafe extern "system" {
    /// Windows' own: lets go of the console the program was given.
    safe fn FreeConsole() -> i32;
}

/// On Windows, lets go of the console before the window opens: the one
/// program is a console program, so that its commands print where they
/// are typed, and Windows opens a console beside one started by a
/// click, which the window has no use for. Elsewhere, nothing.
#[cfg(feature = "client")]
fn leave_the_console() {
    #[cfg(windows)]
    FreeConsole();
}

/// No arguments, built with the client: the window, and nothing else
/// run once it closes. Otherwise, the command line read by the server's
/// flags, and run by the server.
fn main() -> ExitCode {
    let shell_words: Vec<String> = std::env::args().skip(1).collect();

    #[cfg(feature = "client")]
    if shell_words.is_empty() {
        leave_the_console();
        client::run_window();
        return ExitCode::SUCCESS;
    }

    let clean_parameters = match (ServerParameters { shell_words }).cleaned(server::params::server_flags()) {
        Ok(clean_parameters) => clean_parameters,
        Err(FlagError::HelpShown(help_text)) => {
            println!("{help_text}");
            return ExitCode::SUCCESS;
        }
        Err(FlagError::WrongLine(why_wrong)) => {
            eprintln!("{why_wrong}");
            return ExitCode::FAILURE;
        }
    };
    match server::start(&clean_parameters, basegame::default_types::register_types) {
        Ok(said) => {
            // Served over the pipe, nothing is left to say: stdout was the wire.
            if !said.is_empty() {
                println!("{said}");
            }
            ExitCode::SUCCESS
        }
        Err(failure_reason) => {
            eprintln!("{failure_reason}");
            ExitCode::FAILURE
        }
    }
}
