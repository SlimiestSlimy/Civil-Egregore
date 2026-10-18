//! What generation draws its lots by: a word's bits mixed, every bit of
//! it moving every bit of the result -- SplitMix64's finalizer -- the
//! same on every machine.

/// 2^64 over the golden ratio, odd: multiplying by it spreads a word's
/// low bits into the high ones.
pub const GOLDEN_RATIO: u64 = 0x9E37_79B9_7F4A_7C15;
/// SplitMix64's first mixing multiplier...
const MIX_1: u64 = 0xBF58_476D_1CE4_E5B9;
/// ...and its second.
const MIX_2: u64 = 0x94D0_49BB_1331_11EB;

/// `word`'s bits mixed: SplitMix64's finalizer.
#[inline(always)]
pub const fn mix_word(word: u64) -> u64 {
    let first_mix = (word ^ (word >> 30)).wrapping_mul(MIX_1);
    let second_mix = (first_mix ^ (first_mix >> 27)).wrapping_mul(MIX_2);
    second_mix ^ (second_mix >> 31)
}
