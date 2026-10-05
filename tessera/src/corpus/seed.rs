//! Where the corpus' seed comes from, and the table that says whether
//! it is the same one as last time.
//!
//! A seed is not a constant in the code. It lives in a file in the
//! workspace's transient data, the one every crate's runs share, so that using the same bitmaps twice is a thing you can
//! see rather than a thing you have to remember, and using different
//! ones costs no edit.
//!
//! Every corpus bitmap a run grows starts from the one seed: a shape, a plan
//! or a line set draws its bitmaps from consecutive seeds beginning at
//! it, so the seed is the whole of what settles a run's bitmaps, and a
//! number quoted from a run can be traced to the bitmaps it came from.
//! The first corpus bitmap a run asks for settles the seed, and a one-row
//! table on standard error says which it is, which use of it the run
//! is, and where it came from -- on standard error so that a test's
//! output shows it too.
//!
//! Reusing a seed is what comparing two versions of the code needs --
//! holding the bitmaps still while the code moves. But a seed held for
//! long stops being one round's fixed point and becomes the only corpus
//! every change has ever been measured against: the trap
//! `docs/testing_protocol.md` warns about. So the file keeps, with the
//! seed, how many runs have used it, and after
//! [`USES_BEFORE_THE_SEED_ROLLS`] the next run rolls a fresh one by
//! itself -- no one has to remember to. A run that settles on its seed
//! says which use of it it is, and a run that rolls says so.
//!
//! The fine tests use the same seed as every other run, but a use is
//! not counted for them: they are a quick check run far more often than
//! anything measured, and would roll the seed on their own
//! ([`seed_uncounted`]).
//!
//! The file is not tracked by git (`.gitignore`): a seed and its count
//! belong to the working copy they were used in, and checking out or
//! resetting files must not move them.
//!
//! The rotation itself, and the file, are every crate's: [`utilities::seed`].

/// The file that remembers the last seed a run used, and how many runs
/// have used it, as the tables say it: under the workspace's
/// folder, kept out of git ([`utilities::seed::file`]).
pub const WHERE_THE_SEED_IS_KEPT: &str = "transient_data/seed.csv";

/// How many runs may use a seed from the file before the next run rolls
/// a fresh one: a few measure-and-compare cycles on the same bitmaps --
/// enough to compare a change against the code before it -- and never a
/// whole feature's worth.
pub const USES_BEFORE_THE_SEED_ROLLS: u64 = utilities::seed::USES_BEFORE_THE_SEED_ROLLS;

/// `Civil Egregore_SEED`'s value that draws a fresh seed for one run and leaves the
/// file alone: a check on bitmaps never seen, which moves nothing a
/// measurement holds still -- what the fast tier runs on while the code
/// changes.
pub const FRESH: &str = utilities::seed::FRESH;

/// The seed every corpus bitmap starts from.
///
/// `Civil Egregore_SEED` in the environment wins, so a run can be pinned to any
/// bitmaps -- both sides of a comparison, say -- for that run alone:
/// the file is left as it is, its count too, so pinning never holds a
/// seed past its uses. `Civil Egregore_SEED=fresh` draws one for this run alone,
/// likewise. Otherwise the file's seed is used, and counted -- or, once
/// it has been used [`USES_BEFORE_THE_SEED_ROLLS`] times, a fresh one
/// is rolled in its place.
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
