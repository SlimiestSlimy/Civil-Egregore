//! What the tiers' tests share: no test program of its own
//! (`Cargo.toml`), a module of each tier that uses it.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.

// A tier uses what it needs of it.
#![allow(dead_code)]

use bitplane_manager::{BitmapArena, BucketKey};
use chunk_storage::{LayerCodec, LayerType};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::{Attribute, Entities, EntityType, Header, remove_attribute, set_attribute};
use simulation::Turn;


/// The layer type the arena holds: none of its cells are read.
pub const STONE: LayerType = LayerType::of_a_bit(6);
/// The entities' type.
pub const WALKER: EntityType = EntityType::of(40);
/// An attribute counting the times an entity woke.
pub const WOKEN: Attribute<u64> = Attribute::new(41);
/// An attribute a test tells its entities apart by: a number each.
pub const TAG: Attribute<u64> = Attribute::new(43);
/// An attribute present every other time an entity woke.
pub const ODD: Attribute<u64> = Attribute::new(42);

/// An arena with a bitmap hot over the `side` by `side` superchunks from
/// `(10, 10)`, and entities holding the same superchunks.
pub fn world(side: u32) -> (BitmapArena, Entities) {
    let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
    for y in 10..10 + side {
        for x in 10..10 + side {
            for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
                arena.make_hot(BucketKey { layer_type: STONE, chunk }, None, &mut codec);
            }
        }
    }
    let mut entities = Entities::new();
    assert_eq!(entities.align(&arena.superchunk_indices()), 0);
    (arena, entities)
}

/// The cell `(x, y)` cells from the top left of the superchunk `(10, 10)`.
pub fn cell(x: u32, y: u32) -> CellIndex {
    CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + x, y: 10 * SUPERCHUNK_SIDE_CELLS + y }.into()
}

/// A walker on `at`, waking at `wake`.
pub fn walker(at: CellIndex, wake: u64) -> Header {
    Header { kind: WALKER, at, wake }
}

/// Each walker woken counts it, flips `ODD`, and wakes next tick, where
/// it stands: how many woke.
pub fn count_and_flip(turn: &mut Turn) -> usize {
    let mut woken = 0;
    let mut attributes = Vec::new();
    for entity in turn.woken() {
        attributes.clear();
        attributes.extend_from_slice(entity.attributes);
        set_attribute(&mut attributes, WOKEN, entity.attribute(WOKEN).unwrap_or(0) + 1);
        if !remove_attribute(&mut attributes, ODD.attribute_type()) {
            set_attribute(&mut attributes, ODD, 1);
        }
        actions::entities::update(turn, &entity.header, Header { wake: turn.now() + 1, ..entity.header }, &attributes);
        woken += 1;
    }
    woken
}

/// `count` cells drawn from the run's seed, no two the same, each
/// within `side` cells across and down of the superchunk `(10, 10)`'s
/// top left: the same ones whenever asked in a run.
pub fn cells_drawn(count: usize, side: u32) -> Vec<CellIndex> {
    let mut random = utilities::rng::Rng::new(utilities::seed::counted());
    let mut cells = std::collections::BTreeSet::new();
    while cells.len() < count {
        cells.insert((random.below(u64::from(side)) as u32, random.below(u64::from(side)) as u32));
    }
    // In the order drawn in, not the set's: shuffled by a draw each.
    let mut cells: Vec<(u64, CellIndex)> = cells.into_iter().map(|(x, y)| (random.draw(), cell(x, y))).collect();
    cells.sort_unstable();
    cells.into_iter().map(|(_, cell)| cell).collect()
}

/// Queues an entity made as `header` says, with `attributes`, if its
/// cell is free by then.
pub fn put(turn: &mut simulation::Turn, header: Header, attributes: &[entity_manager::AttributeBlock]) {
    actions::entities::spawn(turn, header.kind, header.at, header.wake, attributes);
}

/// [`put`], the compare of its cell left open until
/// `actions::entities::made`.
pub fn make(turn: &mut simulation::Turn, header: Header, attributes: &[entity_manager::AttributeBlock]) {
    actions::entities::make(turn, header.kind, header.at, header.wake, attributes);
}
