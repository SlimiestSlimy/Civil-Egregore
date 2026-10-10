//! What a layer is, and how its bitmap is encoded and decoded.
//!
//! A layer is a pair of a [`LayerType`] -- what it represents, from
//! specific things to properties -- and a bitmap marking the cells where
//! it holds, Tessera-encoded in 64-bit words. A Tessera stream ends
//! itself, so an encoded bitmap's exact length is never kept: decoding
//! reads from its first word, whatever follows its last.
//!
//! [`LayerCodec`] holds what encoding and decoding need, allocated once:
//! a Tessera, its stream, and the bitmap it decodes into. Encoding reads
//! a bitmap's cells from wherever they are held -- an arena's bucket,
//! say -- and decoding writes them there.

use bitmap::{Bitmap, CellWords};
use tessera::{BitStream, Tessera, MOST_BITS};
use type_registry::LayerType;

/// The most words an encoded bitmap takes.
const MOST_WORDS: usize = MOST_BITS.div_ceil(u64::BITS as usize);

/// Encodes and decodes layers' bitmaps, holding everything either
/// needs, allocated once and reused for every bitmap.
pub struct LayerCodec {
    /// The encoding itself.
    tessera: Tessera,
    /// The stream encoding writes and decoding reads.
    stream: BitStream,
    /// The bitmap encoding reads and decoding writes.
    bitmap: Bitmap,
}

impl LayerCodec {
    /// Everything allocated.
    pub fn new() -> Self {
        Self { tessera: Tessera::new(), stream: BitStream::default(), bitmap: Bitmap::new() }
    }

    /// `cells`, encoded: its words, the stream's bits rounded up to the
    /// next word, until the next encoding.
    pub fn encode(&mut self, cells: &CellWords) -> &[u64] {
        self.bitmap.words_mut().copy_from_slice(cells);
        self.tessera.encode(&self.bitmap, &mut self.stream);
        self.stream.words()
    }

    /// `cells` as a layer's words: encoded, or none where no cell is set
    /// -- a type with no cell set has no layer.
    pub fn encode_layer(&mut self, cells: &CellWords) -> &[u64] {
        if cells.iter().all(|&word| word == 0) {
            &[]
        } else {
            self.encode(cells)
        }
    }

    /// Decodes the bitmap whose encoding starts at `words` into `cells`,
    /// whatever they held before. What follows the encoding in `words`
    /// -- other bitmaps, say -- is read past, not decoded.
    pub fn decode(&mut self, words: &[u64], cells: &mut CellWords) {
        self.stream.load_words(&words[..words.len().min(MOST_WORDS)]);
        self.tessera.decode(&self.stream, &mut self.bitmap);
        cells.copy_from_slice(self.bitmap.words());
    }
}

impl Default for LayerCodec {
    /// The same as [`LayerCodec::new`].
    fn default() -> Self {
        Self::new()
    }
}

/// Which bitmap a bucket holds, hot, and which layer of an image it is
/// encoded to: a layer type, in a chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BucketKey {
    /// The layer's type.
    pub layer_type: LayerType,
    /// The chunk it is a layer of.
    pub chunk: coordinates::ChunkIndex,
}
