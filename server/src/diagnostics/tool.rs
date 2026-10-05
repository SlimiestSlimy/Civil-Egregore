//! The server's diagnostics tools: each runs a diagnostic, prints what
//! it gathered and keeps it in `transient_data/measurements/`. They are
//! among the server's commands ([`crate::commands`]), where their
//! parameters and what each is if not given are listed.
//!
//! Threads 0: every one the machine has, no more than the superchunks.

use std::time::Duration;
use crate::diagnostics::{pasture as pasture_run, throughput};
use simulation::threads_for;
use crate::transient_data::TRANSIENT_DATA;
use utilities::commands::Given;
use utilities::diagnostics::process_memory::mebibytes;
use utilities::diagnostics::table::report::Report;
use utilities::diagnostics::table::Table;

/// Ticks run.
pub(crate) const TICKS: &str = "ticks";
/// The share of the cells that start as grass, in thousandths.
pub(crate) const GRASS: &str = "grass, thousandths";
/// Superchunks ticked.
pub(crate) const SUPERCHUNKS: &str = "superchunks";
/// Threads ticking: 0, every one the machine has.
pub(crate) const THREADS: &str = "threads";
/// Sheep on each superchunk.
pub(crate) const FLOCK: &str = "sheep a superchunk";

/// The threads asked for: every one the machine has, no more than
/// `superchunks`, if 0.
fn threads(given: &Given, superchunks: u32) -> Result<usize, String> {
    Ok(match given.number(THREADS)? {
        0 => threads_for(superchunks as usize),
        threads => threads,
    })
}

/// Ticks grass flat out and publishes what each phase took and the
/// memory held.
pub(crate) fn throughput(given: &Given) -> Result<(), String> {
    let (ticks, thousandths, superchunks): (usize, usize, u32) = (given.number(TICKS)?, given.number(GRASS)?, given.number(SUPERCHUNKS)?);
    let threads = threads(given, superchunks)?;
    let run = throughput::run(ticks, thousandths, superchunks, threads);
    let mut report = Report::new(given.name(), &given.resolved());
    report.note(format!(
        "{ticks} ticks over {superchunks} superchunk(s) on {threads} thread(s); grass {} -> {}; {} samples; {} cells missed past the superchunks used",
        run.grass.0, run.grass.1, run.sampled, run.missed
    ));
    let total = run.computing + run.applying;
    let mut phases = Table::new(&["phase", "total ms", "share", "ns a write"]).left_aligned(&["phase"]);
    for (name, time) in [("computing", run.computing), ("applying", run.applying), ("the tick", total)] {
        phases.row(&[
            name.to_string(),
            format!("{:.1}", time.as_secs_f64() * 1e3),
            format!("{:.1}%", 100.0 * time.as_secs_f64() / total.as_secs_f64()),
            format!("{:.1}", time.as_nanos() as f64 / run.writes as f64),
        ]);
    }
    report.add("time", phases);
    let mut rates = Table::new(&["writes a tick", "writes a second", "ticks a second"]);
    rates.row(&[
        format!("{:.0}", run.writes as f64 / ticks as f64),
        format!("{:.2} million", run.writes as f64 / total.as_secs_f64() / 1e6),
        format!("{:.0}", ticks as f64 / total.as_secs_f64()),
    ]);
    report.add("rates", rates);
    let unknown = || "unknown".to_string();
    let mut memory = Table::new(&["memory", "bytes"]).left_aligned(&["memory"]);
    memory.row(&["process, peak".to_string(), run.memory.peak().map_or_else(unknown, mebibytes)]);
    memory.row(&["process, average over the ticks".to_string(), run.memory.average().map_or_else(unknown, mebibytes)]);
    memory.row(&[format!("arena blocks in use ({})", run.arena.allocations), mebibytes(run.arena.bytes_in_use())]);
    memory.row(&[format!("arena blocks made ({})", run.arena.blocks_made), mebibytes(run.arena.bytes_made)]);
    memory.row(&[format!("storage images ({})", run.storage.superchunks), mebibytes(run.storage.image_bytes)]);
    memory.row(&["storage ring".to_string(), mebibytes(run.storage.ring_bytes)]);
    report.add("memory", memory);
    TRANSIENT_DATA.publish(report);
    Ok(())
}

/// Ticks grass and sheep flat out and publishes the flock, what the
/// sheep did, each rule's time and the memory held.
pub(crate) fn pasture(given: &Given) -> Result<(), String> {
    let (ticks, thousandths, flock, superchunks): (usize, usize, usize, u32) = (given.number(TICKS)?, given.number(GRASS)?, given.number(FLOCK)?, given.number(SUPERCHUNKS)?);
    let threads = threads(given, superchunks)?;
    let run = pasture_run::run(ticks, thousandths, flock, superchunks, threads);
    let mut report = Report::new(given.name(), &given.resolved());
    let sheep = run.done.sheep;
    report.note(format!(
        "{ticks} ticks over {superchunks} superchunk(s) on {threads} thread(s); sheep {} -> {}; grass {} -> {}; {} entities lost past the superchunks used",
        run.sheep.0, run.sheep.1, run.grass.0, run.grass.1, run.instructions.lost
    ));
    let mut flock = Table::new(&["wakes", "wakes a tick", "eaten", "born", "died", "put whole", "moved or slept", "removed"]);
    flock.row(&[
        sheep.woken.to_string(),
        format!("{:.1}", sheep.woken as f64 / ticks as f64),
        sheep.eaten.to_string(),
        sheep.births.to_string(),
        sheep.deaths.to_string(),
        run.instructions.puts.to_string(),
        run.instructions.moves.to_string(),
        run.instructions.removes.to_string(),
    ]);
    report.add("sheep", flock);
    let total = run.computing + run.applying;
    let mut phases = Table::new(&["time", "total ms", "share of the tick", "ns each"]).left_aligned(&["time"]);
    let per = |time: Duration, count: usize| if count == 0 { "-".to_string() } else { format!("{:.1}", time.as_nanos() as f64 / count as f64) };
    phases.row(&["computing".to_string(), format!("{:.1}", run.computing.as_secs_f64() * 1e3), share(run.computing, total), "-".to_string()]);
    phases.row(&["  grass rule, a sample (all threads)".to_string(), format!("{:.1}", run.grass_time.as_secs_f64() * 1e3), "-".to_string(), per(run.grass_time, run.done.grass.sampled)]);
    phases.row(&["  sheep rule, a wake (all threads)".to_string(), format!("{:.1}", run.sheep_time.as_secs_f64() * 1e3), "-".to_string(), per(run.sheep_time, sheep.woken)]);
    phases.row(&["applying, a write or instruction".to_string(), format!("{:.1}", run.applying.as_secs_f64() * 1e3), share(run.applying, total), per(run.applying, run.writes + run.instructions.puts + run.instructions.moves + run.instructions.edits + run.instructions.removes)]);
    phases.row(&["the tick".to_string(), format!("{:.1}", total.as_secs_f64() * 1e3), share(total, total), "-".to_string()]);
    report.add("time", phases);
    let mut rates = Table::new(&["ticks a second", "wakes a second"]);
    rates.row(&[format!("{:.0}", ticks as f64 / total.as_secs_f64()), format!("{:.2} million", sheep.woken as f64 / total.as_secs_f64() / 1e6)]);
    report.add("rates", rates);
    let unknown = || "unknown".to_string();
    let mut memory = Table::new(&["memory", "amount"]).left_aligned(&["memory"]);
    memory.row(&["process, peak".to_string(), run.memory.peak().map_or_else(unknown, mebibytes)]);
    memory.row(&["process, average over the ticks".to_string(), run.memory.average().map_or_else(unknown, mebibytes)]);
    memory.row(&["entities".to_string(), run.held.entities.to_string()]);
    memory.row(&["attributes in use, and garbage".to_string(), format!("{}, {}", run.held.attributes, run.held.garbage)]);
    memory.row(&["wakes filed".to_string(), run.held.wakes.to_string()]);
    report.add("held", memory);
    report.add("census", census_table(run.census.iter().map(|census| [census.tick as u64, census.sheep as u64, census.grass, census.woken as u64, census.births as u64, census.deaths as u64])));
    TRANSIENT_DATA.publish(report);
    Ok(())
}

/// The flock and the grass over a run, a row a count: the tick, sheep,
/// grass, and sheep woken, born and starved since the count before.
fn census_table(rows: impl Iterator<Item = [u64; 6]>) -> Table {
    let mut table = Table::new(&["tick", "sheep", "grass", "woken", "born", "starved"]);
    for row in rows {
        table.row(&row.map(|value| value.to_string()));
    }
    table
}

/// `part` as a percentage of `whole`.
fn share(part: Duration, whole: Duration) -> String {
    format!("{:.1}%", 100.0 * part.as_secs_f64() / whole.as_secs_f64())
}
