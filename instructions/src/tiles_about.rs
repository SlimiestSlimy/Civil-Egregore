//! The small tiles about a cell: the four aligned 4x4 tiles, two by
//! two, that it and its eight neighbours lie in, as one word of bits
//! in Morton order -- each tile 16 bits, as it lies in its bitmap --
//! so what is about a cell is asked of a layer with an AND, and whether
//! there is any of it with a compare (`docs/instructions.md`, "The
//! small tiles about a cell").

use bitplane_manager::{SmallTiles, SMALL_TILE_CELLS, SMALL_TILE_SIDE};
use chunk_storage::LayerType;
use coordinates::CellIndex;
use simulation::Turn;

/// Cells along the side of the square the four small tiles make.
const SQUARE_SIDE: u32 = 2 * SMALL_TILE_SIDE;
/// Cells in that square: the bits of its word.
const SQUARE_CELLS: usize = (SQUARE_SIDE * SQUARE_SIDE) as usize;
/// The cells about a cell, its own among them: three by three.
const NINE: usize = 9;

/// The bit, in the word of the four small tiles, of the cell `x`
/// across and `y` down from their top left: the two interleaved.
const fn bit_at(x: u32, y: u32) -> u32 {
    x & 1 | (y & 1) << 1 | (x & 2) << 1 | (y & 2) << 2 | (x & 4) << 2 | (y & 4) << 3
}

/// What [`bit_at`] interleaves, from the bit: across, and down.
const fn across_and_down(bit: u32) -> (u32, u32) {
    (bit & 1 | bit >> 1 & 2 | bit >> 2 & 4, bit >> 1 & 1 | bit >> 2 & 2 | bit >> 3 & 4)
}

/// For a cell at each bit of the word, the nine cells about it: the
/// bit of each, row by row from the top left as [`crate::around`]
/// numbers them, and which of the nine are in the square at all.
const ABOUT: [([u8; NINE], u16); SQUARE_CELLS] = {
    let mut about = [([0u8; NINE], 0u16); SQUARE_CELLS];
    let mut bit = 0;
    while bit < SQUARE_CELLS {
        let (x, y) = across_and_down(bit as u32);
        let mut neighbour = 0;
        while neighbour < NINE {
            // One to the left and above the cell is the first of the nine.
            let (across, down) = (x + neighbour as u32 % 3, y + neighbour as u32 / 3);
            if across >= 1 && down >= 1 && across <= SQUARE_SIDE && down <= SQUARE_SIDE {
                about[bit].0[neighbour] = bit_at(across - 1, down - 1) as u8;
                about[bit].1 |= 1 << neighbour;
            }
            neighbour += 1;
        }
        bit += 1;
    }
    about
};

/// For a cell at each bit of the word, the nine about it that are in
/// the square, as bits of the word.
const NINE_OF: [u64; SQUARE_CELLS] = {
    let mut nine = [0u64; SQUARE_CELLS];
    let mut bit = 0;
    while bit < SQUARE_CELLS {
        let mut neighbour = 0;
        while neighbour < NINE {
            if ABOUT[bit].1 >> neighbour & 1 == 1 {
                nine[bit] |= 1 << ABOUT[bit].0[neighbour];
            }
            neighbour += 1;
        }
        bit += 1;
    }
    nine
};

/// The four small tiles about a cell, and where it is in them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TilesAbout {
    /// The first cell of the first of the four: the top left one.
    first: CellIndex,
    /// The cell's bit in their word.
    bit: u32,
}

/// The four small tiles about `at`: its own, and those beside, above
/// or below it that its neighbours reach into -- the one to the left
/// if it is on its tile's left column, else the one to the right, and
/// so up and down. At the world's edge, where there is none to the
/// left or above, its own is the first.
#[inline]
pub(crate) fn of(at: CellIndex) -> TilesAbout {
    let in_tile = (at.0 & (SMALL_TILE_CELLS as u64 - 1)) as u32;
    let (mut first, (mut x, mut y)) = (CellIndex(at.0 & !(SMALL_TILE_CELLS as u64 - 1)), across_and_down(in_tile));
    let side = SMALL_TILE_SIDE as i32;
    if x == 0 && let Some(left) = first.offset(-side, 0) {
        (first, x) = (left, SMALL_TILE_SIDE);
    }
    if y == 0 && let Some(above) = first.offset(0, -side) {
        (first, y) = (above, SMALL_TILE_SIDE);
    }
    TilesAbout { first, bit: bit_at(x, y) }
}

impl TilesAbout {
    /// The cell itself, as a bit of the word.
    #[inline]
    pub(crate) fn centre(&self) -> u64 {
        1 << self.bit
    }

    /// The cell and its neighbours in the world, as bits of the word:
    /// in one to four of the small tiles.
    #[inline]
    pub(crate) fn nine(&self) -> u64 {
        NINE_OF[self.bit as usize]
    }

    /// Its neighbours in the world, as bits of the word: the hollow
    /// ring about it.
    #[inline]
    pub(crate) fn ring(&self) -> u64 {
        self.nine() & !self.centre()
    }

    /// `cells` of the word as the nine bits about the cell, row by row
    /// from the top left ([`crate::around`]): the same whichever way
    /// the small tiles fall about it, so what is drawn among them, or
    /// said of a way, does not hang on where the tiles' edges are.
    #[inline]
    pub(crate) fn as_nine(&self, cells: u64) -> u16 {
        let (bits, in_square) = &ABOUT[self.bit as usize];
        let mut nine = 0;
        for (neighbour, &bit) in bits.iter().enumerate() {
            nine |= ((cells >> bit & 1) as u16) << neighbour;
        }
        nine & in_square
    }
}

/// `layer_type` in the small tiles `about` a cell, as the tick found
/// it: the cells of `under` alone, and only the tiles it has a cell
/// in looked up -- one to four, for a cell and its neighbours.
#[inline]
pub(crate) fn layer(turn: &Turn, layer_type: LayerType, about: &TilesAbout, under: u64) -> SmallTiles {
    let [tiles] = layers(turn, [layer_type], about, under);
    tiles
}

/// [`layer`], of each of `types` at once.
#[inline]
pub(crate) fn layers<const N: usize>(turn: &Turn, types: [LayerType; N], about: &TilesAbout, under: u64) -> [SmallTiles; N] {
    turn.small_tiles(types, about.first, under)
}
