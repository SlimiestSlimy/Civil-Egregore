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

pub mod polygons;

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

/// How the heights are shaped: the land as polygons ([`polygons`]) --
/// closed shapes that share borders, each ocean, its ground the
/// lowest there is, or land, a plain at a level of its own over the
/// ocean's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    /// The height the lowest ground is at: the ocean's floor.
    pub ground: Height,
    /// The height the ocean stands at, all over the world.
    pub ocean: Height,
    /// The cells along a square of the polygons' sites' grid, as a
    /// power of two, 6 to 24: about a polygon's breadth.
    pub span: u32,
    /// The share of the polygons that are ocean, of [`ONE`].
    pub sea: u64,
    /// The most a land polygon's plain stands over the ocean, in heights.
    pub levels: u64,
    /// The cells from a border over which the levels about it are
    /// mixed: half a square's side at most.
    pub edge: u64,
    /// How far the borders are bent, beside a square's side, of [`ONE`].
    pub warp: u64,
}

/// A cell's water: how deep it stands over the ground, 0 none, a number
/// over eight bitplanes, the lowest bit first.
pub const WATER: [LayerType; 8] = [LayerType(24), LayerType(25), LayerType(26), LayerType(27), LayerType(28), LayerType(29), LayerType(30), LayerType(31)];

impl Shape {
    /// The world's shape: polygons 8 superchunks across, half of them
    /// ocean 255 deep -- as deep as water is kept -- the plains to 200
    /// over it, joined by ramps 2,048 cells from a border.
    pub const DEFAULT: Self = Self { ground: 256, ocean: 511, span: 13, sea: ONE / 2, levels: 200, edge: 2048, warp: ONE * 3 / 10 };
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
/// settled by `seed` and `index`, eased between: what bends the
/// polygons' borders, and what lies in patches lies by.
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

/// The height of the cell at `(x, y)` of the world whose seed is `seed`:
/// whole numbers only, so the same on any machine.
pub fn height(seed: u64, x: u32, y: u32) -> Height {
    height_shaped(&Shape::DEFAULT, seed, x, y)
}

/// [`height`], in a world shaped as `shape` says: what a shape is tried
/// out with before it is the world's.
pub fn height_shaped(shape: &Shape, seed: u64, x: u32, y: u32) -> Height {
    // The highest there is, whatever the shape would come to.
    polygons::land(shape, seed, x, y).min(Height::MAX as u64) as Height
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
