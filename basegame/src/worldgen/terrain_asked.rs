//! The terrain a client asks of the server where no frame brings it
//! (`protocol::pipe::terrain`): worked out from how a world is
//! generated alone, on as many threads as the machine has.

use crate::worldgen::mesh::WorldLand;
use crate::worldgen::WorldGeneration;
use server::simulation::simulation_api::terrain::{CellCover, DrawnHeightRange, Height, HeightsAsk, MapAnswer, MapAsk, MapCell};
use std::thread;
use server::simulation::simulation_api::SUPERCHUNK_SIDE_CELLS;

/// Rows of `part_rows` given out among the machine's threads, each thread
/// handed its first row's number and its rows, `width` items a row:
/// a cell is the same whoever works it out.
fn shared_out<T: Send>(row_items: &mut [T], width: usize, row_work: impl Fn(usize, &mut [T]) + Sync) {
    let thread_count = thread::available_parallelism().map_or(1, |available_threads| available_threads.get());
    let rows_each = (row_items.len() / width.max(1)).div_ceil(thread_count).max(1);
    thread::scope(|thread_scope| {
        for (part, part_rows) in row_items.chunks_mut(rows_each * width.max(1)).enumerate() {
            let shared_row_work = &row_work;
            thread_scope.spawn(move || shared_row_work(part * rows_each, part_rows));
        }
    });
}

/// The heights `heights_ask` asks for, of the world of `seed` generated as
/// `world_generation` says.
pub fn heights(seed: u64, world_generation: &WorldGeneration, heights_ask: &HeightsAsk) -> Vec<Height> {
    let (width, superchunk_side_cells) = (heights_ask.size_in_cells.0 as usize, SUPERCHUNK_SIDE_CELLS);
    let mut heights = vec![0; width * heights_ask.size_in_cells.1 as usize];
    shared_out(&mut heights, width, |first_row, part_rows| {
        let mut mesh_lands = WorldLand::new(&world_generation.land_shape, seed);
        for (row_in_part, heights) in part_rows.chunks_mut(width).enumerate() {
            let (y, mut cells_across) = (heights_ask.first_cell_cartesian.1.wrapping_add((first_row + row_in_part) as u32), 0);
            while cells_across < width {
                // The row's run within one superchunk.
                let x = heights_ask.first_cell_cartesian.0.wrapping_add(cells_across as u32);
                let span_in_superchunk = ((superchunk_side_cells - x % superchunk_side_cells) as usize).min(width - cells_across);
                if !heights_ask.skipped_corners_cartesian.contains(&(x - x % superchunk_side_cells, y - y % superchunk_side_cells)) {
                    heights[cells_across..cells_across + span_in_superchunk].iter_mut().zip(0u32..).for_each(|(height, cells_along)| *height = mesh_lands.height(x.wrapping_add(cells_along), y));
                }
                cells_across += span_in_superchunk;
            }
        }
    });
    heights
}

/// The heights a world generated as `world_generation` says is drawn
/// between. A world with no relief to speak of -- a plain, a step above
/// its ocean -- is drawn between the levels worlds have unless told
/// otherwise: its land is low land, not a peak.
pub fn drawn_heights_of(world_generation: &WorldGeneration) -> DrawnHeightRange {
    let land_shape = &world_generation.land_shape;
    let nearly_flat = land_shape.highest_land <= land_shape.ocean + crate::worldgen::STEP;
    DrawnHeightRange { lowest_height: land_shape.ocean_floor, ocean: land_shape.ocean, highest_land: if nearly_flat { land_shape.ocean + crate::worldgen::LandShape::DEFAULT.highest_land } else { land_shape.highest_land } }
}

/// The map `map_ask` asks for, of the world of `seed` generated as
/// `world_generation` says.
pub fn map(seed: u64, world_generation: &WorldGeneration, map_ask: MapAsk) -> MapAnswer {
    let (width, drawn_heights) = (map_ask.size_in_pixels.0 as usize, drawn_heights_of(world_generation));
    // The cell in the middle of a pixel, or none past the world's edges.
    let cell = |x: i64, y: i64| {
        let cell = |first_coordinate: i64, pixel_offset: i64| u32::try_from(first_coordinate + pixel_offset * i64::from(map_ask.step) + i64::from(map_ask.step) / 2).ok();
        Some((cell(map_ask.first_cell_cartesian.0, x)?, cell(map_ask.first_cell_cartesian.1, y)?))
    };
    let world_growth = &world_generation.world_growth(seed);
    let mut cells = vec![None; width * map_ask.size_in_pixels.1 as usize];
    shared_out(&mut cells, width, |first_row, part_rows| {
        // Each thread its own: both keep what they found about the last cell.
        let (mut mesh_lands, mut thread_growth) = (WorldLand::new(&world_generation.land_shape, seed), *world_growth);
        for (row_in_part, cells) in part_rows.chunks_mut(width).enumerate() {
            let y = (first_row + row_in_part) as i64;
            for (x, map_pixel) in cells.iter_mut().enumerate() {
                let Some((cell_x, cell_y)) = cell(x as i64, y) else {
                    continue;
                };
                let height = mesh_lands.height(cell_x, cell_y);
                let height_above = cell(x as i64, y - 1).map_or(height, |(x, y)| mesh_lands.height(x, y));
                let cell_cover = if height < drawn_heights.ocean {
                    CellCover::Ocean
                } else {
                    let grown_here = thread_growth.grown_at(cell_x, cell_y);
                    match (grown_here.tree, grown_here.has_grass) {
                        (Some(_), _) => CellCover::Tree,
                        (None, true) => CellCover::Grass,
                        (None, false) => CellCover::Dirt,
                    }
                };
                let on_a_mesh_line = map_ask.mesh_lines_asked && mesh_lands.inland_and_line_distance(cell_x, cell_y).1 < u64::from(map_ask.step);
                *map_pixel = Some(MapCell { height, height_above, cell_cover, on_a_mesh_line });
            }
        }
    });
    MapAnswer { map_ask, drawn_heights, cells }
}
