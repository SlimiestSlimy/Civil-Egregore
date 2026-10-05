//! A square of a layer read into masks ([`crate::mask`]): whole, or
//! the cells under another mask alone.

use crate::mask::{Mask, WORD};
use chunk_storage::LayerType;
use coordinates::CellIndex;
use simulation::Turn;

/// Cells along the side of a window, the most a turn reads at once.
const WINDOW: u32 = 8;

/// Reads the square of `layer_type` whose top left cell is `origin`,
/// as the tick found it, into `set` -- the cells it holds at -- and
/// `hot` -- those in hot bitmaps: in the world, and read. Both of one
/// side, the square's.
pub fn layer(turn: &Turn, layer_type: LayerType, origin: CellIndex, set: &mut Mask, hot: &mut Mask) {
    read_where(turn, layer_type, origin, None, set, hot);
}

/// [`layer`], of the cells set in `under` alone: the rest are left
/// clear in `set` and `hot`, and the parts of the square `under` has no
/// cell in are not read at all.
pub fn layer_under(turn: &Turn, layer_type: LayerType, origin: CellIndex, under: &Mask, set: &mut Mask, hot: &mut Mask) {
    read_where(turn, layer_type, origin, Some(under), set, hot);
}

/// [`layer`], under `under` if there is one: a window a time.
fn read_where(turn: &Turn, layer_type: LayerType, origin: CellIndex, under: Option<&Mask>, set: &mut Mask, hot: &mut Mask) {
    let side = set.side;
    assert!(hot.side == side && under.is_none_or(|under| under.side == side), "masks of two sides");
    set.clear();
    hot.clear();
    let (row, across) = (set.row_words(), WINDOW.min(side));
    for (x, y) in (0..side).step_by(WINDOW as usize).flat_map(|y| (0..side).step_by(WINDOW as usize).map(move |x| (x, y))) {
        let (word, shift) = ((x / WORD) as usize, x % WORD);
        let rows = (y..(y + WINDOW).min(side)).map(|y| y as usize * row + word);
        // The window's cells under the mask, as a window lays them out: a row a byte.
        let wanted = under.map_or(u64::MAX, |under| rows.clone().enumerate().fold(0, |wanted, (down, at)| wanted | (under.words[at] >> shift & 0xff) << (8 * down)));
        if wanted == 0 {
            continue;
        }
        // A window past the world's edge is left clear, and not hot.
        let Some(corner) = origin.offset(x as i32, y as i32) else {
            continue;
        };
        let window = turn.window(layer_type, corner, across, across);
        for (down, at) in rows.enumerate() {
            set.words[at] |= ((window.set & wanted) >> (8 * down) & 0xff) << shift;
            hot.words[at] |= ((window.hot & wanted) >> (8 * down) & 0xff) << shift;
        }
    }
}
