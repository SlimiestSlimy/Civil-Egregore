//! A range coder of single bits, each at the probability given:
//! `docs/tessera.md`, "The last pass", the coder.
//!
//! Function by function: `docs/reference.md`, "`arithmetic.rs`".

use crate::bit_stream::{BitReader, BitStream, BitSink};

/// The width the interval is kept at or over: a byte under the window's.
const LEAST_INTERVAL_WIDTH: u32 = 1 << 24;
/// Bits a byte takes, in the stream.
const BYTE_BITS: u8 = u8::BITS as u8;
/// The bits of the window the interval's ends are held in.
const WINDOW_BITS: u32 = u32::BITS;
/// A byte a carry turns to `0x00`, carrying on.
const ALL_ONES: u8 = u8::MAX;

/// Every byte with its bits in the other order: bytes go to the stream
/// highest bit first, so that the stream can end partway through one.
const REVERSED_BYTES: [u8; 1 << u8::BITS] = {
    let mut reversed_bytes = [0; 1 << u8::BITS];
    let mut byte_value = 0;
    while byte_value < reversed_bytes.len() {
        reversed_bytes[byte_value] = (byte_value as u8).reverse_bits();
        byte_value += 1;
    }
    reversed_bytes
};

/// The probability that a bit is clear, as a fraction of `2^32`: neither
/// it nor its complement is ever under `2^21`, a 2048th -- what the
/// caller's odds never go past -- so neither part of a split rounds away.
#[derive(Clone, Copy, Debug)]
pub struct ClearProbability(pub u32);

impl ClearProbability {
    /// Where `interval_width` splits: the width of the clear part.
    #[inline]
    fn clear_part_width(self, interval_width: u32) -> u32 {
        ((interval_width as u64 * self.0 as u64) >> WINDOW_BITS) as u32
    }
}

/// Bits [`RangeEncoder::finish_stream`] takes beyond what the bits coded carry: the
/// final interval always holds an aligned run of numbers half its
/// width's power of two wide, named by at most two bits more than its
/// width's `-log2`.
pub const FINISHING_BITS: usize = 2;

/// Codes bits into a stream.
pub struct RangeEncoder {
    /// The interval's lower end, in the window, and a carry above it.
    interval_low: u64,
    /// The interval's width.
    interval_width: u32,
    /// The last byte settled, held back for a carry, if any is.
    held_byte: Option<u8>,
    /// `0xFF` bytes held back after it.
    held_all_ones: u64,
}

impl Default for RangeEncoder {
    /// The whole interval, nothing coded.
    fn default() -> Self {
        Self { interval_low: 0, interval_width: u32::MAX, held_byte: None, held_all_ones: 0 }
    }
}

impl RangeEncoder {
    /// Codes `coded_bit` at `clear_probability`, the probability it is clear.
    #[inline]
    pub fn encode_bit(&mut self, coded_bit: bool, clear_probability: ClearProbability, stream: &mut BitStream) {
        let clear_width = clear_probability.clear_part_width(self.interval_width);
        if coded_bit {
            self.interval_low += clear_width as u64;
            self.interval_width -= clear_width;
        } else {
            self.interval_width = clear_width;
        }
        while self.interval_width < LEAST_INTERVAL_WIDTH {
            self.settle_top_byte(stream);
            self.interval_width <<= BYTE_BITS;
        }
    }

    /// Settles the window's top byte, and moves the window a byte on:
    /// held back if a carry could still change it, else written with
    /// every byte held before it, the carry added to them.
    fn settle_top_byte(&mut self, stream: &mut BitStream) {
        let top_byte = (self.interval_low >> (WINDOW_BITS - BYTE_BITS as u32)) as u8;
        let carry_out = self.interval_low >> WINDOW_BITS;
        if top_byte != ALL_ONES || carry_out != 0 {
            self.write_held(carry_out as u8, stream);
            self.held_byte = Some(top_byte);
        } else {
            self.held_all_ones += 1;
        }
        self.interval_low = (self.interval_low << BYTE_BITS) & (u32::MAX as u64);
    }

    /// Writes the bytes held back, `carry_out` added to them.
    fn write_held(&mut self, carry_out: u8, stream: &mut BitStream) {
        if let Some(held_byte) = self.held_byte {
            push_byte(held_byte.wrapping_add(carry_out), stream);
        }
        for _ in 0..self.held_all_ones {
            push_byte(ALL_ONES.wrapping_add(carry_out), stream);
        }
        self.held_all_ones = 0;
    }

    /// Ends the stream: the fewest bits that keep every number they
    /// start inside the final interval, whatever bits follow them -- so
    /// the stream ends itself (`docs/reference.md`, "`arithmetic.rs`").
    pub fn finish_stream(mut self, stream: &mut BitStream) {
        let interval_end = self.interval_low + self.interval_width as u64;
        let (pinned_number, free_bits) = (0..=WINDOW_BITS)
            .rev()
            .map(|free_bits| {
                let free_bits_mask = (1u64 << free_bits) - 1;
                ((self.interval_low + free_bits_mask) & !free_bits_mask, free_bits)
            })
            .find(|&(rounded_number, free_bits)| rounded_number + (1u64 << free_bits) <= interval_end)
            .expect("the interval holds at least one whole unit of its last bit");
        self.write_held((pinned_number >> WINDOW_BITS) as u8, stream);
        let window = pinned_number as u32;
        let significant_bits = WINDOW_BITS - free_bits;
        for bit_position in 0..significant_bits {
            stream.push(window >> (WINDOW_BITS - 1 - bit_position) & 1 == 1);
        }
    }
}

/// Writes `byte_value`, highest bit first.
#[inline]
fn push_byte(byte_value: u8, stream: &mut BitStream) {
    stream.push_value(REVERSED_BYTES[byte_value as usize] as u64, BYTE_BITS);
}

/// Reads a byte [`push_byte`] wrote.
#[inline]
fn read_byte(stream_reader: &mut BitReader) -> u8 {
    REVERSED_BYTES[stream_reader.value(BYTE_BITS) as usize]
}

/// Reads back bits an [`RangeEncoder`] coded.
pub struct RangeDecoder {
    /// The interval's width.
    interval_width: u32,
    /// The number the stream spells, less the interval's lower end, in
    /// the window.
    offset_in_interval: u32,
}

impl RangeDecoder {
    /// Starts reading at `stream_reader`'s next bit.
    pub fn new(stream_reader: &mut BitReader) -> Self {
        let mut offset_in_interval = 0;
        for _ in 0..WINDOW_BITS / BYTE_BITS as u32 {
            offset_in_interval = offset_in_interval << BYTE_BITS | read_byte(stream_reader) as u32;
        }
        Self { interval_width: u32::MAX, offset_in_interval }
    }

    /// Reads a bit coded at `clear_probability`.
    #[inline]
    pub fn decode_bit(&mut self, clear_probability: ClearProbability, stream_reader: &mut BitReader) -> bool {
        let clear_width = clear_probability.clear_part_width(self.interval_width);
        let decoded_bit = self.offset_in_interval >= clear_width;
        if decoded_bit {
            self.offset_in_interval -= clear_width;
            self.interval_width -= clear_width;
        } else {
            self.interval_width = clear_width;
        }
        while self.interval_width < LEAST_INTERVAL_WIDTH {
            self.interval_width <<= BYTE_BITS;
            self.offset_in_interval = self.offset_in_interval << BYTE_BITS | read_byte(stream_reader) as u32;
        }
        decoded_bit
    }
}
