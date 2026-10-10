//! Civil Egregore's world generation: every cell's height, from the
//! world's seed and where the cell is alone ([`height`]); the walls
//! heights make ([`Terrain`]); how what grows lies ([`patches`]); how a
//! world is generated as a whole ([`generation`]).
//!
//! The design: `docs/worldgen.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod diagnostics;
pub mod transient_data;

pub mod generated_superchunk;
pub mod generation;
pub mod mesh;
pub mod patches;

pub use generated_superchunk::generate_superchunk;
pub use generation::{has_land_about, seed_with_land, Generation, Growth, TREES_SALT};

use bitmap::{CellWords, BITS_PER_WORD, WORDS};
use chunk_storage::{Height, HeightMap, LayerType};
use type_registry::{WALL_EAST, WALL_SOUTH};
use coordinates::{cartesian_from_place, place_from_cartesian, CellCartesian, SuperchunkIndex, CELLS_IN_CHUNK, CHUNKS_IN_SUPERCHUNK, SUPERCHUNK_SIDE_CELLS};
use utilities::hash::{mix, GOLDEN_RATIO};

/// The most two cells beside one another may differ in height and still
/// be stepped between.
pub const STEP: Height = 1;

/// The walls' layers, and the neighbour each is towards.
pub const WALLS: [(LayerType, (i32, i32)); 2] = [(WALL_EAST, (1, 0)), (WALL_SOUTH, (0, 1))];

/// How the heights are shaped: the land as a mesh ([`mesh`]) --
/// vertices that carry heights, each ocean, at the lowest ground, or
/// land, at a height of its own over the ocean's; and lines between
/// them that carry how those heights are blended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    /// The height the lowest ground is at: the ocean's floor.
    pub ground: Height,
    /// The height the ocean stands at, all over the world.
    pub ocean: Height,
    /// The cells along a square of the vertices' grid, as a power of
    /// two, 6 to 24: about a line's length.
    pub span: u32,
    /// The share of the vertices that are ocean, of [`ONE`].
    pub sea: u64,
    /// The height the highest land vertex may be at: each is between
    /// just over the ocean and this.
    pub highest: Height,
    /// How much smooth noise, some vertices broad, counts beside a
    /// vertex's own lot in whether it is land or ocean, of [`ONE`]: how
    /// much land and ocean clump. The more, the less exactly the
    /// ocean's share is kept.
    pub clumping: u64,
    /// The vertices from the ocean within which land is held low: 4
    /// at most; 0, and none is.
    pub coast: u32,
    /// How low land beside the ocean is held, of
    /// [`mesh::SIGMOID_ONE`]: the power its lot is raised to, so that
    /// the higher a height the less likely. One, and it is not held;
    /// less each vertex from the ocean, to one past the coast.
    pub coast_low: u64,
    /// The least share of its length a line's blend is, of [`ONE`]:
    /// little, and the line is two plains and a cliff
    /// (`docs/worldgen.md`, "The land as a mesh").
    pub narrow: u64,
    /// The most share of its length a line's blend is.
    pub wide: u64,
    /// The least a line's sigmoidness is, of [`mesh::SIGMOID_ONE`]:
    /// each line has one of its own, by lot, from this to the most.
    /// One is an even slope across the blend; more, two levels and a
    /// step between them.
    pub soft: u64,
    /// The most a line's sigmoidness is: 16 at most.
    pub hard: u64,
    /// How far the lines are bent, beside a square's side, of [`ONE`].
    pub warp: u64,
    /// The finer meshes on the land, each with vertices half as far
    /// apart as the one before: 10 at most, and none finer than 16
    /// cells between vertices.
    pub finer_depth: u32,
    /// The share of a finer mesh's vertices that raise or sink the
    /// land, of [`ONE`].
    pub finer_share: u64,
    /// The most a vertex of the broadest of them raises or sinks it,
    /// in heights.
    pub finer_height: u64,
    /// How much of that each mesh finer does, beside the one before,
    /// of [`ONE`]: never less than two heights, so every mesh counts.
    pub finer_fall: u64,
    /// How unevenly the weight that reaches a vertex is shared out, of
    /// [`ONE`]: each takes a share of its own, a byte by lot, from one
    /// less this to one. None, and every vertex has the whole weight.
    pub weight: u64,
    /// The share of those that raise it, of [`ONE`]: the rest sink it.
    pub raised: u64,
}

impl Shape {
    /// The world's shape: vertices 8 superchunks apart, half of them
    /// ocean 255 deep, the land to 200
    /// over it (711); lines blended over a quarter of their length to
    /// all of it, from even slopes to gentle steps; finer meshes on the land down to lines 16 cells long.
    pub const DEFAULT: Self = Self { ground: 256, ocean: 511, span: 13, sea: ONE / 2, highest: 711, clumping: ONE / 4, coast: 2, coast_low: 5 * mesh::SIGMOID_ONE / 2, narrow: ONE / 4, wide: ONE, soft: mesh::SIGMOID_ONE, hard: 3 * mesh::SIGMOID_ONE, warp: ONE * 3 / 10, finer_depth: 9, finer_share: ONE * 7 / 10, finer_height: 120, finer_fall: ONE * 3 / 4, weight: ONE * 4 / 5, raised: ONE * 3 / 5 };
}

/// One: a fraction's whole, 16 bits.
pub const ONE: u64 = 1 << 16;

/// A number settled by `seed`, an octave and a point of it: 16 bits.
fn point(seed: u64, octave: u32, x: u32, y: u32) -> u64 {
    mix(seed ^ (octave as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93) ^ ((x as u64) << 32 | y as u64).wrapping_mul(GOLDEN_RATIO)) >> 48
}

/// `from` to `to`, `along` of [`ONE`] of the way.
fn between(from: u64, to: u64, along: u64) -> u64 {
    (from * (ONE - along) + to * along) >> 16
}

/// Smooth noise, of [`ONE`]: at a cell, the four points about it of
/// a grid `2^shift` cells apart, each a number settled by `seed` and
/// `index`, eased between -- what bends the mesh's lines, and what
/// lies in patches lies by. Asked cell after cell, it keeps the points
/// about the last: the next is nearly always among the same.
#[derive(Clone, Copy, Debug)]
pub struct Noise {
    /// The world's seed.
    seed: u64,
    /// Which noise of the world's it is.
    index: u32,
    /// The cells from a point to the next, as a power of two.
    shift: u32,
    /// The square of the grid the points kept are about, if any are.
    square: Option<(u32, u32)>,
    /// Its points: top left, top right, bottom left, bottom right.
    points: [u64; 4],
}

impl Noise {
    /// The noise numbered `index` of the world of `seed`, its points
    /// `2^shift` cells apart.
    pub const fn new(seed: u64, index: u32, shift: u32) -> Self {
        Self { seed, index, shift, square: None, points: [0; 4] }
    }

    /// The noise at the cell `(x, y)`.
    pub fn at(&mut self, x: u32, y: u32) -> u64 {
        let (left, top) = (x >> self.shift, y >> self.shift);
        if self.square != Some((left, top)) {
            let point = |east: u32, south: u32| point(self.seed, self.index, left.wrapping_add(east), top.wrapping_add(south));
            (self.square, self.points) = (Some((left, top)), [point(0, 0), point(1, 0), point(0, 1), point(1, 1)]);
        }
        let ease = |within: u32| {
            let along = ((within as u64) << 16) >> self.shift;
            // Smoothstep: no crease at an octave's points.
            (along * along * (3 * ONE - 2 * along)) >> 32
        };
        let (across, down) = (ease(x & ((1 << self.shift) - 1)), ease(y & ((1 << self.shift) - 1)));
        between(between(self.points[0], self.points[1], across), between(self.points[2], self.points[3], across), down)
    }
}

/// [`Noise`] at the cell `(x, y)`, for one cell alone.
pub fn noise(seed: u64, index: u32, shift: u32, x: u32, y: u32) -> u64 {
    Noise::new(seed, index, shift).at(x, y)
}

/// The height of the cell at `(x, y)` of the world whose seed is `seed`:
/// whole numbers only, so the same on any machine.
pub fn height(seed: u64, x: u32, y: u32) -> Height {
    height_shaped(&Shape::DEFAULT, seed, x, y)
}

/// [`height`], in a world shaped as `shape` says: what a shape is tried
/// out with before it is the world's.
pub fn height_shaped(shape: &Shape, seed: u64, x: u32, y: u32) -> Height {
    mesh::Lands::new(shape, seed).height(x, y)
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
        // With a cell more all round, as the walls are told from.
        let wide = SUPERCHUNK_SIDE_CELLS + 2;
        let heights = mesh::Lands::new(shape, seed).heights_of_a_square(left.wrapping_sub(1), top.wrapping_sub(1), wide);
        Self::from_heights(|x, y| heights[((y + 1) as u32 * wide + (x + 1) as u32) as usize])
    }

    /// The terrain where `height_at` gives the height of the cell `x`
    /// across and `y` down from the superchunk's top left -- and of the
    /// cells a step past its edges, whose walls with its own are its
    /// neighbours' business too.
    pub fn from_heights(mut height_at: impl FnMut(i32, i32) -> Height) -> Self {
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
