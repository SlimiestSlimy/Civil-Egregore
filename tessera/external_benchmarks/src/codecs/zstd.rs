//! zstd on the raw rows: a general-purpose compressor, knowing nothing
//! of images -- a reference point, not a bitmap codec. Its frame carries
//! a few bytes of header.

use super::BitmapCodec;
use crate::rows::{BitmapRows, BITMAP_BYTES};
use bitmap::Bitmap;
use zstd::bulk::{Compressor, Decompressor};

/// zstd at one level, with its contexts and buffers kept between
/// bitmaps.
pub struct ZstdCodec {
    /// The level compressed at.
    compression_level: i32,
    /// The compression context.
    compression_context: Compressor<'static>,
    /// The decompression context.
    decompression_context: Decompressor<'static>,
    /// The last encoding.
    encoded_bytes: Vec<u8>,
    /// The last rows decoded.
    decoded_rows: BitmapRows,
}

impl ZstdCodec {
    /// zstd at `compression_level`.
    pub fn new(compression_level: i32) -> Self {
        Self {
            compression_level,
            compression_context: Compressor::new(compression_level).expect("a zstd context"),
            decompression_context: Decompressor::new().expect("a zstd context"),
            encoded_bytes: Vec::with_capacity(zstd::zstd_safe::compress_bound(BITMAP_BYTES)),
            decoded_rows: BitmapRows(vec![0; BITMAP_BYTES]),
        }
    }
}

impl BitmapCodec for ZstdCodec {
    fn codec_name(&self) -> String {
        format!("zstd level {}", self.compression_level)
    }

    fn encode_bitmap(&mut self, _: &Bitmap, bitmap_rows: &BitmapRows) {
        self.encoded_bytes.clear();
        self.compression_context.compress_to_buffer(&bitmap_rows.0, &mut self.encoded_bytes).expect("room for the bound");
    }

    fn encoded_bits(&self) -> usize {
        self.encoded_bytes.len() * 8
    }

    fn decode_last_encoding(&mut self) {
        let decompressed_length = self.decompression_context.decompress_to_buffer(&self.encoded_bytes, &mut self.decoded_rows.0[..]).expect("a stream zstd wrote");
        assert_eq!(decompressed_length, BITMAP_BYTES);
    }

    fn decoded_matches(&self, bitmap_rows: &BitmapRows) -> bool {
        self.decoded_rows == *bitmap_rows
    }
}
