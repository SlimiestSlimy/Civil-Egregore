//! What the tiers' tests share: no test program of its own
//! (`Cargo.toml`), a module of each tier that uses it.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.

// A tier uses what it needs of it.
#![allow(dead_code)]

use bitplane_manager::{BitmapArena, BucketKey};
use chunk_storage::{LayerCodec, LayerType};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::{Attribute, WorldEntities, EntityType, Header, remove_attribute, set_attribute};
use simulation::Turn;


/// The layer type the arena holds: none of its cells are read.
pub const STONE_LAYER: LayerType = LayerType::of_a_bit(6);
/// The entities' type.
pub const WALKER_TYPE: EntityType = EntityType::from_number(40);
/// An attribute counting the times an entity woke.
pub const WOKEN_COUNT: Attribute<u64> = Attribute::new(41);
/// An attribute a test tells its entities apart by: a number each.
pub const ENTITY_NUMBER: Attribute<u64> = Attribute::new(43);
/// An attribute present every other time an entity woke.
pub const ODD_WAKE: Attribute<u64> = Attribute::new(42);

/// An arena with a bitmap hot over the `side_superchunks` by `side_superchunks` superchunks from
/// `(10, 10)`, and entities holding the same superchunks.
pub fn world(side_superchunks: u32) -> (BitmapArena, WorldEntities) {
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    for y in 10..10 + side_superchunks {
        for x in 10..10 + side_superchunks {
            for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
                bitmap_arena.make_hot(BucketKey { layer_type: STONE_LAYER, chunk }, None, &mut layer_codec);
            }
        }
    }
    let mut entity_store = WorldEntities::new();
    assert_eq!(entity_store.keep_superchunks(&bitmap_arena.superchunk_indices()), 0);
    (bitmap_arena, entity_store)
}

/// The cell `(x, y)` cells from the top left of the superchunk `(10, 10)`.
pub fn cell(x: u32, y: u32) -> CellIndex {
    CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + x, y: 10 * SUPERCHUNK_SIDE_CELLS + y }.into()
}

/// A walker on `stood_cell`, waking at `wake`.
pub fn walker_header(stood_cell: CellIndex, wake: u64) -> Header {
    Header { entity_type: WALKER_TYPE, stood_cell, wake }
}

/// Each walker woken counts it, flips `ODD_WAKE`, and wakes next tick, where
/// it stands: how many woke.
pub fn count_and_flip(turn: &mut Turn) -> usize {
    let mut woken_count = 0;
    let mut attributes = Vec::new();
    for entity in turn.woken_entities() {
        attributes.clear();
        attributes.extend_from_slice(entity.attributes);
        set_attribute(&mut attributes, WOKEN_COUNT, entity.attribute(WOKEN_COUNT).unwrap_or(0) + 1);
        if !remove_attribute(&mut attributes, ODD_WAKE.attribute_type()) {
            set_attribute(&mut attributes, ODD_WAKE, 1);
        }
        actions::entities::update_entity(turn, &entity.header, Header { wake: turn.current_tick() + 1, ..entity.header }, &attributes);
        woken_count += 1;
    }
    woken_count
}

/// `count` cells drawn from the run's seed, no two the same, each
/// within `side_in_cells` cells across and down of the superchunk `(10, 10)`'s
/// top left: the same ones whenever asked in a run.
pub fn cells_drawn(count: usize, side_in_cells: u32) -> Vec<CellIndex> {
    let mut seeded_rng = utilities::rng::RandomSource::new(utilities::seed::counted_seed());
    let mut cells = std::collections::BTreeSet::new();
    while cells.len() < count {
        cells.insert((seeded_rng.draw_below(u64::from(side_in_cells)) as u32, seeded_rng.draw_below(u64::from(side_in_cells)) as u32));
    }
    // In the order drawn in, not the set's: shuffled by a draw each.
    let mut cells: Vec<(u64, CellIndex)> = cells.into_iter().map(|(x, y)| (seeded_rng.next_draw(), cell(x, y))).collect();
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
pub fn make_with_compare_open(turn: &mut simulation::Turn, header: Header, attributes: &[entity_manager::AttributeBlock]) {
    actions::entities::begin_making_entity(turn, header.entity_type, header.stood_cell, header.wake, attributes);
}
