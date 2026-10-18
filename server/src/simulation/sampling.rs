//! The stochastic cell automaton's sampling of the hot bitplanes: every set cell of a layer
//! type chosen with one chance, in Morton order, none wasted -- the gap
//! to the next drawn in whole numbers, the counts finding it
//! (`docs/simulation.md`, "Sampling").

use utilities::bitmap::BITS_PER_WORD;
use crate::bitplane_manager::{BitmapArena, LayerView, COUNT_TILE_WORDS};
use utilities::coordinates::{CellIndex, ChunkIndex, SuperchunkIndex, CELLS_IN_CHUNK, CHUNKS_IN_SUPERCHUNK};
use crate::chunk_storage::LayerType;
use utilities::chance::Chance;
use utilities::rng::RandomSource;

/// The position of the `set_bit_rank`-th set bit of `word`, counting from 0 at
/// the lowest; `word` has more set bits than `set_bit_rank`.
fn select_set_bit(mut word: u64, set_bit_rank: u32) -> u32 {
    for _ in 0..set_bit_rank {
        word &= word - 1;
    }
    word.trailing_zeros()
}

/// Chooses each hot set cell of `layer` -- of `superchunk` -- with
/// `chance`, independently,
/// and hands every chosen cell to `emit_sample` in Morton order: how many were
/// chosen. What always happens chooses every set cell; what never
/// does, none.
pub fn sample_layer(superchunk: SuperchunkIndex, layer: LayerView, chance: Chance, random_stream: &mut RandomSource, emit_sample: &mut impl FnMut(CellIndex)) -> usize {
    if chance.is_never() || layer.hot_count() == 0 {
        return 0;
    }
    // How many set cells are passed over before the next chosen one.
    let draw_gap = |gap_random_stream: &mut RandomSource| if chance.is_always() { 0 } else { chance.passed_over(gap_random_stream.next_draw()) };
    // The rank, among the layer's hot set cells still ahead, of the next
    // one chosen.
    let mut next_rank = draw_gap(random_stream);
    if next_rank >= layer.hot_count() as u64 {
        return 0;
    }
    let mut chosen_count = 0;
    for chunk in 0..CHUNKS_IN_SUPERCHUNK {
        if !layer.is_hot(chunk) {
            continue;
        }
        let count = layer.count(chunk) as u64;
        if next_rank >= count {
            next_rank -= count;
            continue;
        }
        let (cells, tile_counts, chunk_index) = (layer.cells(chunk), layer.tile_counts(chunk), ChunkIndex::from_superchunk_place(superchunk, chunk));
        // Set cells in the words before `word`, and in those before its count tile.
        let (mut word, mut set_cells_before_word, mut before_tile) = (0, 0u64, 0u64);
        while next_rank < count {
            // The count tiles before the chosen cell's passed over whole, by their counts...
            loop {
                let after_tile = before_tile + tile_counts[word / COUNT_TILE_WORDS] as u64;
                if next_rank < after_tile {
                    break;
                }
                (before_tile, set_cells_before_word, word) = (after_tile, after_tile, (word / COUNT_TILE_WORDS + 1) * COUNT_TILE_WORDS);
            }
            // ...then the words before it in its count tile, by their bits'.
            loop {
                let word_set_cells = cells[word].count_ones() as u64;
                if set_cells_before_word + word_set_cells > next_rank {
                    break;
                }
                set_cells_before_word += word_set_cells;
                word += 1;
            }
            let bit_in_word = select_set_bit(cells[word], (next_rank - set_cells_before_word) as u32);
            emit_sample(CellIndex::from_chunk_place(chunk_index, word * BITS_PER_WORD + bit_in_word as usize));
            chosen_count += 1;
            next_rank += 1 + draw_gap(random_stream);
        }
        next_rank -= count;
    }
    chosen_count
}

/// Chooses each tile of `layer`'s hot chunks -- of `superchunk`, a
/// plane of tiles of `tile_shift` (twice its scale) -- with `chance`,
/// independently, whatever number it holds, and hands the first cell
/// of every chosen tile to `emit_sample` in Morton order: how many were
/// chosen. A plane that holds a number for every tile is sampled so:
/// none of its tiles is unset.
pub fn sample_tiles_of_layer(superchunk: SuperchunkIndex, layer: LayerView, tile_shift: u32, chance: Chance, random_stream: &mut RandomSource, emit_sample: &mut impl FnMut(CellIndex)) -> usize {
    if chance.is_never() {
        return 0;
    }
    let tiles = (CELLS_IN_CHUNK >> tile_shift) as u64;
    let draw_gap = |gap_random_stream: &mut RandomSource| if chance.is_always() { 0 } else { chance.passed_over(gap_random_stream.next_draw()) };
    // The rank, among the tiles of the hot chunks still ahead, of the next one chosen.
    let (mut next_tile_rank, mut chosen_count) = (draw_gap(random_stream), 0);
    for chunk in (0..CHUNKS_IN_SUPERCHUNK).filter(|&chunk| layer.is_hot(chunk)) {
        while next_tile_rank < tiles {
            emit_sample(CellIndex::from_chunk_place(ChunkIndex::from_superchunk_place(superchunk, chunk), (next_tile_rank as usize) << tile_shift));
            chosen_count += 1;
            next_tile_rank += 1 + draw_gap(random_stream);
        }
        next_tile_rank -= tiles;
    }
    chosen_count
}

/// Chooses each set cell of `layer_type`'s hot bitmaps in `bitmap_arena` with
/// `chance`, independently, and hands every chosen cell to `emit_sample`
/// in Morton order -- superchunk by superchunk: how many were chosen.
pub fn sample(bitmap_arena: &BitmapArena, layer_type: LayerType, chance: Chance, random_stream: &mut RandomSource, mut emit_sample: impl FnMut(CellIndex)) -> usize {
    bitmap_arena
        .superchunks()
        .iter()
        .filter_map(|superchunk| superchunk.layer(layer_type).map(|layer| (superchunk.superchunk_index(), layer)))
        .map(|(superchunk, layer)| sample_layer(superchunk, layer, chance, random_stream, &mut emit_sample))
        .sum()
}
