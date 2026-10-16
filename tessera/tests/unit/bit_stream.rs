//! The bit stream: its words and its bytes load back into the same bits.

use crate::bit_stream::{BitStream, BitSink};

/// Bits a word holds.
const WORD_BITS: usize = u64::BITS as usize;
/// Bits a byte holds.
const BYTE_BITS: usize = u8::BITS as usize;

/// A stream's words, loaded into another stream, read back the same
/// bits, and 0 past them.
#[test]
fn words_load_back_into_the_same_bits() {
    let mut written_stream = BitStream::default();
    for value in 0..40u64 {
        written_stream.push_value(value * 2_654_435_761 % 1000, 10);
    }
    written_stream.push(true);
    let mut loaded_stream = BitStream::default();
    loaded_stream.push_value(u64::MAX, 64);
    loaded_stream.load_words(written_stream.words());
    assert_eq!(loaded_stream.len(), written_stream.words().len() * WORD_BITS);
    let (mut expected_reader, mut loaded_reader) = (written_stream.bit_reader(), loaded_stream.bit_reader());
    for bit_index in 0..written_stream.len() {
        assert_eq!(loaded_reader.read_bit(), expected_reader.read_bit(), "bit {bit_index}");
    }
    for _ in written_stream.len()..loaded_stream.len() {
        assert!(!loaded_reader.read_bit(), "padding reads as 0");
    }
}

/// A stream's bytes, loaded into another stream, read back the same
/// bits, whatever that stream held before -- a stream of whole words
/// and one ending partway through a byte alike.
#[test]
fn bytes_load_back_into_the_same_bits() {
    for extra_bits in [0, 1, 7, 8, 63] {
        let mut written_stream = BitStream::default();
        for value in 0..40u64 {
            written_stream.push_value(value * 2_654_435_761 % 1000, 10);
        }
        for _ in 0..extra_bits {
            written_stream.push(true);
        }
        let stream_bytes = written_stream.to_bytes();
        assert_eq!(stream_bytes.len(), written_stream.len().div_ceil(BYTE_BITS));
        let mut loaded_stream = BitStream::default();
        loaded_stream.push_value(u64::MAX, 64);
        loaded_stream.load_bytes(&stream_bytes);
        assert_eq!(loaded_stream.len(), stream_bytes.len() * BYTE_BITS);
        let (mut expected_reader, mut loaded_reader) = (written_stream.bit_reader(), loaded_stream.bit_reader());
        for bit_index in 0..written_stream.len() {
            assert_eq!(loaded_reader.read_bit(), expected_reader.read_bit(), "bit {bit_index}");
        }
        for _ in written_stream.len()..loaded_stream.len() {
            assert!(!loaded_reader.read_bit(), "padding reads as 0");
        }
    }
}
