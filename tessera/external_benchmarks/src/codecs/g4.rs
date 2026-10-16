//! CCITT Group 4 (ITU-T T.6), the fax standard TIFF and PDF use for
//! bitmaps: each row's colour changes coded against the row above's. By
//! the pure-Rust `fax` crate (pdf-rs). Set cells are black.

use super::BitmapCodec;
use crate::rows::{BitmapRows, BITMAP_BYTES, HEIGHT, ROW_BYTES, WIDTH};
use bitmap::Bitmap;
use fax::decoder::decode_g4;
use fax::encoder::Encoder;
use fax::{Color, VecWriter};

/// Group 4, with its last output and the rows last decoded.
pub struct G4Codec {
    /// The last encoding.
    encoded_bytes: Vec<u8>,
    /// The last rows decoded.
    decoded_rows: BitmapRows,
}

impl G4Codec {
    /// Nothing encoded yet.
    pub fn new() -> Self {
        Self { encoded_bytes: Vec::new(), decoded_rows: BitmapRows(vec![0; BITMAP_BYTES]) }
    }
}

impl Default for G4Codec {
    /// The same as [`G4Codec::new`].
    fn default() -> Self {
        Self::new()
    }
}

impl BitmapCodec for G4Codec {
    fn codec_name(&self) -> String {
        "CCITT G4 (fax crate)".to_string()
    }

    fn encode_bitmap(&mut self, _: &Bitmap, bitmap_rows: &BitmapRows) {
        let mut g4_encoder = Encoder::new(VecWriter::with_capacity(BITMAP_BYTES));
        for row_bytes in bitmap_rows.0.as_chunks::<ROW_BYTES>().0 {
            let row_colours = (0..WIDTH).map(|x| if row_bytes[x / 8] & (0x80 >> (x % 8)) != 0 { Color::Black } else { Color::White });
            g4_encoder.encode_line(row_colours, WIDTH as u32).expect("writing to memory");
        }
        self.encoded_bytes = g4_encoder.finish().expect("writing to memory").finish();
    }

    fn encoded_bits(&self) -> usize {
        self.encoded_bytes.len() * 8
    }

    fn decode_last_encoding(&mut self) {
        let decoded_bytes = &mut self.decoded_rows.0;
        decoded_bytes.fill(0);
        let mut y = 0;
        // Each row comes as the places its colour changes, white first:
        // every other run is black.
        decode_g4(self.encoded_bytes.iter().copied(), WIDTH as u32, Some(HEIGHT as u32), |colour_changes| {
            let decoded_row = &mut decoded_bytes[y * ROW_BYTES..(y + 1) * ROW_BYTES];
            for black_run in colour_changes.chunks(2) {
                let (run_start, run_end) = (black_run[0] as usize, black_run.get(1).map_or(WIDTH, |&next_change| next_change as usize));
                for x in run_start..run_end.min(WIDTH) {
                    decoded_row[x / 8] |= 0x80 >> (x % 8);
                }
            }
            y += 1;
        })
        .expect("a stream G4 wrote");
    }

    fn decoded_matches(&self, bitmap_rows: &BitmapRows) -> bool {
        self.decoded_rows == *bitmap_rows
    }
}
