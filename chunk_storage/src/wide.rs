//! A wide plane between hot and cold: hot, a cell's number is `bits`
//! bits together; cold, `bits` bitmaps, a bit of every cell each
//! (`docs/chunk_storage.md`, "Wide planes").

use bitmap::{CellWords, BITS_PER_WORD, WORDS};

/// Puts `plane`, the bit `bit` of every cell, into `wide`, of `bits`
/// bits a cell: where `plane` has a cell set, that bit of the cell.
pub fn spread(plane: &CellWords, bits: u32, bit: u32, wide: &mut [u64]) {
    debug_assert_eq!(wide.len(), WORDS * bits as usize, "a wide bucket");
    for (index, &word) in plane.iter().enumerate() {
        let mut left = word;
        while left != 0 {
            let at = (index * BITS_PER_WORD + left.trailing_zeros() as usize) * bits as usize + bit as usize;
            wide[at / BITS_PER_WORD] |= 1 << (at % BITS_PER_WORD);
            left &= left - 1;
        }
    }
}

/// The bit `bit` of every cell of `wide`, of `bits` bits a cell, as a
/// bitmap: [`spread`] undone.
pub fn plane(wide: &[u64], bits: u32, bit: u32) -> Box<CellWords> {
    debug_assert_eq!(wide.len(), WORDS * bits as usize, "a wide bucket");
    let mut plane = Box::new([0; WORDS]);
    // The lowest bit of every cell of a word.
    let lowest = u64::MAX / ((1u64 << bits) - 1).max(1);
    for (index, &word) in wide.iter().enumerate() {
        let mut left = word >> bit & lowest;
        while left != 0 {
            let cell = (index * BITS_PER_WORD + left.trailing_zeros() as usize) / bits as usize;
            plane[cell / BITS_PER_WORD] |= 1 << (cell % BITS_PER_WORD);
            left &= left - 1;
        }
    }
    plane
}

/// How many cells of `words`, of `bits` bits a cell, hold a number that
/// is not 0.
pub fn cells_set(words: &[u64], bits: u32) -> u32 {
    let lowest = u64::MAX / ((1u64 << bits) - 1).max(1);
    words.iter().map(|&word| ((0..bits).fold(0, |any, bit| any | word >> bit) & lowest).count_ones()).sum()
}
