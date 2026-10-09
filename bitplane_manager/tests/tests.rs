//! What the tiers' tests share: no test program of its own
//! (`Cargo.toml`), a module of each tier that uses it.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.

// A tier uses what it needs of it.
#![allow(dead_code)]

use bitmap::{Bitmap, CellWords};
use bitplane_manager::{WritesApplied, BitmapArena, Write, WriteOp};
use chunk_storage::{ChunkStorage, HeightMap, LayerChange, LayerCodec, LayerType, SuperchunkImage};
use bitmap::morton::morton_index;
use coordinates::{CellIndex, ChunkIndex, SuperchunkIndex};


/// A cell of a chunk, cartesian: across and down from its top left.
pub const CELL: (u8, u8) = (3, 200);

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
