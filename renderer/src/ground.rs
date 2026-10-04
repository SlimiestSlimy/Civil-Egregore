//! The light on the ground: a superchunk's heights, worked out again
//! here from the world's seed -- the simulation is asked for none of
//! it -- and what they do to a cell's colour, seen from straight above
//! with the sun to the top left.
//!
//! - **Hillshade**: a slope facing the sun lighter, one facing away
//!   darker, off the heights smoothed, in a few bands.
//! - **Tint**: high ground a little lighter than low.
//! - **Cast shadows**: a sweep down the sun's diagonal, each cell
//!   keeping how high the shadow line stands over it.
//! - From a cell a pixel outwards, **cliffs** darkened and a **contour**
//!   every so many heights.
//!
//! Heights never change, so a superchunk's ground is made once and
//! kept.

use coordinates::SUPERCHUNK_SIDE_CELLS;
use chunk_storage::Height;
use terrain::{height_shaped, wall, Shape};

/// Cells along a superchunk's side.
pub const SIDE: usize = SUPERCHUNK_SIDE_CELLS as usize;
/// A cell's side, in metres.
const CELL_METRES: f32 = 2.0;
/// A height's unit, in metres.
const HEIGHT_METRES: f32 = 1.0;
/// How high the sun stands, in degrees.
const SUN_ELEVATION: f32 = 35.0;
/// Cells the heights are smoothed over, each way, twice.
const SMOOTHED_OVER: usize = 4;
/// Cells kept before a superchunk's top left: as far as a shadow is
/// followed back, and what smoothing needs past that.
const BEFORE: usize = 138;
/// Cells kept past its bottom right: what smoothing needs.
const AFTER: usize = 10;
/// Cells along the side of the heights worked on.
const WIDE: usize = BEFORE + SIDE + AFTER;
/// Cells along the side of what is kept of them: a cell more all round.
const KEPT: usize = SIDE + 2;

/// A cell's light kept in seven bits: this is flat ground's, unshaded.
const LIT_ONE: f32 = 80.0;
/// The bit of a cell's light saying a shadow falls on it.
const SHADOWED: u8 = 0x80;
/// A colour's factor in a byte: this is one.
pub const FACTOR_ONE: u32 = 128;
/// What a cast shadow does to a colour: darker, and bluer.
pub const SHADOW: [f32; 3] = [0.6, 0.65, 0.82];
/// How much darker a pixel wholly of cliffs is.
const CLIFF: f32 = 0.65;
/// How much darker a pixel a contour passes.
const CONTOUR: f32 = 0.86;
/// The coarsest the ground is drawn: a pixel `2^6` cells a side.
pub const COARSEST: usize = 6;
/// The coarsest of the levels dropped with the fine parts.
const FINE_LEVELS: usize = 2;

/// Heights the shadow line drops over one cell down the diagonal.
pub fn shadow_drop() -> f32 {
    CELL_METRES * std::f32::consts::SQRT_2 * SUN_ELEVATION.to_radians().tan() / HEIGHT_METRES
}

/// The heights between two contours, drawn a pixel `2^detail` cells a side.
const fn contour_every(detail: usize) -> f32 {
    match detail {
        0 | 1 => 8.0,
        2 | 3 => 16.0,
        _ => 32.0,
    }
}

/// What is kept of a superchunk only while it is seen from near: 3 MiB
/// and the two finest levels.
pub struct Fine {
    /// Its cells' heights, and a cell more all round, row by row.
    heights: Vec<Height>,
    /// How high the shadow line stands over each, laid out as the
    /// heights.
    lines: Vec<f32>,
    /// Each cell's light, of [`LIT_ONE`] -- hillshade and tint -- and
    /// whether a shadow falls on it ([`SHADOWED`]).
    lit: Vec<u8>,
}

impl Fine {
    /// Where the cell `(x, y)` from the top left is kept; -1 and
    /// [`SIDE`] are the cells past the edges.
    const fn kept(x: isize, y: isize) -> usize {
        (y + 1) as usize * KEPT + (x + 1) as usize
    }

    /// The height of the cell `(x, y)`.
    pub fn height(&self, x: isize, y: isize) -> Height {
        self.heights[Self::kept(x, y)]
    }

    /// How high the shadow line stands over the cell `(x, y)`, in heights.
    pub fn line(&self, x: isize, y: isize) -> f32 {
        self.lines[Self::kept(x, y)]
    }

    /// The light of the cell `(x, y)`: 1 flat ground's, shadows apart.
    pub fn light(&self, x: usize, y: usize) -> f32 {
        (self.lit[y * SIDE + x] & !SHADOWED) as f32 / LIT_ONE
    }
}

/// A superchunk's ground.
pub struct Ground {
    /// What a pixel `2^detail` cells a side multiplies its colour by,
    /// red, green and blue of [`FACTOR_ONE`], a level a detail up to
    /// [`COARSEST`]. The two finest are empty once [`Self::fine`] is dropped.
    pub levels: Vec<Vec<[u8; 3]>>,
    /// What only a near view needs.
    pub fine: Option<Fine>,
    /// The frame it was last drawn in.
    pub used: u64,
}

impl Ground {
    /// Drops what only a near view needs.
    pub fn coarsen(&mut self) {
        self.fine = None;
        self.levels[..FINE_LEVELS].fill(Vec::new());
    }

    /// The ground of the superchunk whose top left cell is `top_left`,
    /// in the world whose seed is `seed`, shaped as `shape` says.
    pub fn generate(seed: u64, shape: &Shape, top_left: (u32, u32)) -> Self {
        let heights = heights(seed, shape, top_left);
        let smooth = smoothed(&smoothed(&heights.iter().map(|&height| height as f32).collect::<Vec<_>>()));
        let (lines, shadowed) = shadow_lines(&heights);
        let at = |x: usize, y: usize| (y + BEFORE) * WIDE + x + BEFORE;
        let sun = Sun::new();
        // Each cell's light; and for the levels, its factor, height and walls.
        let mut lit = vec![0; SIDE * SIDE];
        let mut level = Level { side: SIDE, factors: Vec::with_capacity(SIDE * SIDE), heights: Vec::with_capacity(SIDE * SIDE), walls: Vec::with_capacity(SIDE * SIDE) };
        for y in 0..SIDE {
            for x in 0..SIDE {
                let here = at(x, y);
                let slope = HEIGHT_METRES / CELL_METRES / 2.0;
                let (across, down) = ((smooth[here + 1] - smooth[here - 1]) * slope, (smooth[here + WIDE] - smooth[here - WIDE]) * slope);
                let light = banded(1.0 + 0.9 * (sun.shade(across, down) - 1.0), 0.07).clamp(0.55, 1.35) * tint(heights[here].saturating_sub(shape.ground) as f32 / (shape.rise + 255) as f32);
                lit[y * SIDE + x] = (light * LIT_ONE).round() as u8 | if shadowed[here] { SHADOWED } else { 0 };
                level.factors.push(if shadowed[here] { SHADOW.map(|shadow| shadow * light) } else { [light; 3] });
                level.heights.push(heights[here] as f32);
                level.walls.push(wall(heights[here], heights[here + 1]) as u16 + wall(heights[here], heights[here + WIDE]) as u16);
            }
        }
        let mut levels = Vec::with_capacity(COARSEST + 1);
        for detail in 0..=COARSEST {
            levels.push(level.drawn(detail));
            level = level.halved();
        }
        // What is kept: the superchunk's cells and one more all round.
        let kept = || (0..KEPT * KEPT).map(|index| (index / KEPT + BEFORE - 1) * WIDE + index % KEPT + BEFORE - 1);
        let fine = Fine { heights: kept().map(|index| heights[index]).collect(), lines: kept().map(|index| lines[index]).collect(), lit };
        Self { levels, fine: Some(fine), used: 0 }
    }
}

/// The heights about the superchunk whose top left cell is `top_left`:
/// [`WIDE`] a side, row by row.
fn heights(seed: u64, shape: &Shape, top_left: (u32, u32)) -> Vec<Height> {
    let (left, top) = (top_left.0.wrapping_sub(BEFORE as u32), top_left.1.wrapping_sub(BEFORE as u32));
    (0..WIDE * WIDE).map(|index| height_shaped(shape, seed, left.wrapping_add((index % WIDE) as u32), top.wrapping_add((index / WIDE) as u32))).collect()
}

/// `heights` smoothed: each the mean of those [`SMOOTHED_OVER`] cells
/// each way of it, across and then down; past an edge, the edge's.
fn smoothed(heights: &[f32]) -> Vec<f32> {
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

/// How high the shadow line stands over each of `heights` -- its own
/// height, or the line of the cell up the diagonal less
/// [`shadow_drop`], whichever is higher -- and whether that line is
/// over the cell: a shadow on it.
fn shadow_lines(heights: &[Height]) -> (Vec<f32>, Vec<bool>) {
    let drop = shadow_drop();
    let (mut lines, mut shadowed) = (vec![0.0; WIDE * WIDE], vec![false; WIDE * WIDE]);
    for index in 0..WIDE * WIDE {
        let here = heights[index] as f32;
        let cast = if index >= WIDE && !index.is_multiple_of(WIDE) { lines[index - WIDE - 1] - drop } else { f32::MIN };
        shadowed[index] = cast > here + 0.01;
        lines[index] = cast.max(here);
    }
    (lines, shadowed)
}

/// The sun, to the top left.
struct Sun {
    /// Towards it: across, down and up.
    towards: [f32; 3],
}

impl Sun {
    /// The sun [`SUN_ELEVATION`] degrees up.
    fn new() -> Self {
        let elevation = SUN_ELEVATION.to_radians();
        let flat = elevation.cos() / std::f32::consts::SQRT_2;
        Self { towards: [-flat, -flat, elevation.sin()] }
    }

    /// The light on ground rising `across` and `down` a cell: 1 flat
    /// ground's, more facing the sun.
    fn shade(&self, across: f32, down: f32) -> f32 {
        let lit = (-across * self.towards[0] - down * self.towards[1] + self.towards[2]) / (across * across + down * down + 1.0).sqrt();
        lit.max(0.0) / self.towards[2]
    }
}

/// `light` in bands `step` apart about 1: pixel art, not airbrushed.
fn banded(light: f32, step: f32) -> f32 {
    1.0 + ((light - 1.0) / step).round() * step
}

/// The tint of ground `over` the lowest there is, as a share of the
/// highest: low a little darker, high a little lighter.
fn tint(over: f32) -> f32 {
    0.86 + 0.26 * over.min(1.0)
}

/// The ground at one detail, before cliffs and contours: a pixel a tile of cells.
struct Level {
    /// Pixels along its side.
    side: usize,
    /// Each pixel's factor: its cells' light and shadows, averaged.
    factors: Vec<[f32; 3]>,
    /// Its cells' mean height.
    heights: Vec<f32>,
    /// The walls its cells keep.
    walls: Vec<u16>,
}

impl Level {
    /// The level twice as coarse: each pixel four of this one's.
    fn halved(&self) -> Self {
        let side = self.side / 2;
        let four = |x: usize, y: usize| [0, 1, self.side, self.side + 1].map(|step| 2 * y * self.side + 2 * x + step);
        let pixels = || (0..side * side).map(|index| four(index % side, index / side));
        Self {
            side,
            factors: pixels().map(|four| std::array::from_fn(|channel| four.iter().map(|&index| self.factors[index][channel]).sum::<f32>() / 4.0)).collect(),
            heights: pixels().map(|four| four.iter().map(|&index| self.heights[index]).sum::<f32>() / 4.0).collect(),
            walls: pixels().map(|four| four.iter().map(|&index| self.walls[index]).sum()).collect(),
        }
    }

    /// The level as drawn at `detail`: its factors, darker where a
    /// contour passes and by the cliffs in the pixel -- wholly so with
    /// as many walls as cells along its side.
    fn drawn(&self, detail: usize) -> Vec<[u8; 3]> {
        let (every, tile_side) = (contour_every(detail), (SIDE / self.side) as f32);
        let band = |index: usize| (self.heights[index] / every) as u32;
        (0..self.side * self.side)
            .map(|index| {
                let (x, y) = (index % self.side, index / self.side);
                let contour = (x + 1 < self.side && band(index) != band(index + 1)) || (y + 1 < self.side && band(index) != band(index + self.side));
                let cliffs = 1.0 - CLIFF * (self.walls[index] as f32 / tile_side).min(1.0);
                let factor = cliffs * if contour { CONTOUR } else { 1.0 };
                self.factors[index].map(|light| (light * factor * FACTOR_ONE as f32).round().min(255.0) as u8)
            })
            .collect()
    }
}

/// `colour` in the light of `factor`.
pub fn lit(colour: [u8; 3], factor: [u8; 3]) -> [u8; 3] {
    std::array::from_fn(|channel| (colour[channel] as u32 * factor[channel] as u32 / FACTOR_ONE).min(255) as u8)
}
