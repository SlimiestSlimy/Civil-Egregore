//! Bits, cells set and encode time, over many bitmaps, each encoded and
//! decoded by one `Tessera`.

use super::examination::first_difference;
use crate::bit_stream::BitStream;
use crate::Tessera;
use bitmap::Bitmap;
use std::time::Instant;

/// What some bitmaps came to.
#[derive(Clone, Debug, Default)]
pub struct MeasuredBitmaps {
    /// How many bitmaps.
    pub bitmaps: usize,
    /// Their cells set, all together.
    pub cells_set: usize,
    /// Tessera's bits for them, all together.
    pub total_bits: usize,
    /// The fewest bits one took; 0 of none.
    pub fewest_bits: usize,
    /// The most bits one took.
    pub most_bits: usize,
    /// Microseconds spent encoding them, all together.
    pub encode_micros: u128,
    /// The bitmaps, by their place among these, that did not decode back
    /// to their own cells.
    pub lost: Vec<usize>,
}

impl MeasuredBitmaps {
    /// Encodes and decodes every bitmap of `bitmaps` with `tessera`, and
    /// gathers what they came to.
    pub fn measure_bitmaps(tessera: &mut Tessera, bitmaps: impl IntoIterator<Item = Bitmap>) -> Self {
        let (mut stream, mut decoded_bitmap) = (BitStream::default(), Bitmap::new());
        let mut measured_so_far = MeasuredBitmaps::default();
        for (case_index, bitmap) in bitmaps.into_iter().enumerate() {
            let encode_start = Instant::now();
            tessera.encode(&bitmap, &mut stream);
            measured_so_far.encode_micros += encode_start.elapsed().as_micros();
            tessera.decode(&stream, &mut decoded_bitmap);
            if first_difference(&bitmap, &decoded_bitmap).is_some() {
                measured_so_far.lost.push(case_index);
            }
            measured_so_far.fewest_bits = if measured_so_far.bitmaps == 0 { stream.len() } else { measured_so_far.fewest_bits.min(stream.len()) };
            measured_so_far.bitmaps += 1;
            measured_so_far.cells_set += bitmap.count_set() as usize;
            measured_so_far.total_bits += stream.len();
            measured_so_far.most_bits = measured_so_far.most_bits.max(stream.len());
        }
        measured_so_far
    }

    /// Adds `other_measured`'s bitmaps to these; `other_measured`'s lost ones are not kept.
    pub fn add_measured(&mut self, other_measured: &MeasuredBitmaps) {
        if other_measured.bitmaps == 0 {
            return;
        }
        self.fewest_bits = if self.bitmaps == 0 { other_measured.fewest_bits } else { self.fewest_bits.min(other_measured.fewest_bits) };
        self.most_bits = self.most_bits.max(other_measured.most_bits);
        self.bitmaps += other_measured.bitmaps;
        self.cells_set += other_measured.cells_set;
        self.total_bits += other_measured.total_bits;
        self.encode_micros += other_measured.encode_micros;
    }
}
