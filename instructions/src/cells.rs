//! The cells: what a rule asks of a layer at a cell and the square
//! about one, the going over the cells sampled -- a rule of the cells
//! is written for one cell -- and a cell written, each a write queued.

use crate::mask::about;
use bitplane_manager::{Window, Write, WriteOp};
use chunk_storage::{LayerType, Wide, Width};
use coordinates::CellIndex;
use simulation::Turn;
use utilities::chance::Chance;

/// Runs `each` on every hot set cell of `layer_type` in the turn's
/// superchunk chosen with `chance`, independently, in Morton
/// order, with what it counts: how many were chosen, and the counts.
/// `samples` is room for them, emptied first.
#[inline]
pub fn each_sampled<C: Default>(turn: &mut Turn, layer_type: LayerType, chance: Chance, samples: &mut Vec<CellIndex>, mut each: impl FnMut(&mut Turn, CellIndex, &mut C)) -> (usize, C) {
    let (sampled, mut counts) = (turn.sample(layer_type, chance, samples), C::default());
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
    let corner = about(cell, side)?;
    Some((corner, turn.window(layer_type, corner, side, side)))
}

/// Queues `layer_type` holding at `cell`.
#[inline]
pub fn set(turn: &mut Turn, layer_type: LayerType, cell: CellIndex) {
    turn.queue(layer_type, Write::cell(cell, WriteOp::Set));
}

/// Queues `layer_type` no longer holding at `cell`.
#[inline]
pub fn clear(turn: &mut Turn, layer_type: LayerType, cell: CellIndex) {
    turn.queue(layer_type, Write::cell(cell, WriteOp::Unset));
}

/// Queues `value` as the number `plane` holds at `cell`.
#[inline]
pub fn set_value<W: Width>(turn: &mut Turn, plane: Wide<W>, cell: CellIndex, value: u32) {
    turn.queue(plane.layer_type(), Write::value(plane, cell, value));
}
