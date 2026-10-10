//! What a rule did on a turn ([`RuleCounts`]): a few numbers, the same
//! shape for every rule, so that all of them go in one table and their
//! counts in one array. A rule names its counts -- a constant each,
//! the count's place -- and lists the names in the same order.

use std::ops::{AddAssign, Index, IndexMut};

/// The most counts a rule keeps.
pub const COUNTS_OF_A_RULE: usize = 8;

/// What a rule did, on a turn or added up over many: its counts, each
/// at the place the rule names it by, the rest 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleCounts(pub [u64; COUNTS_OF_A_RULE]);

impl Index<usize> for RuleCounts {
    type Output = u64;

    /// The count at `place`.
    #[inline(always)]
    fn index(&self, place: usize) -> &u64 {
        &self.0[place]
    }
}

impl IndexMut<usize> for RuleCounts {
    /// The count at `place`, to add to.
    #[inline(always)]
    fn index_mut(&mut self, place: usize) -> &mut u64 {
        &mut self.0[place]
    }
}

impl AddAssign for RuleCounts {
    /// Each count added to its like.
    #[inline]
    fn add_assign(&mut self, other: Self) {
        for (count, more) in self.0.iter_mut().zip(other.0) {
            *count += more;
        }
    }
}
