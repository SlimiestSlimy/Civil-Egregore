//! Civil Egregore's chunks as stored: the cold pool of superchunk
//! images, each one run of words as on disk, and the writeback ring of
//! changed bitmaps that feeds it. Cells are read and changed in the
//! bitplane manager, never here.
//!
//! The design: `docs/chunk_storage.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

mod chunk_storage;
pub mod disk;
mod chunk_maps;
mod height_map;
pub mod jobs;
mod layer_codec;
mod paging;
mod superchunk_cells;
mod superchunk_image;
pub mod wide;
mod writeback_ring;

pub use chunk_storage::{ChunkStorage, Flush};
pub use paging::{read_back, ImageStored};
pub use chunk_maps::{ChunkMaps, MAP_WORDS};
pub use height_map::{height_in, Height, HeightMap, HEIGHT_WORDS, TALL_WORDS};
pub use layer_codec::{BucketKey, LayerCodec};
pub use type_registry::{Bits16, Bits2, Bits32, Bits4, Bits8, LayerType, WidePlane, Width};
pub use superchunk_cells::SuperchunkCells;
pub use superchunk_image::{InvalidImage, LayerChange, SuperchunkImage};
pub use writeback_ring::{RingEntry, WritebackRing};
