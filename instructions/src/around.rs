//! The 3x3 cells around a cell as nine bits, row by row from the top
//! left: cell `(x, y)` at bit `3 * y + x`, the cell itself at `(1, 1)`
//! -- a layer's, those entities stand on, one free among them, and one
//! drawn (`docs/instructions.md`, "The shapes").

use crate::mask::about;
use chunk_storage::LayerType;
use coordinates::CellIndex;
use simulation::Turn;
use utilities::rng::Rng;

/// Cells along the side of the nine.
pub const SIDE: u32 = 3;
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
    Some(set_bit_of_rank(choices as u64, random.below(choices.count_ones() as u64) as u32))
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
pub fn prefer(random: &mut Rng, wanted: u16, open: u16) -> Option<u32> {
    pick(random, if wanted & open != 0 { wanted & open } else { open })
}

/// The 3x3 cells around `at`, of `layer_type`, as the tick found
/// them: one window read. At the world's edge, none.
pub fn layer(turn: &Turn, layer_type: LayerType, at: CellIndex) -> Around {
    let Some(corner) = about(at, SIDE) else {
        return Around::default();
    };
    let window = turn.window(layer_type, corner, SIDE, SIDE);
    Around { set: squeeze(window.set), hot: squeeze(window.hot) }
}

/// Which of the 3x3 cells around `at` an entity stands on, as the
/// tick found them -- `at`'s own among them, if one stands there.
/// Asked when a cell must be had, not before a step, which is turned
/// back if its cell is taken.
pub fn occupied(turn: &Turn, at: CellIndex) -> u16 {
    about(at, SIDE).map_or(0, |corner| {
        let rows = turn.occupied(corner, SIDE, SIDE);
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
