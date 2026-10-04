//! Where grass lies when a superchunk is made: in patches, not
//! scattered cell by cell. Every cell has a number, from the world's
//! seed and where it is -- smooth noise as broad as a patch, finer
//! noise on it, and a lot drawn for the cell alone -- and is grass if
//! the number is under a threshold. Whole numbers only, as the
//! heights, so the same on any machine.
//!
//! Not yet what worlds are made with: tried out in the renderer's lab
//! first.

use terrain::noise;
use utilities::hash::{mix, GOLDEN_RATIO};

/// One: a fraction's whole, 16 bits.
pub const ONE: u64 = 1 << 16;

/// What keeps the grass's noise apart from the heights'.
const SALT: u64 = 0x6772_6173_735F_6C6F;

/// Cells looked at to find a threshold.
const SAMPLED: u64 = 1 << 14;

/// How the grass lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pasture {
    /// The cells across a patch, as a power of two.
    pub patch: u32,
    /// How much the finer noise counts beside the patches', of [`ONE`].
    pub detail: u64,
    /// How much the cell's own lot counts, of [`ONE`]: grass scattered,
    /// not in patches.
    pub scatter: u64,
    /// A cell is grass if its number is under this, of [`ONE`].
    pub threshold: u64,
}

/// The number of the cell at `(x, y)`, of [`ONE`].
pub fn number(pasture: &Pasture, seed: u64, x: u32, y: u32) -> u64 {
    let seed = seed ^ SALT;
    let (broad, fine) = (noise(seed, 0, pasture.patch, x, y), noise(seed, 1, pasture.patch.saturating_sub(2), x, y));
    let lot = mix(seed ^ ((x as u64) << 32 | y as u64).wrapping_mul(GOLDEN_RATIO)) >> 48;
    (broad * ONE + fine * pasture.detail + lot * pasture.scatter) / (ONE + pasture.detail + pasture.scatter)
}

/// Whether the cell at `(x, y)` is grass.
pub fn grows(pasture: &Pasture, seed: u64, x: u32, y: u32) -> bool {
    number(pasture, seed, x, y) < pasture.threshold
}

/// The threshold under which `cover` of [`ONE`] of the cells are grass:
/// found from the numbers of [`SAMPLED`] cells drawn over the world.
pub fn threshold_for(pasture: &Pasture, seed: u64, cover: u64) -> u64 {
    let mut numbers: Vec<u64> = (0..SAMPLED).map(|sample| mix(sample.wrapping_mul(GOLDEN_RATIO))).map(|drawn| number(pasture, seed, (drawn >> 32) as u32, drawn as u32)).collect();
    numbers.sort_unstable();
    match (cover.min(ONE) * SAMPLED / ONE) as usize {
        0 => 0,
        under if under >= numbers.len() => ONE,
        under => numbers[under],
    }
}
