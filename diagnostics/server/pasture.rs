//! Grass and sheep ticked flat out over a plain, on as many threads
//! as asked: the flock and the grass over the run, what the sheep did,
//! each phase's time, each rule's time in the first -- added up over
//! the threads -- and the memory held.

use type_registry::GRASS;
use crate::diagnostics::plain_world::{plain_world, superchunks as superchunks_of};
use crate::host::frame::count;
use crate::{ChosenRules, RulePlace, TickCounts, GRASS_RULE, SHEEP_RULE};
use entity_rules::sheep::{BIRTHS_COUNT, DEATHS_COUNT, WOKEN_COUNT};
use entity_manager::diagnostics::entities::EntityStats;
use entity_manager::InstructionsApplied;
use simulation::simulation_api::RuleCounts;
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
    pub thread_count: usize,
    /// Sheep at the start, and at the end.
    pub sheep_at_start_and_end: (usize, usize),
    /// Cells of grass at the start, and at the end.
    pub grass_at_start_and_end: (u64, u64),
    /// What grass and sheep did, added up, and each rule's time over
    /// every thread.
    pub rule_counts_added: TickCounts,
    /// What applying the instructions did, added up.
    pub instructions: InstructionsApplied,
    /// Writes applied.
    pub writes: usize,
    /// Acts over a border applied: authors that queued for more
    /// than one superchunk in a tick.
    pub acts_over_a_border: u64,
    /// The first phase's time, added up.
    pub computing_time: Duration,
    /// The second phase's time, added up.
    pub applying_time: Duration,
    /// The process's memory, read after every tick.
    pub process_memory: MemoryTrack,
    /// What the entities held at the end.
    pub entity_stats: EntityStats,
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

/// The rules the pasture ticks.
pub const RULES_TICKED: [RulePlace; 2] = [GRASS_RULE, SHEEP_RULE];

/// Ticks grass and sheep `ticks` times over `superchunks` superchunks,
/// grass drawn on `grass_thousandths` of each one's cells and `sheep_per_flock` sheep on
/// each, on `thread_count` threads.
pub fn run_pasture(ticks: usize, grass_thousandths: usize, sheep_per_flock: usize, superchunks: u32, thread_count: usize) -> PastureRun {
    let mut process_memory = MemoryTrack::default();
    process_memory.take_reading();
    let mut world = plain_world(superchunks, grass_thousandths as u64 * worldgen::FRACTION_ONE / 1000, sheep_per_flock, thread_count);
    let (start_sheep, start_grass) = (world.entity_store.len(), count(&world, GRASS));
    let (mut rule_counts_added, mut instructions, mut writes, mut computing_time, mut applying_time) = (TickCounts::default(), InstructionsApplied::default(), 0, Duration::ZERO, Duration::ZERO);
    let mut acts_over_a_border = 0;
    let mut census = vec![Census { tick: 0, sheep: start_sheep, grass: start_grass, ..Census::default() }];
    let mut sheep_counts_since_census = RuleCounts::default();
    let chosen_rules = ChosenRules::from_rules(&RULES_TICKED);
    for tick in 0..ticks {
        let report = world.tick_only(chosen_rules, true);
        rule_counts_added += report.rules;
        sheep_counts_since_census += report.rules.counts_of(SHEEP_RULE);
        if (tick + 1) % CENSUS_EVERY == 0 || tick + 1 == ticks {
            census.push(Census { tick: tick + 1, sheep: world.entity_store.len(), grass: count(&world, GRASS), woken: sheep_counts_since_census[WOKEN_COUNT] as usize, births: sheep_counts_since_census[BIRTHS_COUNT] as usize, deaths: sheep_counts_since_census[DEATHS_COUNT] as usize });
            sheep_counts_since_census = RuleCounts::default();
        }
        instructions += report.instructions_applied;
        writes += report.writes_applied.writes;
        acts_over_a_border += report.acts_over_a_border;
        computing_time += report.computing_time;
        applying_time += report.applying_time;
        process_memory.take_reading();
    }
    PastureRun {
        ticks,
        superchunks: superchunks_of(&world),
        thread_count,
        sheep_at_start_and_end: (start_sheep, world.entity_store.len()),
        grass_at_start_and_end: (start_grass, count(&world, GRASS)),
        rule_counts_added,
        instructions,
        writes,
        acts_over_a_border,
        computing_time,
        applying_time,
        process_memory,
        entity_stats: EntityStats::measure_entities(&world.entity_store),
        census,
    }
}
