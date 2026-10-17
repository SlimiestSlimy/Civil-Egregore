//! Windows: 8x8 cells at any cell as one `u64`, row by row, cut from
//! the word tiles they overlap (`docs/bitmap.md`, "Windows").

/// Cells along a word tile's side, and a window's.
pub const WORD_TILE_SIDE: u32 = 8;

/// The bits of a Morton index that place a cell in its word tile.
pub const PLACE_IN_WORD_TILE: u64 = 63;

/// The column and the row, in its word tile, of the cell whose Morton
/// index -- in the bitmap or the world -- is `cell_morton_index`: the place's even
/// bits, and its odd ones, gathered.
#[inline(always)]
pub const fn in_word_tile(cell_morton_index: u64) -> (u32, u32) {
    let place = cell_morton_index & PLACE_IN_WORD_TILE;
    ((place & 1 | place >> 1 & 2 | place >> 2 & 4) as u32, (place >> 1 & 1 | place >> 2 & 2 | place >> 3 & 4) as u32)
}

/// The bits of a word whose index has bit `low_index_bit` set and bit `high_index_bit`
/// clear: what a delta swap of those two index bits moves up.
const fn swap_mask(low_index_bit: u32, high_index_bit: u32) -> u64 {
    let mut mask = 0u64;
    let mut bit_position = 0;
    while bit_position < u64::BITS {
        if bit_position >> low_index_bit & 1 == 1 && bit_position >> high_index_bit & 1 == 0 {
            mask |= 1 << bit_position;
        }
        bit_position += 1;
    }
    mask
}

/// `word` with its bits' index bits `low_index_bit` and `high_index_bit` exchanged: the bits
/// at [`swap_mask`] and those `2^high_index_bit - 2^low_index_bit` above them traded.
const fn swap_index_bits(word: u64, low_index_bit: u32, high_index_bit: u32) -> u64 {
    let swap_distance = (1 << high_index_bit) - (1 << low_index_bit);
    let mask = swap_mask(low_index_bit, high_index_bit);
    let traded_bits = ((word >> swap_distance) ^ word) & mask;
    word ^ traded_bits ^ (traded_bits << swap_distance)
}

/// The index bits exchanged, in order, turning Morton order into rows.
const MORTON_TO_ROWS: [(u32, u32); 3] = [(1, 2), (2, 4), (3, 4)];

/// A word tile -- one Morton-ordered word of a bitmap -- row by row:
/// cell `(x, y)` of the tile at bit `y * 8 + x`.
pub const fn rows_from_morton(word: u64) -> u64 {
    let mut row_ordered_bits = word;
    let mut step = 0;
    while step < MORTON_TO_ROWS.len() {
        row_ordered_bits = swap_index_bits(row_ordered_bits, MORTON_TO_ROWS[step].0, MORTON_TO_ROWS[step].1);
        step += 1;
    }
    row_ordered_bits
}

/// [`rows_from_morton`] undone: a tile's rows as a Morton-ordered word.
pub const fn morton_from_rows(row_ordered_bits: u64) -> u64 {
    let mut word = row_ordered_bits;
    let mut step = MORTON_TO_ROWS.len();
    while step > 0 {
        step -= 1;
        word = swap_index_bits(word, MORTON_TO_ROWS[step].0, MORTON_TO_ROWS[step].1);
    }
    word
}

/// The first `column_count` columns of every row (0 to 8).
pub const fn left_columns(column_count: u32) -> u64 {
    let one_row_mask = ((1u16 << column_count) - 1) as u64;
    one_row_mask * 0x0101_0101_0101_0101
}

/// The first `row_count` rows (0 to 8).
pub const fn top_rows(row_count: u32) -> u64 {
    if row_count >= WORD_TILE_SIDE { u64::MAX } else { (1 << (row_count * WORD_TILE_SIDE)) - 1 }
}

/// The 8x8 window, row by row, whose top left cell is `(columns_across, rows_down)`
/// -- each 0 to 7 -- in the 16x16 square made of four word tiles, each
/// row by row: `[[top_pair_window left, top_pair_window right], [bottom_pair_window left, bottom_pair_window right]]`.
pub const fn window(word_tiles: [[u64; 2]; 2], columns_across: u32, rows_down: u32) -> u64 {
    let top_pair_window = window_across_pair(word_tiles[0][0], word_tiles[0][1], columns_across);
    let bottom_pair_window = window_across_pair(word_tiles[1][0], word_tiles[1][1], columns_across);
    let rows_from_bottom_pair = match bottom_pair_window.checked_shl(WORD_TILE_SIDE * (WORD_TILE_SIDE - rows_down)) {
        Some(shifted_bottom_rows) => shifted_bottom_rows,
        None => 0,
    };
    top_pair_window >> (WORD_TILE_SIDE * rows_down) | rows_from_bottom_pair
}

/// The 8x8 window `columns_across` columns into two word tiles side by side,
/// row by row: the left one's columns from `columns_across` on, then the right
/// one's first `columns_across`.
const fn window_across_pair(left_word_tile: u64, right_word_tile: u64, columns_across: u32) -> u64 {
    let left_tile_columns = left_columns(WORD_TILE_SIDE - columns_across);
    (left_word_tile >> columns_across) & left_tile_columns | (right_word_tile << (WORD_TILE_SIDE - columns_across)) & !left_tile_columns
}
