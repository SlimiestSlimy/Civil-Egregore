//! The AI's own scratchpad: what it tests Civil Egregore with as it
//! works on it, free for it to use. So far probes: questions asked of a running
//! host ([`server::host`]) while working on it, each a command
//! ([`PROBES`]) printing CSV. No part of the program: nothing depends
//! on this crate, and nothing may.
//!
//! `cargo run --release -p AI_SCRATCHPAD -- --<probe> [--<parameter> <value> ...]`, `--help` for them all

mod acts_over_a_border;
mod asking_the_host;
mod how_steep_the_land_is;
mod ticks_under_a_wide_viewport;
mod what_a_generation_makes;

use protocol::flags::{Flag, FlagError, FlagKind, FlagParser};
use protocol::server_parameters::{CleanServerParameters, ParameterValue, ServerParameters};
use std::str::FromStr;

/// The seed a probe's world is made from, in hex: 0, one drawn with
/// land about the world's middle.
const SEED: &str = "seed";

/// A probe: its flag's name, what it does, what each parameter is if
/// not given, and what runs it.
struct Probe {
    /// Its flag's name.
    probe_name: &'static str,
    /// What it does, as `--help` says it.
    what_it_does: &'static str,
    /// Each parameter it reads, and what it is if not given.
    parameter_defaults: &'static [(&'static str, &'static str)],
    /// What runs it.
    run_probe: fn(&ProbeParameters) -> Result<(), String>,
}

/// What a probe was given within its brackets, and what it falls back to.
pub struct ProbeParameters<'given> {
    /// The parameters given.
    given_parameters: &'given CleanServerParameters,
    /// What each is if not given.
    parameter_defaults: &'static [(&'static str, &'static str)],
}

impl ProbeParameters<'_> {
    /// The parameter `parameter_name` as text: given, or its default.
    pub fn argument_text(&self, parameter_name: &str) -> Result<String, String> {
        self.given_parameters.text(parameter_name).map(str::to_string).or_else(|| self.parameter_defaults.iter().find(|(default_name, _)| *default_name == parameter_name).map(|(_, default_text)| default_text.to_string())).ok_or_else(|| format!("--{parameter_name} not given"))
    }

    /// The parameter `parameter_name` as a number: given, or its default.
    pub fn argument_number<Number: FromStr>(&self, parameter_name: &str) -> Result<Number, String> {
        let argument_text = self.argument_text(parameter_name)?;
        argument_text.parse().map_err(|_| format!("--{parameter_name} {argument_text}: not a number of the kind wanted"))
    }
}

/// A general flag putting its one parameter, as text, under its own name.
macro_rules! parameter_flag {
    ($parameter_name:expr) => {{
        fn parse_flag(flag_parser: &mut FlagParser, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
            clean_parameters.put($parameter_name, ParameterValue::Text(flag_parser.text_parameter()?));
            Ok(())
        }
        Flag { flag_name: $parameter_name, flag_help: "a probe's parameter", flag_kind: FlagKind::General, parse_flag }
    }};
}

/// A probe's terminal flag: its parameters in brackets, nested under
/// the probe's name.
macro_rules! probe_flag {
    ($probe_index:expr) => {{
        fn parse_flag(flag_parser: &mut FlagParser, clean_parameters: &mut CleanServerParameters) -> Result<(), FlagError> {
            let probe_parameters = flag_parser.bracketed(|flag_parser| {
                let mut probe_parameters = CleanServerParameters::default();
                flag_parser.parse_flags_within(&mut probe_parameters)?;
                Ok(probe_parameters)
            })?;
            clean_parameters.put(PROBES[$probe_index].probe_name, ParameterValue::Nested(probe_parameters));
            Ok(())
        }
        Flag { flag_name: PROBES[$probe_index].probe_name, flag_help: PROBES[$probe_index].what_it_does, flag_kind: FlagKind::Terminal, parse_flag }
    }};
}

/// The probes.
const PROBES: [Probe; 4] = [
    Probe {
        probe_name: "acts_over_a_border",
        what_it_does: "times the second phase with lines of entities acting over every border running north to south (flipping a cell beyond it under a compare, or with `crossing` 1 stepping over it and back), and with the same lines moved away from it, on one thread and on all: what an act over a border costs",
        parameter_defaults: &[(acts_over_a_border::WORLD_SIDE_SUPERCHUNKS, "4"), (acts_over_a_border::EVERY_SO_MANY_CELLS, "2"), (acts_over_a_border::STEPPING_A_SUPERCHUNK, "4000"), (acts_over_a_border::TICKS, "200"), (acts_over_a_border::THREADS_OF_SECOND_RUN, "0"), (acts_over_a_border::CROSSING, "0")],
        run_probe: acts_over_a_border::run_probe,
    },
    Probe {
        probe_name: "how_steep_the_land_is",
        what_it_does: "prints the share of the land's cells at each rise to the next cell, over a square about the world's middle: what a shading of slopes has to tell apart",
        parameter_defaults: &[(how_steep_the_land_is::SQUARE_SIDE_CELLS, "16384"), (how_steep_the_land_is::EVERY_SO_MANY_CELLS, "16"), (SEED, "0")],
        run_probe: how_steep_the_land_is::run_probe,
    },
    Probe {
        probe_name: "ticks_under_a_wide_viewport",
        what_it_does: "makes a world with no side whose camera loads, keeps a square viewport hot, and prints its ticks a second with no frame asked and with frames asked as a window asks: what answering a client costs the simulation",
        parameter_defaults: &[(ticks_under_a_wide_viewport::VIEWPORT_SIDE, "12"), (ticks_under_a_wide_viewport::SECONDS_MEASURED, "10"), (ticks_under_a_wide_viewport::SUPERCHUNKS_A_FRAME, "32"), (ticks_under_a_wide_viewport::SHEEP_A_SUPERCHUNK, "250"), (ticks_under_a_wide_viewport::PACE_TICKS_A_SECOND, "0"), (SEED, "0")],
        run_probe: ticks_under_a_wide_viewport::run_probe,
    },
    Probe {
        probe_name: "what_a_generation_makes",
        what_it_does: "makes a small world forced hot on a plain with grass on so many thousandths of its cells, and prints the cells of each layer: whether a plain is dry, flat and as grassy as asked",
        parameter_defaults: &[(what_a_generation_makes::GRASS_THOUSANDTHS, "333"), (what_a_generation_makes::WORLD_SIDE_SUPERCHUNKS, "2")],
        run_probe: what_a_generation_makes::run_probe,
    },
];

/// Every flag: the probes, and every parameter any of them reads.
fn probe_flags() -> Vec<Flag> {
    vec![
        probe_flag!(0),
        probe_flag!(1),
        probe_flag!(2),
        probe_flag!(3),
        parameter_flag!(SEED),
        parameter_flag!(acts_over_a_border::WORLD_SIDE_SUPERCHUNKS),
        parameter_flag!(acts_over_a_border::EVERY_SO_MANY_CELLS),
        parameter_flag!(acts_over_a_border::STEPPING_A_SUPERCHUNK),
        parameter_flag!(acts_over_a_border::TICKS),
        parameter_flag!(acts_over_a_border::THREADS_OF_SECOND_RUN),
        parameter_flag!(acts_over_a_border::CROSSING),
        parameter_flag!(how_steep_the_land_is::SQUARE_SIDE_CELLS),
        parameter_flag!(ticks_under_a_wide_viewport::VIEWPORT_SIDE),
        parameter_flag!(ticks_under_a_wide_viewport::SECONDS_MEASURED),
        parameter_flag!(ticks_under_a_wide_viewport::SUPERCHUNKS_A_FRAME),
        parameter_flag!(ticks_under_a_wide_viewport::SHEEP_A_SUPERCHUNK),
        parameter_flag!(ticks_under_a_wide_viewport::PACE_TICKS_A_SECOND),
        parameter_flag!(what_a_generation_makes::GRASS_THOUSANDTHS),
    ]
}

/// The command line read by the probes' flags, and the probe named run.
fn run_line() -> Result<(), String> {
    let clean_parameters = match (ServerParameters { shell_words: std::env::args().skip(1).collect() }).cleaned(probe_flags()) {
        Ok(clean_parameters) => clean_parameters,
        Err(FlagError::HelpShown(help_text)) => {
            println!("{help_text}");
            return Ok(());
        }
        Err(FlagError::WrongLine(why_wrong)) => return Err(why_wrong),
    };
    let probe = PROBES.iter().find(|probe| clean_parameters.nested(probe.probe_name).is_some()).ok_or("no probe given: --help says them")?;
    let given_parameters = clean_parameters.nested(probe.probe_name).expect("found above");
    (probe.run_probe)(&ProbeParameters { given_parameters, parameter_defaults: probe.parameter_defaults })
}

fn main() {
    if let Err(why_not) = run_line() {
        eprintln!("{why_not}");
        std::process::exit(1);
    }
}
