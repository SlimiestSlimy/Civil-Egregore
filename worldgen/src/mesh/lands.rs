//! The land asked of a cell at a time: the mesh behind it, what it last
//! found kept.

use crate::{ONE, Shape, noise};
use super::{FINER_MOST, FINEST, Mesh, WARP_INDEX};
use chunk_storage::Height;

/// The cells from one point of the lattice to the next, as a power of
/// two: the meshes are worked out at its points alone, a sixteenth of
/// the cells, and no line of a mesh is shorter than four of its steps.
const LATTICE: u32 = 2;
/// The cells from one point of the lattice to the next.
const LATTICE_STEP: u32 = 1 << LATTICE;

/// The land `across` cells east and `down` south of the first of four
/// points of the lattice -- top left, top right, bottom left, bottom
/// right, each in 16-bit fractions of a height -- in heights: each
/// point's by how near it the cell is, made whole once.
fn between_the_lattice(points: [i64; 4], across: u32, down: u32) -> u64 {
    let (across, down, step) = (across as i64, down as i64, LATTICE_STEP as i64);
    let upper = points[0] * (step - across) + points[1] * across;
    let lower = points[2] * (step - across) + points[3] * across;
    let land = (upper * (step - down) + lower * down) >> (2 * LATTICE);
    ((land + (ONE / 2) as i64) >> 16) as u64
}

/// The land of a world, asked for cell after cell: the vertices about
/// the last cell and its triangle are kept, for the next is nearly
/// always in the same -- so they are drawn once for a square of a
/// grid, not once for a cell.
pub struct Lands {
    /// How the heights are shaped.
    shape: Shape,
    /// The world's seed.
    seed: u64,
    /// The cells along a square of the broadest grid, as a power of two.
    span: u32,
    /// Cells a line is bent by at most.
    bend: i64,
    /// The land's and the ocean's mesh.
    broad: Mesh,
    /// The finer meshes, the broadest first.
    finer: [Mesh; FINER_MOST],
}

impl Lands {
    /// The land of the world of `seed`, shaped as `shape` says.
    pub fn new(shape: &Shape, seed: u64) -> Self {
        let span = shape.span.clamp(FINEST, 24);
        Self { shape: *shape, seed, span, bend: (((1u64 << span) * shape.warp) >> 16) as i64, broad: Mesh::new(shape, 0), finer: std::array::from_fn(|finer| Mesh::new(shape, finer as u32 + 1)) }
    }

    /// The cell `(x, y)` moved by broad noise: what bends the lines.
    fn moved(&self, x: u32, y: u32) -> (i64, i64) {
        let moved = |index: u32| ((noise(self.seed, index, self.span - 2, x, y) as i64 - (ONE / 2) as i64) * 2 * self.bend) >> 16;
        (x as i64 + moved(WARP_INDEX), y as i64 + moved(WARP_INDEX + 1))
    }

    /// The land at the lattice's point `(x, y)`, in 16-bit fractions
    /// of a height: the broad mesh's height there, and what the finer
    /// meshes raise or sink it by, as much of it as the land there
    /// stands high -- never under the lowest ground.
    fn at_the_lattice(&mut self, x: u32, y: u32) -> i64 {
        let at = self.moved(x, y);
        let broad = self.broad.blended(&self.shape, self.seed, at);
        let lowest = (self.shape.ground as i64) << 16;
        if broad.inland == 0 {
            return (broad.height << 16).max(lowest);
        }
        // No finer than the finest: a mesh that would be is left out.
        let depth = (self.shape.finer_depth as usize).min(FINER_MOST).min((self.span - FINEST) as usize);
        // The broad mesh has the whole weight; each mesh hands a share of what reached it on to the next, and
        // moves the land by its own heights times what reached it.
        let (mut finer, mut free) = (0i64, (broad.inland * broad.free) >> 16);
        for mesh in self.finer.iter_mut().take(depth) {
            let blended = mesh.blended(&self.shape, self.seed, at);
            (finer, free) = (finer + blended.height * free as i64, (free * blended.free) >> 16);
        }
        ((broad.height << 16) + finer).max(lowest)
    }

    /// The land at the cell `(x, y)`, in heights: the meshes' at the
    /// lattice's four points about it, blended by how near each the
    /// cell is (`docs/worldgen.md`, "On a lattice").
    pub fn land(&mut self, x: u32, y: u32) -> u64 {
        let (left, top) = (x & !(LATTICE_STEP - 1), y & !(LATTICE_STEP - 1));
        let (across, down) = (x - left, y - top);
        // A point no share of the cell is taken from is not worked out.
        let mut point = |east: bool, south: bool| if (east && across == 0) || (south && down == 0) { 0 } else { self.at_the_lattice(left.wrapping_add(east as u32 * LATTICE_STEP), top.wrapping_add(south as u32 * LATTICE_STEP)) };
        between_the_lattice([point(false, false), point(true, false), point(false, true), point(true, true)], across, down)
    }

    /// The heights of the square of `side` cells whose top left cell
    /// is `(left, top)`, row by row: each what [`Lands::height`] says,
    /// the lattice's points worked out once each.
    pub fn heights_of_a_square(&mut self, left: u32, top: u32, side: u32) -> Vec<Height> {
        let (first_across, first_down) = (left & !(LATTICE_STEP - 1), top & !(LATTICE_STEP - 1));
        let points = ((left - first_across + side - 1) / LATTICE_STEP + 2) as usize;
        let mut lattice = Vec::with_capacity(points * points);
        for down in 0..points as u32 {
            for across in 0..points as u32 {
                lattice.push(self.at_the_lattice(first_across.wrapping_add(across * LATTICE_STEP), first_down.wrapping_add(down * LATTICE_STEP)));
            }
        }
        let mut heights = Vec::with_capacity((side * side) as usize);
        for y in 0..side {
            let (row, down) = (((top - first_down + y) / LATTICE_STEP) as usize, (top - first_down + y) % LATTICE_STEP);
            for x in 0..side {
                let (column, across) = (((left - first_across + x) / LATTICE_STEP) as usize, (left - first_across + x) % LATTICE_STEP);
                let here = row * points + column;
                let land = between_the_lattice([lattice[here], lattice[here + 1], lattice[here + points], lattice[here + points + 1]], across, down);
                heights.push(land.min(Height::MAX as u64) as Height);
            }
        }
        heights
    }

    /// The height of the cell `(x, y)`: its land, and no more than the
    /// highest there is, whatever the shape would come to.
    pub fn height(&mut self, x: u32, y: u32) -> Height {
        self.land(x, y).min(Height::MAX as u64) as Height
    }

    /// How high the land at the cell `(x, y)` stands of what it may, of
    /// [`ONE`], and about
    /// how many cells it is from the nearest line of the broad mesh.
    pub fn line(&mut self, x: u32, y: u32) -> (u64, u64) {
        let at = self.moved(x, y);
        let broad = self.broad.blended(&self.shape, self.seed, at);
        (broad.inland, broad.line)
    }
}

/// The land at the cell `(x, y)`: [`Lands::land`], for one cell alone.
pub fn land(shape: &Shape, seed: u64, x: u32, y: u32) -> u64 {
    Lands::new(shape, seed).land(x, y)
}
