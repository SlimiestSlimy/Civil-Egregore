//! The ground at each detail: a pixel a tile of cells, its cells'
//! light averaged, and what is drawn of a pixel only -- a contour
//! passing it, the coast in it.

use super::relief::{FOAM_MOST, SAND_MOST};
use super::{PixelShade, FACTOR_ONE, FOAM_COLOUR, SAND_COLOUR, PALE_COLOUR, SUPERCHUNK_CELLS_A_SIDE};

/// How much darker a pixel a contour passes.
const CONTOUR_DARKENING: f32 = 0.88;
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
const DARKER_CONTOUR_EVERY: i64 = 5;

/// The ground at one detail, before contours and the coast.
pub(super) struct GroundAtDetail {
    /// Pixels along its side.
    pub side_pixels: usize,
    /// What each pixel's colour is multiplied by: its cells' light,
    /// tint and shadows, averaged.
    pub colour_factors: Vec<[f32; 3]>,
    /// What each is multiplied by if it is sand: its cells' light and
    /// shadows, untinted.
    pub untinted_factors: Vec<[f32; 3]>,
    /// How much of each is pale, its cells' averaged.
    pub pale_shares: Vec<f32>,
    /// Its cells' mean height.
    pub heights: Vec<f32>,
    /// The share of its cells under the ocean.
    pub ocean_shares: Vec<f32>,
    /// Cells from the nearest of its cells to the coast, to the coast.
    pub to_coast: Vec<f32>,
}

impl GroundAtDetail {
    /// The level twice as coarse: each pixel four of this one's.
    pub fn twice_as_coarse(&self) -> Self {
        let halved_side = self.side_pixels / 2;
        let four_finer = |x: usize, y: usize| [0, 1, self.side_pixels, self.side_pixels + 1].map(|step| 2 * y * self.side_pixels + 2 * x + step);
        let coarse_pixels = || (0..halved_side * halved_side).map(|coarse_index| four_finer(coarse_index % halved_side, coarse_index / halved_side));
        let mean_of_four = |finer_values: &[f32]| coarse_pixels().map(|finer_four| finer_four.iter().map(|&finer_index| finer_values[finer_index]).sum::<f32>() / 4.0).collect::<Vec<f32>>();
        let mean_of_three = |finer_triples: &[[f32; 3]]| coarse_pixels().map(|finer_quad| std::array::from_fn(|colour_channel| finer_quad.iter().map(|&quad_index| finer_triples[quad_index][colour_channel]).sum::<f32>() / 4.0)).collect::<Vec<[f32; 3]>>();
        Self {
            side_pixels: halved_side,
            colour_factors: mean_of_three(&self.colour_factors),
            untinted_factors: mean_of_three(&self.untinted_factors),
            pale_shares: mean_of_four(&self.pale_shares),
            heights: mean_of_four(&self.heights),
            ocean_shares: mean_of_four(&self.ocean_shares),
            to_coast: coarse_pixels().map(|coast_four| coast_four.iter().map(|&coast_index| self.to_coast[coast_index]).fold(f32::INFINITY, f32::min)).collect(),
        }
    }

    /// The level as drawn at `detail_shift`: darker where a contour passes
    /// -- fewer of them the steeper the ground, so they never crowd --
    /// sand where the land meets water and foam where water meets the
    /// land, a pixel wide.
    pub fn drawn_at_detail(&self, detail_shift: usize) -> Vec<PixelShade> {
        let contour_every = CONTOUR_EVERY * (1 << detail_shift) as f32;
        let coast_wide = ((SUPERCHUNK_CELLS_A_SIDE / self.side_pixels) as f32).clamp(2.0, super::COAST_REACH);
        let contour_band = |band_index: usize, band_every: f32| (self.heights[band_index] / band_every) as i64;
        let is_land = |land_index: usize| self.ocean_shares[land_index] < 0.5;
        (0..self.side_pixels * self.side_pixels)
            .map(|pixel_index| {
                let (x, y) = (pixel_index % self.side_pixels, pixel_index / self.side_pixels);
                let coast_share = ((coast_wide + 1.0 - self.to_coast[pixel_index]) / coast_wide).clamp(0.0, 1.0);
                if !is_land(pixel_index) {
                    return PixelShade::from_factor(self.colour_factors[pixel_index], FOAM_COLOUR, FOAM_MOST * coast_share);
                }
                // The pixel to its right and the one under it, where they are land: no contour over water.
                let neighbour_indices = [(x + 1 < self.side_pixels).then_some(pixel_index + 1), (y + 1 < self.side_pixels).then_some(pixel_index + self.side_pixels)];
                let land_neighbours = || neighbour_indices.into_iter().flatten().filter(|&neighbour_index| is_land(neighbour_index));
                // The steeper the ground, the fewer of its contours are drawn.
                let steepest_rise = land_neighbours().map(|rise_neighbour| (self.heights[pixel_index] - self.heights[rise_neighbour]).abs()).fold(0.0, f32::max);
                let drawn_every = (0..2).map(|fifth_power| contour_every * (DARKER_CONTOUR_EVERY as f32).powi(fifth_power)).find(|&tried_every| steepest_rise <= tried_every * CONTOURS_APART).unwrap_or(contour_every * (DARKER_CONTOUR_EVERY * DARKER_CONTOUR_EVERY) as f32);
                let contour_crossed = |crossed_every: f32| land_neighbours().any(|crossed_neighbour| contour_band(pixel_index, crossed_every) != contour_band(crossed_neighbour, crossed_every));
                let contour_darkness = match (contour_crossed(drawn_every), contour_crossed(drawn_every * DARKER_CONTOUR_EVERY as f32)) {
                    (false, _) => 1.0,
                    (true, false) => CONTOUR_DARKENING,
                    (true, true) => FIFTH_CONTOUR,
                };
                let sand_share = SAND_MOST * coast_share;
                let (laid_colour, part) = if sand_share >= self.pale_shares[pixel_index] { (SAND_COLOUR, sand_share) } else { (PALE_COLOUR, self.pale_shares[pixel_index]) };
                // Sand is its own colour at any height: untinted by as much as the pixel is sand.
                let (tinted_factor, plain) = (self.colour_factors[pixel_index], self.untinted_factors[pixel_index]);
                let pixel_factor: [f32; 3] = std::array::from_fn(|colour_channel| (tinted_factor[colour_channel] + (plain[colour_channel] - tinted_factor[colour_channel]) * sand_share / SAND_MOST) * contour_darkness);
                PixelShade::from_factor(pixel_factor, laid_colour, part)
            })
            .collect()
    }
}

impl PixelShade {
    /// The shade multiplying a colour by `colour_factor`, `part` of it `laid_colour` first.
    fn from_factor(colour_factor: [f32; 3], laid_colour: [u8; 3], part: f32) -> Self {
        let laid_kind = [PALE_COLOUR, SAND_COLOUR, FOAM_COLOUR].iter().position(|&known_colour| known_colour == laid_colour).unwrap_or(0) as u8;
        let factor_bytes = colour_factor.map(|factor_channel| (factor_channel * FACTOR_ONE as f32).round().min(255.0) as u8);
        Self([factor_bytes[0], factor_bytes[1], factor_bytes[2], laid_kind << Self::PART_BITS | (part.clamp(0.0, 1.0) * Self::WHOLE_LAID as f32).round() as u8])
    }
}
