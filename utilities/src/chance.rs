//! A chance ([`Chance`]): how likely a thing is, as a whole number of
//! parts in 2^32 -- never a float, so a draw against it, and the gaps
//! between the things it chooses, are the same on every machine. A
//! rule states its chances as constants, worked out as it is built:
//! `Chance::one_in(100_000)`.

use crate::fixed_point::{log2, LOG2_FRACTION_BITS};

/// The parts a chance is out of: a thing certain has them all.
pub const PARTS: u64 = 1 << 32;

/// The bits of a draw that say how far through `(0, 1]` it is: more
/// than the parts' 32, so that gaps far longer than `PARTS` are drawn.
const DRAW_BITS: u32 = 53;

/// How likely a thing is: so many parts in [`PARTS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chance {
    /// The parts in [`PARTS`]: no more than all of them.
    parts: u64,
    /// The logarithm to base 2 of the chance it does not happen,
    /// negated, in fixed point ([`log2`]): what a gap is divided by.
    /// Worked out once, where the chance is made; 0 if it is certain.
    unlikely_log2: u64,
}

impl Chance {
    /// What never happens.
    pub const NEVER: Self = Self::of_parts(0);
    /// What always happens.
    pub const ALWAYS: Self = Self::of_parts(PARTS);
    /// What happens half the time.
    pub const HALF: Self = Self::of_parts(PARTS / 2);

    /// The chance of `parts` in [`PARTS`]; of all of them, if more.
    pub const fn of_parts(parts: u64) -> Self {
        let parts = if parts < PARTS { parts } else { PARTS };
        let unlikely_log2 = if parts == PARTS { 0 } else { ((PARTS.ilog2() as u64) << LOG2_FRACTION_BITS) - log2(PARTS - parts) };
        Self { parts, unlikely_log2 }
    }

    /// The chance of once in `times`, to the nearest part.
    ///
    /// # Panics
    /// If `times` is 0.
    pub const fn one_in(times: u64) -> Self {
        assert!(times != 0, "once in no times is no chance");
        Self::of_parts((PARTS + times / 2) / times)
    }

    /// The chance of this or `other`, where they never both happen:
    /// their parts together.
    pub const fn plus(self, other: Self) -> Self {
        Self::of_parts(self.parts + other.parts)
    }

    /// Its parts in [`PARTS`].
    pub const fn parts(self) -> u64 {
        self.parts
    }

    /// Whether it never happens.
    pub const fn is_never(self) -> bool {
        self.parts == 0
    }

    /// Whether it always happens.
    pub const fn is_always(self) -> bool {
        self.parts == PARTS
    }

    /// How many things in a row are passed over before the next one
    /// chosen, each chosen with this chance: a gap of the geometric law,
    /// from `draw`, any 64 bits (`docs/utilities.md`, "The gap"). The
    /// chance is neither never nor always: those have no gap.
    #[inline]
    pub fn passed_over(self, draw: u64) -> u64 {
        debug_assert!(!self.is_never() && !self.is_always(), "a chance in between");
        // In 1..=2^53: that over 2^53 is in (0, 1], never 0, so it has a logarithm.
        let uniform = (1 << DRAW_BITS) - (draw >> (u64::BITS - DRAW_BITS));
        (((DRAW_BITS as u64) << LOG2_FRACTION_BITS) - log2(uniform)) / self.unlikely_log2
    }

    /// The chance as a fraction of 1, for a report or a test's
    /// expectation: nothing is drawn against it.
    pub fn fraction(self) -> f64 {
        self.parts as f64 / PARTS as f64
    }
}
