//! Trees planted on a bare plain the server makes: what the tiers'
//! trees tests start from.

// A tier uses what it needs of it.
#![allow(dead_code)]

use protocol::server_parameters::{ParameterValue, SEED};
use crate::probe::Probe;
use crate::worlds::{base, cells_of, first_superchunk, nothing_more, plain_start, probed_world};
use server::simulation::simulation_api::{cell_reads_and_writes, tick_and_random_stream, CellIndex};
use server::World;

/// A world of one superchunk, bare, and its probe: and the superchunk's
/// cells, in rows.
pub fn bare_world(test_name: &str, seed: u64) -> (World, Probe, Vec<CellIndex>) {
    let (mut world, probe) = probed_world(&plain_start(test_name, 1, 0, 0, 2).with(SEED, ParameterValue::Hex(seed)), nothing_more);
    let cells = cells_of(first_superchunk(&mut world, &probe)).collect();
    (world, probe, cells)
}

/// Puts a tree of the stage `stage` gives on each of `cells`, its cell
/// barred in the collision plane, in a tick of the probe alone.
pub fn plant_trees(world: &mut World, probe: &Probe, cells: &[CellIndex], mut stage: impl FnMut() -> u32) {
    let trees: Vec<(CellIndex, u32)> = cells.iter().map(|&planted_cell| (planted_cell, stage())).collect();
    probe.alone(world, move |tick_state| {
        let turn_superchunk = tick_and_random_stream::superchunk(tick_state);
        for &(planted_cell, stage) in trees.iter().filter(|&&(planted_cell, _)| planted_cell.superchunk() == turn_superchunk) {
            cell_reads_and_writes::set(tick_state, base().tree, planted_cell);
            cell_reads_and_writes::set_value(tick_state, base().tree_stage, planted_cell, 0, stage);
            cell_reads_and_writes::set(tick_state, base().collision, planted_cell);
        }
        Vec::<()>::new()
    });
}
