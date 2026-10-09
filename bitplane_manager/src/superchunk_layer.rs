//! A superchunk layer: one layer type over one superchunk, its hot
//! buckets in one allocation -- which chunks have one, their cells read
//! and changed, their set cells counted.

use super::{COUNT_TILES_IN_CHUNK, COUNT_TILE_CELLS, ChunkSet, NO_CELLS, contains, members, put};
use bitmap::{BITS_PER_WORD, CellWords, WORDS};
use chunk_storage::LayerType;
use coordinates::CHUNKS_IN_SUPERCHUNK;

/// A superchunk layer's four chunk sets, a bit a chunk each, packed
/// together in 8 bytes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct ChunkFlags {
    /// The chunks whose buckets are hot.
    pub(crate) hot: ChunkSet,
    /// The hot chunks changed since they were decoded.
    pub(crate) dirty: ChunkSet,
    /// The chunks written back to the ring and not yet flushed: their
    /// buckets hold their newest cells, hot or not.
    pub(crate) in_ring: ChunkSet,
    /// The chunks whose buckets have a cell set.
    pub(crate) nonempty: ChunkSet,
}

const _: () = assert!(size_of::<ChunkFlags>() == 4 * size_of::<ChunkSet>(), "the four chunk sets packed together");

/// An allocation: one layer type over one superchunk. It holds a bucket
/// for each chunk that has needed one, in Morton order -- a chunk with
/// none has no cell set -- beside which chunks are hot, dirty and
/// waiting, and the counts. It owns its buckets, so a superchunk's
/// allocations are changed apart from every other's.
pub(crate) struct SuperchunkLayer {
    /// The layer's type.
    pub(crate) layer_type: LayerType,
    /// The buckets of the chunks in `kept`, one after another in Morton
    /// order: [`SuperchunkLayer::bucket_words`] words each.
    pub(crate) buckets: Vec<u64>,
    /// The chunks that have a bucket.
    pub(crate) kept: ChunkSet,
    /// Which chunks are hot, dirty, waiting in the ring and non-empty.
    pub(crate) flags: ChunkFlags,
    /// How many cells each bucket has set, less one, by place --
    /// a bucket with any cell set has 1 to 65,536 of them, so a `u16`
    /// holds the count -- meaningful where the bucket is non-empty
    /// and hot or waiting in the ring.
    pub(crate) counts_less_one: [u16; CHUNKS_IN_SUPERCHUNK],
    /// How many cells the hot buckets have set, together.
    pub(crate) hot_count: u32,
    /// How many cells each count tile of each bucket has set, by Morton
    /// index: kept in step with every change, as the buckets' counts
    /// are, and meaningful where they are. What sampling passes over
    /// most of a bitmap by, 128 bytes a bucket beside its 8 KiB.
    pub(crate) tile_counts: [[u16; COUNT_TILES_IN_CHUNK]; CHUNKS_IN_SUPERCHUNK],
}

impl SuperchunkLayer {
    /// How many cells the bucket at `chunk` has set.
    pub(crate) fn count(&self, chunk: usize) -> u32 {
        if contains(self.flags.nonempty, chunk) { self.counts_less_one[chunk] as u32 + 1 } else { 0 }
    }

    /// Makes `count` the bucket at `chunk`'s count of cells set.
    pub(crate) fn set_count(&mut self, chunk: usize, count: u32) {
        put(&mut self.flags.nonempty, chunk, count > 0);
        self.counts_less_one[chunk] = count.saturating_sub(1) as u16;
    }

    /// Words a bucket takes: a bitmap's, times the bits a cell.
    fn bucket_words(&self) -> usize {
        WORDS * self.layer_type.bits() as usize
    }

    /// Where the bucket of the chunk at `chunk` starts among the
    /// buckets, each `words` long, if it has one: after those of the
    /// chunks before it.
    #[inline]
    fn start(&self, chunk: usize, words: usize) -> Option<usize> {
        contains(self.kept, chunk).then(|| (self.kept & ((1 << chunk) - 1)).count_ones() as usize * words)
    }

    /// The words of the bucket of the chunk at `chunk`, however wide:
    /// all clear, if it has none.
    pub(crate) fn words(&self, chunk: usize) -> &[u64] {
        let words = self.bucket_words();
        match self.start(chunk, words) {
            Some(start) => &self.buckets[start..][..words],
            None => &NO_CELLS[..words],
        }
    }

    /// The bucket of the chunk at `chunk`, to change: made now, all
    /// clear and its counts none, in its place among the others, if the
    /// chunk had none.
    pub(crate) fn keep(&mut self, chunk: usize) -> &mut [u64] {
        let words = self.bucket_words();
        if !contains(self.kept, chunk) {
            put(&mut self.kept, chunk, true);
            let (start, end) = (self.start(chunk, words).expect("kept now"), self.buckets.len());
            self.buckets.reserve_exact(words);
            self.buckets.resize(end + words, 0);
            self.buckets.copy_within(start..end, start + words);
            self.buckets[start..][..words].fill(0);
            self.tile_counts[chunk] = [0; COUNT_TILES_IN_CHUNK];
            self.set_count(chunk, 0);
        }
        let start = self.start(chunk, words).expect("kept");
        &mut self.buckets[start..][..words]
    }

    /// Lets the bucket of the chunk at `chunk` go, if it has one: the
    /// chunk has no cell set then.
    pub(crate) fn let_bucket_go(&mut self, chunk: usize) {
        let words = self.bucket_words();
        if let Some(start) = self.start(chunk, words) {
            self.buckets.drain(start..start + words);
            put(&mut self.kept, chunk, false);
        }
        self.tile_counts[chunk] = [0; COUNT_TILES_IN_CHUNK];
        self.set_count(chunk, 0);
    }

    /// Lets go the bucket of every chunk neither hot nor waiting in the
    /// ring: nothing reads it any more.
    pub(crate) fn let_unused_go(&mut self) {
        for chunk in members(self.kept & !(self.flags.hot | self.flags.in_ring)) {
            self.let_bucket_go(chunk);
        }
    }

    /// The number at `cell`, in Morton order, of the bucket at `chunk`:
    /// the cell's bits, as many as the layer is wide.
    pub(crate) fn value(&self, chunk: usize, cell: usize) -> u32 {
        self.value_of(chunk, cell, self.layer_type.bits() as usize)
    }

    /// [`SuperchunkLayer::value`], the layer's `bits` a cell given:
    /// known where the plane's width is in its type.
    #[inline]
    pub(crate) fn value_of(&self, chunk: usize, cell: usize, bits: usize) -> u32 {
        debug_assert_eq!(bits, self.layer_type.bits() as usize, "a plane read at its own width");
        let Some(start) = self.start(chunk, WORDS * bits) else {
            return 0;
        };
        let at = cell * bits;
        (self.buckets[start + at / BITS_PER_WORD] >> (at % BITS_PER_WORD)) as u32 & ((1 << bits) - 1)
    }

    /// Makes `value` the number at `cell`, in Morton order, of the hot
    /// bucket at `chunk`, if it is not already: the bucket is then
    /// dirty, and the counts -- of the cells whose number is not 0 --
    /// move if the cell came to 0 or left it. Whether it changed.
    pub(crate) fn put_value(&mut self, chunk: usize, cell: usize, value: u32) -> bool {
        let (bits, at) = (self.layer_type.bits() as usize, cell * self.layer_type.bits() as usize);
        // A chunk with no bucket holds 0 everywhere: it is given one only for another number.
        let start = match self.start(chunk, WORDS * bits) {
            Some(start) => start,
            None if value as u64 & ((1u64 << bits) - 1) == 0 => return false,
            None => {
                self.keep(chunk);
                self.start(chunk, WORDS * bits).expect("kept now")
            }
        };
        let (word, mask) = (&mut self.buckets[start + at / BITS_PER_WORD], (1u64 << bits) - 1);
        let (was, value) = (*word >> (at % BITS_PER_WORD) & mask, value as u64 & mask);
        if was == value {
            return false;
        }
        *word ^= (was ^ value) << (at % BITS_PER_WORD);
        put(&mut self.flags.dirty, chunk, true);
        if (was == 0) != (value == 0) {
            let (count, tile_count) = (self.count(chunk), &mut self.tile_counts[chunk][cell / COUNT_TILE_CELLS]);
            if was == 0 {
                *tile_count += 1;
                self.set_count(chunk, count + 1);
                self.hot_count += 1;
            } else {
                *tile_count -= 1;
                self.set_count(chunk, count - 1);
                self.hot_count -= 1;
            }
        }
        true
    }

    /// The bucket of the chunk at `chunk`, of a layer a bit a cell: all
    /// clear, if it has none.
    pub(crate) fn cells(&self, chunk: usize) -> &CellWords {
        self.words(chunk).try_into().expect("a bucket is a bitmap's words")
    }

    /// Whether the cell at `cell`, in Morton order, of the bucket at
    /// `chunk` is set.
    #[inline]
    pub(crate) fn get(&self, chunk: usize, cell: usize) -> bool {
        self.start(chunk, WORDS).is_some_and(|start| self.buckets[start + cell / BITS_PER_WORD] >> (cell % BITS_PER_WORD) & 1 == 1)
    }

    /// Makes the cell at `cell`, in Morton order, of the hot bucket at
    /// `chunk` set or clear, if it is not already: the bucket is then
    /// dirty, and its count and the hot count move by one. Whether it
    /// changed.
    pub(crate) fn put_cell(&mut self, chunk: usize, cell: usize, set: bool) -> bool {
        // A chunk with no bucket has no cell set: it is given one only to set one.
        let start = match self.start(chunk, WORDS) {
            Some(start) => start,
            None if !set => return false,
            None => {
                self.keep(chunk);
                self.start(chunk, WORDS).expect("kept now")
            }
        };
        // The one word read and written, found once.
        let (word, bit) = (&mut self.buckets[start + cell / BITS_PER_WORD], 1 << (cell % BITS_PER_WORD));
        if (*word & bit != 0) == set {
            return false;
        }
        *word ^= bit;
        put(&mut self.flags.dirty, chunk, true);
        let (count, tile_count) = (self.count(chunk), &mut self.tile_counts[chunk][cell / COUNT_TILE_CELLS]);
        if set {
            *tile_count += 1;
            self.set_count(chunk, count + 1);
            self.hot_count += 1;
        } else {
            *tile_count -= 1;
            self.set_count(chunk, count - 1);
            self.hot_count -= 1;
        }
        true
    }
}
