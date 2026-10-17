//! Skip tags: a compare that does not hold skips what is queued
//! after it to the tag of its skip-tag set, a set within it passed
//! over whole; one within a set skips to its own tag alone.
//!
//! `cargo test --test fine`

use bitplane_manager::{BitmapArena, BucketKey};
use chunk_storage::{LayerCodec, LayerType};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::WorldEntities;
use simulation::simulation_api::compare_and_write;
use simulation::simulation_api::SimulationTickState;
use simulation::Simulation;
use utilities::rng::RandomSource;

/// The layer the arena holds.
const STONE_LAYER: LayerType = LayerType::of_a_bit(6);

/// An arena with `STONE_LAYER` hot over one superchunk at `(10, 10)`, no cell
/// set, and its entities, none.
fn empty_arena() -> (BitmapArena, WorldEntities) {
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    for chunk in SuperchunkIndex::from_cartesian(10, 10).chunks() {
        bitmap_arena.make_hot(BucketKey { layer_type: STONE_LAYER, chunk }, None, &mut layer_codec);
    }
    let mut entity_store = WorldEntities::new();
    assert_eq!(entity_store.keep_superchunks(&bitmap_arena.superchunk_indices()), 0);
    (bitmap_arena, entity_store)
}

/// A set with a compare that holds or not, and within it a set with
/// one that holds or not: a cell written in the inner set, one in the
/// outer after the inner's tag, one after the outer's tag. What is
/// under the outer set lands only if its compare holds; what is under
/// the inner, only if both do; what follows both, either way. The
/// cells are drawn from the seed.
#[test]
fn a_compare_skips_to_the_tag_of_its_own_set() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut random_numbers = RandomSource::new(seed);
    for (outer_holds, inner_holds) in [(true, true), (true, false), (false, true), (false, false)] {
        let (mut bitmap_arena, mut entity_store) = empty_arena();
        let mut drawn_cell = || -> CellIndex { CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + random_numbers.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32, y: 10 * SUPERCHUNK_SIDE_CELLS + random_numbers.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32 }.into() };
        let [outer_seen, inner_seen, inner_written, outer_written, after_written] = std::array::from_fn(|_| drawn_cell());
        if [outer_seen, inner_seen, inner_written, outer_written, after_written].iter().collect::<std::collections::BTreeSet<_>>().len() < 5 {
            continue;
        }
        // Every cell lacks stone: a compare that it holds there does not hold.
        let seen_as = |holds: bool, seen_cell: CellIndex| if holds { compare_and_write::layer_lacks(STONE_LAYER, seen_cell) } else { compare_and_write::layer_holds(STONE_LAYER, seen_cell) };
        Simulation::new(1).tick(&mut bitmap_arena, &mut entity_store, 1, |tick_state: &mut SimulationTickState| {
            {
                let mut outer_set = compare_and_write::open_compare(tick_state, seen_as(outer_holds, outer_seen));
                {
                    let mut inner_set = compare_and_write::open_compare(&mut outer_set, seen_as(inner_holds, inner_seen));
                    inner_set.write_cell(STONE_LAYER, inner_written, 1);
                }
                outer_set.write_cell(STONE_LAYER, outer_written, 1);
            }
            compare_and_write::open_compare(tick_state, compare_and_write::layer_lacks(STONE_LAYER, after_written)).write_cell(STONE_LAYER, after_written, 1);
            0
        });
        let written = |written_cell: CellIndex| bitmap_arena.layer_holds_cell(STONE_LAYER, written_cell) == Ok(true);
        assert_eq!((written(inner_written), written(outer_written), written(after_written)), (outer_holds && inner_holds, outer_holds, true), "outer holding: {outer_holds}, inner: {inner_holds}");
    }
}

/// A compare chained to a set handed down -- a callee's -- skips to
/// the set's tag: queued after a write, it holds back what follows it
/// alone.
#[test]
fn a_chained_compare_skips_to_the_tag_it_was_handed() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut random_numbers = RandomSource::new(seed);
    let (mut bitmap_arena, mut entity_store) = empty_arena();
    let mut drawn_cell = || -> CellIndex { CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + random_numbers.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32, y: 10 * SUPERCHUNK_SIDE_CELLS + random_numbers.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32 }.into() };
    let [first_seen, before_chained, chained_seen, after_chained] = std::array::from_fn(|_| drawn_cell());
    if [first_seen, before_chained, chained_seen, after_chained].iter().collect::<std::collections::BTreeSet<_>>().len() < 4 {
        return;
    }
    /// The callee: a compare that does not hold, chained to the set it is handed, and a write after it.
    fn callee(handed_set: &mut compare_and_write::SkipTagSet, chained_seen: CellIndex, after_chained: CellIndex) {
        handed_set.chain_compare(compare_and_write::layer_holds(STONE_LAYER, chained_seen));
        handed_set.write_cell(STONE_LAYER, after_chained, 1);
    }
    Simulation::new(1).tick(&mut bitmap_arena, &mut entity_store, 1, |tick_state: &mut SimulationTickState| {
        let mut caller_set = compare_and_write::open_compare(tick_state, compare_and_write::layer_lacks(STONE_LAYER, first_seen));
        caller_set.write_cell(STONE_LAYER, before_chained, 1);
        callee(&mut caller_set, chained_seen, after_chained);
        0
    });
    let written = |written_cell: CellIndex| bitmap_arena.layer_holds_cell(STONE_LAYER, written_cell) == Ok(true);
    assert_eq!((written(before_chained), written(after_chained)), (true, false));
}
