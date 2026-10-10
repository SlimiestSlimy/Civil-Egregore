//! The cells: what a rule asks of a layer at a cell and the square
//! about one, the going over the cells sampled -- a rule of the cells
//! is written for one cell -- and a cell written, each a
//! compare-and-write queued: it says what the rule saw at the cell,
//! and is applied only if the cell still holds it.

use crate::mask::about;
use bitplane_manager::Window;
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

/// Queues `layer_type` holding at `cell`, which the rule saw it did
/// not: a compare-and-write, applied only if it still does not
/// (`docs/instructions.md`, "Compare-and-write").
#[inline]
pub fn set(turn: &mut Turn, layer_type: LayerType, cell: CellIndex) {
    turn.queue_seen(layer_type, cell, 0, 1, None);
}

/// [`set`], adding one to the rule's count `counted` if it is applied.
#[inline]
pub fn set_counted(turn: &mut Turn, layer_type: LayerType, cell: CellIndex, counted: usize) {
    turn.queue_seen(layer_type, cell, 0, 1, Some(counted as u32));
}

/// Queues `layer_type` no longer holding at `cell`, which the rule saw
/// it did: a compare-and-write, applied only if it still does.
#[inline]
pub fn clear(turn: &mut Turn, layer_type: LayerType, cell: CellIndex) {
    turn.queue_seen(layer_type, cell, 1, 0, None);
}

/// [`clear`], adding one to the rule's count `counted` if it is
/// applied.
#[inline]
pub fn clear_counted(turn: &mut Turn, layer_type: LayerType, cell: CellIndex, counted: usize) {
    turn.queue_seen(layer_type, cell, 1, 0, Some(counted as u32));
}

/// Queues `value` as the number `plane` holds at `cell`, where the
/// rule saw `seen`: a compare-and-write, applied only if the cell
/// still holds `seen`.
#[inline]
pub fn set_value<W: Width>(turn: &mut Turn, plane: Wide<W>, cell: CellIndex, seen: u32, value: u32) {
    debug_assert!(value <= plane.most(), "{value} in a plane of {} bits a cell", W::BITS);
    turn.queue_seen(plane.layer_type(), cell, seen, value, None);
}

/// [`set_value`], adding one to the rule's count `counted` if it is
/// applied.
#[inline]
pub fn set_value_counted<W: Width>(turn: &mut Turn, plane: Wide<W>, cell: CellIndex, seen: u32, value: u32, counted: usize) {
    debug_assert!(value <= plane.most(), "{value} in a plane of {} bits a cell", W::BITS);
    turn.queue_seen(plane.layer_type(), cell, seen, value, Some(counted as u32));
}
