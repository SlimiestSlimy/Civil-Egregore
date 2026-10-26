//! Each plane's shape ([`PlaneShapes`]): its width and tile size, by
//! type ID -- what a write making a plane in a chunk gives it, and what
//! a value queued for it must fit.

/// Each plane's width and tile size, by type ID.
pub struct PlaneShapes {
    /// Type ID, bits a number, cells a tile's side: sorted by type ID.
    shapes: Vec<(u64, u32, u32)>,
}

impl PlaneShapes {
    /// The shapes of the planes `shapes` -- type ID, width in bits, tile
    /// size -- each type once.
    pub fn new(mut shapes: Vec<(u64, u32, u32)>) -> Self {
        shapes.sort_unstable_by_key(|&(type_id, ..)| type_id);
        Self { shapes }
    }

    /// The width and tile size of the plane of ID `plane`: none if no
    /// plane is of that ID.
    pub(crate) fn of(&self, plane: u64) -> Option<(u32, u32)> {
        let place = self.shapes.binary_search_by_key(&plane, |&(type_id, ..)| type_id).ok()?;
        Some((self.shapes[place].1, self.shapes[place].2))
    }
}
