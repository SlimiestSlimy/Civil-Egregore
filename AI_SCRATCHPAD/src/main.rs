//! The AI's own scratchpad: what it tests Civil Egregore with as it
//! works on it, free for it to use. So far probes: questions asked of a running
//! host ([`server::host`]) while working on it, each a command
//! ([`COMMANDS`]) printing CSV. No part of the program: nothing depends
//! on this crate, and nothing may.
//!
//! `cargo run --release -p AI_SCRATCHPAD -- <command> [what it takes]`

mod acts_over_a_border;
mod asking_the_host;
mod how_steep_the_land_is;
mod ticks_under_a_wide_viewport;
mod what_a_generation_makes;

use utilities::commands::{dispatch_command, NamedCommand, CommandParameter};

/// The seed a probe's world is made from, in hex: 0, one drawn with
/// land about the world's middle.
const SEED: &str = "seed";

/// The probes.
const COMMANDS: [NamedCommand; 4] = [
    NamedCommand {
        command_name: "acts_over_a_border",
        what_it_does: "times the second phase with lines of entities acting over every border running north to south (flipping a cell beyond it under a compare, or with `crossing` 1 stepping over it and back), and with the same lines moved away from it, on one thread and on all: what an act over a border costs",
        command_parameters: &[CommandParameter::new(acts_over_a_border::WORLD_SIDE_SUPERCHUNKS, "4"), CommandParameter::new(acts_over_a_border::EVERY_SO_MANY_CELLS, "2"), CommandParameter::new(acts_over_a_border::STEPPING_A_SUPERCHUNK, "4000"), CommandParameter::new(acts_over_a_border::TICKS, "200"), CommandParameter::new(acts_over_a_border::THREADS_OF_SECOND_RUN, "0"), CommandParameter::new(acts_over_a_border::CROSSING, "0")],
        run_function: acts_over_a_border::run_probe,
    },
    NamedCommand {
        command_name: "how_steep_the_land_is",
        what_it_does: "prints the share of the land's cells at each rise to the next cell, over a square about the world's middle: what a shading of slopes has to tell apart",
        command_parameters: &[CommandParameter::new(how_steep_the_land_is::SQUARE_SIDE_CELLS, "16384"), CommandParameter::new(how_steep_the_land_is::EVERY_SO_MANY_CELLS, "16"), CommandParameter::new(SEED, "0")],
        run_function: how_steep_the_land_is::run_probe,
    },
    NamedCommand {
        command_name: "ticks_under_a_wide_viewport",
        what_it_does: "makes a world with no side whose camera loads, keeps a square viewport hot, and prints its ticks a second with no frame asked and with frames asked as a window asks: what answering a client costs the simulation",
        command_parameters: &[CommandParameter::new(ticks_under_a_wide_viewport::VIEWPORT_SIDE, "12"), CommandParameter::new(ticks_under_a_wide_viewport::SECONDS_MEASURED, "10"), CommandParameter::new(ticks_under_a_wide_viewport::SUPERCHUNKS_A_FRAME, "32"), CommandParameter::new(ticks_under_a_wide_viewport::SHEEP_A_SUPERCHUNK, "250"), CommandParameter::new(ticks_under_a_wide_viewport::PACE_TICKS_A_SECOND, "0"), CommandParameter::new(SEED, "0")],
        run_function: ticks_under_a_wide_viewport::run_probe,
    },
    NamedCommand {
        command_name: "what_a_generation_makes",
        what_it_does: "makes a small world forced hot on a plain with grass on so many thousandths of its cells, and prints the cells of each layer: whether a plain is dry, flat and as grassy as asked",
        command_parameters: &[CommandParameter::new(what_a_generation_makes::GRASS_THOUSANDTHS, "333"), CommandParameter::new(what_a_generation_makes::WORLD_SIDE_SUPERCHUNKS, "2")],
        run_function: what_a_generation_makes::run_probe,
    },
];

fn main() {
    let argument_strings: Vec<String> = std::env::args().skip(1).collect();
    let argument_slices: Vec<&str> = argument_strings.iter().map(String::as_str).collect();
    if let Err(why_not) = dispatch_command("AI_SCRATCHPAD", &COMMANDS, &argument_slices) {
        eprintln!("{why_not}");
        std::process::exit(1);
    }
}
