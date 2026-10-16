//! Complex tiles' payloads, written and read: a value for every tile of
//! the resolution, or, at 1x1, a cell list of the set cells.
//! `docs/tessera.md`, "Payloads and cell lists".
//!
//! Function by function: `docs/reference.md`, "`payload_writer.rs`".

use crate::bit_stream::{gamma_bits, BitReader, BitSink};
use crate::tile::{cells_in_tile, tiles_in_level, Tile, CELL_LEVEL};
use bitmap::Bitmap;

/// A complex tile's payload: the value of each of `tile`'s tiles
/// `size_offset` levels finer, in Morton order -- at 1x1, its cells as
/// they lie in the bitmap, a word at a time.
pub fn write_payload(bit_sink: &mut impl BitSink, bitmap: &Bitmap, tile: Tile, size_offset: u8) {
    if let Some(bit_count) = bit_sink.counted_bits() {
        *bit_count += tiles_in_level(size_offset) as u64;
        return;
    }
    if tile.tile_level + size_offset == CELL_LEVEL {
        let width = cells_in_tile(tile.tile_level).min(u64::BITS as usize) as u8;
        for word in bitmap.tile_words(tile.top_left_cell(), tile.side_in_cells()) {
            bit_sink.push_value(word, width);
        }
        return;
    }
    for resolution_tile in tile.tiles_under(size_offset) {
        bit_sink.push(resolution_tile.top_left_value(bitmap));
    }
}

/// Reads what [`write_payload`] wrote into `cells`.
pub fn read_payload(stream_reader: &mut BitReader, cells: &mut Bitmap, tile: Tile, size_offset: u8) {
    let (corner_cell, tile_side_cells) = (tile.top_left_cell(), tile.side_in_cells());
    if tile.tile_level + size_offset != CELL_LEVEL {
        for resolution_tile in tile.tiles_under(size_offset) {
            if stream_reader.read_bit() {
                cells.set_tile(resolution_tile.top_left_cell(), resolution_tile.side_in_cells());
            }
        }
    } else if tile_side_cells * tile_side_cells < u64::BITS as usize {
        cells.set_in_small_tile(corner_cell, tile_side_cells, stream_reader.value((tile_side_cells * tile_side_cells) as u8));
    } else {
        for word in cells.tile_words_mut(corner_cell, tile_side_cells) {
            *word |= stream_reader.value(u64::BITS as u8);
        }
    }
}

/// The low bits of every gap, given `cells` in the tile and `set_cells` of
/// them set: the whole part of log2 of the mean gap, `(cells - set) /
/// set`, so the unary high part is short on average.
fn rice_parameter(cells: u64, set_cells: u64) -> u8 {
    let mean_gap = (cells - set_cells) / set_cells.max(1);
    mean_gap.checked_ilog2().unwrap_or(0) as u8
}

/// The fewest bits the cell list of a tile of `tile_level`, `set_cells` of its
/// cells set, could take: its count and every gap's unary end and low
/// bits -- every gap's high part taken as nothing.
pub fn cell_list_least_bits(tile_level: u8, set_cells: u64) -> u64 {
    let gap_low_bits = rice_parameter(cells_in_tile(tile_level) as u64, set_cells);
    gamma_bits(set_cells + 1) + set_cells * (1 + gap_low_bits as u64)
}

/// Writes `tile`'s cell list.
pub fn write_cell_list(bit_sink: &mut impl BitSink, bitmap: &Bitmap, tile: Tile) {
    let (corner_cell, tile_side_cells) = (tile.top_left_cell(), tile.side_in_cells());
    let set_cells = bitmap.tile_words(corner_cell, tile_side_cells).map(|word| word.count_ones() as u64).sum();
    bit_sink.push_gamma(set_cells + 1);
    let gap_low_bits = rice_parameter(cells_in_tile(tile.tile_level) as u64, set_cells);
    let mut next_place = 0;
    for place in bitmap.set_cells_in_tile(corner_cell, tile_side_cells) {
        let gap = (place - next_place) as u64;
        bit_sink.push_unary(gap >> gap_low_bits);
        bit_sink.push_value(gap, gap_low_bits);
        next_place = place + 1;
    }
}

/// Reads a cell list for `tile`, setting its cells in `cell_values`;
/// every other cell of the tile stays as it is.
pub fn read_cell_list(stream_reader: &mut BitReader, tile: Tile, cell_values: &mut Bitmap) {
    let set_cells = stream_reader.read_gamma() - 1;
    let gap_low_bits = rice_parameter(cells_in_tile(tile.tile_level) as u64, set_cells);
    let mut place = 0;
    for _ in 0..set_cells {
        let gap = stream_reader.read_unary() << gap_low_bits | stream_reader.value(gap_low_bits);
        place += gap as usize;
        cell_values.set_in_tile(tile.top_left_cell(), place);
        place += 1;
    }
}
