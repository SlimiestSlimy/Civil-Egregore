//! JBIG (ITU-T T.82), the lossless standard for bitmaps: every pixel
//! coded by an adaptive arithmetic coder on the context of the pixels
//! around it. By jbigkit, the reference C implementation, through
//! `csrc/jbig_shim.c`: one stripe, jbigkit's default options. Its
//! stream carries a 20-byte header.

use super::BitmapCodec;
use crate::rows::{BitmapRows, BITMAP_BYTES, HEIGHT, WIDTH};
use bitmap::Bitmap;

extern "C" {
    /// See `csrc/jbig_shim.c`.
    fn external_benchmarks_jbig_encode(row_bytes: *mut u8, width: u64, height: u64, output_bytes: *mut u8, output_capacity: usize) -> usize;
    /// See `csrc/jbig_shim.c`.
    fn external_benchmarks_jbig_decode(input_bytes: *mut u8, input_length: usize, row_bytes: *mut u8, rows_length: usize) -> i32;
}

/// Room for any encoding: a bitmap's own bytes twice over, with room
/// for the header -- an arithmetic coder never comes near that.
const MOST_ENCODED: usize = 2 * BITMAP_BYTES + 1024;

/// JBIG, with its room: the rows handed to jbigkit, its output, and the
/// rows last decoded.
pub struct JbigCodec {
    /// A copy of the rows being encoded: jbigkit's interface takes them
    /// mutable.
    input_rows: Vec<u8>,
    /// Room for the output.
    encoded_bytes: Vec<u8>,
    /// Bytes of it the last encoding wrote.
    encoded_length: usize,
    /// The last rows decoded.
    decoded_rows: BitmapRows,
}

impl JbigCodec {
    /// Everything allocated once.
    pub fn new() -> Self {
        Self { input_rows: vec![0; BITMAP_BYTES], encoded_bytes: vec![0; MOST_ENCODED], encoded_length: 0, decoded_rows: BitmapRows(vec![0; BITMAP_BYTES]) }
    }
}

impl Default for JbigCodec {
    /// The same as [`JbigCodec::new`].
    fn default() -> Self {
        Self::new()
    }
}

impl BitmapCodec for JbigCodec {
    fn codec_name(&self) -> String {
        "JBIG (jbigkit)".to_string()
    }

    fn encode_bitmap(&mut self, _: &Bitmap, bitmap_rows: &BitmapRows) {
        self.input_rows.copy_from_slice(&bitmap_rows.0);
        // Safety: every pointer is to a buffer of the length passed with
        // it, owned here for the call.
        self.encoded_length = unsafe {
            external_benchmarks_jbig_encode(self.input_rows.as_mut_ptr(), WIDTH as u64, HEIGHT as u64, self.encoded_bytes.as_mut_ptr(), MOST_ENCODED)
        };
        assert!(self.encoded_length > 0, "a JBIG stream larger than {MOST_ENCODED} bytes");
    }

    fn encoded_bits(&self) -> usize {
        self.encoded_length * 8
    }

    fn decode_last_encoding(&mut self) {
        // Safety: as for encoding.
        let decode_status = unsafe {
            external_benchmarks_jbig_decode(self.encoded_bytes.as_mut_ptr(), self.encoded_length, self.decoded_rows.0.as_mut_ptr(), BITMAP_BYTES)
        };
        assert_eq!(decode_status, 0, "jbigkit could not decode its own stream");
    }

    fn decoded_matches(&self, bitmap_rows: &BitmapRows) -> bool {
        self.decoded_rows == *bitmap_rows
    }
}
