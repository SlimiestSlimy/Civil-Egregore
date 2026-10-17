//! The server's flags: everything a command line may hold, each read
//! into the clean parameters [`crate::start`] runs from.

use protocol::flags::{Flag, FlagError, FlagKind, FlagParser};
use protocol::server_parameters::{CleanServerParameters, ParameterValue};

/// The world's seed: none given, a fresh one.
pub const SEED: &str = "seed";
/// Sheep on every superchunk of a world with a side, on the origin of one without.
pub const SHEEP: &str = "sheep";
/// Superchunks along a world's side: none, as far as it goes.
pub const SIDE: &str = "side";
/// A world hot throughout, not about its sheep's halos.
pub const FORCED_HOT: &str = "forced_hot";
/// Threads a world ticks on: none, every one the machine has.
pub const THREADS: &str = "threads";
/// Ticks a world is run for.
pub const TICKS: &str = "ticks";
/// A world's folder: a name in the worlds' folder, or a path.
pub const FOLDER: &str = "folder";
/// The command making a world.
pub const NEW: &str = "new";
/// The command running a saved world.
pub const RUN: &str = "run";
/// The command saying what a saved world is.
pub const INFO: &str = "info";

/// Every flag the server's command line may hold.
pub fn server_flags() -> Vec<Flag> {
    vec![
        Flag { flag_name: SEED, flag_help: "the world's seed, in hexadecimal; none, a fresh one", flag_kind: FlagKind::General, parse_flag: parse_seed_flag },
        Flag { flag_name: SHEEP, flag_help: "so many sheep on every superchunk of a world with a side, on its origin of one without", flag_kind: FlagKind::General, parse_flag: parse_sheep_flag },
        Flag { flag_name: SIDE, flag_help: "so many superchunks along a side; none, as far as it goes", flag_kind: FlagKind::General, parse_flag: parse_side_flag },
        Flag { flag_name: FORCED_HOT, flag_help: "[]: hot throughout, not about the sheep's halos; only with a side", flag_kind: FlagKind::General, parse_flag: parse_forced_hot_flag },
        Flag { flag_name: THREADS, flag_help: "on so many threads; none, every one the machine has", flag_kind: FlagKind::General, parse_flag: parse_threads_flag },
        Flag { flag_name: TICKS, flag_help: "so many ticks; none, 10000", flag_kind: FlagKind::General, parse_flag: parse_ticks_flag },
        Flag { flag_name: NEW, flag_help: "[folder ...]: makes a world and saves it in the folder", flag_kind: FlagKind::Terminal, parse_flag: parse_new_flag },
        Flag { flag_name: RUN, flag_help: "[folder ...]: loads the world in the folder, ticks it, and saves it", flag_kind: FlagKind::Terminal, parse_flag: parse_run_flag },
        Flag { flag_name: INFO, flag_help: "[folder]: says what the world in the folder is", flag_kind: FlagKind::Terminal, parse_flag: parse_info_flag },
    ]
}

/// `--seed 0x…`.
fn parse_seed_flag(flag_parser: &mut FlagParser, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
    clean_parameters.put(SEED, ParameterValue::Hex(flag_parser.hex_parameter()?));
    Ok(())
}

/// `--sheep 400`.
fn parse_sheep_flag(flag_parser: &mut FlagParser, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
    parse_integer_flag(flag_parser, clean_parameters, SHEEP)
}

/// `--side 4`.
fn parse_side_flag(flag_parser: &mut FlagParser, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
    parse_integer_flag(flag_parser, clean_parameters, SIDE)
}

/// `--threads 8`.
fn parse_threads_flag(flag_parser: &mut FlagParser, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
    parse_integer_flag(flag_parser, clean_parameters, THREADS)
}

/// `--ticks 2000`.
fn parse_ticks_flag(flag_parser: &mut FlagParser, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
    parse_integer_flag(flag_parser, clean_parameters, TICKS)
}

/// `--forced_hot []`: given at all, the world is forced hot.
fn parse_forced_hot_flag(flag_parser: &mut FlagParser, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
    flag_parser.bracketed(|_| Ok(()))?;
    clean_parameters.put(FORCED_HOT, ParameterValue::Nested(CleanServerParameters::default()));
    Ok(())
}

/// `--new [folder ...]`.
fn parse_new_flag(flag_parser: &mut FlagParser, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
    parse_command_on_folder(flag_parser, clean_parameters, NEW)
}

/// `--run [folder ...]`.
fn parse_run_flag(flag_parser: &mut FlagParser, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
    parse_command_on_folder(flag_parser, clean_parameters, RUN)
}

/// `--info [folder]`.
fn parse_info_flag(flag_parser: &mut FlagParser, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
    parse_command_on_folder(flag_parser, clean_parameters, INFO)
}

/// A flag of one whole number, put under `parameter_name`.
fn parse_integer_flag(flag_parser: &mut FlagParser, clean_parameters: &mut CleanServerParameters, parameter_name: &'static str) -> Result<(), FlagError> {
    clean_parameters.put(parameter_name, ParameterValue::Integer(flag_parser.integer_parameter()?));
    Ok(())
}

/// A command on a folder: `[folder` and its flags `]`, nested under
/// `command_name`.
fn parse_command_on_folder(flag_parser: &mut FlagParser, clean_parameters: &mut CleanServerParameters, command_name: &'static str) -> Result<(), FlagError> {
    let command_parameters = flag_parser.bracketed(|flag_parser| {
        let mut command_parameters = CleanServerParameters::default();
        command_parameters.put(FOLDER, ParameterValue::Text(flag_parser.text_parameter()?));
        flag_parser.parse_flags_within(&mut command_parameters)?;
        Ok(command_parameters)
    })?;
    clean_parameters.put(command_name, ParameterValue::Nested(command_parameters));
    Ok(())
}
