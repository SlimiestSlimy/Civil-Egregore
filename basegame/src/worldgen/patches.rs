//! How something lies in patches when a superchunk is made -- grass,
//! trees: a number a cell from noise and its own lot, under a
//! threshold found for the share asked (`docs/worldgen.md`, "Patches").

use crate::worldgen::{noise_at_cell_cartesian, SmoothNoise};
use crate::worldgen::lots::{mix_word, GOLDEN_RATIO};

use crate::worldgen::FRACTION_ONE;

/// Cells looked at to find a threshold.
const CELLS_LOOKED_AT: u64 = 1 << 14;

/// How something lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Patches {
    /// The share of the cells that have it, of [`ONE`].
    pub cover_share: u64,
    /// The cells across a patch, as a power of two.
    pub patch_size_power: u32,
    /// How much the finer noise counts beside the patches', of [`ONE`].
    pub detail_share: u64,
    /// How much the cell's own lot counts, of [`ONE`]: scattered, not
    /// in patches.
    pub scatter_share: u64,
}

impl Patches {
    /// The number of the cell at `(x, y)`, of [`ONE`], in the world of
    /// `seed` -- one mixed with what lies, so that two things lie apart.
    pub fn cell_number_cartesian(&self, seed: u64, x: u32, y: u32) -> u64 {
        let (broad_noise, fine_noise) = (noise_at_cell_cartesian(seed, 0, self.patch_size_power, x, y), noise_at_cell_cartesian(seed, 1, self.patch_size_power.saturating_sub(2), x, y));
        self.weighed_cartesian(seed, broad_noise, fine_noise, x, y) / self.weights()
    }

    /// The patches' noise and the finer, of the world of `seed`: what
    /// [`Patches::weighed_cartesian`] is given, kept cell after cell.
    pub(crate) fn patch_noises(&self, seed: u64) -> [SmoothNoise; 2] {
        [SmoothNoise::new(seed, 0, self.patch_size_power), SmoothNoise::new(seed, 1, self.patch_size_power.saturating_sub(2))]
    }

    /// The cell's number times [`Patches::weights`]: the patches'
    /// noise there, the finer and the cell's own lot, each by how much
    /// it counts.
    pub(crate) fn weighed_cartesian(&self, seed: u64, broad_noise: u64, fine_noise: u64, x: u32, y: u32) -> u64 {
        let cell_lot = mix_word(seed ^ ((x as u64) << 32 | y as u64).wrapping_mul(GOLDEN_RATIO)) >> 48;
        broad_noise * FRACTION_ONE + fine_noise * self.detail_share + cell_lot * self.scatter_share
    }

    /// What the three count in all.
    pub(crate) fn weights(&self) -> u64 {
        FRACTION_ONE + self.detail_share + self.scatter_share
    }

    /// The threshold under which [`Patches::cover_share`] of the cells'
    /// numbers are: found from those of [`CELLS_LOOKED_AT`] cells drawn over
    /// the world.
    pub fn cover_threshold(&self, seed: u64) -> u64 {
        let mut cell_numbers: Vec<u64> = (0..CELLS_LOOKED_AT).map(|sample| mix_word(sample.wrapping_mul(GOLDEN_RATIO))).map(|drawn_bits| self.cell_number_cartesian(seed, (drawn_bits >> 32) as u32, drawn_bits as u32)).collect();
        cell_numbers.sort_unstable();
        match (self.cover_share.min(FRACTION_ONE) * CELLS_LOOKED_AT / FRACTION_ONE) as usize {
            0 => 0,
            under_index if under_index >= cell_numbers.len() => FRACTION_ONE,
            under_index => cell_numbers[under_index],
        }
    }
}
