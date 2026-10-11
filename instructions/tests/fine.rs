//! The fine tier: instructions queued on a small hot arena --
//! milliseconds. The tiers: `docs/testing_protocol.md`, at the
//! repository's root.
//!
//! `cargo test --test fine`

use bitplane_manager::{BitmapArena, BucketKey};
use chunk_storage::{LayerCodec, LayerType};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::Entities;
use instructions::{compare, write, Turn, Write};
use simulation::Simulation;
use utilities::rng::Rng;

/// The layer the arena holds.
const STONE: LayerType = LayerType::of_a_bit(6);

/// What is queued from a compare on happens if the compare holds as
/// it is come to and not otherwise, up to the compare's end; what is
/// queued after the end happens either way. The cells are drawn from
/// the seed.
#[test]
fn what_is_queued_from_a_compare_to_its_end_hangs_on_it() {
    let mut random = Rng::new(utilities::seed::counted());
    for holding in [true, false] {
        let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
        for chunk in SuperchunkIndex::from_cartesian(10, 10).chunks() {
            arena.make_hot(BucketKey { layer_type: STONE, chunk }, None, &mut codec);
        }
        let mut entities = Entities::new();
        assert_eq!(entities.align(&arena.superchunk_indices()), 0);
        let mut drawn = || -> CellIndex { CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + random.below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32, y: 10 * SUPERCHUNK_SIDE_CELLS + random.below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32 }.into() };
        let (seen, under, after) = (drawn(), drawn(), drawn());
        if [seen, under, after].iter().collect::<std::collections::BTreeSet<_>>().len() < 3 {
            continue;
        }
        Simulation::new(1).tick(&mut arena, &mut entities, 1, |turn: &mut Turn, _: &mut Vec<CellIndex>| {
            // The compare says the cell holds stone, or lacks it: it lacks it.
            compare::from_here(turn, if holding { compare::lacks(STONE, seen) } else { compare::holds(STONE, seen) });
            compare::with_writes(turn, compare::lacks(STONE, under), &[Write::Cell { layer_type: STONE, at: under, value: 1, counted: None }]);
            compare::end(turn);
            compare::open(turn, compare::lacks(STONE, after));
            write::cell(turn, STONE, after, 1, None);
            compare::close(turn);
            0
        });
        assert_eq!((arena.holds(STONE, under), arena.holds(STONE, after)), (Ok(holding), Ok(true)), "holding: {holding}");
    }
}
