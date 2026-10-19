//! An entity that crosses a border is one entity, moved once: what
//! other authors queued for it in the same tick -- by its name, the
//! cell the tick found it on -- lands on it wherever it stands by
//! then, as it would within a superchunk
//! (`docs/simulation.md`, "An entity over a border").
//!
//! `cargo test -p server --lib entity_over_a_border`

use crate::arenas::{cell, walker_header, world};
use crate::test_types::TestTypes;
use utilities::coordinates::{CellIndex, SUPERCHUNK_SIDE_CELLS};
use crate::entity_manager::{WorldEntities, Header, NEVER_WAKES};
use crate::simulation::{Simulation, SimulationTickState};
use utilities::rng::RandomSource;

/// What a writer does to the crosser.
#[derive(Clone, Copy, PartialEq)]
enum WriterDoes {
    /// Gives it an attribute, and queues nothing else.
    GivesAttribute,
    /// Gives it an attribute, and itself one: it lands in two.
    GivesAndKeeps,
    /// Removes it.
    RemovesCrosser,
}

/// The scene: the crosser on the last column of the middle superchunk
/// of three by three, a row drawn from the seed, stepping right over
/// the border; and the writers about it, each with what it does and
/// whether it is after the crosser in the world's order -- before it,
/// beside it in its superchunk, over the border it crosses, and in
/// the superchunk down and left, which is no neighbour of the one it
/// comes to.
fn crossing_scene(crosser_row: u32, last_writer_does: WriterDoes) -> (CellIndex, CellIndex, Vec<(CellIndex, WriterDoes)>) {
    let last_column = 2 * SUPERCHUNK_SIDE_CELLS - 1;
    let writer_cells = vec![
        // Before it in Morton order: across and down both less.
        (cell(last_column - 3, crosser_row - 3), WriterDoes::GivesAttribute),
        // After it, both no less: in its own superchunk, and in the one it crosses to.
        (cell(last_column, crosser_row + 2), WriterDoes::GivesAttribute),
        (cell(last_column + 2, crosser_row + 1), WriterDoes::GivesAttribute),
        (cell(last_column, crosser_row + 3), WriterDoes::GivesAndKeeps),
        (cell(last_column + 3, crosser_row + 2), WriterDoes::GivesAndKeeps),
        // After it, a superchunk down and one left: the bit that tells them apart is the row's.
        (cell(last_column - SUPERCHUNK_SIDE_CELLS, crosser_row + SUPERCHUNK_SIDE_CELLS), WriterDoes::GivesAttribute),
        (cell(last_column - SUPERCHUNK_SIDE_CELLS - 1, crosser_row + SUPERCHUNK_SIDE_CELLS), WriterDoes::GivesAndKeeps),
        (cell(last_column + 1, crosser_row + 4), last_writer_does),
    ];
    (cell(last_column, crosser_row), cell(last_column + 1, crosser_row), writer_cells)
}

/// One tick of the scene: the crosser steps over the border, every
/// writer does what it does to it. What the tick applied, and the
/// entities after.
fn tick(types: TestTypes, seed: u64, thread_count: usize, crosser_row: u32, last_writer_does: WriterDoes, is_blocked: bool) -> (crate::entity_manager::InstructionsApplied, WorldEntities) {
    let (mut bitmap_arena, mut entity_store) = world(types, 3);
    let (from_cell, to_cell, writer_cells) = crossing_scene(crosser_row, last_writer_does);
    let crosser_header = walker_header(types, from_cell, 0);
    entity_store.queue_add(crosser_header, &[crate::entity_manager::AttributeBlock::holding_value(types.name, 7)]);
    for &(writer_cell, _) in &writer_cells {
        entity_store.queue_add(walker_header(types, writer_cell, 0), &[]);
    }
    if is_blocked {
        entity_store.queue_add(walker_header(types, to_cell, NEVER_WAKES), &[]);
    }
    entity_store.apply();
    for &(later_writer_cell, _) in &writer_cells[1..] {
        assert!(later_writer_cell.0 > from_cell.0, "a writer meant to come after the crosser in the world's order");
    }
    assert!(writer_cells[0].0 .0 < from_cell.0);
    let report = Simulation::new(thread_count).tick(&mut bitmap_arena, &mut entity_store, seed, |tick_state: &mut SimulationTickState| {
        let current_tick = tick_state.current_tick();
        crate::simulation::simulation_api::entity_reads_and_writes::each_woken(tick_state, [], &mut (), |tick_state, entity, _| {
            let header: Header = entity.header;
            if header.stood_cell == from_cell {
                crate::simulation::simulation_api::entity_reads_and_writes::step(tick_state, &header, to_cell, current_tick + 1);
                return;
            }
            let Some(writer_index) = writer_cells.iter().position(|&(candidate_cell, _)| candidate_cell == header.stood_cell) else {
                return;
            };
            match writer_cells[writer_index].1 {
                WriterDoes::RemovesCrosser => crate::simulation::simulation_api::entity_reads_and_writes::remove(tick_state, &crosser_header),
                writer_does => {
                    crate::simulation::simulation_api::entity_reads_and_writes::set_attribute_of(tick_state, &crosser_header, types.given[writer_index], None, writer_index as u64 + 1);
                    if writer_does == WriterDoes::GivesAndKeeps {
                        crate::simulation::simulation_api::entity_reads_and_writes::set_attribute_of(tick_state, &header, types.own, None, 1);
                    }
                }
            }
        });
        0
    });
    (report.instructions_applied, entity_store)
}

/// The crosser steps over a border, and eight others write to it in
/// the same tick, before it and after it in the world's order, from
/// its superchunk, from the one it comes to and from one that is no
/// neighbour of that, alone or with a write of their own: it stands
/// over the border with every one of their attributes, and each kept
/// its own -- the same on one thread and on all. Its cell over the
/// border taken, it stays, and has them all the same.
#[test]
fn what_others_write_to_an_entity_follows_it_over_a_border() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let types = TestTypes::seeded(seed);
    let mut seeded_rng = RandomSource::new(seed);
    let crosser_row = 4 + seeded_rng.draw_below(900) as u32;
    for (thread_count, is_blocked) in [(1, false), (crate::simulation::threads_for(9), false), (1, true), (crate::simulation::threads_for(9), true)] {
        let (applied_counts, entity_store) = tick(types, seed, thread_count, crosser_row, WriterDoes::GivesAttribute, is_blocked);
        let (from_cell, to_cell, writer_cells) = crossing_scene(crosser_row, WriterDoes::GivesAttribute);
        assert_eq!((applied_counts.border_crossings, applied_counts.turned_back), (usize::from(!is_blocked), usize::from(is_blocked)), "on {thread_count} threads, row {crosser_row}");
        let crosser_entity = entity_store.get(if is_blocked { from_cell } else { to_cell }).expect("the crosser, where it should stand");
        assert_eq!((crosser_entity.attribute(types.name), entity_store.get(from_cell).is_some()), (Some(7), is_blocked));
        for (writer_index, &(writer_cell, writer_does)) in writer_cells.iter().enumerate() {
            assert_eq!(crosser_entity.attribute(types.given[writer_index]), Some(writer_index as u64 + 1), "writer {writer_index} on {thread_count} threads, row {crosser_row}, blocked {is_blocked}");
            assert_eq!(entity_store.get(writer_cell).expect("the writer").attribute(types.own), (writer_does == WriterDoes::GivesAndKeeps).then_some(1), "writer {writer_index}'s own");
        }
        assert_eq!(entity_store.len(), writer_cells.len() + 1 + usize::from(is_blocked));
    }
}

/// One of those after it removes the crosser instead: it is gone,
/// from the cell it left and from the one it came to, on one thread
/// and on all.
#[test]
fn an_entity_removed_after_it_crossed_is_gone() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let types = TestTypes::seeded(seed);
    let mut seeded_rng = RandomSource::new(seed);
    let crosser_row = 4 + seeded_rng.draw_below(900) as u32;
    for thread_count in [1, crate::simulation::threads_for(9)] {
        let (applied_counts, entity_store) = tick(types, seed, thread_count, crosser_row, WriterDoes::RemovesCrosser, false);
        let (from_cell, to_cell, writer_cells) = crossing_scene(crosser_row, WriterDoes::RemovesCrosser);
        assert_eq!((applied_counts.border_crossings, applied_counts.entities_removed), (1, 1), "on {thread_count} threads, row {crosser_row}");
        assert!(entity_store.get(from_cell).is_none() && entity_store.get(to_cell).is_none(), "on {thread_count} threads, row {crosser_row}");
        assert_eq!(entity_store.len(), writer_cells.len());
    }
}
