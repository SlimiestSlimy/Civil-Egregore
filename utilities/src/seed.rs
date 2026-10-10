//! The seed: the one number every crate's tests and tools grow what
//! they run on from, kept in one file and rolled every few runs
//! (`docs/utilities.md`, "The seed").

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
pub const FILE: &str = "seed.csv";

/// The file's columns.
const COLUMNS: [&str; 2] = ["seed", "uses"];

/// The environment variable that picks a seed for one run.
pub const VARIABLE: &str = "CIVIL_EGREGORE_SEED";

/// The file that keeps the seed and how many runs have used it: in the
/// `transient_data/` of the workspace, the folder the crates are in.
pub fn file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(crate::transient_data::FOLDER).join(FILE)
}

/// `seed` as it is written everywhere: `0x` and its 16 hexadecimal
/// digits.
pub fn hex(seed: u64) -> String {
    format!("{seed:#018x}")
}

/// The seed `text` is, in hexadecimal, with or without `0x` before it.
pub fn of_hex(text: &str) -> Option<u64> {
    let text = text.trim();
    u64::from_str_radix(text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")).unwrap_or(text), 16).ok()
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
        table.row(&[hex(settled.seed), settled.uses.clone(), settled.source.clone()]);
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
    if let Some(seed) = std::env::var(variable).ok().and_then(|value| of_hex(&value)) {
        return Settled { seed, fresh: false, uses: not_counted, source: format!("{variable}, for this run alone; the file left as it is") };
    }
    let held = std::fs::read_to_string(&file).ok();
    let kept = held.as_deref().and_then(|text| crate::csv::rows_by_column(text, &COLUMNS).into_iter().next()).unwrap_or_default();
    let last = kept.first().and_then(|seed| of_hex(seed));
    let uses = kept.get(1).and_then(|uses| uses.trim().parse::<u64>().ok()).unwrap_or(0);

    let same_as_last = format!("{kept_at}: the same as the last run");
    let (seed, uses, uses_note, source) = match (last, counted) {
        (Some(last), Counted::No) => (last, uses, format!("not counted: {uses} of {USES_BEFORE_THE_SEED_ROLLS} so far"), same_as_last),
        (Some(last), Counted::Yes) if uses < USES_BEFORE_THE_SEED_ROLLS => (last, uses + 1, format!("{} of {USES_BEFORE_THE_SEED_ROLLS}", uses + 1), same_as_last),
        _ => {
            let rolled = last.map_or(String::new(), |last| format!(": seed {} was used {uses} times", hex(last)));
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
    let _ = std::fs::write(&file, crate::csv::row(&COLUMNS) + &crate::csv::row(&[hex(seed), uses.to_string()]));
    Settled { seed, fresh: false, uses: uses_note, source }
}
