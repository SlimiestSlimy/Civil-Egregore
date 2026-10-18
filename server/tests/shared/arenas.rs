//! Hot arenas of stone and walkers on them, as actions' tests and the
//! root's tests share them: in a folder of its own, so no test program
//! of its own, a module of each that uses it.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.

// A tier uses what it needs of it.
#![allow(dead_code)]

use server::bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp};
use server::chunk_storage::{LayerCodec, LayerType};
use utilities::coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use server::entity_manager::{WorldEntities, EntityType, Header};

/// The layer type the arena holds.
pub const STONE_LAYER: LayerType = LayerType::of_a_bit(6);
/// The entities' type.
pub const WALKER_TYPE: EntityType = EntityType::from_number(40);

/// An arena with `STONE_LAYER` hot over the `side_superchunks` by `side_superchunks` superchunks from
/// `(10, 10)`, the cells `cells` set.
pub fn stone_arena(side_superchunks: u32, cells: impl Iterator<Item = CellIndex>) -> BitmapArena {
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    for (x, y) in (10..10 + side_superchunks).flat_map(|y| (10..10 + side_superchunks).map(move |x| (x, y))) {
        for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
            bitmap_arena.make_hot(BucketKey { layer_type: STONE_LAYER, chunk }, None, &mut layer_codec);
        }
    }
    cells.for_each(|cell| bitmap_arena.queue_write(STONE_LAYER, Write::cell(cell, WriteOp::SetCell)));
    assert_eq!(bitmap_arena.apply().missed, 0);
    bitmap_arena
}

/// [`bitmap_arena`], no cell set, and entities holding the same superchunks.
pub fn world(side_superchunks: u32) -> (BitmapArena, WorldEntities) {
    let (bitmap_arena, mut entity_store) = (stone_arena(side_superchunks, std::iter::empty()), WorldEntities::new());
    assert_eq!(entity_store.keep_superchunks(&bitmap_arena.superchunk_indices()), 0);
    (bitmap_arena, entity_store)
}

/// The cell `(x, y)` cells from the top left of the superchunk `(10, 10)`.
pub fn cell(x: u32, y: u32) -> CellIndex {
    CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + x, y: 10 * SUPERCHUNK_SIDE_CELLS + y }.into()
}

/// A walker on `walker_cell`, waking at `wake`.
pub fn walker_header(walker_cell: CellIndex, wake: u64) -> Header {
    Header { entity_type: WALKER_TYPE, stood_cell: walker_cell, wake }
}
