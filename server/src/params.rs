//! The server's flags: everything a command line may hold, each read
//! into the clean parameters [`crate::start`] runs from -- the bare
//! few, more to come.

use protocol::flags::{Flag, FlagError, FlagKind, FlagParser};
use protocol::server_parameters::{CleanServerParameters, ParameterValue};

// The parameters a world is started by, as a command line gives them.
pub use protocol::server_parameters::{SEED, SIDE, WORLD};
/// The server closes once no client has been connected to it for a
/// while (`client_connection::CLIENT_WAIT`): what a window starts its
/// server with. Not given, it stays open, clients coming and going.
pub const CLOSE_IF_NO_CLIENTS: &str = "close_if_no_clients";

/// Every flag the server's command line may hold.
pub fn server_flags() -> Vec<Flag> {
    vec![
        Flag { flag_name: SEED, flag_help: "the world's seed, in hexadecimal; none, one drawn", flag_kind: FlagKind::General, parse_flag: parse_seed_flag },
        Flag { flag_name: SIDE, flag_help: "so many superchunks along a side; none, as far as it goes", flag_kind: FlagKind::General, parse_flag: parse_side_flag },
        Flag { flag_name: WORLD, flag_help: "the world's folder, a name in the worlds' folder or a path: the world saved there opened, or a new one made and saved there", flag_kind: FlagKind::General, parse_flag: parse_world_flag },
        Flag { flag_name: CLOSE_IF_NO_CLIENTS, flag_help: "closes once no client has been connected for 30 seconds: what a window starts its server with; none, stays open", flag_kind: FlagKind::General, parse_flag: parse_close_if_no_clients_flag },
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

/// `--close_if_no_clients`.
fn parse_close_if_no_clients_flag(_: &mut FlagParser, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
    clean_parameters.insert(CLOSE_IF_NO_CLIENTS, ParameterValue::Nested(CleanServerParameters::default()));
    Ok(())
}
