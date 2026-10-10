//! Grass and sheep ticked flat out over a plain, on as many threads
//! as asked: the flock and the grass over the run, what the sheep did,
//! each phase's time, each rule's time in the first -- added up over
//! the threads -- and the memory held.

use type_registry::GRASS;
use crate::diagnostics::plain_world::{plain_world, superchunks as superchunks_of};
use crate::host::frame::count;
use crate::{Chosen, TickCounts};
use entity_rules::sheep::{BIRTHS, DEATHS, WOKEN};
use entity_manager::diagnostics::entities::EntityStats;
use entity_manager::InstructionsApplied;
use instructions::RuleCounts;
use std::time::Duration;
use utilities::diagnostics::process_memory::MemoryTrack;

/// What a run did, and what it held.
#[derive(Clone, Debug)]
pub struct PastureRun {
    /// Ticks run.
    pub ticks: usize,
    /// Superchunks ticked.
    pub superchunks: usize,
    /// Threads ticked on.
    pub threads: usize,
    /// Sheep at the start, and at the end.
    pub sheep: (usize, usize),
    /// Cells of grass at the start, and at the end.
    pub grass: (u64, u64),
    /// What grass and sheep did, added up, and each rule's time over
    /// every thread.
    pub done: TickCounts,
    /// What applying the instructions did, added up.
    pub instructions: InstructionsApplied,
    /// Writes applied.
    pub writes: usize,
    /// The first phase's time, added up.
    pub computing: Duration,
    /// The second phase's time, added up.
    pub applying: Duration,
    /// The process's memory, read after every tick.
    pub memory: MemoryTrack,
    /// What the entities held at the end.
    pub held: EntityStats,
    /// The flock and the grass over the run, every [`CENSUS_EVERY`]
    /// ticks and at the end.
    pub census: Vec<Census>,
}

/// Ticks between two counts of the flock and the grass.
pub const CENSUS_EVERY: usize = 100;

/// The flock and the grass at a tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct Census {
    /// The tick about to run.
    pub tick: usize,
    /// Sheep.
    pub sheep: usize,
    /// Cells of grass.
    pub grass: u64,
    /// Sheep woken since the last count.
    pub woken: usize,
    /// Lambs born since the last count.
    pub births: usize,
    /// Sheep starved since the last count.
    pub deaths: usize,
}

/// The rules the pasture ticks, by name.
pub const RULES_TICKED: [&str; 2] = ["grass", "sheep"];

/// Ticks grass and sheep `ticks` times over `superchunks` superchunks,
/// grass drawn on `thousandths` of each one's cells and `sheep` sheep on
/// each, on `threads` threads.
pub fn run(ticks: usize, thousandths: usize, sheep: usize, superchunks: u32, threads: usize) -> PastureRun {
    let mut memory = MemoryTrack::default();
    memory.read();
    let mut world = plain_world(superchunks, thousandths as u64 * worldgen::ONE / 1000, sheep, threads);
    let (start_sheep, start_grass) = (world.entities.len(), count(&world, GRASS));
    let (mut done, mut instructions, mut writes, mut computing, mut applying) = (TickCounts::default(), InstructionsApplied::default(), 0, Duration::ZERO, Duration::ZERO);
    let mut census = vec![Census { tick: 0, sheep: start_sheep, grass: start_grass, ..Census::default() }];
    let mut since = RuleCounts::default();
    let chosen = Chosen::named(&RULES_TICKED);
    for tick in 0..ticks {
        let report = world.tick_only(chosen, true);
        done += report.rules;
        since += report.rules.of("sheep");
        if (tick + 1) % CENSUS_EVERY == 0 || tick + 1 == ticks {
            census.push(Census { tick: tick + 1, sheep: world.entities.len(), grass: count(&world, GRASS), woken: since[WOKEN] as usize, births: since[BIRTHS] as usize, deaths: since[DEATHS] as usize });
            since = RuleCounts::default();
        }
        instructions += report.instructions_applied;
        writes += report.writes_applied.writes;
        computing += report.computing;
        applying += report.applying;
        memory.read();
    }
    PastureRun {
        ticks,
        superchunks: superchunks_of(&world),
        threads,
        sheep: (start_sheep, world.entities.len()),
        grass: (start_grass, count(&world, GRASS)),
        done,
        instructions,
        writes,
        computing,
        applying,
        memory,
        held: EntityStats::of(&world.entities),
        census,
    }
}
