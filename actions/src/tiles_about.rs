//! The small tiles about a cell: the four aligned 4x4 tiles, two by
//! two, that it and its eight neighbours lie in, as one word of bits
//! in Morton order -- each tile 16 bits, as it lies in its bitmap --
//! so what is about a cell is asked of a layer with an AND, and whether
//! there is any of it with a compare (`docs/actions.md`, "The
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

/// The bits of a cell's index that place it in its small tile.
const TILE: u64 = SMALL_TILE_CELLS as u64 - 1;
/// The bits of a cell's index holding x: the even ones.
const X_BITS: u64 = 0x5555_5555_5555_5555;
/// The bits of a cell's index holding y: the odd ones.
const Y_BITS: u64 = !X_BITS;

/// The bit, in the word of the four small tiles, of the cell `x`
/// across and `y` down from their top left: the two interleaved.
const fn bit_at(x: u32, y: u32) -> u32 {
    x & 1 | (y & 1) << 1 | (x & 2) << 1 | (y & 2) << 2 | (x & 4) << 2 | (y & 4) << 3
}

/// What [`bit_at`] interleaves, from the bit: across, and down.
const fn across_and_down(bit: u32) -> (u32, u32) {
    (bit & 1 | bit >> 1 & 2 | bit >> 2 & 4, bit >> 1 & 1 | bit >> 2 & 2 | bit >> 3 & 4)
}

/// Where each way leads, across and down from the cell one to the
/// left of and above the cell it is from: the nine in Morton order of
/// those two, as [`crate::around`] numbers the ways -- so the cell
/// itself is the fourth.
pub(crate) const WAY_OFFSETS: [(u32, u32); NINE] = {
    let (mut offsets, mut way, mut place) = ([(0, 0); NINE], 0, 0);
    // The places of a square of four by four in Morton order: those within three by three are the nine.
    while way < NINE {
        let (across, down) = across_and_down(place);
        if across < 3 && down < 3 {
            offsets[way] = (across, down);
            way += 1;
        }
        place += 1;
    }
    offsets
};

/// The way to each of the nine, row by row from the top left: what
/// [`WAY_OFFSETS`] says, the other way about.
pub(crate) const WAY_AT: [u8; NINE] = {
    let (mut ways, mut way) = ([0; NINE], 0);
    while way < NINE {
        ways[(WAY_OFFSETS[way].1 * 3 + WAY_OFFSETS[way].0) as usize] = way as u8;
        way += 1;
    }
    ways
};

/// For a cell at each bit of the word, the nine cells about it: the
/// bit of each, by its way as [`crate::around`] numbers them, and
/// which of the nine are in the square at all.
const ABOUT: [([u8; NINE], u16); SQUARE_CELLS] = {
    let mut about = [([0u8; NINE], 0u16); SQUARE_CELLS];
    let mut bit = 0;
    while bit < SQUARE_CELLS {
        let (x, y) = across_and_down(bit as u32);
        let mut neighbour = 0;
        while neighbour < NINE {
            // One to the left and above the cell is the first of the nine.
            let (across, down) = (x + WAY_OFFSETS[neighbour].0, y + WAY_OFFSETS[neighbour].1);
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
pub struct About {
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
pub(crate) fn about_cell(at: CellIndex) -> About {
    let in_tile = at.0 & TILE;
    let (mut first, (mut x, mut y)) = (at.0 & !TILE, across_and_down(in_tile as u32));
    // A small tile to the left, or above: its side taken from the one coordinate's bits, where they are not all 0.
    if x == 0 && first & X_BITS != 0 {
        (first, x) = ((first & X_BITS).wrapping_sub(TILE + 1) & X_BITS | first & Y_BITS, SMALL_TILE_SIDE);
    }
    if y == 0 && first & Y_BITS != 0 {
        (first, y) = ((first & Y_BITS).wrapping_sub(2 * (TILE + 1)) & Y_BITS | first & X_BITS, SMALL_TILE_SIDE);
    }
    let first = CellIndex(first);
    About { first, bit: bit_at(x, y) }
}

impl About {
    /// The cell itself, as a bit of the word.
    #[inline]
    pub(crate) fn centre_word(&self) -> u64 {
        1 << self.bit
    }

    /// The cell and its neighbours in the world, as bits of the word:
    /// in one to four of the small tiles.
    #[inline]
    pub(crate) fn nine_word(&self) -> u64 {
        NINE_OF[self.bit as usize]
    }

    /// Its neighbours in the world, as bits of the word: the hollow
    /// ring about it.
    #[inline]
    pub(crate) fn ring_word(&self) -> u64 {
        self.nine_word() & !self.centre_word()
    }

    /// The cell `way` from the cell -- one of the nine, numbered in
    /// Morton order of where they lead, the cell itself the fourth --
    /// as a bit of the word: none if it is not in the world.
    #[inline]
    pub(crate) fn way_word(&self, way: u32) -> u64 {
        let (bits, in_square) = &ABOUT[self.bit as usize];
        ((in_square >> way & 1) as u64) << bits[way as usize]
    }

    /// The cells of `word`, bits of the word, as ways from the cell: a
    /// bit a way, the way's number its place -- the same bits wherever
    /// the cell lies in its tiles.
    #[inline]
    pub(crate) fn ways_of(&self, word: u64) -> u32 {
        let (bits, in_square) = &ABOUT[self.bit as usize];
        let mut ways = 0;
        for (way, &bit) in bits.iter().enumerate() {
            ways |= ((word >> bit & 1) as u32) << way;
        }
        ways & *in_square as u32
    }
}

/// `layer_type` in the small tiles `about` a cell, as the tick found
/// it: the cells of `under` alone, and only the tiles it has a cell
/// in looked up -- one to four, for a cell and its neighbours.
#[inline]
pub(crate) fn read_layer(turn: &Turn, layer_type: LayerType, about: &About, under: u64) -> SmallTiles {
    let [tiles] = read_layers(turn, [layer_type], about, under);
    tiles
}

/// [`read_layer`], of each of `types` at once.
#[inline]
pub(crate) fn read_layers<const N: usize>(turn: &Turn, types: [LayerType; N], about: &About, under: u64) -> [SmallTiles; N] {
    turn.small_tiles(types, about.first, under)
}
