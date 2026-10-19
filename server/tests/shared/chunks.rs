//! Chunks, bitmaps and storage as the tiers' tests share them: in a
//! folder of its own, so no test program of its own, a module of each
//! tier that uses it.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.

// A tier uses what it needs of it.
#![allow(dead_code)]

use utilities::bitmap::{Bitmap, CellWords, BITS_PER_WORD, WORDS};
use crate::test_types::TestTypes;
use crate::bitplane_manager::{WritesApplied, BitmapArena, Write, WriteOp};
use crate::chunk_storage::{ChunkStorage, LayerChange, LayerCodec, LayerType, SuperchunkImage};
use utilities::bitmap::morton::morton_index;
use utilities::coordinates::{CellIndex, ChunkIndex, SuperchunkIndex, CELLS_IN_CHUNK, CHUNKS_IN_SUPERCHUNK};


/// A cell of a chunk, cartesian: across and down from its top left --
/// drawn from the run's seed, the same one all through a run.
pub fn a_cell() -> (u8, u8) {
    let drawn_number = utilities::rng::RandomSource::new(crate::test_seed::TestSeedHandle::open().fine_seed()).next_draw();
    (drawn_number as u8, (drawn_number >> 8) as u8)
}

/// A chunk's place in its superchunk, drawn from the run's seed: the
/// same one all through a run.
pub fn a_place() -> usize {
    (utilities::rng::RandomSource::new(!crate::test_seed::TestSeedHandle::open().fine_seed()).next_draw() % CHUNKS_IN_SUPERCHUNK as u64) as usize
}

/// The superchunk at the world's top left corner.
pub const ORIGIN: SuperchunkIndex = SuperchunkIndex(0);

/// A bitmap's cells, with a rectangle and a circle drawn.
pub fn rectangle_and_circle() -> CellWords {
    let mut bitmap = Bitmap::new();
    bitmap.set_rect(10, 10, 40, 30);
    bitmap.set_circle(180, 180, 25);
    *bitmap.words()
}

/// Queues `write_op` on `cell` of `layer_type`'s bitplane, and applies it:
/// what applying did.
pub fn write(bitmap_arena: &mut BitmapArena, layer_type: LayerType, write_op: WriteOp, cell: CellIndex) -> WritesApplied {
    bitmap_arena.queue_write(layer_type, Write::cell(cell, write_op));
    bitmap_arena.apply()
}

/// The cell `(x, y)` across and down from `chunk`'s top left.
pub fn cell_in(chunk: ChunkIndex, (x, y): (u8, u8)) -> CellIndex {
    CellIndex::from_chunk_place(chunk, morton_index(x, y))
}

/// A bitmap's cells with only `(x, y)` set.
pub fn one_cell((x, y): (u8, u8)) -> CellWords {
    let mut bitmap = Bitmap::new();
    bitmap.set(x, y);
    *bitmap.words()
}

/// Chunk storage holding `superchunk`, whose chunk at `place` has
/// `layers`, each with its cells.
pub fn storage_with(superchunk: SuperchunkIndex, place: usize, layers: &[(LayerType, CellWords)], layer_codec: &mut LayerCodec) -> ChunkStorage {
    let encoded_layers: Vec<(LayerType, Vec<u64>)> = layers.iter().map(|(layer_type, cells)| (*layer_type, layer_codec.encode_cells(cells).to_vec())).collect();
    let layer_changes: Vec<LayerChange> =
        encoded_layers.iter().map(|(layer_type, words)| LayerChange { place, type_id: layer_type.type_id(), encoded_layer: words }).collect();
    let mut cold_storage = ChunkStorage::new(1 << 12);
    cold_storage.insert(superchunk, SuperchunkImage::empty().rewritten_with(&layer_changes));
    cold_storage
}


/// A superchunk of dirt, flat at height 0, with grass on `grass_cells`
/// cells drawn at random from `seed` -- fewer if a cell is drawn twice.
/// Each chunk has a dirt layer, and a grass layer if any grass fell on
/// it.
pub fn grass_on_dirt(types: TestTypes, seed: u64, grass_cells: usize, layer_codec: &mut LayerCodec) -> SuperchunkImage {
    let mut grass_bitmaps: [CellWords; CHUNKS_IN_SUPERCHUNK] = [[0; WORDS]; CHUNKS_IN_SUPERCHUNK];
    let mut xorshift_state = seed | 1;
    for _ in 0..grass_cells {
        // xorshift64*: enough for scattering cells.
        xorshift_state ^= xorshift_state >> 12;
        xorshift_state ^= xorshift_state << 25;
        xorshift_state ^= xorshift_state >> 27;
        let cell = xorshift_state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 44;
        let (chunk, place) = (cell as usize / CELLS_IN_CHUNK, cell as usize % CELLS_IN_CHUNK);
        grass_bitmaps[chunk][place / BITS_PER_WORD] |= 1 << (place % BITS_PER_WORD);
    }
    let mut encoded_layers: Vec<(usize, LayerType, Vec<u64>)> = Vec::new();
    for (chunk, chunk_grass) in grass_bitmaps.iter().enumerate() {
        encoded_layers.push((chunk, types.dirt, layer_codec.encode_cells(&chunk_grass.map(|word| !word)).to_vec()));
        if chunk_grass.iter().any(|&word| word != 0) {
            encoded_layers.push((chunk, types.grass, layer_codec.encode_cells(chunk_grass).to_vec()));
        }
    }
    let layer_changes: Vec<LayerChange> = encoded_layers.iter().map(|(chunk, layer_type, words)| LayerChange { place: *chunk, type_id: layer_type.type_id(), encoded_layer: words }).collect();
    SuperchunkImage::empty().rewritten_with(&layer_changes)
}
