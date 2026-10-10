//! What the tiers' tests share: no test program of its own
//! (`Cargo.toml`), a module of each tier that uses it.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.

// A tier uses what it needs of it.
#![allow(dead_code)]

use bitmap::{Bitmap, CellWords, BITS_PER_WORD, WORDS};
use bitplane_manager::{WritesApplied, BitmapArena, Write, WriteOp};
use chunk_storage::{ChunkStorage, HeightMap, LayerChange, LayerCodec, LayerType, SuperchunkImage};
use bitmap::morton::morton_index;
use coordinates::{CellIndex, ChunkIndex, SuperchunkIndex, CELLS_IN_CHUNK, CHUNKS_IN_SUPERCHUNK};


/// A cell of a chunk, cartesian: across and down from its top left --
/// drawn from the run's seed, the same one all through a run.
pub fn a_cell() -> (u8, u8) {
    let drawn = utilities::rng::Rng::new(utilities::seed::counted()).draw();
    (drawn as u8, (drawn >> 8) as u8)
}

/// A chunk's place in its superchunk, drawn from the run's seed: the
/// same one all through a run.
pub fn a_place() -> usize {
    (utilities::rng::Rng::new(!utilities::seed::counted()).draw() % CHUNKS_IN_SUPERCHUNK as u64) as usize
}

/// The superchunk at the world's top left corner.
pub const ORIGIN: SuperchunkIndex = SuperchunkIndex(0);

/// A bitmap's cells, with a rectangle and a circle drawn.
pub fn drawn() -> CellWords {
    let mut bitmap = Bitmap::new();
    bitmap.set_rect(10, 10, 40, 30);
    bitmap.set_circle(180, 180, 25);
    *bitmap.words()
}

/// Queues `op` on `cell` of `layer_type`'s bitplane, and applies it:
/// what applying did.
pub fn write(arena: &mut BitmapArena, layer_type: LayerType, op: WriteOp, cell: CellIndex) -> WritesApplied {
    arena.queue(layer_type, Write::cell(cell, op));
    arena.apply()
}

/// The cell `(x, y)` across and down from `chunk`'s top left.
pub fn cell_in(chunk: ChunkIndex, (x, y): (u8, u8)) -> CellIndex {
    CellIndex::of(chunk, morton_index(x, y))
}

/// A bitmap's cells with only `(x, y)` set.
pub fn one_cell((x, y): (u8, u8)) -> CellWords {
    let mut bitmap = Bitmap::new();
    bitmap.set(x, y);
    *bitmap.words()
}

/// Chunk storage holding `superchunk`, whose chunk at `place` has
/// `layers`, each with its cells.
pub fn storage_with(superchunk: SuperchunkIndex, place: usize, layers: &[(LayerType, CellWords)], codec: &mut LayerCodec) -> ChunkStorage {
    let encoded: Vec<(LayerType, Vec<u64>)> = layers.iter().map(|(layer_type, cells)| (*layer_type, codec.encode(cells).to_vec())).collect();
    let changes: Vec<LayerChange> =
        encoded.iter().map(|(layer_type, words)| LayerChange { place, layer_type: *layer_type, encoded: words }).collect();
    let mut storage = ChunkStorage::new(1 << 12);
    storage.insert(superchunk, SuperchunkImage::new(&HeightMap::default()).rewritten(&changes));
    storage
}

/// The mock superchunk's dirt: every cell its grass is not on.
pub const MOCK_DIRT: LayerType = LayerType(1);
/// The mock superchunk's grass.
pub const MOCK_GRASS: LayerType = LayerType(2);

/// A superchunk of dirt, flat at height 0, with grass on `grass_cells`
/// cells drawn at random from `seed` -- fewer if a cell is drawn twice.
/// Each chunk has a dirt layer, and a grass layer if any grass fell on
/// it.
pub fn grass_on_dirt(seed: u64, grass_cells: usize, codec: &mut LayerCodec) -> SuperchunkImage {
    let mut grass: [CellWords; CHUNKS_IN_SUPERCHUNK] = [[0; WORDS]; CHUNKS_IN_SUPERCHUNK];
    let mut state = seed | 1;
    for _ in 0..grass_cells {
        // xorshift64*: enough for scattering cells.
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let cell = state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 44;
        let (chunk, place) = (cell as usize / CELLS_IN_CHUNK, cell as usize % CELLS_IN_CHUNK);
        grass[chunk][place / BITS_PER_WORD] |= 1 << (place % BITS_PER_WORD);
    }
    let mut encoded: Vec<(usize, LayerType, Vec<u64>)> = Vec::new();
    for (chunk, grass) in grass.iter().enumerate() {
        encoded.push((chunk, MOCK_DIRT, codec.encode(&grass.map(|word| !word)).to_vec()));
        if grass.iter().any(|&word| word != 0) {
            encoded.push((chunk, MOCK_GRASS, codec.encode(grass).to_vec()));
        }
    }
    let changes: Vec<LayerChange> = encoded.iter().map(|(chunk, layer_type, words)| LayerChange { place: *chunk, layer_type: *layer_type, encoded: words }).collect();
    SuperchunkImage::new(&HeightMap::default()).rewritten(&changes)
}
