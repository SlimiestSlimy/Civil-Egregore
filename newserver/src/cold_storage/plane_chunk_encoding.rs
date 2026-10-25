//! A plane chunk between hot and cold: hot, its numbers -- a cell's, or
//! a tile's -- one after another, each its plane's width in bits; cold, a
//! bitmap a bit of the width, that bit of every number, in order under the
//! plane's one ID ([`encode_plane_chunk`], [`decode_plane_chunk`]).

use crate::cold_storage::bitmap_codec::BitmapCodec;
use protocol::type_registry::PlaneType;
use utilities::bitmap::{CellWords, BITS_PER_WORD, WORDS};

/// A plane chunk of `plane_type` as cold storage keeps it: `hot_words`, its
/// numbers as a hot plane keeps them ([`PlaneType::chunk_words`] words),
/// encoded by `bitmap_codec` -- a plane of bitmaps, its bitmap encoded; any
/// other, the bitmaps of its numbers' bits in order ([`encode_bits`]).
/// Empty if no number is set.
pub fn encode_plane_chunk(plane_type: PlaneType, hot_words: &[u64], bitmap_codec: &mut BitmapCodec) -> Vec<u64> {
    if plane_type.holds_bitmaps() {
        return bitmap_codec.encode_unless_empty(hot_words.try_into().expect("a bitmap's words")).to_vec();
    }
    let bit_bitmaps: Vec<Box<CellWords>> = (0..plane_type.width().bits()).map(|bit_of_number| bitmap_of_bit(plane_type, hot_words, bit_of_number)).collect();
    encode_bits(bit_bitmaps.iter().map(|bit_bitmap| Some(&**bit_bitmap)), bitmap_codec)
}

/// A plane chunk as cold storage keeps it, of a plane whose bits' bitmaps
/// are `bit_bitmaps`, the lowest bit's first -- none for a bit no number
/// has set -- encoded by `bitmap_codec`: each one's words after a word
/// saying how many, 0 for one with nothing set. Empty if no number has
/// any bit set.
pub fn encode_bits<'a>(bit_bitmaps: impl Iterator<Item = Option<&'a CellWords>>, bitmap_codec: &mut BitmapCodec) -> Vec<u64> {
    let (mut cold_words, mut any_set) = (Vec::new(), false);
    for bit_bitmap in bit_bitmaps {
        let encoded = bit_bitmap.map_or(&[][..], |bit_bitmap| bitmap_codec.encode_unless_empty(bit_bitmap));
        any_set |= !encoded.is_empty();
        cold_words.push(encoded.len() as u64);
        cold_words.extend_from_slice(encoded);
    }
    if any_set { cold_words } else { Vec::new() }
}

/// Decodes `cold_words`, a plane chunk of `plane_type` as cold storage keeps
/// it ([`encode_plane_chunk`]), with `bitmap_codec` into `hot_words`, its
/// numbers as a hot plane keeps them: every number of it.
pub fn decode_plane_chunk(plane_type: PlaneType, cold_words: &[u64], bitmap_codec: &mut BitmapCodec, hot_words: &mut [u64]) {
    if plane_type.holds_bitmaps() {
        bitmap_codec.decode_cells(cold_words, hot_words.try_into().expect("a bitmap's words"));
        return;
    }
    hot_words.fill(0);
    let (mut one_bit_bitmap, mut read_at) = ([0; WORDS], 0);
    for bit_of_number in 0..plane_type.width().bits() {
        let bit_words = *cold_words.get(read_at).expect("a plane chunk as cold storage keeps it") as usize;
        read_at += 1;
        if bit_words > 0 {
            bitmap_codec.decode_cells(&cold_words[read_at..read_at + bit_words], &mut one_bit_bitmap);
            spread_into_numbers(plane_type, &one_bit_bitmap, bit_of_number, hot_words);
        }
        read_at += bit_words;
    }
}

/// Puts `one_bit_bitmap`, the bit `bit_of_number` of every number, into
/// `hot_words`, a chunk of `plane_type`'s numbers: where `one_bit_bitmap`
/// has a cell set, that bit of the number. The numbers are
/// `one_bit_bitmap`'s first cells, in Morton order: no more of it is read.
pub fn spread_into_numbers(plane_type: PlaneType, one_bit_bitmap: &CellWords, bit_of_number: u32, hot_words: &mut [u64]) {
    debug_assert_eq!(hot_words.len(), plane_type.chunk_words(), "a chunk of the plane's numbers");
    let width = plane_type.width().bits() as usize;
    for (word_index, &word) in one_bit_bitmap[..plane_type.numbers_in_chunk().div_ceil(BITS_PER_WORD)].iter().enumerate() {
        let mut bits_left = word;
        while bits_left != 0 {
            let bit_in_numbers = (word_index * BITS_PER_WORD + bits_left.trailing_zeros() as usize) * width + bit_of_number as usize;
            hot_words[bit_in_numbers / BITS_PER_WORD] |= 1 << (bit_in_numbers % BITS_PER_WORD);
            bits_left &= bits_left - 1;
        }
    }
}

/// The bit `bit_of_number` of every number of `hot_words`, a chunk of
/// `plane_type`'s numbers, as a bitmap: [`spread_into_numbers`] undone. The
/// numbers are the bitmap's first cells, and the rest of it is clear.
pub fn bitmap_of_bit(plane_type: PlaneType, hot_words: &[u64], bit_of_number: u32) -> Box<CellWords> {
    debug_assert_eq!(hot_words.len(), plane_type.chunk_words(), "a chunk of the plane's numbers");
    let width = plane_type.width().bits();
    let mut one_bit_bitmap = Box::new([0; WORDS]);
    for (word_index, &word) in hot_words.iter().enumerate() {
        let mut bits_left = word >> bit_of_number & lowest_bits(width);
        while bits_left != 0 {
            let number = (word_index * BITS_PER_WORD + bits_left.trailing_zeros() as usize) / width as usize;
            one_bit_bitmap[number / BITS_PER_WORD] |= 1 << (number % BITS_PER_WORD);
            bits_left &= bits_left - 1;
        }
    }
    one_bit_bitmap
}

/// How many numbers of `words`, numbers of `plane_type` one after
/// another, are not 0.
pub fn numbers_set(plane_type: PlaneType, words: &[u64]) -> u32 {
    let width = plane_type.width().bits();
    words.iter().map(|&word| ((0..width).fold(0, |any_bit_so_far, bit_of_number| any_bit_so_far | word >> bit_of_number) & lowest_bits(width)).count_ones()).sum()
}

/// The lowest bit of every number of a word, numbers `width` bits each.
const fn lowest_bits(width: u32) -> u64 {
    u64::MAX / (u64::MAX >> (u64::BITS - width))
}
