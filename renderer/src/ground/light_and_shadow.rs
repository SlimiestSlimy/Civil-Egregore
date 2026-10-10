//! The ground's light: heights smoothed, the shadows cliffs cast, how
//! far the coast is, and light in bands.

use super::{BEFORE, COAST_REACH, SMOOTHED_OVER, WIDE, shadow_drop};
use server::host::terrain_seen::Height;
use std::collections::VecDeque;

/// `heights` smoothed: each the mean of those [`SMOOTHED_OVER`] cells
/// each way of it, across and then down; past an edge, the edge's.
pub(crate) fn smoothed(heights: &[f32]) -> Vec<f32> {
    let (reach, among) = (SMOOTHED_OVER, (2 * SMOOTHED_OVER + 1) as f32);
    let mut across = vec![0.0; WIDE * WIDE];
    for (row, smooth) in heights.as_chunks::<WIDE>().0.iter().zip(across.as_chunks_mut::<WIDE>().0) {
        let mut sum: f32 = row[0] * reach as f32 + row[..=reach].iter().sum::<f32>();
        for x in 0..WIDE {
            smooth[x] = sum / among;
            sum += row[(x + reach + 1).min(WIDE - 1)] - row[x.saturating_sub(reach)];
        }
    }
    let mut down = vec![0.0; WIDE * WIDE];
    let row = |y: usize| &across[y * WIDE..][..WIDE];
    let mut sums: Vec<f32> = (0..WIDE).map(|x| row(0)[x] * reach as f32 + (0..=reach).map(|y| row(y)[x]).sum::<f32>()).collect();
    for y in 0..WIDE {
        let (entering, leaving) = (row((y + reach + 1).min(WIDE - 1)), row(y.saturating_sub(reach)));
        for x in 0..WIDE {
            down[y * WIDE + x] = sums[x] / among;
            sums[x] += entering[x] - leaving[x];
        }
    }
    down
}

/// Cells a shadow is followed over, down the diagonal: no more than
/// are kept before a superchunk ([`BEFORE`]), so a shadow is the same
/// whichever superchunk's ground it is worked out for, and none is cut
/// where two meet.
const SHADOW_REACH: usize = 128;
const _: () = assert!(SHADOW_REACH <= BEFORE);

/// How high the shadow line stands over each of `heights` -- its own
/// height, or that of a cell up the diagonal, within [`SHADOW_REACH`],
/// less [`shadow_drop`] a cell between, whichever is highest -- and
/// whether that line is over the cell: a shadow on it.
pub(crate) fn shadow_lines(heights: &[Height]) -> (Vec<f32>, Vec<bool>) {
    let drop = shadow_drop();
    let (mut lines, mut shadowed) = (vec![0.0; WIDE * WIDE], vec![false; WIDE * WIDE]);
    // The cells of the diagonal that may yet cast the highest line: how far down it each is, and its height, the line of each lower than the one before it.
    let mut casting: VecDeque<(usize, f32)> = VecDeque::new();
    // A diagonal from each cell of the top row and of the left column.
    for (left, top) in (0..WIDE).map(|left| (left, 0)).chain((1..WIDE).map(|top| (0, top))) {
        casting.clear();
        for along in 0..WIDE - left.max(top) {
            let index = (top + along) * WIDE + left + along;
            let here = heights[index] as f32;
            let line_of = |(from, height): (usize, f32)| height - (along - from) as f32 * drop;
            while casting.back().is_some_and(|&last| line_of(last) <= here) {
                casting.pop_back();
            }
            casting.push_back((along, here));
            if casting.front().is_some_and(|first| along - first.0 > SHADOW_REACH) {
                casting.pop_front();
            }
            lines[index] = line_of(casting[0]);
            shadowed[index] = lines[index] > here + 0.01;
        }
    }
    (lines, shadowed)
}

/// How far each cell is from the coast, in cells, [`COAST_REACH`] at
/// most: a cell of land from the nearest under the ocean, one `under`
/// it from the nearest of land -- across or down 1, diagonally the
/// square root of 2.
pub(crate) fn coast_distances(under: &[bool]) -> Vec<f32> {
    let diagonal = std::f32::consts::SQRT_2;
    let mut distances = vec![COAST_REACH + 1.0; WIDE * WIDE];
    // What a cell is from the coast by way of the one `step` before it, `away` from it: a cell of the other kind is the coast itself.
    let by = |distances: &[f32], here: usize, other: usize, away: f32| if under[here] == under[other] { distances[other] + away } else { away };
    for y in 0..WIDE {
        for x in 0..WIDE {
            let here = y * WIDE + x;
            let mut nearest = distances[here];
            if x > 0 { nearest = nearest.min(by(&distances, here, here - 1, 1.0)); }
            if y > 0 { nearest = nearest.min(by(&distances, here, here - WIDE, 1.0)); }
            if x > 0 && y > 0 { nearest = nearest.min(by(&distances, here, here - WIDE - 1, diagonal)); }
            if x + 1 < WIDE && y > 0 { nearest = nearest.min(by(&distances, here, here - WIDE + 1, diagonal)); }
            distances[here] = nearest;
        }
    }
    for y in (0..WIDE).rev() {
        for x in (0..WIDE).rev() {
            let here = y * WIDE + x;
            let mut nearest = distances[here];
            if x + 1 < WIDE { nearest = nearest.min(by(&distances, here, here + 1, 1.0)); }
            if y + 1 < WIDE { nearest = nearest.min(by(&distances, here, here + WIDE, 1.0)); }
            if x + 1 < WIDE && y + 1 < WIDE { nearest = nearest.min(by(&distances, here, here + WIDE + 1, diagonal)); }
            if x > 0 && y + 1 < WIDE { nearest = nearest.min(by(&distances, here, here + WIDE - 1, diagonal)); }
            distances[here] = nearest;
        }
    }
    distances
}

/// `light` in bands `step` apart about 1: pixel art, not airbrushed.
pub(crate) fn banded(light: f32, step: f32) -> f32 {
    1.0 + ((light - 1.0) / step).round() * step
}
