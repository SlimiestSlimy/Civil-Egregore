//! The area about a cell: read at once, it is its cells read one by
//! one.
//!
//! `cargo test -p server --lib fast_tests`

use crate::arenas::{stone_arena, cell};
use crate::test_types::TestTypes;
use utilities::coordinates::{CellCartesian, CellIndex, SuperchunkIndex};
use crate::entity_manager::WorldEntities;
use crate::simulation::simulation_api::square_around_a_cell::{self, AREA_CENTRE, AREA_SIDE};
use crate::simulation::Simulation;

/// Cells scattered over the 3x3 superchunks from `(10, 10)`, some on
/// their borders, drawn from `seed`.
fn scattered_cells(seed: u64) -> impl Iterator<Item = CellIndex> {
    let mut seeded_random = utilities::rng::RandomSource::new(seed);
    (0..3000).map(move |_| cell(seeded_random.draw_below(3072) as u32, seeded_random.draw_below(3072) as u32))
}

/// The area about a cell read at once is its cells read one by one:
/// inside a superchunk, across the borders of four, and at the edge of
/// the hot superchunks, where some cells are not hot.
#[test]
fn areas_read_at_once_are_the_cells_read_one_by_one() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let types = TestTypes::seeded(seed);
    let mut bitmap_arena = stone_arena(types, 3, scattered_cells(seed));
    let corner_cartesian = cell(0, 0).cartesian();
    let centre_offsets = [(300, 300), (1024, 1024), (1020, 1029), (5, 5), (2040, 2047), (1024, 3), (777, 1023)];
    let tick_report = Simulation::new(1).tick(&mut bitmap_arena, &mut WorldEntities::new(), seed, |tick_state| {
        if tick_state.superchunk() != SuperchunkIndex::from_cartesian(10, 10) {
            return 0;
        }
        for (x, y) in centre_offsets {
            let centre_cartesian = CellCartesian { x: corner_cartesian.x + x, y: corner_cartesian.y + y };
            let area = square_around_a_cell::read_layer(tick_state, types.stone, centre_cartesian.into());
            for (across_in_area, down_in_area) in (0..AREA_SIDE as u32).flat_map(|row_down| (0..AREA_SIDE as u32).map(move |column_across| (column_across, row_down))) {
                let cell = CellCartesian { x: centre_cartesian.x + across_in_area - AREA_CENTRE as u32, y: centre_cartesian.y + down_in_area - AREA_CENTRE as u32 };
                let cell_holds = tick_state.layer_holds(types.stone, cell.into());
                let read_bit = |area_rows: [u16; AREA_SIDE]| area_rows[down_in_area as usize] >> across_in_area & 1 == 1;
                assert_eq!((read_bit(area.hot), read_bit(area.set_rows)), (cell_holds.is_ok(), cell_holds == Ok(true)), "({across_in_area}, {down_in_area}) of the area about ({x}, {y})");
            }
        }
        centre_offsets.len()
    });
    assert_eq!(tick_report.rules, centre_offsets.len());
}
