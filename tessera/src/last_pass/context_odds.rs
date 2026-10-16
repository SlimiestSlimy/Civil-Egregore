//! A context's odds: how often its cells were clear and set, the
//! probability coded with, and what a cell costs.

use crate::arithmetic::ClearProbability;

/// A context's weight for clear and for set before any cell: a half, in
/// units of half a cell...
const UNSEEN_WEIGHT: u16 = 1;
/// ...and what each cell coded in it adds to its value's: one cell.
const CELL_WEIGHT: u16 = 2;
/// The cells either value of a context counts at most: reaching it,
/// both are halved. Residual cells are much the same all over a bitmap,
/// so halving forgets what costs bits, the more the sooner; this is
/// where it stops costing any the corpus shows.
const HALVING_COUNT: u16 = 512;
/// The weight that, reached, halves both.
const HALVING_WEIGHT: u16 = UNSEEN_WEIGHT + CELL_WEIGHT * HALVING_COUNT;
/// The most a context's two weights add up to when a cell is coded at
/// them: both just under halving.
const MOST_WEIGHT_TOTAL: usize = 2 * (HALVING_WEIGHT - CELL_WEIGHT) as usize;

/// Bits of a fixed-point `log2` below the point: a 256th of a bit.
pub(crate) const FRACTION_BITS: u32 = 8;

/// `log2(value)` in [`FRACTION_BITS`] fixed point, `value` at least 1:
/// its whole part, and its fraction from the mantissa's top
/// [`FRACTION_BITS`] bits under its leading one, squared a fraction bit
/// at a time.
pub(crate) const fn fixed_point_log2(value: u64) -> u32 {
    // The mantissa in 2.30 fixed point.
    const MANTISSA_POINT_BITS: u32 = 30;
    let whole_part = value.ilog2();
    let top_bits = (value << (u64::BITS - 1 - whole_part)) >> (u64::BITS - 1 - FRACTION_BITS);
    let mut squared_mantissa = top_bits << (MANTISSA_POINT_BITS - FRACTION_BITS);
    let mut fraction_part = 0;
    let mut fraction_bit_index = 0;
    while fraction_bit_index < FRACTION_BITS {
        squared_mantissa = (squared_mantissa * squared_mantissa) >> MANTISSA_POINT_BITS;
        fraction_part <<= 1;
        if squared_mantissa >= 2 << MANTISSA_POINT_BITS {
            squared_mantissa >>= 1;
            fraction_part |= 1;
        }
        fraction_bit_index += 1;
    }
    whole_part << FRACTION_BITS | fraction_part
}

/// `2^32` over every total a context's weights can add up to, and
/// `log2` of every weight and total in fixed point: a probability is a
/// lookup and a multiply, a price two lookups.
static RECIPROCALS_AND_LOG2S: ([u32; MOST_WEIGHT_TOTAL + 1], [u16; MOST_WEIGHT_TOTAL + 1]) = {
    let (mut total_reciprocals, mut weight_log2s) = ([0; MOST_WEIGHT_TOTAL + 1], [0; MOST_WEIGHT_TOTAL + 1]);
    let mut weight_total = 1;
    while weight_total <= MOST_WEIGHT_TOTAL {
        total_reciprocals[weight_total] = ((1u64 << u32::BITS) / weight_total as u64) as u32;
        weight_log2s[weight_total] = fixed_point_log2(weight_total as u64) as u16;
        weight_total += 1;
    }
    (total_reciprocals, weight_log2s)
};

/// A context's odds: its weights for clear and for set, by the value.
#[derive(Clone, Copy)]
pub struct ContextOdds([u16; 2]);

impl ContextOdds {
    /// No cell coded in it: a half each.
    pub(crate) const UNSEEN_CONTEXT: Self = Self([UNSEEN_WEIGHT; 2]);

    /// Its two weights added up.
    #[inline]
    fn weight_total(self) -> usize {
        (self.0[0] + self.0[1]) as usize
    }

    /// The probability a cell in it is clear: clear's share of `2^32`.
    #[inline]
    pub(crate) fn clear_probability(self) -> ClearProbability {
        ClearProbability(self.0[0] as u32 * RECIPROCALS_AND_LOG2S.0[self.weight_total()])
    }

    /// What a cell holding `value` costs in it, in fixed point.
    #[inline]
    pub(crate) fn cell_cost(self, value: bool) -> u32 {
        let weight_log2s = &RECIPROCALS_AND_LOG2S.1;
        (weight_log2s[self.weight_total()] - weight_log2s[self.0[value as usize] as usize]) as u32
    }

    /// A cell holding `value` coded in it: its weight grows, and both
    /// are halved -- counts rounded up -- if it reaches halving.
    #[inline]
    pub(crate) fn learn_cell(&mut self, value: bool) {
        self.0[value as usize] += CELL_WEIGHT;
        if self.0[value as usize] == HALVING_WEIGHT {
            self.0 = self.0.map(|weight| UNSEEN_WEIGHT + CELL_WEIGHT * ((weight - UNSEEN_WEIGHT) / CELL_WEIGHT).div_ceil(2));
        }
    }
}
