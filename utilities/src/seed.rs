//! The seed: what every crate's tests and tools grow their worlds,
//! bitmaps or whatever else from -- not a constant in the code, but one
//! number in one file for the whole workspace ([`file`]), so that using
//! the same one twice is a thing seen and using another costs no edit.
//!
//! Reusing a seed is what comparing two versions of the code needs:
//! holding what is tested still while the code moves. But a seed held
//! for long becomes the only one every change was ever tried on, and
//! what passes may pass on that seed alone. So the file keeps, with the
//! seed, how many runs have used it, and after
//! [`USES_BEFORE_THE_SEED_ROLLS`] the next run rolls a fresh one by
//! itself. The first asking of a run settles the seed, and a one-row
//! table on standard error says which it is, which use of it the run
//! is and where it came from -- so a failure names the seed that made it.
//!
//! [`VARIABLE`] in the environment picks a seed for one run and leaves
//! the file alone, its count too; [`FRESH`] as its value draws one for
//! the run. A use may be left uncounted ([`uncounted`]): for quick
//! checks run far more often than anything measured.
//!
//! The file is in the workspace's own `transient_data/`, beside the
//! crates and not tracked by git: a seed and its count belong to the
//! working copy they were used in.

use crate::diagnostics::table::Table;
use std::path::PathBuf;
use std::io::Write;
use std::sync::OnceLock;

/// How many runs may use a seed from the file before the next run rolls
/// a fresh one: a few compare-and-change cycles on the same seed, and
/// never a whole feature's worth.
pub const USES_BEFORE_THE_SEED_ROLLS: u64 = 5;

/// The variable's value that draws a fresh seed for one run and leaves
/// the file alone.
pub const FRESH: &str = "fresh";

/// The file's name, in the workspace's transient data.
pub const FILE: &str = "seed";

/// The environment variable that picks a seed for one run.
pub const VARIABLE: &str = "TILESIM_SEED";

/// The file that keeps the seed and how many runs have used it: in the
/// `transient_data/` of the workspace, the folder the crates are in.
pub fn file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(crate::transient_data::FOLDER).join(FILE)
}

/// Whether a run counts as a use of the file's seed.
#[derive(Clone, Copy, PartialEq)]
enum Counted {
    /// It does: a measurement, a tool, the fast and complete tests.
    Yes,
    /// It does not: the fine tests.
    No,
}

/// A run's seed, once settled.
struct Settled {
    /// The seed.
    seed: u64,
    /// Whether it was drawn for this run alone, and not kept.
    fresh: bool,
    /// Which use of it this run is, or that the run does not count.
    uses: String,
    /// Where it came from.
    source: String,
}

/// The run's seed, once settled: one a process.
static SETTLED: OnceLock<Settled> = OnceLock::new();

/// A seed drawn from the process's own randomness: std's hasher keys,
/// fresh every run.
fn fresh_seed() -> u64 {
    use std::hash::{BuildHasher, RandomState};
    RandomState::new().hash_one(std::process::id())
}

/// The seed this run settled on, and whether it was fresh, if any has
/// been asked for: what a measurement says it was measured on.
pub fn in_use() -> Option<(u64, bool)> {
    SETTLED.get().map(|settled| (settled.seed, settled.fresh))
}

/// The run's seed, counted as a use of the file's: [`VARIABLE`]'s if it
/// is set, for this run alone; else the file's, or, once that has been
/// used [`USES_BEFORE_THE_SEED_ROLLS`] times, a fresh one rolled in its
/// place.
pub fn counted() -> u64 {
    settled(Counted::Yes).seed
}

/// The run's seed as [`counted`] gives it, but not counted as a use. A
/// file used up is still used; the next counted run rolls it. No file
/// yet: one is rolled, and kept at no uses.
pub fn uncounted() -> u64 {
    settled(Counted::No).seed
}

/// The seed itself, settled once however often it is asked for, and
/// said in a table on standard error.
fn settled(counted: Counted) -> &'static Settled {
    SETTLED.get_or_init(|| {
        let settled = settle(counted);
        let mut table = Table::new(&["seed", "use", "from"]).left_aligned(&["use", "from"]);
        table.row(&[settled.seed.to_string(), settled.uses.clone(), settled.source.clone()]);
        let _ = write!(std::io::stderr(), "{}", table.rendered());
        settled
    })
}

/// [`settled`]'s seed, settled: from the variable, or the file, rolled
/// if used up, and the file rewritten.
fn settle(counted: Counted) -> Settled {
    let (variable, file) = (VARIABLE, file());
    let kept_at = format!("{}/{FILE}", crate::transient_data::FOLDER);
    let not_counted = "not counted".to_string();
    if std::env::var(variable).is_ok_and(|value| value.trim() == FRESH) {
        return Settled { seed: fresh_seed(), fresh: true, uses: not_counted, source: "drawn for this run alone, not kept".to_string() };
    }
    if let Some(seed) = std::env::var(variable).ok().and_then(|value| value.trim().parse::<u64>().ok()) {
        return Settled { seed, fresh: false, uses: not_counted, source: format!("{variable}, for this run alone; the file left as it is") };
    }
    let held = std::fs::read_to_string(&file).ok();
    let mut kept = held.iter().flat_map(|text| text.lines());
    let last = kept.next().and_then(|line| line.trim().parse::<u64>().ok());
    let uses = kept.next().and_then(|line| line.trim().parse::<u64>().ok()).unwrap_or(0);

    let same_as_last = format!("{kept_at}: the same as the last run");
    let (seed, uses, uses_note, source) = match (last, counted) {
        (Some(last), Counted::No) => (last, uses, format!("not counted: {uses} of {USES_BEFORE_THE_SEED_ROLLS} so far"), same_as_last),
        (Some(last), Counted::Yes) if uses < USES_BEFORE_THE_SEED_ROLLS => (last, uses + 1, format!("{} of {USES_BEFORE_THE_SEED_ROLLS}", uses + 1), same_as_last),
        _ => {
            let rolled = last.map_or(String::new(), |last| format!(": seed {last} was used {uses} times"));
            let uses = (counted == Counted::Yes) as u64;
            let uses_note = match counted {
                Counted::Yes => format!("{uses} of {USES_BEFORE_THE_SEED_ROLLS}"),
                Counted::No => format!("not counted: {uses} of {USES_BEFORE_THE_SEED_ROLLS} so far"),
            };
            (fresh_seed(), uses, uses_note, format!("{kept_at}, rolled for this run{rolled}"))
        }
    };
    if let Some(folder) = file.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    let _ = std::fs::write(&file, format!("{seed}\n{uses}\n"));
    Settled { seed, fresh: false, uses: uses_note, source }
}
