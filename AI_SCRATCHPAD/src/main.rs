//! The AI's own scratchpad: what it tests Civil Egregore with as it
//! works on it, free for it to use. So far probes: questions asked of a running
//! host ([`server::host`]) while working on it, each a command
//! ([`COMMANDS`]) printing CSV. No part of the program: nothing depends
//! on this crate, and nothing may.
//!
//! `cargo run --release -p AI_SCRATCHPAD -- <command> [what it takes]`

mod asking_the_host;
mod sheep_under_camera_loading;
mod how_steep_the_land_is;
mod ticks_under_a_wide_viewport;
mod tree_cover_over_ticks;
mod what_a_generation_makes;
mod where_a_stopped_world_parts;
mod where_sheep_go;

use utilities::commands::{dispatch, Command, Parameter};

/// The seed a probe's world is made from, in hex: 0, one drawn with
/// land about the world's middle.
const SEED: &str = "seed";

/// The probes.
const COMMANDS: [Command; 7] = [
    Command {
        name: "how_steep_the_land_is",
        does: "prints the share of the land's cells at each rise to the next cell, over a square about the world's middle: what a shading of slopes has to tell apart",
        parameters: &[Parameter::new(how_steep_the_land_is::SIDE, "16384"), Parameter::new(how_steep_the_land_is::EVERY, "16"), Parameter::new(SEED, "0")],
        run: how_steep_the_land_is::run,
    },
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
        parameters: &[Parameter::new(sheep_under_camera_loading::SHEEP_A_SUPERCHUNK, "10"), Parameter::new(sheep_under_camera_loading::VIEWPORT_SIDE, "6"), Parameter::new(sheep_under_camera_loading::TICKS_WAITED, "16"), Parameter::new(SEED, "0")],
        run: sheep_under_camera_loading::run,
    },
    Command {
        name: "ticks_under_a_wide_viewport",
        does: "makes a world with no side whose camera loads, keeps a square viewport hot, and prints its ticks a second with no frame asked and with frames asked as a window asks: what answering a client costs the simulation",
        parameters: &[Parameter::new(ticks_under_a_wide_viewport::VIEWPORT_SIDE, "12"), Parameter::new(ticks_under_a_wide_viewport::SECONDS, "10"), Parameter::new(ticks_under_a_wide_viewport::SUPERCHUNKS_A_FRAME, "32"), Parameter::new(ticks_under_a_wide_viewport::SHEEP_A_SUPERCHUNK, "250"), Parameter::new(ticks_under_a_wide_viewport::PACE, "0"), Parameter::new(SEED, "0")],
        run: ticks_under_a_wide_viewport::run,
    },
    Command {
        name: "where_sheep_go",
        does: "makes a world with no side whose camera loads, keeps a square viewport about its middle, ticks it with no host between, and prints what its ticks say became of its entities: put, removed, lost, refused, born, dead",
        parameters: &[Parameter::new(where_sheep_go::SHEEP_A_SUPERCHUNK, "250"), Parameter::new(where_sheep_go::VIEWPORT_SIDE, "5"), Parameter::new(where_sheep_go::TICKS, "600"), Parameter::new(SEED, "0")],
        run: where_sheep_go::run,
    },
    Command {
        name: "what_a_generation_makes",
        does: "makes a small world forced hot on a plain with grass on so many thousandths of its cells, and prints the cells of each layer: whether a plain is dry, flat and as grassy as asked",
        parameters: &[Parameter::new(what_a_generation_makes::GRASS_THOUSANDTHS, "333"), Parameter::new(what_a_generation_makes::SIDE, "2")],
        run: what_a_generation_makes::run,
    },
    Command {
        name: "where_a_stopped_world_parts",
        does: "runs a world straight and the same world saved and loaded every so many ticks, and prints the first tick the two differ at and in what: where saving and loading lose something",
        parameters: &[Parameter::new(where_a_stopped_world_parts::TICKS_BETWEEN_STOPS, "5000"), Parameter::new(where_a_stopped_world_parts::TICKS, "30000"), Parameter::new(where_a_stopped_world_parts::SHEEP, "4000"), Parameter::new(SEED, "15")],
        run: where_a_stopped_world_parts::run,
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
