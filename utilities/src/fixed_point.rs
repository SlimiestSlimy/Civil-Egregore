//! Fixed-point numbers: a fraction kept in an integer's low bits, so
//! that what is worked out with them is whole-number arithmetic -- the
//! same to the bit on every machine, where a float's logarithm is
//! whatever the machine's maths library makes of it. How it is worked
//! out, and why so: `docs/utilities.md`, "Fixed point".

/// The bits of fraction [`log2_by_table`] gives: how precise a logarithm is is
/// this number, chosen here, and nothing a machine decides.
pub const LOG2_FRACTION_BITS: u32 = 48;

/// The bits of a number, after its highest, that pick its row of the
/// tables.
const TABLE_BITS: u32 = 7;
/// Rows in the tables.
const TABLE_ROWS: usize = 1 << TABLE_BITS;
/// The bits of fraction the tables' logarithms are kept in: more than
/// are given, so that rounding to those loses nothing.
const TABLE_FRACTION_BITS: u32 = 56;
/// Where 1 is in the series' numbers: bit 62, a sign above it.
const SERIES_ONE_BITS: u32 = 62;
/// 1 over the natural logarithm of 2, 1 at bit [`SERIES_ONE_BITS`]:
/// what turns a natural logarithm into one to base 2.
const ONE_OVER_LN_2: i128 = 6_653_256_548_922_161_245;

/// The logarithm to base 2 of `value`, times 2^`fraction_bits`, by
/// squaring, a bit of fraction a squaring: slow, what [`log2_by_table`]'s tables
/// are made with and what it is tested against (`docs/utilities.md`,
/// "Fixed point").
///
/// # Panics
/// If `value` is 0, or the whole part and `fraction_bits` do not fit
/// in 64 bits together.
pub const fn log2_by_squaring(value: u64, fraction_bits: u32) -> u64 {
    assert!(value != 0, "0 has no logarithm");
    assert!(fraction_bits <= 58, "six bits of whole part and the fraction, in 64");
    let whole_part = value.ilog2();
    // `value` over two to the `whole_part`, 1 at bit 63: in [1, 2).
    let mut mantissa_bits = value << (u64::BITS - 1 - whole_part);
    let mut log_bits = (whole_part as u64) << fraction_bits;
    let mut fraction_bit = fraction_bits;
    // A power of two has no fraction: nothing more to find.
    while fraction_bit > 0 && mantissa_bits != 1 << (u64::BITS - 1) {
        fraction_bit -= 1;
        // In [1, 4), 1 at bit 126.
        let squared_mantissa = (mantissa_bits as u128) * (mantissa_bits as u128);
        if squared_mantissa >> 127 != 0 {
            log_bits |= 1 << fraction_bit;
            mantissa_bits = (squared_mantissa >> 64) as u64;
        } else {
            mantissa_bits = (squared_mantissa >> 63) as u64;
        }
    }
    log_bits
}

/// The middle of row `table_row`'s stretch of `[1, 2)`, 1 at bit 63.
const fn row_middle(table_row: usize) -> u64 {
    (1 << (u64::BITS - 1)) + ((2 * table_row as u64 + 1) << (u64::BITS - 2 - TABLE_BITS))
}

/// Each row's middle's logarithm to base 2 -- in `[0, 1)` -- times
/// 2^[`TABLE_FRACTION_BITS`].
const MIDDLE_LOG2: [u64; TABLE_ROWS] = {
    let mut middle_logs = [0; TABLE_ROWS];
    let mut table_row = 0;
    while table_row < TABLE_ROWS {
        middle_logs[table_row] = log2_by_squaring(row_middle(table_row), TABLE_FRACTION_BITS) - ((u64::BITS as u64 - 1) << TABLE_FRACTION_BITS);
        table_row += 1;
    }
    middle_logs
};

/// 1 over each row's middle, 1 at bit 63: in `(1/2, 1]`.
const ONE_OVER_MIDDLE: [u64; TABLE_ROWS] = {
    let mut middle_inverses = [0; TABLE_ROWS];
    let mut table_row = 0;
    while table_row < TABLE_ROWS {
        middle_inverses[table_row] = ((1u128 << 126) / row_middle(table_row) as u128) as u64;
        table_row += 1;
    }
    middle_inverses
};

/// `left_factor` times `right_factor`, both with 1 at bit [`SERIES_ONE_BITS`].
const fn fixed_times(left_factor: i128, right_factor: i128) -> i128 {
    (left_factor * right_factor) >> SERIES_ONE_BITS
}

/// The logarithm to base 2 of `value`, times 2^[`LOG2_FRACTION_BITS`],
/// by table and series: whole numbers all, the same on every machine,
/// a power of two's exact (`docs/utilities.md`, "Fixed point").
///
/// # Panics
/// If `value` is 0, which has no logarithm.
pub const fn log2_by_table(value: u64) -> u64 {
    assert!(value != 0, "0 has no logarithm");
    let whole_part = value.ilog2();
    let whole_log = (whole_part as u64) << LOG2_FRACTION_BITS;
    // `value` over two to the `whole_part`, 1 at bit 63: in [1, 2).
    let mantissa_bits = value << (u64::BITS - 1 - whole_part);
    if mantissa_bits == 1 << (u64::BITS - 1) {
        return whole_log;
    }
    let table_row = ((mantissa_bits >> (u64::BITS - 1 - TABLE_BITS)) as usize) & (TABLE_ROWS - 1);
    // The mantissa over its row's middle, less 1: within a part in 256 of 0.
    let offset_from_middle = (((mantissa_bits as u128 * ONE_OVER_MIDDLE[table_row] as u128) >> 64) as i64 - (1 << SERIES_ONE_BITS)) as i128;
    // The natural logarithm of 1 + off: off - off^2/2 + off^3/3 - ... to off^6/6, the next term under a part in 2^58.
    const SERIES_ONE: i128 = 1 << SERIES_ONE_BITS;
    let mut series_sum = SERIES_ONE / 6;
    series_sum = SERIES_ONE / 5 - fixed_times(offset_from_middle, series_sum);
    series_sum = SERIES_ONE / 4 - fixed_times(offset_from_middle, series_sum);
    series_sum = SERIES_ONE / 3 - fixed_times(offset_from_middle, series_sum);
    series_sum = SERIES_ONE / 2 - fixed_times(offset_from_middle, series_sum);
    series_sum = SERIES_ONE - fixed_times(offset_from_middle, series_sum);
    let off_log2 = fixed_times(fixed_times(offset_from_middle, series_sum), ONE_OVER_LN_2) >> (SERIES_ONE_BITS - TABLE_FRACTION_BITS);
    let fraction_part = (MIDDLE_LOG2[table_row] as i128 + off_log2) >> (TABLE_FRACTION_BITS - LOG2_FRACTION_BITS);
    // Kept a fraction, whatever the last bit's rounding: never under 0, never a whole.
    let most_fraction = (1 << LOG2_FRACTION_BITS) - 1;
    whole_log + if fraction_part < 0 { 0 } else if fraction_part > most_fraction { most_fraction as u64 } else { fraction_part as u64 }
}
