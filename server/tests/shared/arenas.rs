//! Hot arenas of stone and walkers on them, as the server's tests share them
//! (the root's keep a TEMPORARY copy, `tests/shared/arenas.rs`).
//! The tiers: `docs/testing_protocol.md`, at the repository's root.

// A tier uses what it needs of it.
#![allow(dead_code)]

use crate::bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp};
use crate::test_types::TestTypes;
use crate::chunk_storage::LayerCodec;
use utilities::coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use crate::entity_manager::WorldEntities;
use protocol::server_types::entity::EntityBaseData;


/// An arena with `types.stone` hot over the `side_superchunks` by `side_superchunks` superchunks from
/// `(10, 10)`, the cells `cells` set.
pub fn stone_arena(types: TestTypes, side_superchunks: u32, cells: impl Iterator<Item = CellIndex>) -> BitmapArena {
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    for (x, y) in (10..10 + side_superchunks).flat_map(|y| (10..10 + side_superchunks).map(move |x| (x, y))) {
        for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
            bitmap_arena.make_hot(BucketKey { layer_type: types.stone, chunk }, None, &mut layer_codec);
        }
    }
    cells.for_each(|cell| bitmap_arena.queue_write(types.stone, Write::cell(cell, WriteOp::SetCell)));
    assert_eq!(bitmap_arena.apply().missed, 0);
    bitmap_arena
}

/// [`bitmap_arena`], no cell set, and entities holding the same superchunks.
pub fn world(types: TestTypes, side_superchunks: u32) -> (BitmapArena, WorldEntities) {
    let (bitmap_arena, mut entity_store) = (stone_arena(types, side_superchunks, std::iter::empty()), WorldEntities::new());
    assert_eq!(entity_store.keep_superchunks(&bitmap_arena.superchunk_indices()), 0);
    (bitmap_arena, entity_store)
}

/// The cell `(x, y)` cells from the top left of the superchunk `(10, 10)`.
pub fn cell(x: u32, y: u32) -> CellIndex {
    CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + x, y: 10 * SUPERCHUNK_SIDE_CELLS + y }.into()
}

/// A walker on `walker_cell`, waking at `wake`.
pub fn walker_base_data(types: TestTypes, walker_cell: CellIndex, wake: u64) -> EntityBaseData {
    EntityBaseData { entity_type: types.walker, stood_cell: walker_cell, wake }
}
