//! A layer written under a mask ([`crate::mask`]): its cells set or
//! cleared, queued as the few rectangles that cover them.

use crate::mask::{Mask, WORD};
use bitplane_manager::{Shape, Write, WriteOp};
use chunk_storage::LayerType;
use coordinates::CellIndex;
use simulation::Turn;

/// The most cells along a rectangle write's side.
const RECT: u32 = u8::MAX as u32;

/// Queues `layer_type` holding at every cell set in `mask`, the square
/// whose top left cell is `origin`: how many writes it took.
pub fn set(turn: &mut Turn, layer_type: LayerType, origin: CellIndex, mask: &Mask) -> usize {
    write(turn, layer_type, origin, mask, WriteOp::Set)
}

/// Queues `layer_type` no longer holding at any cell set in `mask`,
/// the square whose top left cell is `origin`: how many writes it took.
pub fn clear(turn: &mut Turn, layer_type: LayerType, origin: CellIndex, mask: &Mask) -> usize {
    write(turn, layer_type, origin, mask, WriteOp::Unset)
}

/// Queues `op` on every cell set in `mask`, as rectangles: each row's
/// runs of set cells, a run the same in the rows under it one
/// rectangle with them. How many writes.
fn write(turn: &mut Turn, layer_type: LayerType, origin: CellIndex, mask: &Mask, op: WriteOp) -> usize {
    // The rectangles still growing down: where each starts, and its sides.
    let mut open: Vec<(u32, u32, u32, u32)> = Vec::new();
    let mut runs: Vec<(u32, u32)> = Vec::new();
    let mut queued = 0;
    let mut queue = |turn: &mut Turn, (x, y, width, height): (u32, u32, u32, u32)| {
        // A rectangle starting past the world's edge has no cell in it.
        if let Some(at) = origin.offset(x as i32, y as i32) {
            turn.queue(layer_type, Write { at, op, shape: Shape::Rect { width: width as u8, height: height as u8 } });
            queued += 1;
        }
    };
    for y in 0..mask.side {
        runs.clear();
        let mut x = 0;
        while let Some(start) = next(mask, y, x, true) {
            let end = next(mask, y, start, false).unwrap_or(mask.side).min(start + RECT);
            runs.push((start, end - start));
            x = end;
        }
        // A rectangle whose run is not this row's, or as tall as one gets, is done.
        open.retain_mut(|rect| {
            let grows = rect.3 < RECT && runs.iter().any(|&(start, width)| (start, width) == (rect.0, rect.2));
            if grows {
                rect.3 += 1;
            } else {
                queue(turn, *rect);
            }
            grows
        });
        for &(start, width) in &runs {
            if !open.iter().any(|rect| (rect.0, rect.2) == (start, width) && rect.1 + rect.3 > y) {
                open.push((start, y, width, 1));
            }
        }
    }
    open.into_iter().for_each(|rect| queue(turn, rect));
    queued
}

/// The first cell of row `y` of `mask`, from `x` on, that is set --
/// or, `set` false, that is clear: none if the row has none.
fn next(mask: &Mask, y: u32, x: u32, set: bool) -> Option<u32> {
    let row = mask.row(y);
    let mut at = x;
    while at < mask.side {
        let word = if set { row[(at / WORD) as usize] } else { !row[(at / WORD) as usize] } >> (at % WORD);
        if word != 0 {
            return Some(at + word.trailing_zeros()).filter(|&found| found < mask.side);
        }
        at = (at / WORD + 1) * WORD;
    }
    None
}
