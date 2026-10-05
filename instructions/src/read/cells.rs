//! The cells read: what a rule asks of a layer at a cell, the square
//! about one, and the going over the cells sampled -- a rule of the
//! cells is written for one cell.

use bitplane_manager::Window;
use chunk_storage::{LayerType, Wide, Width};
use coordinates::CellIndex;
use simulation::Turn;

/// Runs `each` on every hot set cell of `layer_type` in the turn's
/// superchunk chosen with `probability`, independently, in Morton
/// order, with what it counts: how many were chosen, and the counts.
/// `samples` is room for them, emptied first.
#[inline]
pub fn each_sampled<C: Default>(turn: &mut Turn, layer_type: LayerType, probability: f64, samples: &mut Vec<CellIndex>, mut each: impl FnMut(&mut Turn, CellIndex, &mut C)) -> (usize, C) {
    let (sampled, mut counts) = (turn.sample(layer_type, probability, samples), C::default());
    for &cell in samples.iter() {
        each(turn, cell, &mut counts);
    }
    (sampled, counts)
}

/// Whether `cell` is hot in `layer_type`: in the world, and read.
#[inline]
pub fn hot(turn: &Turn, layer_type: LayerType, cell: CellIndex) -> bool {
    turn.holds(layer_type, cell).is_ok()
}

/// Whether `layer_type` holds at `cell`, as the tick found it: not
/// where it is not hot.
#[inline]
pub fn holds(turn: &Turn, layer_type: LayerType, cell: CellIndex) -> bool {
    turn.holds(layer_type, cell) == Ok(true)
}

/// Whether `cell` is hot in `layer_type` and it does not hold there, as
/// the tick found it: a cell to put it on.
#[inline]
pub fn lacks(turn: &Turn, layer_type: LayerType, cell: CellIndex) -> bool {
    turn.holds(layer_type, cell) == Ok(false)
}

/// The number `plane` holds at `cell`, as the tick found it: none
/// where it is not hot.
#[inline]
pub fn value<W: Width>(turn: &Turn, plane: Wide<W>, cell: CellIndex) -> Option<u32> {
    turn.value(plane, cell).ok()
}

/// The `side` by `side` cells (up to 8) of `layer_type` about `cell`
/// -- it `side / 2` across and down -- as the tick found them, and
/// their top left cell: cell `(x, y)` of them at bit `8 * y + x`. None
/// at the world's edge.
#[inline]
pub fn square(turn: &Turn, layer_type: LayerType, cell: CellIndex, side: u32) -> Option<(CellIndex, Window)> {
    let reach = side as i32 / 2;
    let corner = cell.offset(-reach, -reach)?;
    Some((corner, turn.window(layer_type, corner, side, side)))
}
