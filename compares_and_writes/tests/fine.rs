//! The fine tier: instructions queued on a small hot arena --
//! milliseconds. The tiers: `docs/testing_protocol.md`, at the
//! repository's root.
//!
//! `cargo test --test fine`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;
use bitplane_manager::{BitmapArena, BucketKey};
use chunk_storage::{LayerCodec, LayerType};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::WorldEntities;
use compares_and_writes::{compare, write, Turn, Write};
use simulation::Simulation;
use utilities::rng::RandomSource;

/// The layer the arena holds.
const STONE_LAYER: LayerType = LayerType::of_a_bit(6);

/// What is queued from a compare on happens if the compare holds as
/// it is come to and not otherwise, up to the compare's end; what is
/// queued after the end happens either way. The cells are drawn from
/// the seed.
#[test]
fn what_is_queued_from_a_compare_to_its_end_hangs_on_it() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut random_numbers = RandomSource::new(seed);
    for compare_holds in [true, false] {
        let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
        for chunk in SuperchunkIndex::from_cartesian(10, 10).chunks() {
            bitmap_arena.make_hot(BucketKey { layer_type: STONE_LAYER, chunk }, None, &mut layer_codec);
        }
        let mut entity_store = WorldEntities::new();
        assert_eq!(entity_store.keep_superchunks(&bitmap_arena.superchunk_indices()), 0);
        let mut drawn_cell = || -> CellIndex { CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + random_numbers.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32, y: 10 * SUPERCHUNK_SIDE_CELLS + random_numbers.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32 }.into() };
        let (seen_cell, cell_under_compare, cell_after_end) = (drawn_cell(), drawn_cell(), drawn_cell());
        if [seen_cell, cell_under_compare, cell_after_end].iter().collect::<std::collections::BTreeSet<_>>().len() < 3 {
            continue;
        }
        Simulation::new(1).tick(&mut bitmap_arena, &mut entity_store, 1, |turn: &mut Turn| {
            // The compare says the cell holds stone, or lacks it: it lacks it.
            compare::from_here(turn, if compare_holds { compare::layer_lacks(STONE_LAYER, seen_cell) } else { compare::layer_holds(STONE_LAYER, seen_cell) });
            compare::with_writes(turn, compare::layer_lacks(STONE_LAYER, cell_under_compare), &[Write::Cell { layer_type: STONE_LAYER, target_cell: cell_under_compare, value: 1, counted_place: None }]);
            compare::end_from_here(turn);
            compare::open_compare(turn, compare::layer_lacks(STONE_LAYER, cell_after_end));
            write::cell(turn, STONE_LAYER, cell_after_end, 1, None);
            compare::close_compare(turn);
            0
        });
        assert_eq!((bitmap_arena.layer_holds_cell(STONE_LAYER, cell_under_compare), bitmap_arena.layer_holds_cell(STONE_LAYER, cell_after_end)), (Ok(compare_holds), Ok(true)), "holding: {compare_holds}");
    }
}
