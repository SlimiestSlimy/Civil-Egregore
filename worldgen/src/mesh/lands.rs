//! The land asked of a cell at a time: the mesh behind it, what it last
//! found kept.

use crate::{ONE, Shape, noise};
use super::{FINER_MOST, FINEST, Mesh, WARP_INDEX};
use chunk_storage::Height;

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

    /// The land at the cell `(x, y)`, in heights: the broad mesh's
    /// height there, and what the finer meshes raise or sink it by, as
    /// much of it as the land there stands high -- never under the lowest ground.
    pub fn land(&mut self, x: u32, y: u32) -> u64 {
        let at = self.moved(x, y);
        let broad = self.broad.blended(&self.shape, self.seed, at);
        if broad.inland == 0 {
            return broad.height.max(self.shape.ground as i64) as u64;
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
        // Summed in fractions and made whole once: a mesh at a time, each would leave its own steps in long lines.
        (broad.height + ((finer + (ONE / 2) as i64) >> 16)).max(self.shape.ground as i64) as u64
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
