//! The cells about a floor tile, in one word: a cell's context read off
//! it by tables.

use super::{CONTEXT_CELLS, FLOOR_TILE_CELLS, FLOOR_TILE_SIDE, FLOOR_X_BITS, FLOOR_Y_BITS};
use bitmap::Bitmap;
use bitmap::morton::morton_coordinates;

/// A floor tile and the three floor tiles before it -- above left, above, left --
/// as an 8x8 square of cells, a bit each, row after row: bit `8y + x`,
/// the floor tile's own cells at `x`, `y` from 4 to 7. A floor tile off the bitmap
/// is clear.
pub(crate) struct Window(pub(crate) u64);

/// Cells a window row: two floor tiles side by side.
const WINDOW_SIDE: u32 = 2 * FLOOR_TILE_SIDE;

/// A floor tile's first eight cells in Morton order -- its top two rows -- by
/// their values, in a window's rows; the next eight are the same two
/// rows lower.
const FLOOR_TILE_ROWS: [u64; 1 << (FLOOR_TILE_CELLS / 2)] = {
    let mut floor_tile_rows = [0; 1 << (FLOOR_TILE_CELLS / 2)];
    let mut cell_run = 0;
    while cell_run < floor_tile_rows.len() {
        let mut cell_place = 0;
        while cell_place < FLOOR_TILE_CELLS / 2 {
            if cell_run >> cell_place & 1 == 1 {
                let (x, y) = morton_coordinates(cell_place);
                floor_tile_rows[cell_run] |= 1 << (y as u32 * WINDOW_SIDE + x as u32);
            }
            cell_place += 1;
        }
        cell_run += 1;
    }
    floor_tile_rows
};

/// How far left and up a context reaches, in cells: the square from
/// that far up and left of a cell to the cell itself -- its
/// neighbourhood -- holds all of its context.
const CONTEXT_REACH: u32 = 2;
/// A neighbourhood's side, in cells.
const NEIGHBOURHOOD_SIDE: u32 = CONTEXT_REACH + 1;
const _: () = {
    let mut context_cell_index = 0;
    while context_cell_index < CONTEXT_CELLS.len() {
        let (dx, dy) = CONTEXT_CELLS[context_cell_index];
        assert!(dx <= 0 && dy <= 0 && -dx as u32 <= CONTEXT_REACH && -dy as u32 <= CONTEXT_REACH, "every context cell is in the neighbourhood");
        context_cell_index += 1;
    }
};

/// Every neighbourhood's context, by its cells, a bit each, row after
/// row: a bit for each of [`CONTEXT_CELLS`] set.
const NEIGHBOURHOOD_CONTEXTS: [u8; 1 << (NEIGHBOURHOOD_SIDE * NEIGHBOURHOOD_SIDE)] = {
    let mut neighbourhood_contexts = [0; 1 << (NEIGHBOURHOOD_SIDE * NEIGHBOURHOOD_SIDE)];
    let mut neighbourhood_cells = 0;
    while neighbourhood_cells < neighbourhood_contexts.len() {
        let mut context_bit = 0;
        while context_bit < CONTEXT_CELLS.len() {
            let (dx, dy) = CONTEXT_CELLS[context_bit];
            let x = (CONTEXT_REACH as i32 + dx as i32) as u32;
            let y = (CONTEXT_REACH as i32 + dy as i32) as u32;
            if neighbourhood_cells >> (y * NEIGHBOURHOOD_SIDE + x) & 1 == 1 {
                neighbourhood_contexts[neighbourhood_cells] |= 1 << context_bit;
            }
            context_bit += 1;
        }
        neighbourhood_cells += 1;
    }
    neighbourhood_contexts
};

impl Window {
    /// The window of the floor tile at `cell_place`, read off `cells`.
    #[inline]
    pub(crate) fn around(cells: &Bitmap, floor_tile_index: usize) -> Self {
        let rows_of = |floor_tile: Option<usize>| {
            floor_tile.map_or(0, |floor_tile| {
                let floor_tile_run = cells.morton_run(floor_tile * FLOOR_TILE_CELLS, FLOOR_TILE_CELLS);
                FLOOR_TILE_ROWS[floor_tile_run as usize & 0xFF] | FLOOR_TILE_ROWS[floor_tile_run as usize >> (FLOOR_TILE_CELLS / 2)] << (2 * WINDOW_SIDE)
            })
        };
        // The floor tiles left and above, one step back in x or y: each a field
        // of the Morton index, decremented in place.
        let (x_bits, y_bits) = (floor_tile_index & FLOOR_X_BITS, floor_tile_index & FLOOR_Y_BITS);
        let (x_before, y_before) = (x_bits.wrapping_sub(1) & FLOOR_X_BITS, y_bits.wrapping_sub(1) & FLOOR_Y_BITS);
        let (has_left, has_above) = (x_bits != 0, y_bits != 0);
        let floor_tile_row = FLOOR_TILE_SIDE * WINDOW_SIDE;
        Self(
            rows_of((has_left && has_above).then_some(x_before | y_before))
                | rows_of(has_above.then_some(x_bits | y_before)) << FLOOR_TILE_SIDE
                | rows_of(has_left.then_some(x_before | y_bits)) << floor_tile_row
                | rows_of(Some(floor_tile_index)) << (floor_tile_row + FLOOR_TILE_SIDE),
        )
    }

    /// The context of the floor tile's cell at `place` in its Morton order:
    /// its neighbourhood's, three rows of the window.
    #[inline]
    pub(crate) fn cell_context(&self, place: usize) -> usize {
        let top_left = WINDOW_PLACES[place] - CONTEXT_REACH * (WINDOW_SIDE + 1);
        let neighbourhood_row = |dy: u32| (self.0 >> (top_left + dy * WINDOW_SIDE)) as usize & ((1 << NEIGHBOURHOOD_SIDE) - 1);
        NEIGHBOURHOOD_CONTEXTS[neighbourhood_row(0) | neighbourhood_row(1) << NEIGHBOURHOOD_SIDE | neighbourhood_row(2) << (2 * NEIGHBOURHOOD_SIDE)] as usize
    }
}

/// Each of a floor tile's cells, by its place in the floor tile's Morton order:
/// its bit in the window.
pub(crate) const WINDOW_PLACES: [u32; FLOOR_TILE_CELLS] = {
    let mut places = [0; FLOOR_TILE_CELLS];
    let mut place = 0;
    while place < FLOOR_TILE_CELLS {
        let (x, y) = morton_coordinates(place);
        places[place] = (FLOOR_TILE_SIDE + y as u32) * WINDOW_SIDE + FLOOR_TILE_SIDE + x as u32;
        place += 1;
    }
    places
};
