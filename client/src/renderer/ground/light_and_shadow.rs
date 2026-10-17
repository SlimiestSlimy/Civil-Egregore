//! The ground's light: heights smoothed, the shadows cliffs cast, how
//! far the coast is, and light in bands.

use super::{CELLS_KEPT_BEFORE, COAST_REACH, SMOOTHED_OVER, WORKED_SIDE_CELLS, shadow_drop};
use server::host::terrain::Height;
use std::collections::VecDeque;

/// `heights` smoothed: each the mean of those [`SMOOTHED_OVER`] cells
/// each way of it, across and then down; past an edge, the edge's.
pub(crate) fn box_smoothed(heights: &[f32]) -> Vec<f32> {
    let (reach, cells_among) = (SMOOTHED_OVER, (2 * SMOOTHED_OVER + 1) as f32);
    let mut smoothed_across = vec![0.0; WORKED_SIDE_CELLS * WORKED_SIDE_CELLS];
    for (height_row, smoothed_row) in heights.as_chunks::<WORKED_SIDE_CELLS>().0.iter().zip(smoothed_across.as_chunks_mut::<WORKED_SIDE_CELLS>().0) {
        let mut running_sum: f32 = height_row[0] * reach as f32 + height_row[..=reach].iter().sum::<f32>();
        for x in 0..WORKED_SIDE_CELLS {
            smoothed_row[x] = running_sum / cells_among;
            running_sum += height_row[(x + reach + 1).min(WORKED_SIDE_CELLS - 1)] - height_row[x.saturating_sub(reach)];
        }
    }
    let mut smoothed_down = vec![0.0; WORKED_SIDE_CELLS * WORKED_SIDE_CELLS];
    let across_row = |y: usize| &smoothed_across[y * WORKED_SIDE_CELLS..][..WORKED_SIDE_CELLS];
    let mut running_sums: Vec<f32> = (0..WORKED_SIDE_CELLS).map(|x| across_row(0)[x] * reach as f32 + (0..=reach).map(|y| across_row(y)[x]).sum::<f32>()).collect();
    for y in 0..WORKED_SIDE_CELLS {
        let (entering_row, leaving_row) = (across_row((y + reach + 1).min(WORKED_SIDE_CELLS - 1)), across_row(y.saturating_sub(reach)));
        for x in 0..WORKED_SIDE_CELLS {
            smoothed_down[y * WORKED_SIDE_CELLS + x] = running_sums[x] / cells_among;
            running_sums[x] += entering_row[x] - leaving_row[x];
        }
    }
    smoothed_down
}

/// Cells a shadow is followed over, down the diagonal: no more than
/// are kept before a superchunk ([`BEFORE`]), so a shadow is the same
/// whichever superchunk's ground it is worked out for, and none is cut
/// where two meet.
const SHADOW_REACH: usize = 128;
const _: () = assert!(SHADOW_REACH <= CELLS_KEPT_BEFORE);

/// How high the shadow line stands over each of `heights` -- its own
/// height, or that of a cell up the diagonal, within [`SHADOW_REACH`],
/// less [`shadow_drop`] a cell between, whichever is highest -- and
/// whether that line is over the cell: a shadow on it.
pub(crate) fn shadow_lines(heights: &[Height]) -> (Vec<f32>, Vec<bool>) {
    let drop = shadow_drop();
    let (mut lines, mut shadowed_cells) = (vec![0.0; WORKED_SIDE_CELLS * WORKED_SIDE_CELLS], vec![false; WORKED_SIDE_CELLS * WORKED_SIDE_CELLS]);
    // The cells of the diagonal that may yet cast the highest line: how far down it each is, and its height, the line of each lower than the one before it.
    let mut casting_cells: VecDeque<(usize, f32)> = VecDeque::new();
    // A diagonal from each cell of the top row and of the left column.
    for (diagonal_left, diagonal_top) in (0..WORKED_SIDE_CELLS).map(|left_column| (left_column, 0)).chain((1..WORKED_SIDE_CELLS).map(|top_row| (0, top_row))) {
        casting_cells.clear();
        for cells_along in 0..WORKED_SIDE_CELLS - diagonal_left.max(diagonal_top) {
            let worked_index = (diagonal_top + cells_along) * WORKED_SIDE_CELLS + diagonal_left + cells_along;
            let own_height = heights[worked_index] as f32;
            let line_of = |(cast_from, height): (usize, f32)| height - (cells_along - cast_from) as f32 * drop;
            while casting_cells.back().is_some_and(|&last_casting| line_of(last_casting) <= own_height) {
                casting_cells.pop_back();
            }
            casting_cells.push_back((cells_along, own_height));
            if casting_cells.front().is_some_and(|first_casting| cells_along - first_casting.0 > SHADOW_REACH) {
                casting_cells.pop_front();
            }
            lines[worked_index] = line_of(casting_cells[0]);
            shadowed_cells[worked_index] = lines[worked_index] > own_height + 0.01;
        }
    }
    (lines, shadowed_cells)
}

/// How far each cell is from the coast, in cells, [`COAST_REACH`] at
/// most: a cell of land from the nearest under the ocean, one `under_ocean`
/// it from the nearest of land -- across or down 1, diagonally the
/// square root of 2.
pub(crate) fn coast_distances(under_ocean: &[bool]) -> Vec<f32> {
    let diagonal_distance = std::f32::consts::SQRT_2;
    let mut distances_to_coast = vec![COAST_REACH + 1.0; WORKED_SIDE_CELLS * WORKED_SIDE_CELLS];
    // What a cell is from the coast by way of the one `step` before it, `away_cells` from it: a cell of the other kind is the coast itself.
    let distance_via = |known_distances: &[f32], here_index: usize, other_index: usize, away_cells: f32| if under_ocean[here_index] == under_ocean[other_index] { known_distances[other_index] + away_cells } else { away_cells };
    for y in 0..WORKED_SIDE_CELLS {
        for x in 0..WORKED_SIDE_CELLS {
            let forward_index = y * WORKED_SIDE_CELLS + x;
            let mut forward_nearest = distances_to_coast[forward_index];
            if x > 0 { forward_nearest = forward_nearest.min(distance_via(&distances_to_coast, forward_index, forward_index - 1, 1.0)); }
            if y > 0 { forward_nearest = forward_nearest.min(distance_via(&distances_to_coast, forward_index, forward_index - WORKED_SIDE_CELLS, 1.0)); }
            if x > 0 && y > 0 { forward_nearest = forward_nearest.min(distance_via(&distances_to_coast, forward_index, forward_index - WORKED_SIDE_CELLS - 1, diagonal_distance)); }
            if x + 1 < WORKED_SIDE_CELLS && y > 0 { forward_nearest = forward_nearest.min(distance_via(&distances_to_coast, forward_index, forward_index - WORKED_SIDE_CELLS + 1, diagonal_distance)); }
            distances_to_coast[forward_index] = forward_nearest;
        }
    }
    for y in (0..WORKED_SIDE_CELLS).rev() {
        for x in (0..WORKED_SIDE_CELLS).rev() {
            let backward_index = y * WORKED_SIDE_CELLS + x;
            let mut backward_nearest = distances_to_coast[backward_index];
            if x + 1 < WORKED_SIDE_CELLS { backward_nearest = backward_nearest.min(distance_via(&distances_to_coast, backward_index, backward_index + 1, 1.0)); }
            if y + 1 < WORKED_SIDE_CELLS { backward_nearest = backward_nearest.min(distance_via(&distances_to_coast, backward_index, backward_index + WORKED_SIDE_CELLS, 1.0)); }
            if x + 1 < WORKED_SIDE_CELLS && y + 1 < WORKED_SIDE_CELLS { backward_nearest = backward_nearest.min(distance_via(&distances_to_coast, backward_index, backward_index + WORKED_SIDE_CELLS + 1, diagonal_distance)); }
            if x > 0 && y + 1 < WORKED_SIDE_CELLS { backward_nearest = backward_nearest.min(distance_via(&distances_to_coast, backward_index, backward_index + WORKED_SIDE_CELLS - 1, diagonal_distance)); }
            distances_to_coast[backward_index] = backward_nearest;
        }
    }
    distances_to_coast
}

/// `unbanded_light` in bands `step` apart about 1: pixel art, not airbrushed.
pub(crate) fn banded_light(unbanded_light: f32, step: f32) -> f32 {
    1.0 + ((unbanded_light - 1.0) / step).round() * step
}
