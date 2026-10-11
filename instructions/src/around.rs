//! The cells about a cell: it and its eight neighbours, as bits of
//! the small tiles they lie in, in Morton order as they lie in their
//! bitmaps -- a layer's, what is open among them, one drawn -- and the
//! ways from the cell to each (`docs/instructions.md`, "The shapes").

use crate::tiles_about;
pub use crate::tiles_about::About;
use chunk_storage::LayerType;
use coordinates::CellIndex;
use simulation::Turn;
use std::ops::{BitAnd, BitOr};
use utilities::rng::Rng;

/// The ways from a cell: to each of the nine about it, numbered row
/// by row from the top left -- the way `3 * down + across`, each 0 to
/// 2. The name of a neighbour that stays the same wherever the cell
/// is: what is kept of one from tick to tick.
pub const WAYS: u32 = 9;
/// The way from a cell to itself: the middle of the nine.
pub const HERE: u32 = 4;

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

    /// Its cells that `other` has not.
    #[inline]
    pub fn without(self, other: Cells) -> Cells {
        Cells(self.0 & !other.0)
    }
}

impl BitAnd for Cells {
    type Output = Cells;

    /// The cells both have.
    #[inline]
    fn bitand(self, other: Cells) -> Cells {
        Cells(self.0 & other.0)
    }
}

impl BitOr for Cells {
    type Output = Cells;

    /// The cells either has.
    #[inline]
    fn bitor(self, other: Cells) -> Cells {
        Cells(self.0 | other.0)
    }
}

/// The cells about a cell, of one layer type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Around {
    /// The cells the type holds at.
    pub set: Cells,
    /// The cells in hot bitmaps: in the world, and read.
    pub hot: Cells,
}

/// What is about `at`: asked once, and every set of cells about it
/// made and read with it.
#[inline]
pub fn about(at: CellIndex) -> About {
    tiles_about::of(at)
}

impl About {
    /// The cell itself.
    #[inline]
    pub fn centre(&self) -> Cells {
        Cells(self.centre_word())
    }

    /// Its eight neighbours, those in the world.
    #[inline]
    pub fn ring(&self) -> Cells {
        Cells(self.ring_word())
    }

    /// The cell and its neighbours.
    #[inline]
    pub fn nine(&self) -> Cells {
        Cells(self.nine_word())
    }

    /// The one cell `way` from it: none, off the world.
    #[inline]
    pub fn towards(&self, way: u32) -> Cells {
        Cells(self.way_word(way))
    }
}

/// The cell `way` from `at`: none off the world.
pub fn cell(at: CellIndex, way: u32) -> Option<CellIndex> {
    at.offset(way as i32 % 3 - 1, way as i32 / 3 - 1)
}

/// The way from `at` to `cell`, one of the nine about it.
pub fn way_of(at: CellIndex, cell: CellIndex) -> u32 {
    let (at, cell) = (at.cartesian(), cell.cartesian());
    (cell.y + 1 - at.y) * 3 + cell.x + 1 - at.x
}

/// The way to one of `choices`, cells `about` a cell, drawn from
/// `random`: none if there is none to choose, and then nothing is
/// drawn. Which a draw comes to goes by how the cells lie in their
/// tiles: any is as likely as another.
pub fn pick(random: &mut Rng, about: &About, choices: Cells) -> Option<u32> {
    if choices.is_empty() {
        return None;
    }
    Some(about.way_of_bit(set_bit_of_rank(choices.0, random.below(choices.count() as u64) as u32)))
}

/// The set bit of `bits` with `rank` set bits under it.
#[inline]
pub(crate) fn set_bit_of_rank(bits: u64, rank: u32) -> u32 {
    let mut left = bits;
    (0..rank).for_each(|_| left &= left - 1);
    left.trailing_zeros()
}

/// One of `wanted` among `open`, if any is, else any of `open`: where
/// to step when some neighbours are better than others.
pub fn prefer(random: &mut Rng, about: &About, wanted: Cells, open: Cells) -> Option<u32> {
    pick(random, about, if (wanted & open).is_empty() { open } else { wanted & open })
}

/// The cells `about` a cell, of `layer_type`, as the tick found them:
/// the one to four small tiles they lie in read as they lie
/// ([`crate::tiles_about`]), under the nine.
#[inline]
pub fn layer(turn: &Turn, layer_type: LayerType, about: &About) -> Around {
    let tiles = tiles_about::layer(turn, layer_type, about, about.nine_word());
    Around { set: Cells(tiles.set), hot: Cells(tiles.hot) }
}
