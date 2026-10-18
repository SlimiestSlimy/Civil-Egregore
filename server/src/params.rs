//! The server's flags: everything a command line may hold, each read
//! into the clean parameters [`crate::start`] runs from -- the bare
//! few, more to come.

use protocol::flags::{Flag, FlagError, FlagKind, FlagParser};
use protocol::server_parameters::{CleanServerParameters, ParameterValue};

// The parameters a world is started by, as a command line gives them.
pub use protocol::server_parameters::{SEED, SIDE, WORLD};
/// Serving the client that started the process, over stdin and stdout:
/// what a window starts its server with.
pub const PIPE: &str = "pipe";

/// Every flag the server's command line may hold.
pub fn server_flags() -> Vec<Flag> {
    vec![
        Flag { flag_name: SEED, flag_help: "the world's seed, in hexadecimal; none, one drawn", flag_kind: FlagKind::General, parse_flag: parse_seed_flag },
        Flag { flag_name: SIDE, flag_help: "so many superchunks along a side; none, as far as it goes", flag_kind: FlagKind::General, parse_flag: parse_side_flag },
        Flag { flag_name: WORLD, flag_help: "the world's folder: a name in the worlds' folder, or a path", flag_kind: FlagKind::General, parse_flag: parse_world_flag },
        Flag { flag_name: PIPE, flag_help: "serves the client that started it, over stdin and stdout: what a window starts its server with", flag_kind: FlagKind::Terminal, parse_flag: parse_pipe_flag },
    ]
}

/// `--seed 0x…`.
fn parse_seed_flag(flag_parser: &mut FlagParser, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
    clean_parameters.insert(SEED, ParameterValue::Hex(flag_parser.hex_parameter()?));
    Ok(())
}

/// `--side 4`.
fn parse_side_flag(flag_parser: &mut FlagParser, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
    clean_parameters.insert(SIDE, ParameterValue::Integer(flag_parser.integer_parameter()?));
    Ok(())
}

/// `--world saves/first`.
fn parse_world_flag(flag_parser: &mut FlagParser, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
    clean_parameters.insert(WORLD, ParameterValue::Text(flag_parser.text_parameter()?));
    Ok(())
}

/// `--pipe`.
fn parse_pipe_flag(_: &mut FlagParser, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
    clean_parameters.insert(PIPE, ParameterValue::Nested(CleanServerParameters::default()));
    Ok(())
}
