//! A superchunk's planes, kept sparse: a superchunk keeps a plane only
//! if it has a cell set, a plane keeps a chunk only if it has a cell
//! set ([`hot_plane`]); and the reads of planes across the hot
//! superchunks ([`hot_plane_reads`]).

pub(crate) mod hot_plane_reads;
pub(crate) mod hot_plane;

use utilities::bitmap::{BITS_PER_WORD, WORDS};
use utilities::coordinates::{SuperchunkIndex, CHUNKS_IN_SUPERCHUNK};

/// A set of a superchunk's chunks, one bit each, by place.
pub(crate) type ChunkSet = u16;

const _: () = assert!(ChunkSet::BITS as usize == CHUNKS_IN_SUPERCHUNK, "a chunk set holds a bit for every chunk");

/// Whether `chunk_set` holds the chunk at `chunk_place`.
pub(crate) const fn contains(chunk_set: ChunkSet, chunk_place: usize) -> bool {
    chunk_set >> chunk_place & 1 == 1
}

/// The chunks in `chunk_set`, in Morton order.
pub(crate) fn members(chunk_set: ChunkSet) -> impl Iterator<Item = usize> {
    (0..CHUNKS_IN_SUPERCHUNK).filter(move |&chunk_place| contains(chunk_set, chunk_place))
}

/// Words in a count tile: the 32x32 cells whose cells set are counted,
/// so sampling and the far reads pass over an empty one whole.
pub(crate) const COUNT_TILE_WORDS: usize = 16;

/// Count tiles in a chunk.
pub(crate) const COUNT_TILES_IN_CHUNK: usize = WORDS / COUNT_TILE_WORDS;

/// Cells in a count tile.
pub(crate) const COUNT_TILE_CELLS: usize = COUNT_TILE_WORDS * BITS_PER_WORD;

/// The coarsest scale a tile is read at whole ([`hot_plane_reads::HotPlaneReader::any_in_tile`]):
/// tiles `2^6` cells a side, four count tiles each.
pub(crate) const COARSEST_SCALE: u32 = 6;

/// Tiles of the coarsest scale in a chunk.
pub(crate) const COARSEST_TILES_IN_CHUNK: usize = 16;

/// Count tiles in a tile of the coarsest scale.
pub(crate) const COUNTS_IN_COARSEST: usize = COUNT_TILES_IN_CHUNK / COARSEST_TILES_IN_CHUNK;

const _: () = assert!(COUNT_TILE_CELLS * COUNTS_IN_COARSEST == 1 << (2 * COARSEST_SCALE), "a coarsest tile is whole count tiles");

/// A cell was read in a superchunk that is not hot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotHot(pub SuperchunkIndex);
