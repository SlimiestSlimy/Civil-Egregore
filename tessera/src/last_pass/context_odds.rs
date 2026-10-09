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
    const POINT: u32 = 30;
    let whole = value.ilog2();
    let top = (value << (u64::BITS - 1 - whole)) >> (u64::BITS - 1 - FRACTION_BITS);
    let mut mantissa = top << (POINT - FRACTION_BITS);
    let mut fraction = 0;
    let mut bit = 0;
    while bit < FRACTION_BITS {
        mantissa = (mantissa * mantissa) >> POINT;
        fraction <<= 1;
        if mantissa >= 2 << POINT {
            mantissa >>= 1;
            fraction |= 1;
        }
        bit += 1;
    }
    whole << FRACTION_BITS | fraction
}

/// `2^32` over every total a context's weights can add up to, and
/// `log2` of every weight and total in fixed point: a probability is a
/// lookup and a multiply, a price two lookups.
static RECIPROCALS_AND_LOG2S: ([u32; MOST_WEIGHT_TOTAL + 1], [u16; MOST_WEIGHT_TOTAL + 1]) = {
    let (mut reciprocals, mut log2s) = ([0; MOST_WEIGHT_TOTAL + 1], [0; MOST_WEIGHT_TOTAL + 1]);
    let mut total = 1;
    while total <= MOST_WEIGHT_TOTAL {
        reciprocals[total] = ((1u64 << u32::BITS) / total as u64) as u32;
        log2s[total] = fixed_point_log2(total as u64) as u16;
        total += 1;
    }
    (reciprocals, log2s)
};

/// A context's odds: its weights for clear and for set, by the value.
#[derive(Clone, Copy)]
pub struct ContextOdds([u16; 2]);

impl ContextOdds {
    /// No cell coded in it: a half each.
    pub(crate) const UNSEEN: Self = Self([UNSEEN_WEIGHT; 2]);

    /// Its two weights added up.
    #[inline]
    fn total(self) -> usize {
        (self.0[0] + self.0[1]) as usize
    }

    /// The probability a cell in it is clear: clear's share of `2^32`.
    #[inline]
    pub(crate) fn clear_probability(self) -> ClearProbability {
        ClearProbability(self.0[0] as u32 * RECIPROCALS_AND_LOG2S.0[self.total()])
    }

    /// What a cell holding `value` costs in it, in fixed point.
    #[inline]
    pub(crate) fn cost(self, value: bool) -> u32 {
        let log2s = &RECIPROCALS_AND_LOG2S.1;
        (log2s[self.total()] - log2s[self.0[value as usize] as usize]) as u32
    }

    /// A cell holding `value` coded in it: its weight grows, and both
    /// are halved -- counts rounded up -- if it reaches halving.
    #[inline]
    pub(crate) fn learn(&mut self, value: bool) {
        self.0[value as usize] += CELL_WEIGHT;
        if self.0[value as usize] == HALVING_WEIGHT {
            self.0 = self.0.map(|weight| UNSEEN_WEIGHT + CELL_WEIGHT * ((weight - UNSEEN_WEIGHT) / CELL_WEIGHT).div_ceil(2));
        }
    }
}
