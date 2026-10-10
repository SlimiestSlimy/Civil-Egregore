//! Civil Egregore's bitplane manager: the bitmap arena -- the hot
//! bitmaps, raw, one a bucket -- where cells are read and changed.
//! Chunk storage holds whole encoded layers only.
//!
//! The design: `docs/bitplane_manager.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

mod arena;
pub mod diagnostics;
mod making_hot;
mod superchunk_layer;
mod reader;
mod superchunk;
pub mod transient_data;
mod write_back;
mod writes;

pub use arena::BitmapArena;
pub use reader::{Reader, Window};
pub use superchunk::{Bucket, LayerView, Superchunk};
pub use writes::{count_missed, WritesApplied, Shape, Write, WriteOp, WriteQueues};

use bitmap::{BITS_PER_WORD, WORDS};
use coordinates::CHUNKS_IN_SUPERCHUNK;

pub use chunk_storage::BucketKey;

/// A cell was asked of a bitmap that is not hot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotHot(pub BucketKey);

/// A set of a superchunk's chunks, one bit each, by place.
type ChunkSet = u16;

const _: () = assert!(ChunkSet::BITS as usize == CHUNKS_IN_SUPERCHUNK, "a chunk set holds a bit for every chunk");

/// Whether `set` holds the chunk at `index`.
fn contains(set: ChunkSet, index: usize) -> bool {
    set >> index & 1 == 1
}

/// Puts the chunk at `index` in `set`, or takes it out.
fn put(set: &mut ChunkSet, index: usize, member: bool) {
    if member {
        *set |= 1 << index;
    } else {
        *set &= !(1 << index);
    }
}

/// The chunks in `set`, in Morton order.
fn members(set: ChunkSet) -> impl Iterator<Item = usize> {
    (0..CHUNKS_IN_SUPERCHUNK).filter(move |&index| contains(set, index))
}

/// The most bits a cell a layer has.
const WIDEST: usize = 16;

/// A bucket with no cell set, however wide: what a chunk with no bucket
/// is read as.
static NO_CELLS: [u64; WORDS * WIDEST] = [0; WORDS * WIDEST];

/// Words in a count tile: the 32x32 cells whose set cells are counted,
/// so sampling and the far search pass over an empty one whole. Two
/// lines of memory, so a cell is found in its count tile in a short walk.
pub const COUNT_TILE_WORDS: usize = 16;
/// Count tiles in a bitmap.
pub const COUNT_TILES_IN_CHUNK: usize = WORDS / COUNT_TILE_WORDS;
/// Cells in a count tile.
const COUNT_TILE_CELLS: usize = COUNT_TILE_WORDS * BITS_PER_WORD;

/// The coarsest scale [`Reader::any_in_tile`] is asked of: tiles `2^6`
/// cells a side, four count tiles each.
pub const COARSEST_SCALE: u32 = 6;
/// Tiles of the coarsest scale in a chunk.
pub const COARSEST_TILES_IN_CHUNK: usize = 16;
/// Count tiles in a tile of the coarsest scale.
const COUNTS_IN_COARSEST: usize = COUNT_TILES_IN_CHUNK / COARSEST_TILES_IN_CHUNK;
const _: () = assert!(COUNT_TILE_CELLS * COUNTS_IN_COARSEST == 1 << (2 * COARSEST_SCALE));
