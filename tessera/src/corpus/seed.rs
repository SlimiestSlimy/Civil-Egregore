//! Where the corpus' seed comes from: the workspace's one seed file,
//! rolled after so many uses ([`utilities::seed`]), and the table that
//! says which seed a run used
//! (`docs/testing_protocol.md`, "Where the seed comes from").

/// The file that remembers the last seed a run used, and how many runs
/// have used it, as the tables say it: under the workspace's
/// folder, kept out of git ([`utilities::seed::file`]).
pub const WHERE_THE_SEED_IS_KEPT: &str = "transient_data/seed.csv";

/// How many runs may use a seed from the file before the next run rolls
/// a fresh one: a few measure-and-compare cycles on the same bitmaps --
/// enough to compare a change against the code before it -- and never a
/// whole feature's worth.
pub const USES_BEFORE_THE_SEED_ROLLS: u64 = utilities::seed::USES_BEFORE_THE_SEED_ROLLS;

/// `CIVIL_EGREGORE_SEED`'s value that draws a fresh seed for one run and leaves the
/// file alone: a check on bitmaps never seen, which moves nothing a
/// measurement holds still -- what the fast tier runs on while the code
/// changes.
pub const FRESH: &str = utilities::seed::FRESH;

/// The seed every corpus bitmap starts from, counted as a use:
/// `CIVIL_EGREGORE_SEED` if set, else the file's, rolled once used up
/// (`docs/lab.md`, "`corpus/`").
pub fn seed_counted() -> u64 {
    utilities::seed::counted()
}

/// The seed every corpus bitmap starts from, as [`seed_counted`] gives it, but
/// not counted as a use: for the fine tests, which run on the same
/// bitmaps as everything else without using them up. A file used up is
/// still used; the next counted run rolls it. No file yet: one is
/// rolled, and kept at no uses.
pub fn seed_uncounted() -> u64 {
    utilities::seed::uncounted()
}

/// The seed this run's corpus came from, and whether it was fresh, if
/// any corpus bitmap has been asked for: what a measurement says it was
/// measured on.
pub fn seed_in_use() -> Option<(u64, bool)> {
    utilities::seed::in_use()
}
