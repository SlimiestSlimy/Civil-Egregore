//! The 3x3 cells about a cell read as nine bits ([`crate::around`]): a
//! layer's, those entities stand on, and one free among them.

use crate::around::{pick, squeeze, Around};
use chunk_storage::LayerType;
use coordinates::CellIndex;
use simulation::Turn;

/// The 3x3 cells around `at`, of `layer_type`, as the tick found
/// them: one window read. At the world's edge, none.
pub fn layer(turn: &Turn, layer_type: LayerType, at: CellIndex) -> Around {
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
