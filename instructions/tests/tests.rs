//! What the tiers' tests share: no test program of its own
//! (`Cargo.toml`), a module of each tier that uses it.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.

// A tier uses what it needs of it.
#![allow(dead_code)]

use bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp};
use chunk_storage::{LayerCodec, LayerType};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::{Entities, EntityId, EntityType, Header};

/// The layer type the arena holds.
pub const STONE: LayerType = LayerType(6);
/// The entities' type.
pub const WALKER: EntityType = EntityType(40);

/// An arena with `STONE` hot over the `side` by `side` superchunks from
/// `(10, 10)`, the cells `cells` set.
pub fn arena(side: u32, cells: impl Iterator<Item = CellIndex>) -> BitmapArena {
    let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
    for (x, y) in (10..10 + side).flat_map(|y| (10..10 + side).map(move |x| (x, y))) {
        for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
            arena.make_hot(BucketKey { layer_type: STONE, chunk }, None, &mut codec);
        }
    }
    cells.for_each(|cell| arena.queue(STONE, Write::cell(cell, WriteOp::Set)));
    assert_eq!(arena.apply().missed, 0);
    arena
}

/// [`arena`], no cell set, and entities holding the same superchunks.
pub fn world(side: u32) -> (BitmapArena, Entities) {
    let (arena, mut entities) = (arena(side, std::iter::empty()), Entities::new());
    assert_eq!(entities.align(&arena.superchunk_indices()), 0);
    (arena, entities)
}

/// The cell `(x, y)` cells from the top left of the superchunk `(10, 10)`.
pub fn cell(x: u32, y: u32) -> CellIndex {
    CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + x, y: 10 * SUPERCHUNK_SIDE_CELLS + y }.into()
}

/// A walker with ID `id` on `at`, waking at `wake`.
pub fn walker(id: u64, at: CellIndex, wake: u64) -> Header {
    Header { id: EntityId(id), kind: WALKER, at, wake }
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
