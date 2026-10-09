//! The AI's own scratchpad: what it tests Civil Egregore with as it
//! works on it, free for it to use. So far probes: questions asked of a running
//! host ([`server::host`]) while working on it, each a command
//! ([`COMMANDS`]) printing CSV. No part of the program: nothing depends
//! on this crate, and nothing may.
//!
//! `cargo run --release -p AI_SCRATCHPAD -- <command> [what it takes]`

mod asking_the_host;
mod sheep_under_camera_loading;
mod tree_cover_over_ticks;

use utilities::commands::{dispatch, Command, Parameter};

/// The seed a probe's world is made from, in hex: 0, one drawn with
/// land about the world's middle.
const SEED: &str = "seed";

/// The probes.
const COMMANDS: [Command; 2] = [
    Command {
        name: "tree_cover_over_ticks",
        does: "makes a world forced hot with the tree cover given, resets it to the second cover if one is given, and prints its trees and grass as it ticks flat out: whether a reset is whole at once, and where the tree rule takes a cover",
        parameters: &[
            Parameter::new(tree_cover_over_ticks::COVER, "0.5"),
            Parameter::new(tree_cover_over_ticks::COVER_RESET_TO, ""),
            Parameter::new(tree_cover_over_ticks::TICKS, "100000"),
            Parameter::new(tree_cover_over_ticks::SIDE, "2"),
            Parameter::new(SEED, "0"),
        ],
        run: tree_cover_over_ticks::run,
    },
    Command {
        name: "sheep_under_camera_loading",
        does: "makes a world with no side whose camera loads, asks for a square viewport about its middle, and prints the sheep on each superchunk hot in it: whether every superchunk generated in the viewport is given its flock",
        parameters: &[Parameter::new(sheep_under_camera_loading::SHEEP_A_SUPERCHUNK, "10"), Parameter::new(sheep_under_camera_loading::VIEWPORT_SIDE, "6"), Parameter::new(SEED, "0")],
        run: sheep_under_camera_loading::run,
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
