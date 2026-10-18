//! TEMPORARY: a copy of the server's `tests/shared/arenas.rs` -- hot
//! arenas of stone and walkers on them -- for the root's tests that
//! still build on the server's internals (`server::internals_for_root_tests`),
//! until the world has reads of its own (AI.txt, LATER).

// A tier uses what it needs of it.
#![allow(dead_code)]

use server::internals_for_root_tests::{BitmapArena, BucketKey, LayerCodec, WorldEntities, Write, WriteOp};
use server::simulation::simulation_api::{EntityType, Header, LayerType};
use utilities::coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};

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
