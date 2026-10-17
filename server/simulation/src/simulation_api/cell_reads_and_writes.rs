//! The cells: what a rule asks of a layer at a cell and the square
//! about one, and a cell written, each a
//! compare-and-write queued: it says what the rule saw at the cell,
//! and is applied only if the cell still holds it.

use bitplane_manager::Window;
use chunk_storage::{LayerType, WidePlane, Width};
use coordinates::CellIndex;
use crate::simulation_api::compare_and_write;
use crate::{Compare, Write};
use crate::SimulationTickState;

/// How many cells of the tile 8 by 8 cells that `cell` is in
/// `layer_type` holds at, as the tick found them: none where it is
/// not hot.
#[inline]
pub fn held_in_tile(tick_state: &SimulationTickState, layer_type: LayerType, cell: CellIndex) -> u32 {
    tick_state.aligned_tile(layer_type, cell, 8).map_or(0, |words| words.iter().map(|word| word.count_ones()).sum())
}

/// Queues one taken from the number `wide_plane` holds at `from_cell`, where the
/// rule saw `seen_from`, and one added to that at `to_cell`, where it saw
/// `seen_to`: what is at `from_cell` moved a step. Both under both
/// compares, so one is taken only if one is added -- whichever
/// superchunk each is in: nothing is made or lost. One added to the
/// rule's count `count_place` if it is applied.
#[inline]
pub fn move_one<W: Width>(tick_state: &mut SimulationTickState, wide_plane: WidePlane<W>, from_cell: CellIndex, seen_from: u32, to_cell: CellIndex, seen_to: u32, count_place: usize) {
    debug_assert!(seen_from > 0 && seen_to < wide_plane.most_value(), "one moved from {seen_from} to {seen_to}");
    change_both(tick_state, wide_plane, (from_cell, seen_from, seen_from - 1), (to_cell, seen_to, seen_to + 1), count_place);
}

/// Queues the numbers `wide_plane` holds at two cells changed together:
/// each of `first_change` and `second_change` a cell, what the rule saw there and
/// what it becomes. Both under both compares, so the one changes
/// only if the other does -- whichever superchunk each is in. One
/// added to the rule's count `count_place` if it is applied.
#[inline]
pub fn change_both<W: Width>(tick_state: &mut SimulationTickState, wide_plane: WidePlane<W>, first_change: (CellIndex, u32, u32), second_change: (CellIndex, u32, u32), count_place: usize) {
    debug_assert!(first_change.2 <= wide_plane.most_value() && second_change.2 <= wide_plane.most_value(), "{} and {} in a plane of {} bits a cell", first_change.2, second_change.2, W::BITS_A_CELL);
    let mut both_as_seen = compare_and_write::open_compare(tick_state, compare_and_write::value(wide_plane, first_change.0, first_change.1)).and_compare(compare_and_write::value(wide_plane, second_change.0, second_change.1));
    both_as_seen.write_cell_counted(wide_plane.layer_type(), first_change.0, first_change.2, count_place);
    both_as_seen.write_cell(wide_plane.layer_type(), second_change.0, second_change.2);
}

/// Whether `cell` is hot in `layer_type`: in the world, and read.
#[inline]
pub fn hot(tick_state: &SimulationTickState, layer_type: LayerType, cell: CellIndex) -> bool {
    tick_state.layer_holds(layer_type, cell).is_ok()
}

/// Whether `layer_type` holds at `cell`, as the tick found it: not
/// where it is not hot.
#[inline]
pub fn layer_holds(tick_state: &SimulationTickState, layer_type: LayerType, cell: CellIndex) -> bool {
    tick_state.layer_holds(layer_type, cell) == Ok(true)
}

/// Whether `cell` is hot in `layer_type` and it does not hold there, as
/// the tick found it: a cell to put it on.
#[inline]
pub fn layer_lacks(tick_state: &SimulationTickState, layer_type: LayerType, cell: CellIndex) -> bool {
    tick_state.layer_holds(layer_type, cell) == Ok(false)
}

/// The number `wide_plane` holds at `cell`, as the tick found it: none
/// where it is not hot.
#[inline]
pub fn value<W: Width>(tick_state: &SimulationTickState, wide_plane: WidePlane<W>, cell: CellIndex) -> Option<u32> {
    tick_state.value(wide_plane, cell).ok()
}

/// The `window_side` by `window_side` cells (up to 8) of `layer_type` about `cell`
/// -- it `window_side / 2` across and down -- as the tick found them, and
/// their top left cell: cell `(x, y)` of them at bit `8 * y + x`. None
/// at the world's edge.
#[inline]
pub fn window_about(tick_state: &SimulationTickState, layer_type: LayerType, cell: CellIndex, window_side: u32) -> Option<(CellIndex, Window)> {
    let window_corner = corner_about(cell, window_side)?;
    Some((window_corner, tick_state.window(layer_type, window_corner, window_side, window_side)))
}

/// Queues the number of `layer_type` at `cell` becoming `value` if it
/// is still the `value_seen` the rule read there: three instructions -- a
/// compare of the cell, the write, the compare's end, queued at once -- so none lands
/// on another's, under whatever compare the rule is queuing under as
/// well. One added to the rule's count `count_place` if it is applied.
#[inline]
pub fn seen_becoming(tick_state: &mut SimulationTickState, layer_type: LayerType, cell: CellIndex, value_seen: u32, value: u32, count_place: Option<usize>) {
    debug_assert!(value_seen != value, "a write that changes nothing");
    compare_and_write::with_writes(tick_state, Compare::Cell { layer_type, target_cell: cell, seen_value: value_seen }, &[Write::Cell { layer_type, target_cell: cell, value, counted_place: count_place.map(|place| place as u32) }]);
}

/// Queues `layer_type` holding at `cell`, which the rule saw it did
/// not: a compare-and-write, applied only if it still does not
/// (`docs/actions.md`, "Compare-and-write").
#[inline]
pub fn set(tick_state: &mut SimulationTickState, layer_type: LayerType, cell: CellIndex) {
    seen_becoming(tick_state, layer_type, cell, 0, 1, None);
}

/// [`set`], adding one to the rule's count `count_place` if it is applied.
#[inline]
pub fn set_counted(tick_state: &mut SimulationTickState, layer_type: LayerType, cell: CellIndex, count_place: usize) {
    seen_becoming(tick_state, layer_type, cell, 0, 1, Some(count_place));
}

/// Queues `layer_type` no longer holding at `cell`, which the rule saw
/// it did: a compare-and-write, applied only if it still does.
#[inline]
pub fn clear(tick_state: &mut SimulationTickState, layer_type: LayerType, cell: CellIndex) {
    seen_becoming(tick_state, layer_type, cell, 1, 0, None);
}

/// [`clear`], adding one to the rule's count `count_place` if it is
/// applied.
#[inline]
pub fn clear_counted(tick_state: &mut SimulationTickState, layer_type: LayerType, cell: CellIndex, count_place: usize) {
    seen_becoming(tick_state, layer_type, cell, 1, 0, Some(count_place));
}

/// Queues `value` as the number `wide_plane` holds at `cell`, where the
/// rule saw `value_seen`: a compare-and-write, applied only if the cell
/// still holds `value_seen`.
#[inline]
pub fn set_value<W: Width>(tick_state: &mut SimulationTickState, wide_plane: WidePlane<W>, cell: CellIndex, value_seen: u32, value: u32) {
    debug_assert!(value <= wide_plane.most_value(), "{value} in a plane of {} bits a cell", W::BITS_A_CELL);
    seen_becoming(tick_state, wide_plane.layer_type(), cell, value_seen, value, None);
}

/// [`set_value`], adding one to the rule's count `count_place` if it is
/// applied.
#[inline]
pub fn set_value_counted<W: Width>(tick_state: &mut SimulationTickState, wide_plane: WidePlane<W>, cell: CellIndex, value_seen: u32, value: u32, count_place: usize) {
    debug_assert!(value <= wide_plane.most_value(), "{value} in a plane of {} bits a cell", W::BITS_A_CELL);
    seen_becoming(tick_state, wide_plane.layer_type(), cell, value_seen, value, Some(count_place));
}

/// The top left cell of the square `square_side` cells a side about `centre_cell`
/// -- it `square_side / 2` across and down: none past the world's edge.
pub fn corner_about(centre_cell: CellIndex, square_side: u32) -> Option<CellIndex> {
    let reach = square_side as i32 / 2;
    centre_cell.at_cartesian_offset(-reach, -reach)
}
