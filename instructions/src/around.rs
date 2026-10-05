//! The 3x3 cells around a cell, as nine bits: what an entity sees
//! beside it, and where it may step. Row by row from the top left, the
//! cell `(x, y)` -- each 0 to 2, the cell itself at `(1, 1)` -- at bit
//! `3 * y + x`. A set of neighbours is a mask, narrowed with `&`: those
//! with grass, those no entity stands on, those in the world hot. A
//! neighbour chosen is a bit's index, turned into a cell only when it
//! is stepped to.
//!
//! Read at once: a window of the bitplane from the cell up and left
//! ([`read`]), its three rows of three squeezed together; no cell is
//! looked at alone.

use chunk_storage::LayerType;
use coordinates::CellIndex;
use simulation::Turn;
use utilities::rng::Rng;

/// The cell itself: the middle of the nine.
pub const CENTRE: u16 = 1 << 4;
/// All nine.
pub const ALL: u16 = 0o777;
/// The eight neighbours.
pub const RING: u16 = ALL & !CENTRE;

/// The 3x3 cells around a cell, of one layer type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Around {
    /// The cells the type holds at.
    pub set: u16,
    /// The cells in hot bitmaps: in the world, and read.
    pub hot: u16,
}

/// A window of 3x3 cells, row by row a byte each, as nine bits: row
/// `r`'s three cells, at bits `8r` to `8r + 2`, to bits `3r` to
/// `3r + 2`.
pub const fn squeeze(rows: u64) -> u16 {
    (rows & 0o7 | rows >> 5 & 0o70 | rows >> 10 & 0o700) as u16
}

/// The cell at bit `bit` of the nine around `at`: none off the world.
pub fn cell(at: CellIndex, bit: u32) -> Option<CellIndex> {
    at.offset(bit as i32 % 3 - 1, bit as i32 / 3 - 1)
}

/// The bit of `cell`, one of the nine around `at`.
pub fn bit_of(at: CellIndex, cell: CellIndex) -> u32 {
    let (at, cell) = (at.cartesian(), cell.cartesian());
    (cell.y + 1 - at.y) * 3 + cell.x + 1 - at.x
}

/// The bit of one of `choices`, drawn from `random`: none if there is
/// none to choose, and then nothing is drawn.
pub fn pick(random: &mut Rng, choices: u16) -> Option<u32> {
    if choices == 0 {
        return None;
    }
    let mut left = choices;
    for _ in 0..random.below(choices.count_ones() as u64) {
        left &= left - 1;
    }
    Some(left.trailing_zeros())
}

/// One of `wanted` among `open`, if any is, else any of `open`: where
/// to step when some neighbours are better than others.
pub fn prefer(random: &mut Rng, wanted: u16, open: u16) -> Option<u32> {
    pick(random, if wanted & open != 0 { wanted & open } else { open })
}

/// The 3x3 cells around `at`, of `layer_type`, as the tick found
/// them: one window read. At the world's edge, none.
pub fn read(turn: &Turn, layer_type: LayerType, at: CellIndex) -> Around {
    let Some(corner) = at.offset(-1, -1) else {
        return Around::default();
    };
    let window = turn.window(layer_type, corner, 3, 3);
    Around { set: squeeze(window.set), hot: squeeze(window.hot) }
}

/// Which of the 3x3 cells around `at` an entity stands on, as the
/// tick found them -- `at`'s own among them, if one stands there.
/// Asked when a cell must be had, not before a step, which is turned
/// back if its cell is taken.
pub fn occupied(turn: &Turn, at: CellIndex) -> u16 {
    at.offset(-1, -1).map_or(0, |corner| {
        let rows = turn.occupied(corner, 3, 3);
        rows[0] | rows[1] << 3 | rows[2] << 6
    })
}

/// One of the neighbours of `at` among `open` that no entity stood on
/// as the tick found them, drawn at random: where to make an entity,
/// which must have its cell. None if every one is taken.
pub fn free_beside(turn: &mut Turn, at: CellIndex, open: u16) -> Option<u32> {
    let free = open & !occupied(turn, at);
    pick(turn.random(), free)
}
