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
pub const FRESH_SEED_VALUE: &str = "fresh";

/// The file's name, in the workspace's transient data.
pub const SEED_FILE_NAME: &str = "seed.csv";

/// The file's columns.
const SEED_COLUMNS: [&str; 2] = ["seed", "uses"];

/// The environment variable that picks a seed for one run.
pub const SEED_VARIABLE: &str = "CIVIL_EGREGORE_SEED";

/// The file that keeps the seed and how many runs have used it: in the
/// `transient_data/` of the workspace, the folder the crates are in.
pub fn seed_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(crate::transient_data::FOLDER).join(SEED_FILE_NAME)
}

/// `seed` as it is written everywhere: `0x` and its 16 hexadecimal
/// digits.
pub fn seed_as_hex(seed: u64) -> String {
    format!("{seed:#018x}")
}

/// The seed `hex_text` is, in hexadecimal, with or without `0x` before it.
pub fn of_hex(hex_text: &str) -> Option<u64> {
    let trimmed_text = hex_text.trim();
    u64::from_str_radix(trimmed_text.strip_prefix("0x").or_else(|| trimmed_text.strip_prefix("0X")).unwrap_or(trimmed_text), 16).ok()
}

/// Whether a run counts as a use of the file's seed.
#[derive(Clone, Copy, PartialEq)]
enum UseCounting {
    /// It does: a measurement, a tool, the fast and complete tests.
    CountsAUse,
    /// It does not: the fine tests.
    CountsNoUse,
}

/// A run's seed, once settled.
struct SettledSeed {
    /// The seed.
    seed: u64,
    /// Whether it was drawn for this run alone, and not kept.
    is_fresh: bool,
    /// Which use of it this run is, or that the run does not count.
    use_note: String,
    /// Where it came from.
    seed_source: String,
}

/// The run's seed, once settled: one a process.
static SETTLED_SEED: OnceLock<SettledSeed> = OnceLock::new();

/// A seed drawn from the process's own randomness: std's hasher keys,
/// fresh every run.
fn fresh_seed() -> u64 {
    use std::hash::{BuildHasher, RandomState};
    RandomState::new().hash_one(std::process::id())
}

/// The seed this run settled on, and whether it was fresh, if any has
/// been asked for: what a measurement says it was measured on.
pub fn in_use() -> Option<(u64, bool)> {
    SETTLED_SEED.get().map(|settled_seed| (settled_seed.seed, settled_seed.is_fresh))
}

/// The run's seed, counted as a use of the file's: [`SEED_VARIABLE`]'s if it
/// is set, for this run alone; else the file's, or, once that has been
/// used [`USES_BEFORE_THE_SEED_ROLLS`] times, a fresh one rolled in its
/// place.
pub fn counted_seed() -> u64 {
    settled_once(UseCounting::CountsAUse).seed
}

/// The run's seed as [`counted_seed`] gives it, but not counted as a use. A
/// file used up is still used; the next counted run rolls it. No file
/// yet: one is rolled, and kept at no uses.
pub fn uncounted_seed() -> u64 {
    settled_once(UseCounting::CountsNoUse).seed
}

/// The seed itself, settled once however often it is asked for, and
/// said in a table on standard error.
fn settled_once(use_counting: UseCounting) -> &'static SettledSeed {
    SETTLED_SEED.get_or_init(|| {
        let settled_seed = settle_seed(use_counting);
        let mut table = Table::new(&["seed", "use", "from"]).left_aligned(&["use", "from"]);
        table.add_row(&[seed_as_hex(settled_seed.seed), settled_seed.use_note.clone(), settled_seed.seed_source.clone()]);
        let _ = write!(std::io::stderr(), "{}", table.rendered_text());
        settled_seed
    })
}

/// [`settled_once`]'s seed, settled: from the variable, or the file, rolled
/// if used up, and the file rewritten.
fn settle_seed(use_counting: UseCounting) -> SettledSeed {
    let (variable_name, seed_path) = (SEED_VARIABLE, seed_file());
    let kept_at = format!("{}/{SEED_FILE_NAME}", crate::transient_data::FOLDER);
    let not_counted = "not counted".to_string();
    if std::env::var(variable_name).is_ok_and(|value| value.trim() == FRESH_SEED_VALUE) {
        return SettledSeed { seed: fresh_seed(), is_fresh: true, use_note: not_counted, seed_source: "drawn for this run alone, not kept".to_string() };
    }
    if let Some(seed) = std::env::var(variable_name).ok().and_then(|value| of_hex(&value)) {
        return SettledSeed { seed, is_fresh: false, use_note: not_counted, seed_source: format!("{variable_name}, for this run alone; the file left as it is") };
    }
    let file_text = std::fs::read_to_string(&seed_path).ok();
    let kept_row = file_text.as_deref().and_then(|kept_text| crate::csv::rows_by_column(kept_text, &SEED_COLUMNS).into_iter().next()).unwrap_or_default();
    let last_seed = kept_row.first().and_then(|seed| of_hex(seed));
    let past_uses = kept_row.get(1).and_then(|uses_field| uses_field.trim().parse::<u64>().ok()).unwrap_or(0);

    let same_as_last = format!("{kept_at}: the same as the last run");
    let (seed, new_uses, uses_note, seed_source) = match (last_seed, use_counting) {
        (Some(reused_seed), UseCounting::CountsNoUse) => (reused_seed, past_uses, format!("not counted: {past_uses} of {USES_BEFORE_THE_SEED_ROLLS} so far"), same_as_last),
        (Some(counted_reused_seed), UseCounting::CountsAUse) if past_uses < USES_BEFORE_THE_SEED_ROLLS => (counted_reused_seed, past_uses + 1, format!("{} of {USES_BEFORE_THE_SEED_ROLLS}", past_uses + 1), same_as_last),
        _ => {
            let rolled_note = last_seed.map_or(String::new(), |used_up_seed| format!(": seed {} was used {past_uses} times", seed_as_hex(used_up_seed)));
            let first_uses = (use_counting == UseCounting::CountsAUse) as u64;
            let uses_note = match use_counting {
                UseCounting::CountsAUse => format!("{first_uses} of {USES_BEFORE_THE_SEED_ROLLS}"),
                UseCounting::CountsNoUse => format!("not counted: {first_uses} of {USES_BEFORE_THE_SEED_ROLLS} so far"),
            };
            (fresh_seed(), first_uses, uses_note, format!("{kept_at}, rolled for this run{rolled_note}"))
        }
    };
    if let Some(folder) = seed_path.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    let _ = std::fs::write(&seed_path, crate::csv::row_text(&SEED_COLUMNS) + &crate::csv::row_text(&[seed_as_hex(seed), new_uses.to_string()]));
    SettledSeed { seed, is_fresh: false, use_note: uses_note, seed_source }
}
