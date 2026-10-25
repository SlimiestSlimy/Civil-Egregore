//! What a plane chunk is, and how its bitmap is encoded and decoded
//! (`docs/chunk_storage.md`, "Layers and their codec").

use utilities::bitmap::{Bitmap, CellWords};
use tessera::{BitStream, Tessera, MOST_BITS};
use protocol::type_registry::PlaneType;

/// The most words an encoded bitmap takes.
const MOST_WORDS: usize = MOST_BITS.div_ceil(u64::BITS as usize);

/// Encodes and decodes plane chunks' bitmaps, holding everything either
/// needs, allocated once and reused for every bitmap.
pub struct BitmapCodec {
    /// The encoding itself.
    tessera: Tessera,
    /// The stream encoding writes and decoding reads.
    stream: BitStream,
    /// The bitmap encoding reads and decoding writes.
    bitmap: Bitmap,
}

impl BitmapCodec {
    /// Everything allocated.
    pub fn new() -> Self {
        Self { tessera: Tessera::new(), stream: BitStream::default(), bitmap: Bitmap::new() }
    }

    /// `cells`, encoded: its words, the stream's bits rounded up to the
    /// next word, until the next encoding.
    pub fn encode_cells(&mut self, cells: &CellWords) -> &[u64] {
        self.bitmap.words_mut().copy_from_slice(cells);
        self.tessera.encode(&self.bitmap, &mut self.stream);
        self.stream.words()
    }

    /// `cells` as a plane chunk's words: encoded, or none where no cell is set
    /// -- a type with no cell set has no plane chunk.
    pub fn encode_unless_empty(&mut self, cells: &CellWords) -> &[u64] {
        if cells.iter().all(|&word| word == 0) {
            &[]
        } else {
            self.encode_cells(cells)
        }
    }

    /// Decodes the bitmap whose encoding starts at `words` into `cells`,
    /// whatever they held before. What follows the encoding in `words`
    /// -- other bitmaps, say -- is read past, not decoded.
    pub fn decode_cells(&mut self, words: &[u64], cells: &mut CellWords) {
        self.stream.load_words(&words[..words.len().min(MOST_WORDS)]);
        self.tessera.decode(&self.stream, &mut self.bitmap);
        cells.copy_from_slice(self.bitmap.words());
    }
}

impl Default for BitmapCodec {
    /// The same as [`BitmapCodec::new`].
    fn default() -> Self {
        Self::new()
    }
}

/// One plane over one chunk: what a hot superchunk writes back, and what
/// a cold superchunk encodes, a chunk at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlaneChunk {
    /// The plane's type.
    pub plane_type: PlaneType,
    /// The chunk.
    pub chunk: utilities::coordinates::ChunkIndex,
}
