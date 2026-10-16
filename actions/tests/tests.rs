//! What the tiers' tests share: no test program of its own
//! (`Cargo.toml`), a module of each tier that uses it.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.

// A tier uses what it needs of it.
#![allow(dead_code)]

use bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp};
use chunk_storage::{LayerCodec, LayerType};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::{WorldEntities, EntityType, Header};

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

/// `count` cells drawn from the run's seed, no two the same, each
/// within `side_cells` cells across and down of the superchunk `(10, 10)`'s
/// top left: the same ones whenever asked in a run.
pub fn cells_drawn(count: usize, side_cells: u32) -> Vec<CellIndex> {
    let mut seeded_random = utilities::rng::RandomSource::new(utilities::seed::counted_seed());
    let mut cells = std::collections::BTreeSet::new();
    while cells.len() < count {
        cells.insert((seeded_random.draw_below(u64::from(side_cells)) as u32, seeded_random.draw_below(u64::from(side_cells)) as u32));
    }
    // In the order drawn in, not the set's: shuffled by a draw each.
    let mut cells: Vec<(u64, CellIndex)> = cells.into_iter().map(|(x, y)| (seeded_random.next_draw(), cell(x, y))).collect();
    cells.sort_unstable();
    cells.into_iter().map(|(_, cell)| cell).collect()
}

/// Queues an entity made as `header` says, with `attributes`, if its
/// cell is free by then.
pub fn put(turn: &mut simulation::Turn, header: Header, attributes: &[entity_manager::AttributeBlock]) {
    actions::entities::make_entity(turn, header.entity_type, header.stood_cell, header.wake, attributes);
}

/// [`put`], the compare of its cell left open until
/// `actions::entities::made`.
pub fn begin_making_from_header(turn: &mut simulation::Turn, header: Header, attributes: &[entity_manager::AttributeBlock]) {
    actions::entities::begin_making_entity(turn, header.entity_type, header.stood_cell, header.wake, attributes);
}
