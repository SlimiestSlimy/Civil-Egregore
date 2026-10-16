//! Grass ticked flat out over a plain, on as many threads as
//! asked: each phase's time, the samples and writes, and the memory held
//! -- the process's, sampled every tick, and the arena's and storage's
//! own.

use type_registry::GRASS;
use crate::diagnostics::plain_world::{plain_world, superchunks as superchunks_of};
use crate::host::frame::count;
use crate::ChosenRules;
use bitplane_manager::diagnostics::arena::ArenaStats;
use chunk_storage::diagnostics::storage::StorageStats;
use std::time::Duration;
use utilities::diagnostics::process_memory::MemoryTrack;

/// What a run did, and what it held.
#[derive(Clone, Debug)]
pub struct ThroughputRun {
    /// Ticks run.
    pub ticks: usize,
    /// Superchunks ticked.
    pub superchunks: usize,
    /// Threads ticked on.
    pub thread_count: usize,
    /// Cells of grass at the start, and at the end.
    pub grass_at_start_and_end: (u64, u64),
    /// Cells of grass sampled.
    pub grass_sampled: usize,
    /// Writes applied: a write landing in two superchunks counted in each.
    pub writes: usize,
    /// Cells written past the superchunks used.
    pub missed: u64,
    /// The first phase's time, added up: sampling and computing.
    pub computing_time: Duration,
    /// The second phase's time, added up: applying.
    pub applying_time: Duration,
    /// The process's memory, read after every tick.
    pub process_memory: MemoryTrack,
    /// What the arena held at the end.
    pub arena_stats: ArenaStats,
    /// What storage held at the end.
    pub storage_stats: StorageStats,
}

/// Ticks grass `ticks` times over `superchunks` superchunks, grass drawn
/// on `grass_thousandths` of each one's cells, on `thread_count` threads.
pub fn run_throughput(ticks: usize, grass_thousandths: usize, superchunks: u32, thread_count: usize) -> ThroughputRun {
    let mut process_memory = MemoryTrack::default();
    process_memory.take_reading();
    let mut world = plain_world(superchunks, grass_thousandths as u64 * worldgen::FRACTION_ONE / 1000, 0, thread_count);
    let start_grass = count(&world, GRASS);
    let (mut computing_time, mut applying_time, mut writes, mut grass_sampled, mut missed) = (Duration::ZERO, Duration::ZERO, 0, 0, 0);
    let chosen_rules = ChosenRules::from_rules(&[crate::GRASS_RULE]);
    for _ in 0..ticks {
        let report = world.tick_only(chosen_rules, false);
        computing_time += report.computing_time;
        applying_time += report.applying_time;
        writes += report.writes_applied.writes;
        missed += report.writes_applied.missed;
        grass_sampled += report.rules.counts_of(crate::GRASS_RULE)[sca_rules::grass::SAMPLED_COUNT] as usize;
        process_memory.take_reading();
    }
    ThroughputRun {
        ticks,
        superchunks: superchunks_of(&world),
        thread_count,
        grass_at_start_and_end: (start_grass, count(&world, GRASS)),
        grass_sampled,
        writes,
        missed,
        computing_time,
        applying_time,
        process_memory,
        arena_stats: ArenaStats::measure_arena(&world.bitmap_arena),
        storage_stats: StorageStats::measure_storage(&world.chunk_storage),
    }
}
