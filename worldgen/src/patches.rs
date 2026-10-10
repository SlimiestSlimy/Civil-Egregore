//! How something lies in patches when a superchunk is made -- grass,
//! trees: a number a cell from noise and its own lot, under a
//! threshold found for the share asked (`docs/worldgen.md`, "Patches").

use crate::noise;
use utilities::hash::{mix, GOLDEN_RATIO};

use crate::ONE;

/// Cells looked at to find a threshold.
const SAMPLED: u64 = 1 << 14;

/// How something lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Patches {
    /// The share of the cells that have it, of [`ONE`].
    pub cover: u64,
    /// The cells across a patch, as a power of two.
    pub patch: u32,
    /// How much the finer noise counts beside the patches', of [`ONE`].
    pub detail: u64,
    /// How much the cell's own lot counts, of [`ONE`]: scattered, not
    /// in patches.
    pub scatter: u64,
}

impl Patches {
    /// The number of the cell at `(x, y)`, of [`ONE`], in the world of
    /// `seed` -- one mixed with what lies, so that two things lie apart.
    pub fn number(&self, seed: u64, x: u32, y: u32) -> u64 {
        let (broad, fine) = (noise(seed, 0, self.patch, x, y), noise(seed, 1, self.patch.saturating_sub(2), x, y));
        let lot = mix(seed ^ ((x as u64) << 32 | y as u64).wrapping_mul(GOLDEN_RATIO)) >> 48;
        (broad * ONE + fine * self.detail + lot * self.scatter) / (ONE + self.detail + self.scatter)
    }

    /// The threshold under which [`Patches::cover`] of the cells'
    /// numbers are: found from those of [`SAMPLED`] cells drawn over
    /// the world.
    pub fn threshold(&self, seed: u64) -> u64 {
        let mut numbers: Vec<u64> = (0..SAMPLED).map(|sample| mix(sample.wrapping_mul(GOLDEN_RATIO))).map(|drawn| self.number(seed, (drawn >> 32) as u32, drawn as u32)).collect();
        numbers.sort_unstable();
        match (self.cover.min(ONE) * SAMPLED / ONE) as usize {
            0 => 0,
            under if under >= numbers.len() => ONE,
            under => numbers[under],
        }
    }
}
