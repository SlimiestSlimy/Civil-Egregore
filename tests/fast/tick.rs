//! The tick: rules run superchunk by superchunk, writes queued for the
//! superchunks they land in and applied in a second phase -- the same on
//! any number of threads, across borders, reading the world as the tick
//! found it, and never past the superchunks next door.

use bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp};
use entity_manager::WorldEntities;
use simulation::{Simulation, SimulationTickState};
use utilities::chance::Chance;
use chunk_storage::{LayerCodec, LayerType};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};

/// The layer type the tests run on.
const STONE_LAYER: LayerType = LayerType::of_a_bit(6);

/// An arena with `STONE_LAYER` hot over the `side_superchunks` by `side_superchunks` superchunks from
/// `(10, 10)`, the cells `cells` set.
fn stone_arena(side_superchunks: u32, cells: impl Iterator<Item = CellCartesian>) -> BitmapArena {
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    for y in 10..10 + side_superchunks {
        for x in 10..10 + side_superchunks {
            for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
                bitmap_arena.make_hot(BucketKey { layer_type: STONE_LAYER, chunk }, None, &mut layer_codec);
            }
        }
    }
    cells.for_each(|cell| bitmap_arena.queue_write(STONE_LAYER, Write::cell(cell.into(), WriteOp::SetCell)));
    assert_eq!(bitmap_arena.apply().missed, 0);
    bitmap_arena
}

/// The top left cell of the superchunk `(x, y)`, cartesian.
fn top_left_cartesian(x: u32, y: u32) -> CellCartesian {
    SuperchunkIndex::from_cartesian(x, y).top_left().cartesian()
}

/// Stone creeping: each cell sampled at 5% sets a random neighbour it
/// sees clear -- within the 3x3 around it -- or clears itself: how
/// many it sampled.
fn stone_creep(tick_state: &mut SimulationTickState) -> usize {
    tick_state.each_sampled(STONE_LAYER, Chance::one_in(20), &mut (), |tick_state, cell, _| {
        let (dx, dy) = (tick_state.random_stream().draw_below(3) as i32 - 1, tick_state.random_stream().draw_below(3) as i32 - 1);
        if (dx, dy) == (0, 0) {
            simulation::simulation_api::cell_reads_and_writes::seen_becoming(tick_state, STONE_LAYER, cell, 1, 0, None);
        } else if let Some(clear_neighbour) = cell.at_cartesian_offset(dx, dy).filter(|&candidate_neighbour| tick_state.layer_holds(STONE_LAYER, candidate_neighbour) == Ok(false)) {
            simulation::simulation_api::cell_reads_and_writes::seen_becoming(tick_state, STONE_LAYER, clear_neighbour, 0, 1, None);
        }
    })
}

/// Every hot bitmap's cells, in the arena's order.
fn every_cell(bitmap_arena: &BitmapArena) -> Vec<(BucketKey, Vec<u64>)> {
    bitmap_arena.hot_bucket_keys().map(|bucket_key| (bucket_key, bitmap_arena.bucket(bucket_key).expect("hot").cells().to_vec())).collect()
}

/// Cells scattered over the 3x3 superchunks from `(10, 10)`, some on
/// their borders.
fn scattered_cells(seed: u64) -> impl Iterator<Item = CellCartesian> {
    let (start_cartesian, mut seeded_rng) = (top_left_cartesian(10, 10), utilities::rng::RandomSource::new(seed));
    // One in four on a superchunk's border, each way.
    let mut draw_coordinate = move || if seeded_rng.draw_below(4) == 0 { (seeded_rng.draw_below(4) * 1024).saturating_sub(seeded_rng.draw_below(2)).min(3071) as u32 } else { seeded_rng.draw_below(3072) as u32 };
    (0..3000).map(move |_| CellCartesian { x: start_cartesian.x + draw_coordinate(), y: start_cartesian.y + draw_coordinate() })
}

/// A tick comes out the same on one thread and on four: each
/// superchunk's random numbers are its own, and the second phase applies
/// in a fixed order.
#[test]
fn any_number_of_threads_ticks_the_same() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut one_thread_arena, mut four_thread_arena) = (stone_arena(3, scattered_cells(seed)), stone_arena(3, scattered_cells(seed)));
    for tick in 0..20 {
        let (one_thread_report, four_thread_report) = (Simulation::new(1).tick(&mut one_thread_arena, &mut WorldEntities::new(), seed, stone_creep), Simulation::new(4).tick(&mut four_thread_arena, &mut WorldEntities::new(), seed, stone_creep));
        assert_eq!((one_thread_report.rules, one_thread_report.writes_applied), (four_thread_report.rules, four_thread_report.writes_applied), "tick {tick}");
    }
    assert_eq!(every_cell(&one_thread_arena), every_cell(&four_thread_arena));
}

/// A write lands in the neighbour it falls in; reads in the first phase
/// see the world as the tick found it, writes queued or not.
#[test]
fn writes_cross_borders_and_reads_see_the_tick_start() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    // Any cell down the superchunk's east edge.
    let east_border_cell = CellCartesian { x: top_left_cartesian(11, 10).x - 1, y: top_left_cartesian(10, 10).y + utilities::rng::RandomSource::new(seed).draw_below(1024) as u32 };
    let mut bitmap_arena = stone_arena(2, [east_border_cell].into_iter());
    let across_cell: CellIndex = CellCartesian { x: east_border_cell.x + 1, y: east_border_cell.y }.into();
    let report = Simulation::new(1).tick(&mut bitmap_arena, &mut WorldEntities::new(), seed, |tick_state| {
        tick_state.each_sampled(STONE_LAYER, Chance::ALWAYS_HAPPENS, &mut (), |tick_state, cell, _| {
            let east_neighbour = cell.at_cartesian_offset(1, 0).expect("in the world");
            simulation::simulation_api::cell_reads_and_writes::seen_becoming(tick_state, STONE_LAYER, east_neighbour, 0, 1, None);
            assert_eq!(tick_state.layer_holds(STONE_LAYER, east_neighbour), Ok(false), "still as the tick found it");
        })
    });
    assert_eq!((report.rules, report.writes_applied.cells_changed), (1, 1));
    assert_eq!(bitmap_arena.layer_holds_cell(STONE_LAYER, across_cell), Ok(true), "set in the neighbour");
    assert_eq!(bitmap_arena.superchunk_count(STONE_LAYER, SuperchunkIndex::from_cartesian(11, 10)), 1);
}

/// The cells of a square straddling the corner four superchunks meet
/// at, written one by one, land in all four, each applying its own.
#[test]
fn writes_about_a_corner_land_in_the_four_superchunks() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let meeting_corner = top_left_cartesian(11, 11);
    let mut bitmap_arena = stone_arena(2, [CellCartesian { x: meeting_corner.x - 1, y: meeting_corner.y - 1 }].into_iter());
    let report = Simulation::new(2).tick(&mut bitmap_arena, &mut WorldEntities::new(), seed, |tick_state| {
        tick_state.each_sampled(STONE_LAYER, Chance::ALWAYS_HAPPENS, &mut (), |tick_state, cell, _| {
            for (x, y) in (-1..3).flat_map(|y| (-1..3).map(move |x| (x, y))) {
                let square_cell = cell.at_cartesian_offset(x, y).expect("in the world");
                if tick_state.layer_holds(STONE_LAYER, square_cell) == Ok(false) {
                    simulation::simulation_api::cell_reads_and_writes::seen_becoming(tick_state, STONE_LAYER, square_cell, 0, 1, None);
                }
            }
        });
        0
    });
    assert_eq!(report.writes_applied.cells_changed, 15, "the 4x4 from two up and left of the meeting point, one cell set already");
    for (x, y, cells) in [(10, 10, 4), (11, 10, 4), (10, 11, 4), (11, 11, 4)] {
        assert_eq!(bitmap_arena.superchunk_count(STONE_LAYER, SuperchunkIndex::from_cartesian(x, y)), cells, "superchunk ({x}, {y})");
    }
}

/// Writes landing in a superchunk with no bitmap in use are missed, and
/// counted.
#[test]
fn writes_to_cold_neighbours_are_missed() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let east_border_cell = CellCartesian { x: top_left_cartesian(11, 10).x - 1, y: top_left_cartesian(10, 10).y + utilities::rng::RandomSource::new(seed).draw_below(1024) as u32 };
    let mut bitmap_arena = stone_arena(1, [east_border_cell].into_iter());
    let report = Simulation::new(1).tick(&mut bitmap_arena, &mut WorldEntities::new(), seed, |tick_state| {
        tick_state.each_sampled(STONE_LAYER, Chance::ALWAYS_HAPPENS, &mut (), |tick_state, cell, _| {
            simulation::simulation_api::cell_reads_and_writes::seen_becoming(tick_state, STONE_LAYER, cell.at_cartesian_offset(1, 0).expect("in the world"), 0, 1, None);
        });
        0
    });
    assert_eq!((report.writes_applied.cells_changed, report.writes_applied.missed), (0, 1));
}

/// A write two superchunks away is past the speed of light.
#[test]
#[should_panic(expected = "past the speed of light")]
fn writes_past_the_speed_of_light_panic() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut bitmap_arena = stone_arena(1, [top_left_cartesian(10, 10)].into_iter());
    Simulation::new(1).tick(&mut bitmap_arena, &mut WorldEntities::new(), seed, |tick_state| {
        tick_state.each_sampled(STONE_LAYER, Chance::ALWAYS_HAPPENS, &mut (), |tick_state, cell, _| {
            simulation::simulation_api::cell_reads_and_writes::seen_becoming(tick_state, STONE_LAYER, cell.at_cartesian_offset(2 * SUPERCHUNK_SIDE_CELLS as i32, 0).expect("in the world"), 0, 1, None);
        });
        0
    });
}
