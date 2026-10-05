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

/// The most words an encoded bitmap takes.
const MOST_WORDS: usize = MOST_BITS.div_ceil(u64::BITS as usize);

/// What a layer represents: a `u64` naming anything from a specific
/// thing to a property. What each value means is not this crate's
/// business; only that two layers of a chunk never share one.
///
/// A type may be wide ([`LayerType::wide`]): 2, 4, 8 or 16 bits a cell
/// where it is hot, one bucket holding a cell's whole number. Cold, it
/// is as many layers of a bit a cell, the bit `b` of every cell the
/// layer of type `first + b` ([`LayerType::plane`], [`crate::wide`]):
/// storage and the codec know bitmaps only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerType(pub u64);

impl LayerType {
    /// Where a type keeps how wide it is: the power of two, over its number.
    const WIDTH_SHIFT: u32 = 56;

    /// The type of `bits` bits a cell -- 1, 2, 4, 8 or 16 -- whose
    /// planes are the types `first` on; of one bit, `LayerType(first)`.
    pub const fn wide(first: u64, bits: u32) -> Self {
        assert!(bits.is_power_of_two() && bits <= 16 && first >> Self::WIDTH_SHIFT == 0, "a plane is 1, 2, 4, 8 or 16 bits a cell");
        Self(first | (bits.trailing_zeros() as u64) << Self::WIDTH_SHIFT)
    }

    /// Bits a cell: 1 unless the type is wide.
    pub const fn bits(self) -> u32 {
        1 << (self.0 >> Self::WIDTH_SHIFT)
    }

    /// The type of its plane `bit`, a bit a cell: what it is kept cold as.
    pub const fn plane(self, bit: u32) -> Self {
        Self((self.0 & ((1 << Self::WIDTH_SHIFT) - 1)) + bit as u64)
    }

    /// The types of its planes, the lowest bit's first.
    pub fn planes(self) -> impl Iterator<Item = Self> {
        (0..self.bits()).map(move |bit| self.plane(bit))
    }

    /// Whether `plane`, a type of a bit a cell, is one of its planes.
    pub const fn holds(self, plane: Self) -> bool {
        plane.0 >= self.plane(0).0 && plane.0 < self.plane(0).0 + self.bits() as u64
    }
}

/// How wide a plane is, as a type: what a [`Wide`] plane is of, so that
/// its width is known where it is read and written, and a number of
/// one width is never read as another.
pub trait Width: Copy {
    /// Bits a cell.
    const BITS: u32;
}

/// Two bits a cell: numbers 0 to 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bits2;
/// Four bits a cell: numbers 0 to 15.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bits4;
/// Eight bits a cell: numbers 0 to 255.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bits8;
/// Sixteen bits a cell: numbers 0 to 65,535.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bits16;

impl Width for Bits2 {
    const BITS: u32 = 2;
}
impl Width for Bits4 {
    const BITS: u32 = 4;
}
impl Width for Bits8 {
    const BITS: u32 = 8;
}
impl Width for Bits16 {
    const BITS: u32 = 16;
}

/// A wide plane, its width in its type: a cell of it holds a number,
/// read and written whole. A layer of a bit a cell is a [`LayerType`]
/// alone, set or clear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wide<W: Width> {
    /// Its layer type, which says its width too ([`LayerType::wide`]).
    layer_type: LayerType,
    /// Its width.
    width: std::marker::PhantomData<W>,
}

impl<W: Width> Wide<W> {
    /// The plane whose bits are kept cold as the types `first` on.
    pub const fn new(first: u64) -> Self {
        Self { layer_type: LayerType::wide(first, W::BITS), width: std::marker::PhantomData }
    }

    /// Its layer type: what it is made hot, and found in the arena, by.
    pub const fn layer_type(self) -> LayerType {
        self.layer_type
    }

    /// The most a cell of it holds.
    pub const fn most(self) -> u32 {
        (1 << W::BITS) - 1
    }
}

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
