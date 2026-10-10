//! Grass ticked flat out over a plain, on as many threads as
//! asked: each phase's time, the samples and writes, and the memory held
//! -- the process's, sampled every tick, and the arena's and storage's
//! own.

use type_registry::GRASS;
use crate::diagnostics::plain_world::{plain_world, superchunks as superchunks_of};
use crate::host::frame::count;
use crate::Chosen;
use bitplane_manager::diagnostics::arena::ArenaStats;
use chunk_storage::diagnostics::storage::StorageStats;
use std::time::Duration;
use utilities::diagnostics::process_memory::MemoryTrack;

/// What a run did, and what it held.
#[derive(Clone, Debug)]
pub struct Throughput {
    /// Ticks run.
    pub ticks: usize,
    /// Superchunks ticked.
    pub superchunks: usize,
    /// Threads ticked on.
    pub threads: usize,
    /// Cells of grass at the start, and at the end.
    pub grass: (u64, u64),
    /// Cells of grass sampled.
    pub sampled: usize,
    /// Writes applied: a write landing in two superchunks counted in each.
    pub writes: usize,
    /// Cells written past the superchunks used.
    pub missed: u64,
    /// The first phase's time, added up: sampling and computing.
    pub computing: Duration,
    /// The second phase's time, added up: applying.
    pub applying: Duration,
    /// The process's memory, read after every tick.
    pub memory: MemoryTrack,
    /// What the arena held at the end.
    pub arena: ArenaStats,
    /// What storage held at the end.
    pub storage: StorageStats,
}

/// Ticks grass `ticks` times over `superchunks` superchunks, grass drawn
/// on `thousandths` of each one's cells, on `threads` threads.
pub fn run(ticks: usize, thousandths: usize, superchunks: u32, threads: usize) -> Throughput {
    let mut memory = MemoryTrack::default();
    memory.read();
    let mut world = plain_world(superchunks, thousandths as u64 * worldgen::ONE / 1000, 0, threads);
    let start_grass = count(&world, GRASS);
    let (mut computing, mut applying, mut writes, mut sampled, mut missed) = (Duration::ZERO, Duration::ZERO, 0, 0, 0);
    let chosen = Chosen::of(&[crate::GRASS_RULE]);
    for _ in 0..ticks {
        let report = world.tick_only(chosen, false);
        computing += report.computing;
        applying += report.applying;
        writes += report.writes_applied.writes;
        missed += report.writes_applied.missed;
        sampled += report.rules.of(crate::GRASS_RULE)[sca_rules::grass::SAMPLED] as usize;
        memory.read();
    }
    Throughput {
        ticks,
        superchunks: superchunks_of(&world),
        threads,
        grass: (start_grass, count(&world, GRASS)),
        sampled,
        writes,
        missed,
        computing,
        applying,
        memory,
        arena: ArenaStats::of(&world.arena),
        storage: StorageStats::of(&world.storage),
    }
}
