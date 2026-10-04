//! TileSim's terrain: every cell's height, from the world's seed and
//! where the cell is, and nothing else ([`height`]) -- so a superchunk
//! is the same whenever it is generated, and meets its neighbours with
//! no seam -- and the **walls**: two cells beside one another, across
//! or down, more than [`STEP`] apart in height cannot be stepped
//! between ([`Terrain`]). A diagonal step has no wall of its own: it is
//! open only when both ways round it, across then down and down then
//! across, are.
//!
//! The design: `docs/terrain.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod diagnostics;
pub mod transient_data;

use bitmap::{CellWords, BITS_PER_WORD, WORDS};
use chunk_storage::{Height, HeightMap, LayerType};
use coordinates::{cartesian_from_place, place_from_cartesian, CellCartesian, SuperchunkIndex, CELLS_IN_CHUNK, CHUNKS_IN_SUPERCHUNK, SUPERCHUNK_SIDE_CELLS};
use utilities::hash::{mix, GOLDEN_RATIO};

/// The most two cells beside one another may differ in height and still
/// be stepped between.
pub const STEP: Height = 1;

/// A wall between a cell and the cell to its east: the layer of the
/// cells that keep one.
pub const WALL_EAST: LayerType = LayerType(8);
/// ...to its south.
pub const WALL_SOUTH: LayerType = LayerType(9);

/// The walls' layers, and the neighbour each is towards.
pub const WALLS: [(LayerType, (i32, i32)); 2] = [(WALL_EAST, (1, 0)), (WALL_SOUTH, (0, 1))];

/// The hills' octaves, each the cells between two of its points as a
/// power of two, and the number its noise is drawn by: every one from
/// hills eight superchunks across to rough ground, so that no one
/// octave's grid shows through.
const OCTAVES: [(u32, u32); 11] = [(13, 23), (12, 22), (11, 21), (10, 20), (9, 0), (8, 1), (7, 2), (6, 3), (5, 4), (4, 5), (3, 6)];
/// The number the first octave of the land's rise is drawn by.
const RISE_INDEX: u32 = 7;
/// The number the first octave of the shore's wandering is drawn by.
const SHORE_INDEX: u32 = 12;

/// How the heights are shaped. The land rises and falls by far more
/// than a hill over many superchunks ([`rise`]), too gently for a wall;
/// what of it is under the ocean's level is the ocean's floor, what is
/// over it islands, dozens to hundreds of superchunks each. Hills
/// stand on the islands, rising from nothing to their whole height
/// over the coast's heights of land -- from the shore on average, but
/// from under the ocean here and from well inland there, as broad
/// noise says, so that no level band rings an island. The hills' octaves' shares are heights: together, the highest a hill stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    /// Each hill octave's share of a height, the broadest first.
    pub weights: [u64; 11],
    /// The height the ocean stands at, all over the world.
    pub ocean: Height,
    /// How far under the ocean its deepest floor is: the land under
    /// the ocean falls to that, not to the lowest ground.
    pub depth: u64,
    /// How far over the ocean the land is where the hills are whole.
    pub coast: u64,
    /// The height the lowest ground is at: what all the rest stands on.
    pub ground: Height,
    /// The most the land rises over the lowest ground, in heights.
    pub rise: u64,
    /// The cells between two points of the land's rise, as a power of
    /// two: at most 24.
    pub rise_span: u32,
    /// The share each octave of the land's rise has, the broadest
    /// first, each half as broad as the one before.
    pub rise_shares: [u64; 5],
    /// How far where the hills begin is moved up or down about the
    /// shore, beside the coast's heights, of [`ONE`].
    pub shore: u64,
    /// The cells between two points of the noise that moves it, as a
    /// power of two: at least 2.
    pub shore_span: u32,
}

/// A cell's water: how deep it stands over the ground, 0 none, a number
/// over eight bitplanes, the lowest bit first.
pub const WATER: [LayerType; 8] = [LayerType(24), LayerType(25), LayerType(26), LayerType(27), LayerType(28), LayerType(29), LayerType(30), LayerType(31)];

impl Shape {
    /// The world's shape: islands some 16 superchunks across in an
    /// ocean a little over half the world, hills as tuned by eye in the
    /// renderer's lab.
    pub const DEFAULT: Self = Self { weights: [0, 0, 0, 0, 99, 58, 14, 25, 37, 20, 2], ocean: 800, depth: 255, coast: 64, ground: 256, rise: 1024, rise_span: 14, rise_shares: [625, 250, 100, 40, 16], shore: ONE, shore_span: 10 };
}

/// One: a fraction's whole, 16 bits.
const ONE: u64 = 1 << 16;

/// A number settled by `seed`, an octave and a point of it: 16 bits.
fn point(seed: u64, octave: u32, x: u32, y: u32) -> u64 {
    mix(seed ^ (octave as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93) ^ ((x as u64) << 32 | y as u64).wrapping_mul(GOLDEN_RATIO)) >> 48
}

/// `from` to `to`, `along` of [`ONE`] of the way.
fn between(from: u64, to: u64, along: u64) -> u64 {
    (from * (ONE - along) + to * along) >> 16
}

/// Smooth noise at the cell `(x, y)`, of [`ONE`]: the four points
/// about the cell of a grid `2^shift` cells apart, each a number
/// settled by `seed` and `index`, eased between. One octave of a height;
/// and what else is to lie in patches.
pub fn noise(seed: u64, index: u32, shift: u32, x: u32, y: u32) -> u64 {
    let (left, top) = (x >> shift, y >> shift);
    let ease = |within: u32| {
        let along = ((within as u64) << 16) >> shift;
        // Smoothstep: no crease at an octave's points.
        (along * along * (3 * ONE - 2 * along)) >> 32
    };
    let (across, down) = (ease(x & ((1 << shift) - 1)), ease(y & ((1 << shift) - 1)));
    let upper = between(point(seed, index, left, top), point(seed, index, left.wrapping_add(1), top), across);
    let lower = between(point(seed, index, left, top.wrapping_add(1)), point(seed, index, left.wrapping_add(1), top.wrapping_add(1)), across);
    between(upper, lower, down)
}

/// How far the land has risen over the lowest ground at the cell
/// `(x, y)`, in heights: noise far broader than a superchunk and far
/// higher than a hill, five octaves of it ([`Shape::rise_shares`]):
/// the finer add shape to a shore.
pub fn rise(shape: &Shape, seed: u64, x: u32, y: u32) -> u64 {
    let index = RISE_INDEX;
    let shares = &shape.rise_shares;
    let span = shape.rise_span.clamp(shares.len() as u32, 24);
    let land: u64 = shares.iter().enumerate().map(|(octave, share)| share * noise(seed, index + octave as u32, span - octave as u32, x, y)).sum();
    (land / shares.iter().sum::<u64>().max(1) * shape.rise) >> 16
}

/// The height of the cell at `(x, y)` of the world whose seed is `seed`:
/// whole numbers only, so the same on any machine.
pub fn height(seed: u64, x: u32, y: u32) -> Height {
    height_shaped(&Shape::DEFAULT, seed, x, y)
}

/// [`height`], in a world shaped as `shape` says: what a shape is tried
/// out with before it is the world's.
pub fn height_shaped(shape: &Shape, seed: u64, x: u32, y: u32) -> Height {
    let land = shape.ground as u64 + rise(shape, seed, x, y);
    // Under the ocean the land falls gently: to the ocean's depth where it would have been the lowest ground.
    let (ocean, lowest) = (shape.ocean as u64, shape.ground as u64);
    let land = if land < ocean { ocean - (ocean - land) * shape.depth.min(ocean - lowest) / (ocean - lowest).max(1) } else { land };
    // Where the hills begin: at the ocean's level, moved up or down by as much as this.
    let index = SHORE_INDEX;
    let (span, wander) = (shape.shore_span.clamp(2, 24), (shape.coast * shape.shore) >> 16);
    let moved = ((noise(seed, index, span, x, y) + noise(seed, index + 1, span - 2, x, y)) * wander) >> 16;
    // How much of the hills stands here: none under where they begin.
    let relief = (((land + moved).saturating_sub(shape.ocean as u64 + wander) << 16) / shape.coast.max(1)).min(ONE);
    if relief == 0 {
        return land.min(Height::MAX as u64) as Height;
    }
    let hills: u64 = OCTAVES.iter().zip(shape.weights).filter(|&(_, weight)| weight > 0).map(|(&(shift, index), weight)| noise(seed, index, shift, x, y) * weight).sum();
    // The highest there is, whatever the shape would come to.
    (land + (((hills * relief) >> 16) >> 16)).min(Height::MAX as u64) as Height
}

/// Whether two heights are too far apart to step between.
pub const fn wall(a: Height, b: Height) -> bool {
    a.abs_diff(b) > STEP
}

/// A superchunk's terrain: its heights, and its walls -- for each of
/// the two ways ([`WALLS`]), each chunk's cells that keep one.
pub struct Terrain {
    /// Every cell's height.
    pub heights: HeightMap,
    /// The walls' bitmaps: a way, then a chunk in Morton order.
    pub walls: [Box<[CellWords; CHUNKS_IN_SUPERCHUNK]>; 2],
}

impl Terrain {
    /// The terrain of `superchunk` in the world whose seed is `seed`.
    pub fn generate(seed: u64, superchunk: SuperchunkIndex) -> Self {
        Self::generate_shaped(&Shape::DEFAULT, seed, superchunk)
    }

    /// [`Terrain::generate`], in a world shaped as `shape` says.
    pub fn generate_shaped(shape: &Shape, seed: u64, superchunk: SuperchunkIndex) -> Self {
        let CellCartesian { x: left, y: top } = superchunk.top_left().cartesian();
        Self::from_heights(|x, y| height_shaped(shape, seed, left.wrapping_add_signed(x), top.wrapping_add_signed(y)))
    }

    /// The terrain where `height_at` gives the height of the cell `x`
    /// across and `y` down from the superchunk's top left -- and of the
    /// cells a step past its edges, whose walls with its own are its
    /// neighbours' business too.
    pub fn from_heights(height_at: impl Fn(i32, i32) -> Height) -> Self {
        let side = SUPERCHUNK_SIDE_CELLS as i32;
        // The heights with a cell more all round, row by row: each read once.
        let wide = side as usize + 2;
        let mut grid = vec![0; wide * wide];
        for y in -1..=side {
            for x in -1..=side {
                grid[(y + 1) as usize * wide + (x + 1) as usize] = height_at(x, y);
            }
        }
        let at = |x: i32, y: i32| grid[(y + 1) as usize * wide + (x + 1) as usize];
        let heights = HeightMap::from_heights(|place| {
            let (x, y) = cartesian_from_place(place);
            at(x as i32, y as i32)
        });
        let mut walls: [Box<[CellWords; CHUNKS_IN_SUPERCHUNK]>; 2] = std::array::from_fn(|_| Box::new([[0; WORDS]; CHUNKS_IN_SUPERCHUNK]));
        for y in 0..side {
            for x in 0..side {
                let place = place_from_cartesian(x as u32, y as u32);
                let here = at(x, y);
                let (chunk, cell) = (place / CELLS_IN_CHUNK, place % CELLS_IN_CHUNK);
                for (way, &(_, (dx, dy))) in WALLS.iter().enumerate() {
                    if wall(here, at(x + dx, y + dy)) {
                        walls[way][chunk][cell / BITS_PER_WORD] |= 1 << (cell % BITS_PER_WORD);
                    }
                }
            }
        }
        Self { heights, walls }
    }

    /// The height of the cell at `place` in the superchunk.
    pub fn height(&self, place: usize) -> Height {
        self.heights.get(place)
    }

    /// Whether the cell at `place` in the superchunk keeps a wall the
    /// `way`-th way of [`WALLS`].
    pub fn walled(&self, way: usize, place: usize) -> bool {
        let (chunk, cell) = (place / CELLS_IN_CHUNK, place % CELLS_IN_CHUNK);
        self.walls[way][chunk][cell / BITS_PER_WORD] >> (cell % BITS_PER_WORD) & 1 == 1
    }

    /// How many walls it has, each of the two ways.
    pub fn wall_counts(&self) -> [u64; 2] {
        std::array::from_fn(|way| self.walls[way].iter().flatten().map(|word| word.count_ones() as u64).sum())
    }
}
