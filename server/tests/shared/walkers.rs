//! Walkers that count their wakes, as the root's tiers share them, on
//! the stone arenas of `simulation/tests/shared/arenas.rs`.

// A tier uses what it needs of it.
#![allow(dead_code)]

use crate::arenas::cell;
use crate::test_types::TestTypes;
use utilities::coordinates::CellIndex;
use crate::entity_manager::{Header, remove_attribute, set_attribute};
use crate::simulation::SimulationTickState;


/// Each walker woken counts it, flips `types.odd_wake`, and wakes next tick, where
/// it stands: how many woke.
pub fn count_and_flip(types: TestTypes, tick_state: &mut SimulationTickState) -> usize {
    let mut woken_count = 0;
    let mut attributes = Vec::new();
    for entity in tick_state.woken_entities() {
        attributes.clear();
        attributes.extend_from_slice(entity.attributes);
        set_attribute(&mut attributes, types.woken_count, entity.attribute(types.woken_count).unwrap_or(0) + 1);
        if !remove_attribute(&mut attributes, types.odd_wake.attribute_type()) {
            set_attribute(&mut attributes, types.odd_wake, 1);
        }
        crate::simulation::simulation_api::entity_reads_and_writes::update_entity(tick_state, &entity.header, Header { wake: tick_state.current_tick() + 1, ..entity.header }, &attributes);
        woken_count += 1;
    }
    woken_count
}

/// `count` cells drawn from `seed`, no two the same, each within
/// `side_in_cells` cells across and down of the superchunk `(10, 10)`'s
/// top left.
pub fn cells_drawn(seed: u64, count: usize, side_in_cells: u32) -> Vec<CellIndex> {
    let mut seeded_rng = utilities::rng::RandomSource::new(seed);
    let mut cells = std::collections::BTreeSet::new();
    while cells.len() < count {
        cells.insert((seeded_rng.draw_below(u64::from(side_in_cells)) as u32, seeded_rng.draw_below(u64::from(side_in_cells)) as u32));
    }
    // In the order drawn in, not the set's: shuffled by a draw each.
    let mut cells: Vec<(u64, CellIndex)> = cells.into_iter().map(|(x, y)| (seeded_rng.next_draw(), cell(x, y))).collect();
    cells.sort_unstable();
    cells.into_iter().map(|(_, cell)| cell).collect()
}
