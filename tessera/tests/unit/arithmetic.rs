//! The range coder: what it takes, and that it reads back what it wrote
//! at the most skewed odds and through carries.

use crate::arithmetic::{ClearProbability, RangeDecoder, RangeEncoder, FINISHING_BITS};
use crate::bit_stream::{BitStream, BitSink};

/// Codes `coded_bits`, each at the probability `clear_probability_of` gives it, and reads
/// them back with set bits after the stream -- a stream ends itself:
/// how many bits the stream took.
fn round_trip(coded_bits: &[bool], clear_probability_of: impl Fn(usize) -> ClearProbability) -> usize {
    let mut stream = BitStream::default();
    let mut range_encoder = RangeEncoder::default();
    for (bit_index, &coded_bit) in coded_bits.iter().enumerate() {
        range_encoder.encode_bit(coded_bit, clear_probability_of(bit_index), &mut stream);
    }
    range_encoder.finish_stream(&mut stream);
    let written_bits = stream.len();
    stream.push_value(u64::MAX, u64::BITS as u8);
    let mut stream_reader = stream.bit_reader();
    let mut range_decoder = RangeDecoder::new(&mut stream_reader);
    for (bit_index, &coded_bit) in coded_bits.iter().enumerate() {
        assert_eq!(range_decoder.decode_bit(clear_probability_of(bit_index), &mut stream_reader), coded_bit, "bit {bit_index}");
    }
    written_bits
}

/// A 2^32 fraction of `clear_count` in `total_count`.
fn clear_fraction(clear_count: u64, total_count: u64) -> ClearProbability {
    ClearProbability(((clear_count << u32::BITS) / total_count) as u32)
}

/// The stream takes what the bits' probabilities say they carry --
/// `-log2` of each's -- and no more than [`FINISHING_BITS`] and the
/// rounding over it.
#[test]
fn the_stream_takes_the_bits_information_and_little_more() {
    let coded_bits: Vec<bool> = (0..4000).map(|bit_index| bit_index % 97 == 0).collect();
    let clear_probability = clear_fraction(96, 97);
    let written_bits = round_trip(&coded_bits, |_| clear_probability);
    let clear_share = clear_probability.0 as f64 / (1u64 << u32::BITS) as f64;
    let information_bits: f64 = coded_bits.iter().map(|&coded_bit| -(if coded_bit { 1.0 - clear_share } else { clear_share }).log2()).sum();
    // The rounding: under 2^-12 bits a bit coded.
    let most_bits = information_bits + FINISHING_BITS as f64 + coded_bits.len() as f64 / 4096.0;
    assert!((written_bits as f64) <= most_bits, "{written_bits} bits, the information {information_bits:.1}");
}

#[test]
fn the_most_skewed_probabilities_round_trip() {
    let coded_bits: Vec<bool> = (0..3000).map(|bit_index| (bit_index * 7919) % 13 < 6).collect();
    let (rare_count, total_count) = (1, 2048);
    round_trip(&coded_bits, |bit_index| if bit_index % 3 == 0 { clear_fraction(total_count - rare_count, total_count) } else { clear_fraction(rare_count, total_count) });
}

#[test]
fn carries_through_held_bytes_round_trip() {
    // Long runs of the likelier value push the lower end up against
    // the window's top, holding 0xFF bytes for a carry.
    let coded_bits: Vec<bool> = (0..20000).map(|bit_index| bit_index % 1000 != 999).collect();
    round_trip(&coded_bits, |_| clear_fraction(1, 2048));
    round_trip(&coded_bits, |bit_index| clear_fraction(1 + (bit_index % 2047) as u64, 2048));
}

/// The whole interval is a hair under `2^32` wide, so ending it names
/// its lower half: a bit. The last pass, coding no cell, never ends its
/// coder at all.
#[test]
fn nothing_coded_takes_a_bit() {
    let mut stream = BitStream::default();
    RangeEncoder::default().finish_stream(&mut stream);
    assert_eq!(stream.len(), 1);
}
