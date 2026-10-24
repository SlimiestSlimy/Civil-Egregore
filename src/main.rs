//! Civil Egregore, from the command line: the one program -- the window,
//! or a server run headless -- and its initialization: what registers the base game's types
//! ([`basegame::default_types`]), handed to the server, which makes and
//! freezes each world's registry with it before the world. It hands its
//! command line to the server's flags ([`server::params`]) and the
//! server ([`server::start`]).
//!
//! No arguments: the window ([`client::run_window`]).
//! `cargo run --release -- --help`: every flag, and what it takes.
//! `cargo run --release -- --seed 0x5 --side 4`: a server running headless, listening for a client.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

use protocol::flags::FlagError;
use protocol::server_parameters::{CleanServerParameters, ParameterValue, ServerParameters, CAMERA_FORCES_SUPERCHUNKS_HOT, WORLD};
use std::process::ExitCode;

#[cfg(windows)]
unsafe extern "system" {
    /// Windows' own: lets go of the console the program was given.
    safe fn FreeConsole() -> i32;
}

/// On Windows, lets go of the console before the window opens: the one
/// program is a console program, so that its commands print where they
/// are typed, and Windows opens a console beside one started by a
/// click, which the window has no use for. Elsewhere, nothing.
fn leave_the_console() {
    #[cfg(windows)]
    FreeConsole();
}

/// The folder of the world the window opens with, in the worlds' folder.
const FIRST_WORLD: &str = "window test world";

/// The world the window opens with, until it has its menus: in its own
/// folder, of no size, the superchunks the window shows kept hot -- and
/// no sheep in it, basegame's config written so before the window asks
/// for it to be made, a world not there yet. Until a new world's config
/// is the client's to give, the program writes it.
fn first_world() -> CleanServerParameters {
    let config_folder = utilities::settings::world(FIRST_WORLD).join(server::MOD_CONFIG).join(basegame::default_types::BUNDLE);
    if !config_folder.exists() {
        let no_sheep = basegame::worldgen::WorldGeneration { sheep_per_superchunk: 0, ..basegame::worldgen::WorldGeneration::DEFAULT };
        if let Err(why_not) = no_sheep.write_config(&config_folder) {
            eprintln!("the window's world keeps its sheep: {why_not}");
        }
    }
    CleanServerParameters::default().with(WORLD, ParameterValue::Text(FIRST_WORLD.to_string())).with(CAMERA_FORCES_SUPERCHUNKS_HOT, ParameterValue::Nested(CleanServerParameters::default()))
}

/// No arguments: the window, and nothing else run once it closes.
/// Otherwise, the command line read by the server's flags, and run by
/// the server.
fn main() -> ExitCode {
    let shell_words: Vec<String> = std::env::args().skip(1).collect();

    if shell_words.is_empty() {
        leave_the_console();
        client::run_window(basegame::default_types::register_types, first_world());
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
