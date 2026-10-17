//! Tessera itself, one [`tessera::TesseraCodec`] kept for every bitmap, as a
//! caller encoding many bitmaps would use it.

use super::BitmapCodec;
use crate::rows::{BitmapRows, HEIGHT, WIDTH};
use tessera::BitStream;
use bitmap::Bitmap;

/// Tessera, with its stream and decoded bitmap kept between bitmaps.
pub struct TesseraCodec {
    /// Every structure encoding and decoding need.
    tessera: tessera::Tessera,
    /// The last stream written.
    stream: BitStream,
    /// The last bitmap decoded.
    decoded_bitmap: Bitmap,
}

impl TesseraCodec {
    /// Everything allocated once.
    pub fn new() -> Self {
        Self { tessera: tessera::Tessera::new(), stream: BitStream::default(), decoded_bitmap: Bitmap::new() }
    }
}

impl Default for TesseraCodec {
    /// The same as [`TesseraCodec::new`].
    fn default() -> Self {
        Self::new()
    }
}

impl BitmapCodec for TesseraCodec {
    fn codec_name(&self) -> String {
        "Tessera".to_string()
    }

    fn encode_bitmap(&mut self, bitmap: &Bitmap, _: &BitmapRows) {
        self.tessera.encode(bitmap, &mut self.stream);
    }

    fn encoded_bits(&self) -> usize {
        self.stream.len()
    }

    fn decode_last_encoding(&mut self) {
        self.tessera.decode(&self.stream, &mut self.decoded_bitmap);
    }

    fn decoded_matches(&self, bitmap_rows: &BitmapRows) -> bool {
        (0..HEIGHT).all(|y| (0..WIDTH).all(|x| self.decoded_bitmap.get(x as u8, y as u8) == bitmap_rows.get(x, y)))
    }
}
