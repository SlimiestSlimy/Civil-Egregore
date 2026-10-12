//! The cells: what a rule asks of a layer at a cell and the square
//! about one, and a cell written, each a
//! compare-and-write queued: it says what the rule saw at the cell,
//! and is applied only if the cell still holds it.

use bitplane_manager::Window;
use chunk_storage::{LayerType, Wide, Width};
use coordinates::CellIndex;
use compares_and_writes::{compare, Write};
use simulation::Turn;

/// How many cells of the tile 8 by 8 cells that `cell` is in
/// `layer_type` holds at, as the tick found them: none where it is
/// not hot.
#[inline]
pub fn held_in_tile(turn: &Turn, layer_type: LayerType, cell: CellIndex) -> u32 {
    turn.aligned_tile(layer_type, cell, 8).map_or(0, |words| words.iter().map(|word| word.count_ones()).sum())
}

/// Queues one taken from the number `plane` holds at `from`, where the
/// rule saw `seen_from`, and one added to that at `to`, where it saw
/// `seen_to`: what is at `from` moved a step. Both under both
/// compares, so one is taken only if one is added -- whichever
/// superchunk each is in: nothing is made or lost. One added to the
/// rule's count `counted` if it is applied.
#[inline]
pub fn move_one<W: Width>(turn: &mut Turn, plane: Wide<W>, from: CellIndex, seen_from: u32, to: CellIndex, seen_to: u32, counted: usize) {
    debug_assert!(seen_from > 0 && seen_to < plane.most(), "one moved from {seen_from} to {seen_to}");
    compare::open(turn, compare::value(plane, from, seen_from));
    compare::open(turn, compare::value(plane, to, seen_to));
    turn.write(Write::Cell { layer_type: plane.layer_type(), at: from, value: seen_from - 1, counted: Some(counted as u32) });
    turn.write(Write::Cell { layer_type: plane.layer_type(), at: to, value: seen_to + 1, counted: None });
    compare::close(turn);
    compare::close(turn);
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
    let corner = corner_about(cell, side)?;
    Some((corner, turn.window(layer_type, corner, side, side)))
}

/// Queues the number of `layer_type` at `cell` becoming `value` if it
/// is still the `seen` the rule read there: three instructions -- a
/// compare of the cell, the write, the compare's end, queued at once -- so none lands
/// on another's, under whatever compare the rule is queuing under as
/// well. One added to the rule's count `counted` if it is applied.
#[inline]
pub fn seen_becoming(turn: &mut Turn, layer_type: LayerType, cell: CellIndex, seen: u32, value: u32, counted: Option<usize>) {
    debug_assert!(seen != value, "a write that changes nothing");
    compare::with_writes(turn, compare::Compare::Cell { layer_type, at: cell, seen: seen as u16 }, &[Write::Cell { layer_type, at: cell, value, counted: counted.map(|place| place as u32) }]);
}

/// Queues `layer_type` holding at `cell`, which the rule saw it did
/// not: a compare-and-write, applied only if it still does not
/// (`docs/actions.md`, "Compare-and-write").
#[inline]
pub fn set(turn: &mut Turn, layer_type: LayerType, cell: CellIndex) {
    seen_becoming(turn, layer_type, cell, 0, 1, None);
}

/// [`set`], adding one to the rule's count `counted` if it is applied.
#[inline]
pub fn set_counted(turn: &mut Turn, layer_type: LayerType, cell: CellIndex, counted: usize) {
    seen_becoming(turn, layer_type, cell, 0, 1, Some(counted));
}

/// Queues `layer_type` no longer holding at `cell`, which the rule saw
/// it did: a compare-and-write, applied only if it still does.
#[inline]
pub fn clear(turn: &mut Turn, layer_type: LayerType, cell: CellIndex) {
    seen_becoming(turn, layer_type, cell, 1, 0, None);
}

/// [`clear`], adding one to the rule's count `counted` if it is
/// applied.
#[inline]
pub fn clear_counted(turn: &mut Turn, layer_type: LayerType, cell: CellIndex, counted: usize) {
    seen_becoming(turn, layer_type, cell, 1, 0, Some(counted));
}

/// Queues `value` as the number `plane` holds at `cell`, where the
/// rule saw `seen`: a compare-and-write, applied only if the cell
/// still holds `seen`.
#[inline]
pub fn set_value<W: Width>(turn: &mut Turn, plane: Wide<W>, cell: CellIndex, seen: u32, value: u32) {
    debug_assert!(value <= plane.most(), "{value} in a plane of {} bits a cell", W::BITS);
    seen_becoming(turn, plane.layer_type(), cell, seen, value, None);
}

/// [`set_value`], adding one to the rule's count `counted` if it is
/// applied.
#[inline]
pub fn set_value_counted<W: Width>(turn: &mut Turn, plane: Wide<W>, cell: CellIndex, seen: u32, value: u32, counted: usize) {
    debug_assert!(value <= plane.most(), "{value} in a plane of {} bits a cell", W::BITS);
    seen_becoming(turn, plane.layer_type(), cell, seen, value, Some(counted));
}

/// The top left cell of the square `side` cells a side about `centre`
/// -- it `side / 2` across and down: none past the world's edge.
pub fn corner_about(centre: CellIndex, side: u32) -> Option<CellIndex> {
    let reach = side as i32 / 2;
    centre.offset(-reach, -reach)
}
