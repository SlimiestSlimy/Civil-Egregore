//! The codecs compared, one file each, behind one interface: encode a
//! bitmap, decode it back, check it came back whole.

pub mod g4;
pub mod tessera;
pub mod jbig;
pub mod zstd;

use crate::rows::BitmapRows;
use bitmap::Bitmap;

/// One codec, holding its own room and its last output.
pub trait BitmapCodec {
    /// What the tables call it.
    fn codec_name(&self) -> String;

    /// Encodes one bitmap, given both as Tessera takes it and in rows. Only
    /// this is timed as encoding.
    fn encode_bitmap(&mut self, bitmap: &Bitmap, bitmap_rows: &BitmapRows);

    /// The last encoding's size, in bits.
    fn encoded_bits(&self) -> usize;

    /// Decodes what [`BitmapCodec::encode_bitmap`] last wrote. Only this is timed as
    /// decoding.
    fn decode_last_encoding(&mut self);

    /// Whether the last decode gave back `bitmap_rows`. Not timed.
    fn decoded_matches(&self, bitmap_rows: &BitmapRows) -> bool;
}
