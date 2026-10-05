//! The area about a cell: the 16x16 cells around it, a row a word --
//! what an entity sees of the world about it at once, and what paths
//! are found over -- the cells entities stand on among them, and the
//! same from further off, a tile a bit.


/// Cells along the side of an [`Area`].
pub const AREA_SIDE: usize = 16;
/// The column and the row of an [`Area`] its centre is at.
pub const AREA_CENTRE: usize = AREA_SIDE / 2;

/// The cells of one layer type around a cell (`read::area::layer`),
/// a row a word: cell `(x, y)` from the area's top left at bit `x` of
/// row `y`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Area {
    /// The cells the type holds at: hot ones only.
    pub set: [u16; AREA_SIDE],
    /// The cells in hot bitmaps: in the world, and read.
    pub hot: [u16; AREA_SIDE],
}

impl Area {
    /// How many of its cells the type holds at.
    pub fn count(&self) -> u32 {
        self.set.iter().map(|row| row.count_ones()).sum()
    }
}

/// The coarsest scale `read::area::of_tiles` looks over: tiles `2^6`
/// cells a side, [`AREA_SIDE`] of them 1,024 cells -- an entity's reach.
pub const FARTHEST_SCALE: u32 = bitplane_manager::COARSEST_SCALE;
