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

pub mod climate;

pub mod generated_superchunk;
pub mod generation;
pub mod mesh;
pub mod patches;
pub mod world_generation;

pub use climate::{ClimateNoises, CLIMATE_SALT};
pub use generated_superchunk::generate_superchunk;
pub use generation::{has_land_about, seed_with_land, WorldGeneration, WorldGrowth, FLOCK, TREES_SALT};

use utilities::bitmap::{CellWords, BITS_PER_WORD, WORDS};
use chunk_storage::LayerType;
use base_types::height_map::{Height, HeightMap};
use base_types::{WALL_EAST, WALL_NORTH, WALL_SOUTH, WALL_WEST};
use utilities::coordinates::{cartesian_from_place, place_from_cartesian, CellCartesian, SuperchunkIndex, CELLS_IN_CHUNK, CHUNKS_IN_SUPERCHUNK, SUPERCHUNK_SIDE_CELLS};
use utilities::hash::{mix_word, GOLDEN_RATIO};

/// The most a cell may be higher than the one beside it and still be
/// stepped up to from it.
pub const STEP: Height = 1;
/// The most a cell may be lower than the one beside it and still be
/// stepped down to from it: a hop down, further than a step up.
pub const DROP: Height = 4;

/// The walls' layers, and the neighbour each is towards.
pub const WALLS: [(LayerType, (i32, i32)); 4] = [(WALL_EAST, (1, 0)), (WALL_SOUTH, (0, 1)), (WALL_WEST, (-1, 0)), (WALL_NORTH, (0, -1))];

/// How the heights are shaped: the land as a mesh ([`mesh`]) --
/// vertices that carry heights, each ocean, at the lowest ground, or
/// land, at a height of its own over the ocean's; and lines between
/// them that carry how those heights are blended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LandShape {
    /// The height the lowest ground is at: the ocean's floor.
    pub ocean_floor: Height,
    /// The height the ocean stands at, all over the world.
    pub ocean: Height,
    /// The cells along a square of the vertices' grid, as a power of
    /// two, 6 to 24: about a line's length.
    pub vertex_spacing_power: u32,
    /// The share of the vertices that are ocean, of [`FRACTION_ONE`].
    pub ocean_share: u64,
    /// The height the highest land vertex may be at: each is between
    /// just over the ocean and this.
    pub highest_land: Height,
    /// How much smooth noise, some vertices broad, counts beside a
    /// vertex's own lot in whether it is land or ocean, of [`FRACTION_ONE`]: how
    /// much land and ocean clump. The more, the less exactly the
    /// ocean's share is kept.
    pub land_clumping: u64,
    /// The vertices from the ocean within which land is held low: 4
    /// at most; 0, and none is.
    pub coast_breadth: u32,
    /// How low land beside the ocean is held, of
    /// [`mesh::SIGMOID_ONE`]: the power its lot is raised to, so that
    /// the higher a height the less likely. One, and it is not held;
    /// less each vertex from the ocean, to one past the coast.
    pub coast_low: u64,
    /// The least share of its length a line's blend is, of [`FRACTION_ONE`]:
    /// little, and the line is two plains and a cliff
    /// (`docs/worldgen.md`, "The land as a mesh").
    pub narrowest_blend: u64,
    /// The most share of its length a line's blend is.
    pub widest_blend: u64,
    /// The least a line's sigmoidness is, of [`mesh::SIGMOID_ONE`]:
    /// each line has one of its own, by lot, from this to the most.
    /// One is an even slope across the blend; more, two levels and a
    /// step between them.
    pub least_sigmoidness: u64,
    /// The most a line's sigmoidness is: 16 at most.
    pub most_sigmoidness: u64,
    /// How far the lines are bent, beside a square's side, of [`FRACTION_ONE`].
    pub line_bending: u64,
    /// The finer meshes on the land, each with vertices half as far
    /// apart as the one before: 10 at most, and none finer than 16
    /// cells between vertices.
    pub finer_depth: u32,
    /// The share of a finer mesh's vertices that raise or sink the
    /// land, of [`FRACTION_ONE`].
    pub finer_share: u64,
    /// The most a vertex of the broadest of them raises or sinks it,
    /// in heights.
    pub finer_height: u64,
    /// How much of that each mesh finer does, beside the one before,
    /// of [`FRACTION_ONE`]: never less than two heights, so every mesh counts.
    pub finer_fall: u64,
    /// How unevenly the weight that reaches a vertex is shared out, of
    /// [`FRACTION_ONE`]: each takes a share of its own, a byte by lot, from one
    /// less this to one. None, and every vertex has the whole weight.
    pub weight: u64,
    /// The share of those that raise it, of [`FRACTION_ONE`]: the rest sink it.
    pub raised_share: u64,
}

impl LandShape {
    /// The world's shape: vertices 8 superchunks apart, half of them
    /// ocean 255 deep, the land to 200
    /// over it (711); lines blended over a quarter of their length to
    /// all of it, from even slopes to gentle steps; finer meshes on the land down to lines 16 cells long.
    pub const DEFAULT: Self = Self { ocean_floor: 256, ocean: 511, vertex_spacing_power: 13, ocean_share: FRACTION_ONE / 2, highest_land: 711, land_clumping: FRACTION_ONE / 4, coast_breadth: 2, coast_low: 5 * mesh::SIGMOID_ONE / 2, narrowest_blend: FRACTION_ONE / 4, widest_blend: FRACTION_ONE, least_sigmoidness: mesh::SIGMOID_ONE, most_sigmoidness: 3 * mesh::SIGMOID_ONE, line_bending: FRACTION_ONE * 3 / 10, finer_depth: 9, finer_share: FRACTION_ONE * 7 / 10, finer_height: 120, finer_fall: FRACTION_ONE * 3 / 4, weight: FRACTION_ONE * 4 / 5, raised_share: FRACTION_ONE * 3 / 5 };
}

/// One: a fraction's whole, 16 bits.
pub const FRACTION_ONE: u64 = 1 << 16;

/// A number settled by `seed`, an octave and a point of it: 16 bits.
fn point_number_cartesian(seed: u64, octave_index: u32, x: u32, y: u32) -> u64 {
    mix_word(seed ^ (octave_index as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93) ^ ((x as u64) << 32 | y as u64).wrapping_mul(GOLDEN_RATIO)) >> 48
}

/// `from_value` to `to_value`, `along_share` of [`FRACTION_ONE`] of the way.
fn value_between(from_value: u64, to_value: u64, along_share: u64) -> u64 {
    (from_value * (FRACTION_ONE - along_share) + to_value * along_share) >> 16
}

/// Smooth noise, of [`FRACTION_ONE`]: at a cell, the four points about it of
/// a grid `2^shift` cells apart, each a number settled by `seed` and
/// `index`, eased between -- what bends the mesh's lines, and what
/// lies in patches lies by. Asked cell after cell, it keeps the points
/// about the last: the next is nearly always among the same.
#[derive(Clone, Copy, Debug)]
pub struct SmoothNoise {
    /// The world's seed.
    seed: u64,
    /// Which noise of the world's it is.
    noise_index: u32,
    /// The cells from a point to the next, as a power of two.
    spacing_shift: u32,
    /// The square of the grid the points kept are about, if any are.
    kept_square: Option<(u32, u32)>,
    /// Its points: top left, top right, bottom left, bottom right.
    kept_points: [u64; 4],
}

impl SmoothNoise {
    /// The noise numbered `noise_index` of the world of `seed`, its points
    /// `2^spacing_shift` cells apart.
    pub const fn new(seed: u64, noise_index: u32, spacing_shift: u32) -> Self {
        Self { seed, noise_index, spacing_shift, kept_square: None, kept_points: [0; 4] }
    }

    /// The noise at the cell `(x, y)`.
    pub fn noise_at(&mut self, x: u32, y: u32) -> u64 {
        let (square_left, square_top) = (x >> self.spacing_shift, y >> self.spacing_shift);
        if self.kept_square != Some((square_left, square_top)) {
            let corner_number = |east_offset: u32, south_offset: u32| point_number_cartesian(self.seed, self.noise_index, square_left.wrapping_add(east_offset), square_top.wrapping_add(south_offset));
            (self.kept_square, self.kept_points) = (Some((square_left, square_top)), [corner_number(0, 0), corner_number(1, 0), corner_number(0, 1), corner_number(1, 1)]);
        }
        let ease_share = |cells_within: u32| {
            let along_share = ((cells_within as u64) << 16) >> self.spacing_shift;
            // Smoothstep: no crease at an octave's points.
            (along_share * along_share * (3 * FRACTION_ONE - 2 * along_share)) >> 32
        };
        let (eased_across, eased_down) = (ease_share(x & ((1 << self.spacing_shift) - 1)), ease_share(y & ((1 << self.spacing_shift) - 1)));
        value_between(value_between(self.kept_points[0], self.kept_points[1], eased_across), value_between(self.kept_points[2], self.kept_points[3], eased_across), eased_down)
    }
}

/// [`SmoothNoise`] at the cell `(x, y)`, for one cell alone.
pub fn noise_at_cell_cartesian(seed: u64, noise_index: u32, spacing_shift: u32, x: u32, y: u32) -> u64 {
    SmoothNoise::new(seed, noise_index, spacing_shift).noise_at(x, y)
}

/// The height of the cell at `(x, y)` of the world whose seed is `seed`:
/// whole numbers only, so the same on any machine.
pub fn height(seed: u64, x: u32, y: u32) -> Height {
    height_shaped(&LandShape::DEFAULT, seed, x, y)
}

/// [`height`], in a world shaped as `land_shape` says: what a shape is tried
/// out with before it is the world's.
pub fn height_shaped(land_shape: &LandShape, seed: u64, x: u32, y: u32) -> Height {
    mesh::WorldLand::new(land_shape, seed).height(x, y)
}

/// Whether no step goes from a cell as high as `from_height` to one beside
/// it as high as `to_height`: more than a [`STEP`] up, or than a [`DROP`]
/// down. One way's: the step back is asked the other way round.
pub const fn wall(from_height: Height, to_height: Height) -> bool {
    if to_height > from_height { to_height - from_height > STEP } else { from_height - to_height > DROP }
}

/// A superchunk's terrain: its heights, and its walls -- for each of
/// the four ways ([`WALLS`]), each chunk's cells that keep one.
pub struct Terrain {
    /// Every cell's height.
    pub heights: HeightMap,
    /// The walls' bitmaps: a way, then a chunk in Morton order.
    pub walls: [Box<[CellWords; CHUNKS_IN_SUPERCHUNK]>; 4],
}

impl Terrain {
    /// The terrain of `superchunk` in the world whose seed is `seed`.
    pub fn generate_terrain(seed: u64, superchunk: SuperchunkIndex) -> Self {
        Self::generate_shaped(&LandShape::DEFAULT, seed, superchunk)
    }

    /// [`Terrain::generate_terrain`], in a world shaped as `land_shape` says.
    pub fn generate_shaped(land_shape: &LandShape, seed: u64, superchunk: SuperchunkIndex) -> Self {
        let CellCartesian { x: superchunk_left, y: superchunk_top } = superchunk.top_left().cartesian();
        // With a cell more all round, as the walls are told from.
        let side_with_margin = SUPERCHUNK_SIDE_CELLS + 2;
        let heights = mesh::WorldLand::new(land_shape, seed).heights_of_a_square(superchunk_left.wrapping_sub(1), superchunk_top.wrapping_sub(1), side_with_margin);
        Self::from_heights(|x, y| heights[((y + 1) as u32 * side_with_margin + (x + 1) as u32) as usize])
    }

    /// The terrain where `height_at` gives the height of the cell `x`
    /// across and `y` down from the superchunk's top left -- and of the
    /// cells a step past its edges, whose walls with its own are its
    /// neighbours' business too.
    pub fn from_heights(mut height_at: impl FnMut(i32, i32) -> Height) -> Self {
        let side_cells = SUPERCHUNK_SIDE_CELLS as i32;
        // The heights with a cell more all round, row by row: each read once.
        let side_with_margin = side_cells as usize + 2;
        let mut height_grid = vec![0; side_with_margin * side_with_margin];
        for y in -1..=side_cells {
            for x in -1..=side_cells {
                height_grid[(y + 1) as usize * side_with_margin + (x + 1) as usize] = height_at(x, y);
            }
        }
        let grid_height_cartesian = |x: i32, y: i32| height_grid[(y + 1) as usize * side_with_margin + (x + 1) as usize];
        let heights = HeightMap::from_heights(|place| {
            let (x, y) = cartesian_from_place(place);
            grid_height_cartesian(x as i32, y as i32)
        });
        let mut walls: [Box<[CellWords; CHUNKS_IN_SUPERCHUNK]>; 4] = std::array::from_fn(|_| Box::new([[0; WORDS]; CHUNKS_IN_SUPERCHUNK]));
        for y in 0..side_cells {
            for x in 0..side_cells {
                let place = place_from_cartesian(x as u32, y as u32);
                let height_here = grid_height_cartesian(x, y);
                let (chunk, cell) = (place / CELLS_IN_CHUNK, place % CELLS_IN_CHUNK);
                for (way, &(_, (dx, dy))) in WALLS.iter().enumerate() {
                    if wall(height_here, grid_height_cartesian(x + dx, y + dy)) {
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
    pub fn keeps_wall(&self, way: usize, place: usize) -> bool {
        let (chunk, cell) = (place / CELLS_IN_CHUNK, place % CELLS_IN_CHUNK);
        self.walls[way][chunk][cell / BITS_PER_WORD] >> (cell % BITS_PER_WORD) & 1 == 1
    }

    /// How many walls it has, each of the four ways.
    pub fn wall_counts(&self) -> [u64; 4] {
        std::array::from_fn(|way| self.walls[way].iter().flatten().map(|word| word.count_ones() as u64).sum())
    }
}
