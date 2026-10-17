//! The cells about a cell: it and its eight neighbours, as bits of
//! the small tiles they lie in, in Morton order as they lie in their
//! bitmaps -- a layer's, what is open among them, one drawn -- and the
//! ways from the cell to each (`docs/actions.md`, "The shapes").

use crate::simulation_api::four_tiles_under_the_neighbours::{self, WAY_AT, WAY_OFFSETS};
pub use crate::simulation_api::four_tiles_under_the_neighbours::TilesAbout;
use chunk_storage::LayerType;
use coordinates::CellIndex;
use crate::SimulationTickState;
use std::ops::{BitAnd, BitOr};
use utilities::rng::RandomSource;

/// The ways from a cell: to each of the nine about it, numbered in
/// Morton order of where they lead -- up and left, up, left, the cell
/// itself, up and right, right, down and left, down, down and right.
/// The name of a neighbour that stays the same wherever the cell is:
/// what is drawn, and what is kept of one from tick to tick.
pub const WAYS: u32 = 9;
/// The way up and left.
pub const NORTH_WEST: u32 = 0;
/// The way up.
pub const NORTH: u32 = 1;
/// The way left.
pub const WEST: u32 = 2;
/// The way from a cell to itself: the middle of the nine.
pub const HERE: u32 = 3;
/// The way up and right.
pub const NORTH_EAST: u32 = 4;
/// The way right.
pub const EAST: u32 = 5;
/// The way down and left.
pub const SOUTH_WEST: u32 = 6;
/// The way down.
pub const SOUTH: u32 = 7;
/// The way down and right.
pub const SOUTH_EAST: u32 = 8;

/// Some of the cells about a cell: a set of them, a bit each, put
/// together with `&` and `|`. Of the cell it was asked about alone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cells(pub(crate) u64);

impl Cells {
    /// Whether it has no cell.
    #[inline]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// How many cells it has.
    #[inline]
    pub fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// Its cells that `other_cells` has not.
    #[inline]
    pub fn cells_not_in(self, other_cells: Cells) -> Cells {
        Cells(self.0 & !other_cells.0)
    }
}

impl BitAnd for Cells {
    type Output = Cells;

    /// The cells both have.
    #[inline]
    fn bitand(self, other_cells: Cells) -> Cells {
        Cells(self.0 & other_cells.0)
    }
}

impl BitOr for Cells {
    type Output = Cells;

    /// The cells either has.
    #[inline]
    fn bitor(self, other_cells: Cells) -> Cells {
        Cells(self.0 | other_cells.0)
    }
}

/// The cells about a cell, of one layer type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Around {
    /// The cells the type holds at.
    pub set_cells: Cells,
    /// The cells in hot bitmaps: in the world, and read.
    pub hot: Cells,
}

/// What is about `asked_cell`: asked once, and every set of cells about it
/// made and read with it.
#[inline]
pub fn tiles_about_cell(asked_cell: CellIndex) -> TilesAbout {
    four_tiles_under_the_neighbours::about_cell(asked_cell)
}

impl TilesAbout {
    /// The cell itself.
    #[inline]
    pub fn centre_cell(&self) -> Cells {
        Cells(self.centre_word())
    }

    /// Its eight neighbours, those in the world.
    #[inline]
    pub fn neighbour_cells(&self) -> Cells {
        Cells(self.ring_word())
    }

    /// The cell and its neighbours.
    #[inline]
    pub fn nine_cells(&self) -> Cells {
        Cells(self.nine_word())
    }

    /// The one cell `way` from it: none, off the world.
    #[inline]
    pub fn cell_towards(&self, way: u32) -> Cells {
        Cells(self.way_word(way))
    }
}

/// The cell `way` from `from_cell`: none off the world.
pub fn cell(from_cell: CellIndex, way: u32) -> Option<CellIndex> {
    let (across_offset, down_offset) = WAY_OFFSETS[way as usize];
    from_cell.at_cartesian_offset(across_offset as i32 - 1, down_offset as i32 - 1)
}

/// The way from `from_cell` to `cell`, one of the nine about it.
pub fn way_of(from_cell: CellIndex, cell: CellIndex) -> u32 {
    let (from_cell_cartesian, cell) = (from_cell.cartesian(), cell.cartesian());
    u32::from(WAY_AT[((cell.y + 1 - from_cell_cartesian.y) * 3 + cell.x + 1 - from_cell_cartesian.x) as usize])
}

/// The way to one of `choice_cells`, cells `about_tiles` a cell, drawn from
/// `random_stream`: none if there is none to choose, and then nothing is
/// drawn. Any is as likely as another, and which a draw comes to goes
/// by the ways' own order -- Morton order -- so it is the same
/// neighbour wherever the cell lies in its tiles.
pub fn pick_way(random_stream: &mut RandomSource, about_tiles: &TilesAbout, choice_cells: Cells) -> Option<u32> {
    if choice_cells.is_empty() {
        return None;
    }
    let ways = about_tiles.ways_of(choice_cells.0);
    Some(set_bit_of_rank(u64::from(ways), random_stream.draw_below(u64::from(ways.count_ones())) as u32))
}

/// The set bit of `bit_word` with `set_bit_rank` set bits under it.
#[inline]
pub(crate) fn set_bit_of_rank(bit_word: u64, set_bit_rank: u32) -> u32 {
    let mut bits_left = bit_word;
    (0..set_bit_rank).for_each(|_| bits_left &= bits_left - 1);
    bits_left.trailing_zeros()
}

/// One of `wanted_cells` among `open_cells`, if any is, else any of `open_cells`: where
/// to step when some neighbours are better than others.
pub fn prefer_way(random_stream: &mut RandomSource, about_tiles: &TilesAbout, wanted_cells: Cells, open_cells: Cells) -> Option<u32> {
    pick_way(random_stream, about_tiles, if (wanted_cells & open_cells).is_empty() { open_cells } else { wanted_cells & open_cells })
}

/// The cells `about_tiles` a cell, of `layer_type`, as the tick found them:
/// the one to four small tiles they lie in read as they lie
/// ([`crate::simulation_api::tiles_about`]), under the nine.
#[inline]
pub fn read_layer(tick_state: &SimulationTickState, layer_type: LayerType, about_tiles: &TilesAbout) -> Around {
    let tiles = four_tiles_under_the_neighbours::read_layer(tick_state, layer_type, about_tiles, about_tiles.nine_word());
    Around { set_cells: Cells(tiles.cells_set), hot: Cells(tiles.hot) }
}
