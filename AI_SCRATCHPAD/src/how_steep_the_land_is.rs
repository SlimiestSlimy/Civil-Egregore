//! How steep the land is: the rise from a cell to the next over a
//! square about the world's middle, as the share of cells at each
//! rise -- what a shading of slopes has to tell apart.

use coordinates::{SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE};
use server::host::frame::FrameAsk;
use server::host::terrain::HeightsAsk;
use server::host::Host;
use server::WorldStart;
use crate::ProbeParameters;
use utilities::tuning::default_tuning;

/// Cells along the side of the square looked at.
pub const SQUARE_SIDE_CELLS: &str = "square_side_cells";
/// Every how many cells one is looked at.
pub const EVERY_SO_MANY_CELLS: &str = "every";

/// The rises counted apart: up to each of these, and past the last.
const RISE_BOUNDS: [u32; 12] = [0, 1, 2, 3, 4, 6, 8, 12, 16, 32, 64, 256];

/// Runs the probe: the land's and the ocean's share, the heights
/// between, and a line a rise.
pub fn run_probe(given_parameters: &ProbeParameters) -> Result<(), String> {
    let (square_side_cells, every_so_many_cells): (u32, u32) = (given_parameters.argument_number(SQUARE_SIDE_CELLS)?, given_parameters.argument_number(EVERY_SO_MANY_CELLS)?);
    let world_start = WorldStart::from_tuning(None, &default_tuning());
    let seed = crate::asking_the_host::seed(given_parameters)?.unwrap_or_else(|| server::seed_with_land(utilities::seed::given_or_fresh_seed(None), &world_start.world_generation));
    // The heights are asked of a host running the world, paused: two rows of cells at a time, the row looked at and the one under it.
    let (host, frame_receiver) = Host::start_on_thread();
    host.set_paused(true);
    host.make_world(WorldStart { seed, ..world_start });
    host.ask_frame(FrameAsk { viewport: None, pixel_side_log2: 0, superchunks_skipped: 0, most_superchunks: 0, seen_near: None });
    let first_frame = crate::asking_the_host::whole_answer(&frame_receiver)?;
    let (frame_levels, terrain) = (first_frame.drawn_heights, host.terrain());
    let middle_cartesian = WORLD_MIDDLE.top_left().cartesian();
    let first_cell_cartesian = (middle_cartesian.x + SUPERCHUNK_SIDE_CELLS / 2 - square_side_cells / 2, middle_cartesian.y + SUPERCHUNK_SIDE_CELLS / 2 - square_side_cells / 2);
    let (mut counts, mut land_cells, mut ocean, mut lowest_land, mut highest_land) = ([0u64; RISE_BOUNDS.len() + 1], 0u64, 0u64, u32::MAX, 0);
    for y in (0..square_side_cells).step_by(every_so_many_cells as usize) {
        let height_rows = terrain.heights(HeightsAsk { world: first_frame.world, first_cell_cartesian: (first_cell_cartesian.0, first_cell_cartesian.1 + y), size_in_cells: (square_side_cells + 1, 2), skipped_corners_cartesian: Vec::new() }).ok_or("the host runs another world")?;
        let height = |x: u32, row_down: u32| height_rows[(row_down * (square_side_cells + 1) + x) as usize] as u32;
        for x in (0..square_side_cells).step_by(every_so_many_cells as usize) {
            let height_here = height(x, 0);
            if height_here < frame_levels.ocean as u32 {
                ocean += 1;
                continue;
            }
            let rise_to_next = height_here.abs_diff(height(x + 1, 0)).max(height_here.abs_diff(height(x, 1)));
            counts[RISE_BOUNDS.iter().position(|&rise_bound| rise_to_next <= rise_bound).unwrap_or(RISE_BOUNDS.len())] += 1;
            (land_cells, lowest_land, highest_land) = (land_cells + 1, lowest_land.min(height_here), highest_land.max(height_here));
        }
    }
    println!("# seed {}, ocean level {}, highest land {}", utilities::seed::seed_as_hex(seed), frame_levels.ocean, frame_levels.highest_land);
    println!("# land {land_cells} cells looked at, ocean {ocean}; the land from {lowest_land} to {highest_land}");
    println!("rise to the next cell at most,share of the land,share so far");
    let mut so_far = 0;
    for (place, count) in counts.iter().enumerate() {
        so_far += count;
        let bound_name = RISE_BOUNDS.get(place).map_or("more".to_string(), |named_bound| named_bound.to_string());
        println!("{bound_name},{:.4},{:.4}", *count as f64 / land_cells.max(1) as f64, so_far as f64 / land_cells.max(1) as f64);
    }
    Ok(())
}
