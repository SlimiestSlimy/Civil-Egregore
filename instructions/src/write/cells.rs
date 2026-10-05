//! The cells written: a layer set or cleared at a cell, a wide plane's
//! number put, each a write queued.

use bitplane_manager::{Write, WriteOp};
use chunk_storage::{LayerType, Wide, Width};
use coordinates::CellIndex;
use simulation::Turn;

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
