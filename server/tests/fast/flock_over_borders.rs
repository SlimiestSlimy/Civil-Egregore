//! A flock over the borders of chunks and superchunks comes to the
//! same on one thread as on all of them, tick after tick: what it
//! queues is applied in the one order of the world, Morton order of
//! its authors' cells, however the superchunks are gone over
//! (`docs/actions.md`, "The same wherever the borders fall").
//!
//! All of it is the test's own, and nothing outside it is made for it:
//! where the flock is put, drawn from the seed so that other borders
//! cross it every run, the flock's rule, and the one generator every
//! chance of the rule is read from, by the tick and the entity.
//!
//! `cargo test -p server --lib flock_over_borders`

use crate::test_types::TestTypes;
use crate::arenas::{cell, walker_header, world};
use crate::bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp};
use crate::chunk_storage::LayerCodec;
use utilities::coordinates::{CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use crate::entity_manager::{AttributeBlock, WorldEntities, InstructionsApplied};
use crate::simulation::simulation_api::cell_and_its_neighbours;
use crate::simulation::simulation_api::{cell_reads_and_writes, compare_and_write, SimulationTickState};
use crate::simulation::tick::Simulation;
use std::collections::BTreeSet;
use utilities::rng::RandomSource;
use crate::collision::Collision;

/// The base game's collision plane's library, of a new world's ID.
fn collision_library(types: TestTypes) -> Collision {
    Collision { plane: types.collision }
}

/// Superchunks along the side of the world.
const WORLD: u32 = 3;
/// Cells along the side of the square the flock is put on: with no
/// shift, its middle the corner four superchunks meet at.
const PASTURE_SIDE: u32 = 240;
/// Walkers put.
const FLOCK: usize = 8000;
/// Ticks run.
const TICKS: u64 = 300;
/// Cells past the pasture, each way, that what came of it is read on:
/// further than a walker comes in the ticks run.
const READ_PAST: i64 = 320;

/// The cell `(x, y)` cells east and south of the pasture's top left,
/// the pasture `pasture_shift` cells east and south of the corner.
fn pasture_cell(pasture_shift: (u32, u32), x: i64, y: i64) -> CellIndex {
    let coordinate_from = |shift_along: u32, cells_along: i64| (i64::from(SUPERCHUNK_SIDE_CELLS - PASTURE_SIDE / 2 + shift_along) + cells_along) as u32;
    cell(coordinate_from(pasture_shift.0, x), coordinate_from(pasture_shift.1, y))
}

/// Where `asked_cell` is from the top left of the pasture `pasture_shift` cells east
/// and south.
fn from_pasture(pasture_shift: (u32, u32), asked_cell: CellIndex) -> (i64, i64) {
    let (origin, asked_cartesian) = (pasture_cell(pasture_shift, 0, 0).cartesian(), asked_cell.cartesian());
    (i64::from(asked_cartesian.x) - i64::from(origin.x), i64::from(asked_cartesian.y) - i64::from(origin.y))
}

/// What the test's one generator holds, at the tick `tick_now`, for the
/// entity that tick found on the cell `from_pasture`: every chance of
/// the flock's rule is read from it.
fn drawn_for(seed: u64, tick_now: u64, from_pasture: (i64, i64)) -> RandomSource {
    RandomSource::for_stream(seed ^ tick_now.wrapping_mul(0x9e37_79b9_7f4a_7c15), (from_pasture.0 as u64) << 32 | from_pasture.1 as u64 & 0xffff_ffff)
}

/// The entity on `asked_cell` as the tick found it, if one stood there.
fn entity_standing_on<'a>(tick_state: &SimulationTickState<'a>, asked_cell: CellIndex) -> Option<crate::entity_manager::EntityRef<'a>> {
    tick_state.entities_in(asked_cell.chunk())?.find(|entity| entity.header.stood_cell == asked_cell)
}

/// One superchunk's turn of the flock's rule: each walker woken may
/// die; eats the stone under it, counting the meal, or lays one beside
/// it, counting that on itself only if it is laid; counts on a
/// neighbour too; may make another beside it, counting that only if
/// it is made; and steps to a neighbour, whoever stands there. Every
/// cell and every attribute written is a compare-and-write, and what
/// is counted of a cell beside it is under a compare of that cell --
/// in another superchunk, where a border lies between.
fn flock(types: TestTypes, tick_state: &mut SimulationTickState, seed: u64, pasture_shift: (u32, u32)) -> u64 {
    let tick_now = tick_state.current_tick();
    for entity in tick_state.woken_entities() {
        let (header, walker_cell) = (entity.header, entity.header.stood_cell);
        tick_state.seeing_to(walker_cell);
        let mut walker_random = drawn_for(seed, tick_now, from_pasture(pasture_shift, walker_cell));
        let about_tiles = cell_and_its_neighbours::tiles_about_cell(walker_cell);
        let stone_about = cell_and_its_neighbours::read_layer(tick_state, types.stone, &about_tiles);
        let (open_cells, collision_cells) = (stone_about.hot & about_tiles.neighbour_cells(), cell_and_its_neighbours::read_layer(tick_state, types.collision, &about_tiles).set_cells & about_tiles.neighbour_cells());
        let chance = walker_random.draw_below(100);
        if chance < 4 {
            collision_library(types).remove(tick_state, &header);
            continue;
        }
        let meals_seen = entity.attribute(types.counted);
        if !(stone_about.set_cells & about_tiles.centre_cell()).is_empty() {
            // The meal is counted only if the stone is there still: another on its way here may not eat it, but one counting on this one may.
            crate::simulation::simulation_api::entity_reads_and_writes::set_attribute_of(&mut compare_and_write::open_compare(tick_state, compare_and_write::layer_holds(types.stone, walker_cell)), &header, types.counted, meals_seen, meals_seen.unwrap_or(0) + 1);
            cell_reads_and_writes::clear(tick_state, types.stone, walker_cell);
        } else if let Some(laid_cell) = cell_and_its_neighbours::pick_way(&mut walker_random, &about_tiles, open_cells.cells_not_in(stone_about.set_cells)).filter(|_| chance < 40).and_then(|laid_way| cell_and_its_neighbours::cell(walker_cell, laid_way)) {
            // Laid and counted, or neither: the cell may be another's stone by then, whichever side of a border it is on.
            let laid_seen = entity.attribute(types.laid);
            let mut cell_bare_still = compare_and_write::open_compare(tick_state, compare_and_write::layer_lacks(types.stone, laid_cell));
            cell_reads_and_writes::set(&mut cell_bare_still, types.stone, laid_cell);
            crate::simulation::simulation_api::entity_reads_and_writes::set_attribute_of(&mut cell_bare_still, &header, types.laid, laid_seen, laid_seen.unwrap_or(0) + 1);
        }
        if let Some(neighbour_entity) = cell_and_its_neighbours::pick_way(&mut walker_random, &about_tiles, collision_cells).and_then(|neighbour_way| cell_and_its_neighbours::cell(walker_cell, neighbour_way)).and_then(|neighbour_cell| entity_standing_on(tick_state, neighbour_cell)) {
            let count_seen = neighbour_entity.attribute(types.counted);
            crate::simulation::simulation_api::entity_reads_and_writes::set_attribute_of(tick_state, &neighbour_entity.header, types.counted, count_seen, count_seen.unwrap_or(0) + 1000);
        }
        if let Some(made_cell) = cell_and_its_neighbours::pick_way(&mut walker_random, &about_tiles, open_cells.cells_not_in(collision_cells)).filter(|_| chance < 9).and_then(|made_way| cell_and_its_neighbours::cell(walker_cell, made_way)) {
            // Made and counted, or neither.
            let made_seen = entity.attribute(types.made);
            let mut entity_made = collision_library(types).begin_making_entity(tick_state, types.walker, made_cell, tick_now + 1 + walker_random.draw_below(3), &[]);
            crate::simulation::simulation_api::entity_reads_and_writes::set_attribute_of(&mut entity_made, &header, types.made, made_seen, made_seen.unwrap_or(0) + 1);
        }
        let step_cell = cell_and_its_neighbours::pick_way(&mut walker_random, &about_tiles, open_cells).and_then(|step_way| cell_and_its_neighbours::cell(walker_cell, step_way)).unwrap_or(walker_cell);
        collision_library(types).step(tick_state, &header, step_cell, tick_now + 1 + walker_random.draw_below(3));
    }
    0
}

/// A world with the pasture on it, `pasture_shift` cells east and south: stone
/// on the cells the seed draws, walkers on others it draws, each set
/// in the collision plane.
fn pasture_world(types: TestTypes, seed: u64, pasture_shift: (u32, u32)) -> (BitmapArena, WorldEntities) {
    let (mut bitmap_arena, mut entity_store) = world(types, WORLD);
    let mut layer_codec = LayerCodec::new();
    for (x, y) in (10..10 + WORLD).flat_map(|y| (10..10 + WORLD).map(move |x| (x, y))) {
        for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
            bitmap_arena.make_hot(BucketKey { layer_type: types.collision, chunk }, None, &mut layer_codec);
        }
    }
    let mut seeded_random = RandomSource::new(seed);
    for (x, y) in (0..i64::from(PASTURE_SIDE)).flat_map(|y| (0..i64::from(PASTURE_SIDE)).map(move |x| (x, y))) {
        if seeded_random.draw_below(100) < 40 {
            bitmap_arena.queue_write(types.stone, Write::cell(pasture_cell(pasture_shift, x, y), WriteOp::SetCell));
        }
    }
    assert_eq!(bitmap_arena.apply().missed, 0, "the pasture on hot superchunks");
    let mut drawn_places = BTreeSet::new();
    while drawn_places.len() < FLOCK {
        let drawn_place = (seeded_random.draw_below(u64::from(PASTURE_SIDE)) as i64, seeded_random.draw_below(u64::from(PASTURE_SIDE)) as i64);
        if drawn_places.insert(drawn_place) {
            entity_store.queue_add(walker_header(types, pasture_cell(pasture_shift, drawn_place.0, drawn_place.1), seeded_random.draw_below(3)), &[]);
            bitmap_arena.queue_write(types.collision, Write::cell(pasture_cell(pasture_shift, drawn_place.0, drawn_place.1), WriteOp::SetCell));
        }
    }
    entity_store.apply();
    assert_eq!(bitmap_arena.apply().missed, 0);
    (bitmap_arena, entity_store)
}

/// What came of the pasture, said from its own top left: the cells of
/// stone on it and about it, those the collision plane holds, and
/// every walker by its cell -- where, when it wakes, its attributes.
type PastureOutcome = (BTreeSet<(i64, i64)>, BTreeSet<(i64, i64)>, Vec<(i64, i64, u64, Vec<AttributeBlock>)>);

/// [`PastureOutcome`] of a world whose pasture is `pasture_shift` cells east and south.
fn pasture_outcome(types: TestTypes, bitmap_arena: &BitmapArena, entity_store: &WorldEntities, pasture_shift: (u32, u32)) -> PastureOutcome {
    let read_range = -READ_PAST..i64::from(PASTURE_SIDE) + READ_PAST;
    let cells_holding = |layer| read_range.clone().flat_map(|y| read_range.clone().map(move |x| (x, y))).filter(|&(x, y)| bitmap_arena.layer_holds_cell(layer, pasture_cell(pasture_shift, x, y)) == Ok(true)).collect();
    let (stone_cells, collision_cells) = (cells_holding(types.stone), cells_holding(types.collision));
    let place_in_pasture = |walker_cell: CellIndex| from_pasture(pasture_shift, walker_cell);
    let mut walker_rows: Vec<_> = entity_store.iter().map(|walker_entity| (place_in_pasture(walker_entity.header.stood_cell).0, place_in_pasture(walker_entity.header.stood_cell).1, walker_entity.header.wake, walker_entity.attributes.to_vec())).collect();
    walker_rows.sort_unstable_by_key(|walker_row| (walker_row.0, walker_row.1));
    (stone_cells, collision_cells, walker_rows)
}

/// The same flock, put a drawn way east and south of the corner four
/// superchunks meet at, is the same on one thread and on every thread
/// the machine has, before a tick and after each: every cell of
/// stone, every cell the collision plane holds -- the walkers' --
/// every walker, and as many acts over a border.
/// And the flock did all the test is of: walkers crossed borders, were
/// refused cells and writes, were made and removed and written.
#[test]
fn a_flock_over_borders_comes_to_the_same_on_any_number_of_threads() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let types = TestTypes::seeded(seed);
    let mut seeded_random = RandomSource::new(!seed);
    // Under half the pasture each way: the corner four superchunks meet at stays on it, wherever on it.
    let pasture_shift = (seeded_random.draw_below(u64::from(PASTURE_SIDE / 2 - 10)) as u32, seeded_random.draw_below(u64::from(PASTURE_SIDE / 2 - 10)) as u32);
    let ((mut bitmap_arena, mut entity_store), (mut arena_there, mut entities_there)) = (pasture_world(types, seed, pasture_shift), pasture_world(types, seed, pasture_shift));
    let (mut test_simulation, mut simulation_there) = (Simulation::new(1), Simulation::for_superchunks((WORLD * WORLD) as usize));
    assert_eq!(pasture_outcome(types, &bitmap_arena, &entity_store, pasture_shift), pasture_outcome(types, &arena_there, &entities_there, pasture_shift), "the same pasture put, shifted {pasture_shift:?}");
    let (mut instructions_done, mut writes_refused, mut compares_refused, mut over_a_border) = (InstructionsApplied::default(), 0, 0, (0, 0));
    for _ in 0..TICKS {
        let report = test_simulation.tick(&mut bitmap_arena, &mut entity_store, seed, |tick_state: &mut SimulationTickState| flock(types, tick_state, seed, pasture_shift));
        let report_there = simulation_there.tick(&mut arena_there, &mut entities_there, seed, |tick_state: &mut SimulationTickState| flock(types, tick_state, seed, pasture_shift));
        over_a_border = (over_a_border.0 + report.acts_over_a_border, over_a_border.1 + report_there.acts_over_a_border);
        instructions_done += report.instructions_applied;
        writes_refused += report.writes_applied.writes_refused;
        compares_refused += report.instructions_compared.1;
        let ((stone_cells, collision_cells, walker_rows), (stone_there, held_there, walkers_there)) = (pasture_outcome(types, &bitmap_arena, &entity_store, pasture_shift), pasture_outcome(types, &arena_there, &entities_there, pasture_shift));
        // The collision plane holds the cells the walkers stand on, and no other.
        assert_eq!(collision_cells, walker_rows.iter().map(|walker_row| (walker_row.0, walker_row.1)).collect(), "after tick {}", entity_store.current_tick());
        let stone_apart: Vec<_> = stone_cells.symmetric_difference(&stone_there).chain(collision_cells.symmetric_difference(&held_there)).take(4).collect();
        let walkers_apart: Vec<_> = walker_rows.iter().zip(&walkers_there).filter(|(row_here, row_there)| row_here != row_there).take(4).collect();
        assert!(stone_apart.is_empty() && walkers_apart.is_empty() && walker_rows.len() == walkers_there.len(), "after tick {}, shifted {pasture_shift:?}, seed {seed:#x}: {} walkers on one thread and {} on all; apart, stone {stone_apart:?}, walkers {walkers_apart:?}; done here {instructions_done:?}", entity_store.current_tick(), walker_rows.len(), walkers_there.len());
    }
    // A cell held is refused by the compare of the plane, so no step is turned back by the entities themselves.
    assert!(instructions_done.border_crossings > 0 && compares_refused > 0 && instructions_done.adds_refused > 0 && instructions_done.added > 0 && instructions_done.entities_removed > 0 && instructions_done.edits > 0 && writes_refused > 0, "{instructions_done:?}, {writes_refused} writes and {compares_refused} compares refused");
    // Both borders of superchunks cross the pasture wherever it was put, and acts reach over them.
    assert!(over_a_border.0 > 0 && over_a_border.0 == over_a_border.1, "acts over a border: {over_a_border:?}");
}
