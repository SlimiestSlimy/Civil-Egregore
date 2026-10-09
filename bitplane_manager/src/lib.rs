//! Civil Egregore's bitplane manager: the bitmap arena, the hot bitmaps, raw,
//! one a bucket -- the layers whose cells are being read or changed,
//! decoded from chunk storage's cold pool and nothing more. It is where
//! cells are read and changed: chunk storage holds whole encoded layers
//! only.
//!
//! The arena is made of allocations, each holding one layer type over
//! one superchunk: an array of buckets, one for each of its chunks that
//! has a cell set -- 16 at the most -- in the chunks' Morton order
//! ([`ChunkIndex::place`]). A chunk with no cell set has no bucket: its
//! cells are read as clear, and the first cell set in it makes it one,
//! put in its place among the others. A chunk's bucket is found by how
//! many chunks before it have one -- a count of bits, no search.
//!
//! Which allocation holds which layer type over which superchunk is a
//! small directory: the superchunks in use, sorted by superchunk index,
//! and for each its allocations, sorted by layer type -- the one thing
//! ever sorted, and it holds no bitmaps. The last superchunk looked up
//! is remembered, so the runs of lookups in one superchunk that
//! Morton-ordered work makes search only its few types. The allocations lie
//! wherever they were made; each is one run of memory in Morton order.
//!
//! The arena grows an allocation at a time, as a layer type turns hot
//! over a superchunk it had none of, and an allocation a bucket at a
//! time. An allocation none of whose chunks is hot or waiting in the
//! ring (below) leaves the directory, its buckets with it.
//!
//! Cells are changed by writes, batched (`writes`): queued, then applied
//! in order. Each superchunk ([`Superchunk`]) owns its blocks, so
//! superchunks are read and changed apart: the simulation
//! (`../simulation`) samples and reads them on as many threads as it
//! likes ([`LayerView`], [`Reader`]), and applies writes to each
//! ([`Superchunk::apply`]).
//!
//! A bucket changed since it was decoded is dirty, and is written back
//! into chunk storage's writeback ring (`../chunk_storage`) encoded --
//! no words where no cell is set, since a type with no cell set has no
//! layer -- in two halves, so encoding can be off the tick: taken
//! ([`BitmapArena::take_dirty`]), then put in the ring
//! ([`BitmapArena::written_back`]). A dirty bucket must be written back
//! before it is evicted. The ring is never read to make a bitmap hot,
//! so a bucket written back stays in its allocation, evicted or not,
//! until chunk storage flushes its superchunk into the cold pool: a
//! bitmap evicted and made hot again before then is the bucket as it
//! was, not decoded.
//!
//! A superchunk made cold as a whole ([`BitmapArena::make_cold_superchunk`])
//! leaves the directory at once, nothing encoded or flushed: its
//! allocations are set aside, lingering, until chunk storage holds its
//! changes -- made hot again as they are if wanted before then.

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
