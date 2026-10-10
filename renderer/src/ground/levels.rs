//! The ground at each detail: a pixel a tile of cells, its cells'
//! light averaged, and what is drawn of a pixel only -- a contour
//! passing it, the coast in it.

use super::relief::{FOAM_MOST, SAND_MOST};
use super::{Shade, FACTOR_ONE, FOAM, SAND, PALE, SIDE};

/// How much darker a pixel a contour passes.
const CONTOUR: f32 = 0.88;
/// How much darker a pixel every fifth contour passes.
const FIFTH_CONTOUR: f32 = 0.68;
/// The share of the heights between two contours that the ground may
/// rise a pixel, at most, for each to be drawn: steeper, they would
/// lie under eight pixels apart, and only every fifth is drawn; five
/// times steeper again, every twenty-fifth.
const CONTOURS_APART: f32 = 0.125;
/// The heights between two contours where a cell is a pixel: twice as
/// many each time a pixel is twice as many cells a side, so contours
/// lie as far apart on the screen however far it is seen from.
const CONTOUR_EVERY: f32 = 128.0;
/// Contours from one drawn darker to the next.
const FIFTH: i64 = 5;

/// The ground at one detail, before contours and the coast.
pub(super) struct Level {
    /// Pixels along its side.
    pub side: usize,
    /// What each pixel's colour is multiplied by: its cells' light,
    /// tint and shadows, averaged.
    pub factors: Vec<[f32; 3]>,
    /// What each is multiplied by if it is sand: its cells' light and
    /// shadows, untinted.
    pub lights: Vec<[f32; 3]>,
    /// How much of each is pale, its cells' averaged.
    pub pale: Vec<f32>,
    /// Its cells' mean height.
    pub heights: Vec<f32>,
    /// The share of its cells under the ocean.
    pub under: Vec<f32>,
    /// Cells from the nearest of its cells to the coast, to the coast.
    pub to_coast: Vec<f32>,
}

impl Level {
    /// The level twice as coarse: each pixel four of this one's.
    pub fn halved(&self) -> Self {
        let side = self.side / 2;
        let four = |x: usize, y: usize| [0, 1, self.side, self.side + 1].map(|step| 2 * y * self.side + 2 * x + step);
        let pixels = || (0..side * side).map(|index| four(index % side, index / side));
        let mean = |of: &[f32]| pixels().map(|four| four.iter().map(|&index| of[index]).sum::<f32>() / 4.0).collect::<Vec<f32>>();
        let mean_of_three = |of: &[[f32; 3]]| pixels().map(|four| std::array::from_fn(|channel| four.iter().map(|&index| of[index][channel]).sum::<f32>() / 4.0)).collect::<Vec<[f32; 3]>>();
        Self {
            side,
            factors: mean_of_three(&self.factors),
            lights: mean_of_three(&self.lights),
            pale: mean(&self.pale),
            heights: mean(&self.heights),
            under: mean(&self.under),
            to_coast: pixels().map(|four| four.iter().map(|&index| self.to_coast[index]).fold(f32::INFINITY, f32::min)).collect(),
        }
    }

    /// The level as drawn at `detail`: darker where a contour passes
    /// -- fewer of them the steeper the ground, so they never crowd --
    /// sand where the land meets water and foam where water meets the
    /// land, a pixel wide.
    pub fn drawn(&self, detail: usize) -> Vec<Shade> {
        let every = CONTOUR_EVERY * (1 << detail) as f32;
        let coast_wide = ((SIDE / self.side) as f32).clamp(2.0, super::COAST_REACH);
        let band = |index: usize, every: f32| (self.heights[index] / every) as i64;
        let land = |index: usize| self.under[index] < 0.5;
        (0..self.side * self.side)
            .map(|index| {
                let (x, y) = (index % self.side, index / self.side);
                let coast = ((coast_wide + 1.0 - self.to_coast[index]) / coast_wide).clamp(0.0, 1.0);
                if !land(index) {
                    return Shade::of(self.factors[index], FOAM, FOAM_MOST * coast);
                }
                // The pixel to its right and the one under it, where they are land: no contour over water.
                let others = [(x + 1 < self.side).then_some(index + 1), (y + 1 < self.side).then_some(index + self.side)];
                let others = || others.into_iter().flatten().filter(|&other| land(other));
                // The steeper the ground, the fewer of its contours are drawn.
                let rise = others().map(|other| (self.heights[index] - self.heights[other]).abs()).fold(0.0, f32::max);
                let drawn = (0..2).map(|fifths| every * (FIFTH as f32).powi(fifths)).find(|&every| rise <= every * CONTOURS_APART).unwrap_or(every * (FIFTH * FIFTH) as f32);
                let crossed = |every: f32| others().any(|other| band(index, every) != band(other, every));
                let darker = match (crossed(drawn), crossed(drawn * FIFTH as f32)) {
                    (false, _) => 1.0,
                    (true, false) => CONTOUR,
                    (true, true) => FIFTH_CONTOUR,
                };
                let sand = SAND_MOST * coast;
                let (over, part) = if sand >= self.pale[index] { (SAND, sand) } else { (PALE, self.pale[index]) };
                // Sand is its own colour at any height: untinted by as much as the pixel is sand.
                let (tinted, plain) = (self.factors[index], self.lights[index]);
                let factor: [f32; 3] = std::array::from_fn(|channel| (tinted[channel] + (plain[channel] - tinted[channel]) * sand / SAND_MOST) * darker);
                Shade::of(factor, over, part)
            })
            .collect()
    }
}

impl Shade {
    /// The shade multiplying a colour by `factor`, `part` of it `over` first.
    fn of(factor: [f32; 3], over: [u8; 3], part: f32) -> Self {
        let kind = [PALE, SAND, FOAM].iter().position(|&colour| colour == over).unwrap_or(0) as u8;
        let factor = factor.map(|factor| (factor * FACTOR_ONE as f32).round().min(255.0) as u8);
        Self([factor[0], factor[1], factor[2], kind << Self::PART_BITS | (part.clamp(0.0, 1.0) * Self::WHOLE as f32).round() as u8])
    }
}
