//! How steep the land is: the rise from a cell to the next over a
//! square about the world's middle, as the share of cells at each
//! rise -- what a shading of slopes has to tell apart.

use coordinates::{SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE};
use server::host::terrain_seen::{levels, seed_with_land, HeightsSeen};
use server::Start;
use utilities::commands::Given;
use utilities::tuning::defaults;

/// Cells along the side of the square looked at.
pub const SIDE: &str = "cells along the side";
/// Every how many cells one is looked at.
pub const EVERY: &str = "every so many cells";

/// The rises counted apart: up to each of these, and past the last.
const RISES: [u32; 12] = [0, 1, 2, 3, 4, 6, 8, 12, 16, 32, 64, 256];

/// Runs the probe: the land's and the ocean's share, the heights
/// between, and a line a rise.
pub fn run(given: &Given) -> Result<(), String> {
    let (side, every): (u32, u32) = (given.number(SIDE)?, given.number(EVERY)?);
    let start = Start::from_tuning(None, &defaults());
    let seed = crate::asking_the_host::seed(given)?.unwrap_or_else(|| seed_with_land(utilities::seed::counted(), &start.generation, WORLD_MIDDLE));
    let (levels, mut seen) = (levels(&start.generation), HeightsSeen::of(&start.generation, seed));
    let middle = WORLD_MIDDLE.top_left().cartesian();
    let first = (middle.x + SUPERCHUNK_SIDE_CELLS / 2 - side / 2, middle.y + SUPERCHUNK_SIDE_CELLS / 2 - side / 2);
    let (mut counts, mut land, mut ocean, mut lowest, mut highest) = ([0u64; RISES.len() + 1], 0u64, 0u64, u32::MAX, 0);
    for y in (0..side).step_by(every as usize) {
        for x in (0..side).step_by(every as usize) {
            let here = seen.height(first.0 + x, first.1 + y) as u32;
            if here < levels.ocean as u32 {
                ocean += 1;
                continue;
            }
            let rise = here.abs_diff(seen.height(first.0 + x + 1, first.1 + y) as u32).max(here.abs_diff(seen.height(first.0 + x, first.1 + y + 1) as u32));
            counts[RISES.iter().position(|&most| rise <= most).unwrap_or(RISES.len())] += 1;
            (land, lowest, highest) = (land + 1, lowest.min(here), highest.max(here));
        }
    }
    println!("# seed {}, ocean level {}, highest land {}", utilities::seed::hex(seed), levels.ocean, levels.highest);
    println!("# land {land} cells looked at, ocean {ocean}; the land from {lowest} to {highest}");
    println!("rise to the next cell at most,share of the land,share so far");
    let mut so_far = 0;
    for (place, count) in counts.iter().enumerate() {
        so_far += count;
        let name = RISES.get(place).map_or("more".to_string(), |most| most.to_string());
        println!("{name},{:.4},{:.4}", *count as f64 / land.max(1) as f64, so_far as f64 / land.max(1) as f64);
    }
    Ok(())
}
