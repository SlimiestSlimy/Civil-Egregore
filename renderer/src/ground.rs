//! The light on the ground: a superchunk's heights -- brought by the
//! first frame it is hot in, and past its edges, or if no frame brings
//! them, worked out again here from the world's seed -- and what they
//! do to a cell's colour, seen from straight above with the sun to the
//! top left ([`relief`]).
//!
//! - **Slope light**: a slope facing the sun lighter, one facing away
//!   darker, off the heights smoothed, in bands.
//! - **Height tint**: low ground dark, higher lighter and warmer, the
//!   highest pale.
//! - **Cast shadows**: a sweep down the sun's diagonal, each cell
//!   keeping how high the shadow line stands over it.
//! - **Contours**, as far apart on the screen at every detail, every
//!   fifth darker.
//! - **The coast**: sand on the land beside water and foam on the
//!   water beside it, a pixel wide at every detail; the water lighter
//!   over the shallows and darker over the deep, in bands.
//!
//! Heights never change, so a superchunk's ground is made once and
//! kept.

mod levels;
mod light_and_shadow;
pub mod relief;
use levels::Level;
use light_and_shadow::{banded, coast_distances, shadow_lines, smoothed};
use relief::{slope_light, tint, water_light, FOAM, PALE, SAND};

use coordinates::{place_from_cartesian, SUPERCHUNK_SIDE_CELLS};
use std::collections::HashMap;
use server::host::terrain_seen::{height_in_frame, levels, Generation, Height, HeightsSeen};

/// Cells along a superchunk's side.
pub const SIDE: usize = SUPERCHUNK_SIDE_CELLS as usize;
/// Heights the shadow line drops over one cell down the diagonal: a
/// face steeper than this away from the sun is in its own shadow, and
/// casts one a cell long for each so many heights it stands.
const SHADOW_DROP: f32 = 24.0;
/// How far apart the bands of the slopes' light are.
const LIGHT_BAND: f32 = 0.05;
/// Cells from the coast it is looked for no farther than: as wide as
/// it is ever drawn.
pub(crate) const COAST_REACH: f32 = 24.0;
/// Cells the heights are smoothed over, each way, twice.
const SMOOTHED_OVER: usize = 4;
/// Cells kept before a superchunk's top left: as far as a shadow is
/// followed back, and what smoothing needs past that.
const BEFORE: usize = 138;
/// Cells kept past its bottom right: what smoothing needs, and as far
/// as the coast is looked for.
const AFTER: usize = COAST_REACH as usize + 2;
/// Cells along the side of the heights worked on.
const WIDE: usize = BEFORE + SIDE + AFTER;
/// Cells kept of them past each side of the superchunk: as far as a
/// wall's band is looked for.
pub const MARGIN: usize = 8;
/// Cells along the side of what is kept of them.
const KEPT: usize = SIDE + 2 * MARGIN;

/// A cell's light kept in seven bits: this is flat ground's, unshaded.
const LIT_ONE: f32 = 80.0;
/// The bit of a cell's light saying a shadow falls on it.
const SHADOWED: u8 = 0x80;
/// A colour's factor in a byte: this is one.
pub const FACTOR_ONE: u32 = 128;
/// What a cast shadow does to a colour: darker, and bluer.
pub const SHADOW: [f32; 3] = [0.66, 0.70, 0.86];
/// The coarsest the ground is drawn: a pixel `2^8` cells a side, a
/// chunk.
pub const COARSEST: usize = 8;
/// The coarsest of the levels dropped with the fine parts.
const FINE_LEVELS: usize = 2;

/// Heights the shadow line drops over one cell down the diagonal.
pub const fn shadow_drop() -> f32 {
    SHADOW_DROP
}

/// What a pixel's colour is drawn through: what it is multiplied by,
/// red, green and blue of [`FACTOR_ONE`], and before that what is laid
/// over it -- which of [`PALE`], [`SAND`] and [`FOAM`] in the last
/// byte's two high bits, how much of it in the six low.
#[derive(Clone, Copy)]
pub struct Shade([u8; 4]);

impl Shade {
    /// The bits saying how much is laid over.
    const PART_BITS: u32 = 6;
    /// All of it laid over.
    const WHOLE: u32 = (1 << Self::PART_BITS) - 1;
}

/// `colour` through `shade`.
pub fn lit(colour: [u8; 3], shade: Shade) -> [u8; 3] {
    let Shade([red, green, blue, over]) = shade;
    let (laid, part) = ([PALE, SAND, FOAM][(over >> Shade::PART_BITS) as usize % 3], over as u32 & Shade::WHOLE);
    let factor = [red, green, blue];
    std::array::from_fn(|channel| ((colour[channel] as u32 * (Shade::WHOLE - part) + laid[channel] as u32 * part) / Shade::WHOLE * factor[channel] as u32 / FACTOR_ONE).min(255) as u8)
}

/// What is kept of a superchunk only while it is seen from near: 3 MiB
/// and the two finest levels.
pub struct Fine {
    /// Its cells' heights, and [`MARGIN`] cells more all round, row by row.
    heights: Vec<Height>,
    /// How high the shadow line stands over each, laid out as the
    /// heights.
    lines: Vec<f32>,
    /// Each cell's light, of [`LIT_ONE`] -- its slope's -- and
    /// whether a shadow falls on it ([`SHADOWED`]).
    lit: Vec<u8>,
    /// The ocean's level, and the highest land.
    between: (Height, Height),
    /// The deepest the ocean is.
    deepest: f32,
}

impl Fine {
    /// Where the cell `(x, y)` from the top left is kept: [`MARGIN`]
    /// cells past each edge are.
    const fn kept(x: isize, y: isize) -> usize {
        (y + MARGIN as isize) as usize * KEPT + (x + MARGIN as isize) as usize
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

    /// How far the cell `(x, y)` is under the ocean's level, if it is.
    pub fn under_ocean(&self, x: isize, y: isize) -> Option<Height> {
        self.between.0.checked_sub(self.height(x, y)).filter(|&depth| depth > 0)
    }

    /// What a shadow falls on at the cell `(x, y)`: its ground, or the
    /// ocean over it.
    pub fn surface(&self, x: isize, y: isize) -> Height {
        self.height(x, y).max(self.between.0)
    }

    /// The deepest the ocean is.
    pub fn deepest(&self) -> f32 {
        self.deepest
    }

    /// The tint of the cell `(x, y)`'s height ([`relief::tint`]).
    pub fn tint(&self, x: isize, y: isize) -> ([f32; 3], f32) {
        tint(share(self.height(x, y), self.between))
    }
}

/// How far `height` is from the ocean's level to the highest land, the
/// two `between`.
fn share(height: Height, between: (Height, Height)) -> f32 {
    height.saturating_sub(between.0) as f32 / between.1.saturating_sub(between.0).max(1) as f32
}

/// A superchunk's ground.
pub struct Ground {
    /// What a pixel `2^detail` cells a side draws its colour through,
    /// a level a detail up to [`COARSEST`]. The two finest are empty
    /// once [`Self::fine`] is dropped.
    pub levels: Vec<Vec<Shade>>,
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
    /// in the world whose seed is `seed`, generated as `generation` says; the
    /// heights `given` taken as they are.
    pub fn generate(seed: u64, generation: &Generation, top_left: (u32, u32), given: &Given) -> Self {
        let heights = heights(seed, generation, top_left, given);
        let levels = levels(generation);
        let (between, deepest) = ((levels.ocean, levels.highest), levels.ocean.saturating_sub(levels.ground) as f32);
        let smooth = smoothed(&smoothed(&heights.iter().map(|&height| height as f32).collect::<Vec<_>>()));
        let (lines, shadowed) = shadow_lines(&heights);
        let to_coast = coast_distances(&heights.iter().map(|&height| height < levels.ocean).collect::<Vec<_>>());
        let at = |x: usize, y: usize| (y + BEFORE) * WIDE + x + BEFORE;
        // Each cell's light; and for the levels, what its colour is multiplied by, its height and how far the coast is.
        let mut lit = vec![0; SIDE * SIDE];
        let mut level = Level { side: SIDE, factors: Vec::with_capacity(SIDE * SIDE), lights: Vec::with_capacity(SIDE * SIDE), pale: Vec::with_capacity(SIDE * SIDE), heights: Vec::with_capacity(SIDE * SIDE), under: Vec::with_capacity(SIDE * SIDE), to_coast: Vec::with_capacity(SIDE * SIDE) };
        for y in 0..SIDE {
            for x in 0..SIDE {
                let here = at(x, y);
                let (across, down) = ((smooth[here + 1] - smooth[here - 1]) / 2.0, (smooth[here + WIDE] - smooth[here - WIDE]) / 2.0);
                let light = banded(slope_light(across, down), LIGHT_BAND);
                let under = heights[here] < levels.ocean;
                // A shadow falls on the water, not on the ground under it.
                let shadowed = if under { lines[here] > levels.ocean as f32 + 0.01 } else { shadowed[here] };
                lit[y * SIDE + x] = (light * LIT_ONE).round() as u8 | if shadowed { SHADOWED } else { 0 };
                // Water's light is its depth's; the land's its slope's, tinted by its height.
                let (factor, pale) = if under { (water_light((levels.ocean - heights[here]) as f32, deepest), 0.0) } else { (tint(share(heights[here], between)).0.map(|tint| tint * light), tint(share(heights[here], between)).1) };
                let shadow = if shadowed { SHADOW } else { [1.0; 3] };
                level.factors.push(std::array::from_fn(|channel| factor[channel] * shadow[channel]));
                level.lights.push(shadow.map(|shadow| shadow * light));
                level.pale.push(pale);
                level.heights.push(heights[here] as f32);
                level.under.push(under as u8 as f32);
                level.to_coast.push(to_coast[here]);
            }
        }
        let mut levels = Vec::with_capacity(COARSEST + 1);
        for detail in 0..=COARSEST {
            levels.push(level.drawn(detail));
            level = level.halved();
        }
        // What is kept: the superchunk's cells and the margin all round.
        let kept = || (0..KEPT * KEPT).map(|index| (index / KEPT + BEFORE - MARGIN) * WIDE + index % KEPT + BEFORE - MARGIN);
        let fine = Fine { heights: kept().map(|index| heights[index]).collect(), lines: kept().map(|index| lines[index]).collect(), lit, between, deepest };
        Self { levels, fine: Some(fine), used: 0 }
    }
}

/// Heights already worked out: a superchunk's height words
/// ([`height_in_frame`]) by its top left cell.
pub type Given<'a> = HashMap<(u32, u32), &'a [u64]>;

/// The heights about the superchunk whose top left cell is `top_left`:
/// [`WIDE`] a side, row by row. Those of a superchunk `given` are read;
/// the others are worked out from the seed.
fn heights(seed: u64, generation: &Generation, top_left: (u32, u32), given: &Given) -> Vec<Height> {
    let (left, top) = (top_left.0.wrapping_sub(BEFORE as u32), top_left.1.wrapping_sub(BEFORE as u32));
    // Rows shared out among the machine's threads: a cell's height is the same whoever works it out.
    let mut heights = vec![0; WIDE * WIDE];
    let threads = std::thread::available_parallelism().map_or(1, |threads| threads.get());
    let rows_each = WIDE.div_ceil(threads);
    std::thread::scope(|scope| {
        for (part, rows) in heights.chunks_mut(rows_each * WIDE).enumerate() {
            scope.spawn(move || {
                let mut seen = HeightsSeen::of(generation, seed);
                for (row, heights) in rows.chunks_mut(WIDE).enumerate() {
                    let (y, mut across) = (top.wrapping_add((part * rows_each + row) as u32), 0);
                    while across < WIDE {
                        // The row's run within one superchunk.
                        let x = left.wrapping_add(across as u32);
                        let (in_x, in_y) = (x % SIDE as u32, y % SIDE as u32);
                        let run = (SIDE - in_x as usize).min(WIDE - across);
                        let cells = heights[across..across + run].iter_mut().zip(0u32..);
                        match given.get(&(x - in_x, y - in_y)) {
                            Some(words) => cells.for_each(|(height, along)| *height = height_in_frame(words, place_from_cartesian(in_x + along, in_y))),
                            None => cells.for_each(|(height, along)| *height = seen.height(x.wrapping_add(along), y)),
                        }
                        across += run;
                    }
                }
            });
        }
    });
    heights
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two superchunks side by side, and one below: the shadow lines
    /// over the cells they both keep are the same from either; and the
    /// heights a frame brings are the ones worked out from the seed.
    #[test]
    fn shadows_are_the_same_from_both_sides_of_an_edge() {
        let generation = Generation::DEFAULT;
        let seed = server::host::terrain_seen::seed_with_land(utilities::seed::counted(), &generation, coordinates::WORLD_MIDDLE);
        let middle = coordinates::WORLD_MIDDLE.top_left().cartesian();
        let (left, top, side) = (middle.x, middle.y, SIDE as u32);
        let fine = |top_left: (u32, u32), given: &Given| Ground::generate(seed, &generation, top_left, given).fine.expect("made fine");
        let (here, beside, below) = (fine((left, top), &Given::new()), fine((left + side, top), &Given::new()), fine((left, top + side), &Given::new()));
        let mut differing = 0;
        for along in 0..SIDE as isize {
            for off in 0..MARGIN as isize {
                differing += (here.line(SIDE as isize + off, along) != beside.line(off, along)) as usize;
                differing += (here.line(along, SIDE as isize + off) != below.line(along, off)) as usize;
            }
        }
        assert_eq!(differing, 0, "shadow lines unlike over an edge");
        let world = server::start(server::Start { seed, sheep: 1, ..server::Start::default() });
        let image = world.storage.image(coordinates::WORLD_MIDDLE).expect("the origin's image");
        let brought = fine((left, top), &[((left, top), image.height_words())].into_iter().collect());
        let cells = || (0..SIDE as isize).flat_map(|y| (0..SIDE as isize).map(move |x| (x, y)));
        assert!(cells().all(|(x, y)| brought.height(x, y) == here.height(x, y) && brought.line(x, y) == here.line(x, y)), "heights brought unlike those worked out");
    }
}
