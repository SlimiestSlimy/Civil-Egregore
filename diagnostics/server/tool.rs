//! The server's diagnostics tools: each runs a diagnostic, prints what
//! it gathered and keeps it in `transient_data/measurements/`. They
//! are among the server's commands ([`crate::commands`]).

use std::time::Duration;
use crate::diagnostics::{pasture as pasture_run, throughput};
use simulation::threads_for;
use crate::transient_data::TRANSIENT_DATA;
use utilities::commands::GivenCommand;
use utilities::diagnostics::process_memory::mebibytes_text;
use utilities::diagnostics::table::report::Report;
use utilities::diagnostics::table::Table;

/// Ticks run.
pub(crate) const TICKS: &str = "ticks";
/// The share of the cells that start as grass, in thousandths.
pub(crate) const GRASS_THOUSANDTHS: &str = "grass, thousandths";
/// Superchunks ticked.
pub(crate) const SUPERCHUNKS: &str = "superchunks";
/// Threads ticking: 0, every one the machine has.
pub(crate) const THREAD_COUNT: &str = "threads";
/// Sheep on each superchunk.
pub(crate) const FLOCK: &str = "sheep a superchunk";
/// The seed of the world checked, in hexadecimal: 0, the run's own.
pub(crate) const SEED: &str = "seed";
/// Ticks between two hashes printed.
pub(crate) const TICKS_BETWEEN_HASHES: &str = "ticks between hashes";
/// Sheep the world checked starts with, on its origin.
pub(crate) const ORIGIN_SHEEP: &str = "sheep";

/// The threads asked for: every one the machine has, no more than
/// `superchunks`, if 0.
fn threads_asked(given_arguments: &GivenCommand, superchunks: u32) -> Result<usize, String> {
    Ok(match given_arguments.argument_number(THREAD_COUNT)? {
        0 => threads_for(superchunks as usize),
        asked_threads => asked_threads,
    })
}

/// Ticks grass flat out and publishes what each phase took and the
/// memory held.
pub(crate) fn print_throughput(given_arguments: &GivenCommand) -> Result<(), String> {
    let (ticks, grass_thousandths, superchunks): (usize, usize, u32) = (given_arguments.argument_number(TICKS)?, given_arguments.argument_number(GRASS_THOUSANDTHS)?, given_arguments.argument_number(SUPERCHUNKS)?);
    let thread_count = threads_asked(given_arguments, superchunks)?;
    let measured_run = throughput::run_throughput(ticks, grass_thousandths, superchunks, thread_count);
    let mut report = Report::new(given_arguments.command_name(), &given_arguments.resolved_line());
    report.add_note(format!(
        "{ticks} ticks over {superchunks} superchunk(s) on {thread_count} thread(s); grass {} -> {}; {} samples; {} cells missed past the superchunks used",
        measured_run.grass_at_start_and_end.0, measured_run.grass_at_start_and_end.1, measured_run.grass_sampled, measured_run.missed
    ));
    let tick_time = measured_run.computing_time + measured_run.applying_time;
    let mut phases = Table::new(&["phase", "total ms", "share", "ns a write"]).left_aligned(&["phase"]);
    for (phase_name, phase_time) in [("computing", measured_run.computing_time), ("applying", measured_run.applying_time), ("the tick", tick_time)] {
        phases.add_row(&[
            phase_name.to_string(),
            format!("{:.1}", phase_time.as_secs_f64() * 1e3),
            format!("{:.1}%", 100.0 * phase_time.as_secs_f64() / tick_time.as_secs_f64()),
            format!("{:.1}", phase_time.as_nanos() as f64 / measured_run.writes as f64),
        ]);
    }
    report.add_table("time", phases);
    let mut rates_table = Table::new(&["writes a tick", "writes a second", "ticks a second"]);
    rates_table.add_row(&[
        format!("{:.0}", measured_run.writes as f64 / ticks as f64),
        format!("{:.2} million", measured_run.writes as f64 / tick_time.as_secs_f64() / 1e6),
        format!("{:.0}", ticks as f64 / tick_time.as_secs_f64()),
    ]);
    report.add_table("rates", rates_table);
    let unknown_text = || "unknown".to_string();
    let mut memory_table = Table::new(&["memory", "bytes"]).left_aligned(&["memory"]);
    memory_table.add_row(&["process, peak".to_string(), measured_run.process_memory.peak_bytes().map_or_else(unknown_text, mebibytes_text)]);
    memory_table.add_row(&["process, average over the ticks".to_string(), measured_run.process_memory.average_resident_bytes().map_or_else(unknown_text, mebibytes_text)]);
    memory_table.add_row(&[format!("arena buckets kept ({} of {} hot bitmaps)", measured_run.arena_stats.buckets, measured_run.arena_stats.hot_bitmaps), mebibytes_text(measured_run.arena_stats.bucket_bytes)]);
    memory_table.add_row(&[format!("storage images in memory ({} of {}, the rest paged to disk)", measured_run.storage_stats.superchunks - measured_run.storage_stats.on_disk, measured_run.storage_stats.superchunks), mebibytes_text(measured_run.storage_stats.image_bytes)]);
    memory_table.add_row(&["storage ring".to_string(), mebibytes_text(measured_run.storage_stats.ring_bytes)]);
    report.add_table("memory", memory_table);
    TRANSIENT_DATA.publish_report(report);
    Ok(())
}

/// Ticks grass and sheep flat out and publishes the flock, what the
/// sheep did, each rule's time and the memory held.
pub(crate) fn print_pasture(given_arguments: &GivenCommand) -> Result<(), String> {
    let (ticks, grass_thousandths, flock, superchunks): (usize, usize, usize, u32) = (given_arguments.argument_number(TICKS)?, given_arguments.argument_number(GRASS_THOUSANDTHS)?, given_arguments.argument_number(FLOCK)?, given_arguments.argument_number(SUPERCHUNKS)?);
    let thread_count = threads_asked(given_arguments, superchunks)?;
    let measured_run = pasture_run::run_pasture(ticks, grass_thousandths, flock, superchunks, thread_count);
    let mut report = Report::new(given_arguments.command_name(), &given_arguments.resolved_line());
    let (grass_counts, sheep_counts) = (measured_run.rule_counts_added.counts_of(crate::GRASS_RULE), measured_run.rule_counts_added.counts_of(crate::SHEEP_RULE));
    let sheep_woken = sheep_counts[entity_rules::sheep::WOKEN_COUNT] as usize;
    report.add_note(format!(
        "{ticks} ticks over {superchunks} superchunk(s) on {thread_count} thread(s); sheep {} -> {}; grass {} -> {}; {} entities lost past the superchunks used; {} acts over a border",
        measured_run.sheep_at_start_and_end.0, measured_run.sheep_at_start_and_end.1, measured_run.grass_at_start_and_end.0, measured_run.grass_at_start_and_end.1, measured_run.instructions.lost, measured_run.acts_over_a_border
    ));
    let mut flock = Table::new(&["wakes", "wakes a tick", "eaten", "born", "died", "put", "moved", "wakes written", "removed"]);
    flock.add_row(&[
        sheep_woken.to_string(),
        format!("{:.1}", sheep_woken as f64 / ticks as f64),
        sheep_counts[entity_rules::sheep::EATEN_COUNT].to_string(),
        sheep_counts[entity_rules::sheep::BIRTHS_COUNT].to_string(),
        sheep_counts[entity_rules::sheep::DEATHS_COUNT].to_string(),
        measured_run.instructions.puts.to_string(),
        measured_run.instructions.moves.to_string(),
        measured_run.instructions.wakes.to_string(),
        measured_run.instructions.entities_removed.to_string(),
    ]);
    report.add_table("sheep", flock);
    let tick_time = measured_run.computing_time + measured_run.applying_time;
    let mut phases = Table::new(&["time", "total ms", "share of the tick", "ns each"]).left_aligned(&["time"]);
    let time_each = |added_time: Duration, count: usize| if count == 0 { "-".to_string() } else { format!("{:.1}", added_time.as_nanos() as f64 / count as f64) };
    phases.add_row(&["computing".to_string(), format!("{:.1}", measured_run.computing_time.as_secs_f64() * 1e3), share_percent(measured_run.computing_time, tick_time), "-".to_string()]);
    phases.add_row(&["  grass rule, a sample (all threads)".to_string(), format!("{:.1}", measured_run.rule_counts_added.time_of(crate::GRASS_RULE).as_secs_f64() * 1e3), "-".to_string(), time_each(measured_run.rule_counts_added.time_of(crate::GRASS_RULE), grass_counts[sca_rules::grass::SAMPLED_COUNT] as usize)]);
    phases.add_row(&["  sheep rule, a wake (all threads)".to_string(), format!("{:.1}", measured_run.rule_counts_added.time_of(crate::SHEEP_RULE).as_secs_f64() * 1e3), "-".to_string(), time_each(measured_run.rule_counts_added.time_of(crate::SHEEP_RULE), sheep_woken)]);
    phases.add_row(&["applying, a write or instruction".to_string(), format!("{:.1}", measured_run.applying_time.as_secs_f64() * 1e3), share_percent(measured_run.applying_time, tick_time), time_each(measured_run.applying_time, measured_run.writes + measured_run.instructions.puts + measured_run.instructions.moves + measured_run.instructions.wakes + measured_run.instructions.edits + measured_run.instructions.entities_removed)]);
    phases.add_row(&["the tick".to_string(), format!("{:.1}", tick_time.as_secs_f64() * 1e3), share_percent(tick_time, tick_time), "-".to_string()]);
    report.add_table("time", phases);
    let mut rates_table = Table::new(&["ticks a second", "wakes a second"]);
    rates_table.add_row(&[format!("{:.0}", ticks as f64 / tick_time.as_secs_f64()), format!("{:.2} million", sheep_woken as f64 / tick_time.as_secs_f64() / 1e6)]);
    report.add_table("rates", rates_table);
    let unknown_text = || "unknown".to_string();
    let mut memory_table = Table::new(&["memory", "amount"]).left_aligned(&["memory"]);
    memory_table.add_row(&["process, peak".to_string(), measured_run.process_memory.peak_bytes().map_or_else(unknown_text, mebibytes_text)]);
    memory_table.add_row(&["process, average over the ticks".to_string(), measured_run.process_memory.average_resident_bytes().map_or_else(unknown_text, mebibytes_text)]);
    memory_table.add_row(&["entities".to_string(), measured_run.entity_stats.entity_count.to_string()]);
    memory_table.add_row(&["attribute blocks in use, and garbage".to_string(), format!("{}, {}", measured_run.entity_stats.attributes, measured_run.entity_stats.garbage_blocks)]);
    memory_table.add_row(&["wakes filed".to_string(), measured_run.entity_stats.wakes.to_string()]);
    report.add_table("held", memory_table);
    report.add_table("census", census_table(measured_run.census.iter().map(|census| [census.tick as u64, census.sheep as u64, census.grass, census.woken as u64, census.births as u64, census.deaths as u64])));
    TRANSIENT_DATA.publish_report(report);
    Ok(())
}

/// The flock and the grass over a run, a row a count: the tick, sheep,
/// grass, and sheep woken, born and starved since the count before.
fn census_table(census_rows: impl Iterator<Item = [u64; 6]>) -> Table {
    let mut table = Table::new(&["tick", "sheep", "grass", "woken", "born", "starved"]);
    for census_row in census_rows {
        table.add_row(&census_row.map(|value| value.to_string()));
    }
    table
}

/// `part` as a percentage of `whole_time`.
fn share_percent(part: Duration, whole_time: Duration) -> String {
    format!("{:.1}%", 100.0 * part.as_secs_f64() / whole_time.as_secs_f64())
}

/// Ticks a world from a seed, its camera loading nothing, and prints
/// its hash before the first tick and every so many after, part by
/// part, nothing of the machine in it (`docs/server.md`, "The same on
/// every machine").
pub(crate) fn print_world_hashes(given_arguments: &GivenCommand) -> Result<(), String> {
    let (ticks, ticks_between_hashes, origin_sheep): (u64, u64, usize) = (given_arguments.argument_number(TICKS)?, given_arguments.argument_number(TICKS_BETWEEN_HASHES)?, given_arguments.argument_number(ORIGIN_SHEEP)?);
    let seed_text = given_arguments.argument_text(SEED)?;
    let seed = utilities::seed::given_or_fresh_seed(utilities::seed::of_hex(seed_text).ok_or_else(|| format!("`{seed_text}` is not a seed: 64 bits, in hexadecimal"))?);
    let thread_count = Some(given_arguments.argument_number(THREAD_COUNT)?).filter(|&asked_threads: &usize| asked_threads > 0);
    let mut world = crate::start_world(crate::WorldStart { seed, sheep_per_flock: origin_sheep, thread_count, ..crate::WorldStart::default() });
    println!("# seed {}, {origin_sheep} sheep", utilities::seed::seed_as_hex(seed));
    println!("tick,world,cells,entities,random streams,halos,cold");
    loop {
        let hash_by_part = crate::world_hash(&mut world);
        println!("{},{:016x},{:016x},{:016x},{:016x},{:016x},{:016x}", hash_by_part.tick, hash_by_part.whole_hash(seed), hash_by_part.cells, hash_by_part.entities, hash_by_part.random_streams, hash_by_part.halos, hash_by_part.cold);
        if hash_by_part.tick >= ticks {
            return Ok(());
        }
        for _ in 0..ticks_between_hashes.max(1).min(ticks - hash_by_part.tick) {
            world.tick();
        }
    }
}

