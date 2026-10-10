//! Monte Carlo sampling of the hot bitplanes: every set cell of a layer
//! type chosen with one chance, in Morton order, none wasted -- the gap
//! to the next drawn in whole numbers, the counts finding it
//! (`docs/simulation.md`, "Sampling").

use bitmap::BITS_PER_WORD;
use bitplane_manager::{BitmapArena, LayerView, COUNT_TILE_WORDS};
use coordinates::{CellIndex, ChunkIndex, SuperchunkIndex, CHUNKS_IN_SUPERCHUNK};
use chunk_storage::LayerType;
use utilities::chance::Chance;
use utilities::rng::Rng;

/// The position of the `rank`-th set bit of `word`, counting from 0 at
/// the lowest; `word` has more set bits than `rank`.
fn select(mut word: u64, rank: u32) -> u32 {
    for _ in 0..rank {
        word &= word - 1;
    }
    word.trailing_zeros()
}

/// Chooses each hot set cell of `layer` -- of `superchunk` -- with
/// `chance`, independently,
/// and hands every chosen cell to `emit` in Morton order: how many were
/// chosen. What always happens chooses every set cell; what never
/// does, none.
pub fn sample_layer(superchunk: SuperchunkIndex, layer: LayerView, chance: Chance, random: &mut Rng, emit: &mut impl FnMut(CellIndex)) -> usize {
    if chance.is_never() || layer.hot_count() == 0 {
        return 0;
    }
    // How many set cells are passed over before the next chosen one.
    let draw = |random: &mut Rng| if chance.is_always() { 0 } else { chance.passed_over(random.draw()) };
    // The rank, among the layer's hot set cells still ahead, of the next
    // one chosen.
    let mut next = draw(random);
    if next >= layer.hot_count() as u64 {
        return 0;
    }
    let mut chosen = 0;
    for chunk in 0..CHUNKS_IN_SUPERCHUNK {
        if !layer.is_hot(chunk) {
            continue;
        }
        let count = layer.count(chunk) as u64;
        if next >= count {
            next -= count;
            continue;
        }
        let (cells, tile_counts, chunk_index) = (layer.cells(chunk), layer.tile_counts(chunk), ChunkIndex::of(superchunk, chunk));
        // Set cells in the words before `word`, and in those before its count tile.
        let (mut word, mut before, mut before_tile) = (0, 0u64, 0u64);
        while next < count {
            // The count tiles before the chosen cell's passed over whole, by their counts...
            loop {
                let after_tile = before_tile + tile_counts[word / COUNT_TILE_WORDS] as u64;
                if next < after_tile {
                    break;
                }
                (before_tile, before, word) = (after_tile, after_tile, (word / COUNT_TILE_WORDS + 1) * COUNT_TILE_WORDS);
            }
            // ...then the words before it in its count tile, by their bits'.
            loop {
                let ones = cells[word].count_ones() as u64;
                if before + ones > next {
                    break;
                }
                before += ones;
                word += 1;
            }
            let bit = select(cells[word], (next - before) as u32);
            emit(CellIndex::of(chunk_index, word * BITS_PER_WORD + bit as usize));
            chosen += 1;
            next += 1 + draw(random);
        }
        next -= count;
    }
    chosen
}

/// Chooses each set cell of `layer_type`'s hot bitmaps in `arena` with
/// `chance`, independently, and hands every chosen cell to `emit`
/// in Morton order -- superchunk by superchunk: how many were chosen.
pub fn sample(arena: &BitmapArena, layer_type: LayerType, chance: Chance, random: &mut Rng, mut emit: impl FnMut(CellIndex)) -> usize {
    arena
        .superchunks()
        .iter()
        .filter_map(|superchunk| superchunk.layer(layer_type).map(|layer| (superchunk.index(), layer)))
        .map(|(superchunk, layer)| sample_layer(superchunk, layer, chance, random, &mut emit))
        .sum()
}
