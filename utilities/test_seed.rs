//! The test seed: the one seed every crate's tests grow what they run
//! on from, kept in the workspace's `transient_data/test_seed.csv` and
//! rolled every few uses (`docs/utilities.md`, "The seed"). Test
//! infrastructure: beside `src/`, compiled into no library, each test
//! program taking it by path --
//! `#[path = "../../test_seed.rs"] mod test_seed;` from `utilities/coordinates/tests/`.
//! Freeze a seed with [`FREEZE_SEED`]; delete the file to roll one.

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use utilities::seed::{fresh_seed, of_hex, seed_as_hex};

/// How many uses a seed serves before the next rolls a fresh one.
pub const USES_BEFORE_THE_SEED_ROLLS: u64 = 5;

/// The environment variable that freezes the seed, for a failure to be
/// run again: set to a seed in hexadecimal, every test takes that seed
/// and the file is left as it is; set empty, every test takes the
/// file's seed, and it never rolls.
pub const FREEZE_SEED: &str = "FREEZE_SEED";

/// The file's columns.
const SEED_COLUMNS: [&str; 2] = ["seed", "uses"];

/// A handle on the workspace's test seed file. Every handle locks the
/// file while it reads and writes it, so tests on parallel threads, and
/// other processes, take turns.
#[derive(Clone)]
pub struct TestSeedHandle {
    /// `transient_data/test_seed.csv` in the workspace.
    seed_file: PathBuf,
}

impl TestSeedHandle {
    /// The handle on the workspace's seed file: the workspace is the
    /// folder above the test's crate that holds `Cargo.lock`.
    pub fn open() -> Self {
        let crate_folder = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_folder = crate_folder.ancestors().find(|folder| folder.join("Cargo.lock").is_file()).expect("a test inside the workspace");
        Self { seed_file: workspace_folder.join("transient_data").join("test_seed.csv") }
    }

    /// The seed, counted as a use.
    pub fn seed(&self) -> u64 {
        self.seed_counting(true)
    }

    /// The seed, counted as no use: the fine tests'.
    pub fn fine_seed(&self) -> u64 {
        self.seed_counting(false)
    }

    /// The file's seed, read under the file's lock: rolled if there is
    /// none, or if a counted use finds it used up; the file rewritten
    /// when a use is counted or a seed rolled.
    fn seed_counting(&self, counts_a_use: bool) -> u64 {
        let frozen_text = std::env::var(FREEZE_SEED).ok();
        if let Some(frozen_seed_text) = frozen_text.as_deref().filter(|frozen_seed_text| !frozen_seed_text.trim().is_empty()) {
            return of_hex(frozen_seed_text).unwrap_or_else(|| panic!("{FREEZE_SEED}={frozen_seed_text}: not a seed in hexadecimal"));
        }
        let counts_a_use = counts_a_use && frozen_text.is_none();
        std::fs::create_dir_all(self.seed_file.parent().expect("the file is in a folder")).expect("the transient data folder made");
        let mut file = OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&self.seed_file).expect("the test seed file opened");
        file.lock().expect("the test seed file locked");
        let mut kept_text = String::new();
        file.read_to_string(&mut kept_text).expect("the test seed file read");
        let kept_row = utilities::csv::rows_by_column(&kept_text, &SEED_COLUMNS).into_iter().next().unwrap_or_default();
        let kept_seed = kept_row.first().and_then(|seed| of_hex(seed));
        let past_uses: u64 = kept_row.get(1).and_then(|uses| uses.trim().parse().ok()).unwrap_or(0);

        let (seed, uses) = match kept_seed {
            Some(seed) if !counts_a_use => return seed,
            Some(seed) if past_uses < USES_BEFORE_THE_SEED_ROLLS => (seed, past_uses + 1),
            _ => (fresh_seed(), counts_a_use as u64),
        };
        let new_text = utilities::csv::row_text(&SEED_COLUMNS) + &utilities::csv::row_text(&[seed_as_hex(seed), uses.to_string()]);
        file.set_len(0).and_then(|()| file.seek(SeekFrom::Start(0))).and_then(|_| file.write_all(new_text.as_bytes())).expect("the test seed file written");
        seed
    }
}
