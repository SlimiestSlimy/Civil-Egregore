//! The AI's own scratchpad: what it tests Civil Egregore with as it
//! works on it, free for it to use. So far probes: questions asked of a running
//! host ([`server::host`]) while working on it, each a command
//! ([`COMMANDS`]) printing CSV. No part of the program: nothing depends
//! on this crate, and nothing may.
//!
//! `cargo run --release -p AI_SCRATCHPAD -- <command> [what it takes]`

mod asking_the_host;
mod how_steep_the_land_is;
mod ticks_under_a_wide_viewport;
mod what_a_generation_makes;

use utilities::commands::{dispatch, Command, Parameter};

/// The seed a probe's world is made from, in hex: 0, one drawn with
/// land about the world's middle.
const SEED: &str = "seed";

/// The probes.
const COMMANDS: [Command; 3] = [
    Command {
        name: "how_steep_the_land_is",
        does: "prints the share of the land's cells at each rise to the next cell, over a square about the world's middle: what a shading of slopes has to tell apart",
        parameters: &[Parameter::new(how_steep_the_land_is::SIDE, "16384"), Parameter::new(how_steep_the_land_is::EVERY, "16"), Parameter::new(SEED, "0")],
        run: how_steep_the_land_is::run,
    },
    Command {
        name: "ticks_under_a_wide_viewport",
        does: "makes a world with no side whose camera loads, keeps a square viewport hot, and prints its ticks a second with no frame asked and with frames asked as a window asks: what answering a client costs the simulation",
        parameters: &[Parameter::new(ticks_under_a_wide_viewport::VIEWPORT_SIDE, "12"), Parameter::new(ticks_under_a_wide_viewport::SECONDS, "10"), Parameter::new(ticks_under_a_wide_viewport::SUPERCHUNKS_A_FRAME, "32"), Parameter::new(ticks_under_a_wide_viewport::SHEEP_A_SUPERCHUNK, "250"), Parameter::new(ticks_under_a_wide_viewport::PACE, "0"), Parameter::new(SEED, "0")],
        run: ticks_under_a_wide_viewport::run,
    },
    Command {
        name: "what_a_generation_makes",
        does: "makes a small world forced hot on a plain with grass on so many thousandths of its cells, and prints the cells of each layer: whether a plain is dry, flat and as grassy as asked",
        parameters: &[Parameter::new(what_a_generation_makes::GRASS_THOUSANDTHS, "333"), Parameter::new(what_a_generation_makes::SIDE, "2")],
        run: what_a_generation_makes::run,
    },
];

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    if let Err(why_not) = dispatch("AI_SCRATCHPAD", &COMMANDS, &arguments) {
        eprintln!("{why_not}");
        std::process::exit(1);
    }
}
