//! A wide plane between hot and cold: hot, a cell's number is `bits`
//! bits together; cold, `bits` bitmaps, a bit of every cell each, in
//! order under the plane's one ID ([`encode_bucket`], [`decode_bucket`];
//! `docs/chunk_storage.md`, "Wide planes").

use crate::chunk_storage::layer_codec::LayerCodec;
use crate::chunk_storage::LayerType;
use utilities::bitmap::{CellWords, BITS_PER_WORD, WORDS};

/// The cold layer of `bucket`, a hot bucket of `layer_type`, encoded by
/// `layer_codec`: a plane a bit a cell, its bitmap encoded; any other,
/// its bits' bitmaps in order ([`encode_bits`]). Empty if no cell is set.
pub fn encode_bucket(layer_type: LayerType, bucket: &[u64], layer_codec: &mut LayerCodec) -> Vec<u64> {
    if layer_type.is_a_bitplane() {
        return layer_codec.encode_layer(bucket.try_into().expect("a bitmap's words")).to_vec();
    }
    let bits_a_cell = layer_type.width_bits();
    let bit_bitmaps: Vec<Box<CellWords>> = (0..bits_a_cell).map(|bit_of_cell| bitmap_of_bit(bucket, bits_a_cell, bit_of_cell)).collect();
    encode_bits(bit_bitmaps.iter().map(|bit_bitmap| Some(&**bit_bitmap)), layer_codec)
}

/// The cold layer of a plane whose bits' bitmaps are `bit_bitmaps`, the
/// lowest bit's first -- none for a bit no cell has set -- encoded by
/// `layer_codec`: each one's words after a word saying how many, 0 for
/// one with no cell set. Empty if no cell has any bit set.
pub fn encode_bits<'a>(bit_bitmaps: impl Iterator<Item = Option<&'a CellWords>>, layer_codec: &mut LayerCodec) -> Vec<u64> {
    let (mut layer, mut any_set) = (Vec::new(), false);
    for bit_bitmap in bit_bitmaps {
        let encoded = bit_bitmap.map_or(&[][..], |bit_bitmap| layer_codec.encode_layer(bit_bitmap));
        any_set |= !encoded.is_empty();
        layer.push(encoded.len() as u64);
        layer.extend_from_slice(encoded);
    }
    if any_set { layer } else { Vec::new() }
}

/// Decodes `layer`, the cold layer of `layer_type` ([`encode_bucket`]),
/// with `layer_codec` into `bucket`, a hot bucket of it: every cell of it.
pub fn decode_bucket(layer_type: LayerType, layer: &[u64], layer_codec: &mut LayerCodec, bucket: &mut [u64]) {
    if layer_type.is_a_bitplane() {
        layer_codec.decode_cells(layer, bucket.try_into().expect("a bitmap's words"));
        return;
    }
    bucket.fill(0);
    let (bits_a_cell, mut one_bit_bitmap, mut read_at) = (layer_type.width_bits(), [0; WORDS], 0);
    for bit_of_cell in 0..bits_a_cell {
        let bit_words = *layer.get(read_at).expect("a plane's cold layer as it was encoded") as usize;
        read_at += 1;
        if bit_words > 0 {
            layer_codec.decode_cells(&layer[read_at..read_at + bit_words], &mut one_bit_bitmap);
            spread_into_wide(&one_bit_bitmap, bits_a_cell, bit_of_cell, bucket);
        }
        read_at += bit_words;
    }
}

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
