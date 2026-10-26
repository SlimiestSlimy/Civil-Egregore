//! Each plane's shape ([`PlaneShape`], [`PlaneShapes`]): its width and
//! tile scale, by type ID -- what a write making a plane in a chunk
//! gives it, and what a value queued for it must fit.

/// A plane's shape: how wide its numbers are, and the tiles they are of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaneShape {
    /// Bits a number: 1, 2, 4, 8, 16 or 32.
    pub width_bits: u32,
    /// A tile is `2^tile_scale` cells a side: 0 to 8.
    pub tile_scale: u32,
}

impl PlaneShape {
    /// Cells a tile.
    pub fn tile_cells(self) -> usize {
        1 << (2 * self.tile_scale)
    }
}

/// Each plane's shape, by type ID.
pub struct PlaneShapes {
    /// Each plane's type ID and shape, sorted by type ID.
    shapes: Vec<(u64, PlaneShape)>,
}

impl PlaneShapes {
    /// The shapes `shapes` of the planes, each by its type ID, each type
    /// once.
    pub fn new(mut shapes: Vec<(u64, PlaneShape)>) -> Self {
        shapes.sort_unstable_by_key(|&(plane_type_id, _)| plane_type_id);
        Self { shapes }
    }

    /// The shape of the plane of type ID `plane_type_id`: none if no plane
    /// is of that ID.
    pub(crate) fn of(&self, plane_type_id: u64) -> Option<PlaneShape> {
        let place = self.shapes.binary_search_by_key(&plane_type_id, |&(each_type_id, _)| each_type_id).ok()?;
        Some(self.shapes[place].1)
    }
}
