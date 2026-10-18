//! A wide plane between hot and cold: hot, a cell's number is `bits`
//! bits together; cold, `bits` bitmaps, a bit of every cell each
//! (`docs/chunk_storage.md`, "Wide planes").

use crate::chunk_storage::LayerType;
use utilities::bitmap::{CellWords, BITS_PER_WORD, WORDS};

/// Words the hot bucket of a chunk takes, of `layer_type`: a
/// bitmap's, times its bits, over the cells of one of its tiles -- a
/// number a tile, however many cells a tile is.
pub const fn bucket_words(layer_type: LayerType) -> usize {
    (WORDS * layer_type.width_bits() as usize) >> layer_type.tile_shift()
}

/// Puts `one_bit_bitmap`, the bit `bit_of_cell` of every cell, into `wide_bucket`, of `bits_a_cell`
/// bits a cell: where `one_bit_bitmap` has a cell set, that bit of the cell.
/// Of a plane of tiles, `wide_bucket` is as much shorter and the tiles are
/// `one_bit_bitmap`'s first cells, in Morton order: no more of it is read.
pub fn spread_into_wide(one_bit_bitmap: &CellWords, bits_a_cell: u32, bit_of_cell: u32, wide_bucket: &mut [u64]) {
    debug_assert!(wide_bucket.len() <= WORDS * bits_a_cell as usize && wide_bucket.len().is_multiple_of(bits_a_cell as usize), "a wide bucket");
    for (word_index, &word) in one_bit_bitmap[..wide_bucket.len() / bits_a_cell as usize].iter().enumerate() {
        let mut bits_left = word;
        while bits_left != 0 {
            let bit_in_wide = (word_index * BITS_PER_WORD + bits_left.trailing_zeros() as usize) * bits_a_cell as usize + bit_of_cell as usize;
            wide_bucket[bit_in_wide / BITS_PER_WORD] |= 1 << (bit_in_wide % BITS_PER_WORD);
            bits_left &= bits_left - 1;
        }
    }
}

/// The bit `bit_of_cell` of every cell of `wide_bucket`, of `bits_a_cell` bits a cell, as a
/// bitmap: [`spread_into_wide`] undone. Of a plane of tiles, the tiles are the
/// bitmap's first cells and the rest of it is clear.
pub fn bitmap_of_bit(wide_bucket: &[u64], bits_a_cell: u32, bit_of_cell: u32) -> Box<CellWords> {
    debug_assert!(wide_bucket.len() <= WORDS * bits_a_cell as usize, "a wide bucket");
    let mut one_bit_bitmap = Box::new([0; WORDS]);
    // The lowest bit of every cell of a word.
    let lowest_bits = u64::MAX / ((1u64 << bits_a_cell) - 1).max(1);
    for (word_index, &word) in wide_bucket.iter().enumerate() {
        let mut bits_left = word >> bit_of_cell & lowest_bits;
        while bits_left != 0 {
            let cell = (word_index * BITS_PER_WORD + bits_left.trailing_zeros() as usize) / bits_a_cell as usize;
            one_bit_bitmap[cell / BITS_PER_WORD] |= 1 << (cell % BITS_PER_WORD);
            bits_left &= bits_left - 1;
        }
    }
    one_bit_bitmap
}

/// How many cells of `words`, of `bits_a_cell` bits a cell, hold a number that
/// is not 0.
pub fn cells_set(words: &[u64], bits_a_cell: u32) -> u32 {
    let lowest_bits = u64::MAX / ((1u64 << bits_a_cell) - 1).max(1);
    words.iter().map(|&word| ((0..bits_a_cell).fold(0, |any_bit_so_far, bit_of_cell| any_bit_so_far | word >> bit_of_cell) & lowest_bits).count_ones()).sum()
}
