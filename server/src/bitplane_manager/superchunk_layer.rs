//! A superchunk layer: one layer type over one superchunk, its hot
//! buckets in one allocation -- which chunks have one, their cells read
//! and changed, their set cells counted.

use super::{COUNT_TILES_IN_CHUNK, COUNT_TILE_CELLS, COUNT_TILE_WORDS, ChunkSet, NO_CELLS, contains, members, set_membership};
use utilities::bitmap::{BITS_PER_WORD, CellWords, WORDS};
use crate::chunk_storage::LayerType;
use utilities::coordinates::CHUNKS_IN_SUPERCHUNK;

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
    pub(crate) with_cells_set: ChunkSet,
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
    pub(crate) chunks_with_bucket: ChunkSet,
    /// Which chunks are hot, dirty, waiting in the ring and non-empty.
    pub(crate) chunk_flags: ChunkFlags,
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
    /// An allocation of `layer_type` with no bucket and nothing set, its
    /// chunks hot as `hot` says.
    pub(crate) fn empty(layer_type: LayerType, hot: ChunkSet) -> Self {
        Self { layer_type, buckets: Vec::new(), chunks_with_bucket: 0, chunk_flags: ChunkFlags { hot, ..ChunkFlags::default() }, counts_less_one: [0; CHUNKS_IN_SUPERCHUNK], hot_count: 0, tile_counts: [[0; COUNT_TILES_IN_CHUNK]; CHUNKS_IN_SUPERCHUNK] }
    }

    /// Makes the bucket of the chunk at `chunk` hot, `fill_bucket` writing
    /// its cells into it -- unless it is hot already, or waiting in the
    /// ring, its bucket holding its newest cells -- then counted: whether
    /// it was made hot now. With no `fill_bucket` it has no cell set, and
    /// no bucket.
    pub(crate) fn make_chunk_hot(&mut self, chunk: usize, fill_bucket: Option<impl FnOnce(&mut [u64])>) -> bool {
        if contains(self.chunk_flags.hot, chunk) {
            return false;
        }
        if !contains(self.chunk_flags.in_ring, chunk) {
            match fill_bucket {
                Some(fill_given) => {
                    let (bits_a_cell, tile_shift) = (self.layer_type.width_bits(), self.layer_type.tile_shift());
                    let bucket = self.bucket_or_insert(chunk);
                    fill_given(bucket);
                    // A count tile's cells: as many times its words as the layer has bits a cell -- over its tile's cells, of a plane of tiles.
                    let tile_words = (COUNT_TILE_WORDS * bits_a_cell as usize) >> tile_shift;
                    assert!(tile_words > 0, "a plane of tiles has a word a count tile at the least");
                    let tile_counts: [u16; COUNT_TILES_IN_CHUNK] = std::array::from_fn(|tile| crate::chunk_storage::wide::cells_set(&bucket[tile * tile_words..][..tile_words], bits_a_cell) as u16);
                    self.tile_counts[chunk] = tile_counts;
                    self.set_count(chunk, tile_counts.iter().map(|&count| count as u32).sum());
                }
                None => self.let_bucket_go(chunk),
            }
        }
        set_membership(&mut self.chunk_flags.hot, chunk, true);
        self.hot_count += self.count(chunk);
        true
    }

    /// How many cells the bucket at `chunk` has set.
    pub(crate) fn count(&self, chunk: usize) -> u32 {
        if contains(self.chunk_flags.with_cells_set, chunk) { self.counts_less_one[chunk] as u32 + 1 } else { 0 }
    }

    /// Makes `count` the bucket at `chunk`'s count of cells set.
    pub(crate) fn set_count(&mut self, chunk: usize, count: u32) {
        set_membership(&mut self.chunk_flags.with_cells_set, chunk, count > 0);
        self.counts_less_one[chunk] = count.saturating_sub(1) as u16;
    }

    /// Words a bucket takes: a bitmap's, times the bits a cell -- of
    /// a plane of tiles, a number a tile and so much the fewer
    /// (`crate::chunk_storage::wide::bucket_words`).
    #[inline]
    fn bucket_words(&self) -> usize {
        crate::chunk_storage::wide::bucket_words(self.layer_type)
    }

    /// Where the bucket of the chunk at `chunk` starts among the
    /// buckets, each `words` long, if it has one: after those of the
    /// chunks before it.
    #[inline]
    fn bucket_offset(&self, chunk: usize, words: usize) -> Option<usize> {
        contains(self.chunks_with_bucket, chunk).then(|| (self.chunks_with_bucket & ((1 << chunk) - 1)).count_ones() as usize * words)
    }

    /// The words of the bucket of the chunk at `chunk`, however wide:
    /// all clear, if it has none.
    pub(crate) fn words(&self, chunk: usize) -> &[u64] {
        let words = self.bucket_words();
        match self.bucket_offset(chunk, words) {
            Some(bucket_start) => &self.buckets[bucket_start..][..words],
            None => &NO_CELLS[..words],
        }
    }

    /// The bucket of the chunk at `chunk`, to change: made now, all
    /// clear and its counts none, in its place among the others, if the
    /// chunk had none.
    pub(crate) fn bucket_or_insert(&mut self, chunk: usize) -> &mut [u64] {
        let words = self.bucket_words();
        if !contains(self.chunks_with_bucket, chunk) {
            set_membership(&mut self.chunks_with_bucket, chunk, true);
            let (new_bucket_start, old_buckets_end) = (self.bucket_offset(chunk, words).expect("kept now"), self.buckets.len());
            self.buckets.reserve_exact(words);
            self.buckets.resize(old_buckets_end + words, 0);
            self.buckets.copy_within(new_bucket_start..old_buckets_end, new_bucket_start + words);
            self.buckets[new_bucket_start..][..words].fill(0);
            self.tile_counts[chunk] = [0; COUNT_TILES_IN_CHUNK];
            self.set_count(chunk, 0);
        }
        let bucket_start = self.bucket_offset(chunk, words).expect("kept");
        &mut self.buckets[bucket_start..][..words]
    }

    /// Lets the bucket of the chunk at `chunk` go, if it has one: the
    /// chunk has no cell set then.
    pub(crate) fn let_bucket_go(&mut self, chunk: usize) {
        let words = self.bucket_words();
        if let Some(bucket_start) = self.bucket_offset(chunk, words) {
            self.buckets.drain(bucket_start..bucket_start + words);
            set_membership(&mut self.chunks_with_bucket, chunk, false);
        }
        self.tile_counts[chunk] = [0; COUNT_TILES_IN_CHUNK];
        self.set_count(chunk, 0);
    }

    /// Lets go the bucket of every chunk neither hot nor waiting in the
    /// ring: nothing reads it any more.
    pub(crate) fn let_unused_go(&mut self) {
        for chunk in members(self.chunks_with_bucket & !(self.chunk_flags.hot | self.chunk_flags.in_ring)) {
            self.let_bucket_go(chunk);
        }
    }

    /// The number at `cell`, in Morton order, of the bucket at `chunk`:
    /// the cell's bits, as many as the layer is wide -- of a plane of
    /// tiles, the number of the tile the cell is in.
    pub(crate) fn value(&self, chunk: usize, cell: usize) -> u32 {
        self.value_of(chunk, cell, self.layer_type.width_bits() as usize)
    }

    /// [`SuperchunkLayer::value`], the layer's `bits_a_cell` a cell given:
    /// known where the plane's width is in its type.
    #[inline]
    pub(crate) fn value_of(&self, chunk: usize, cell: usize, bits_a_cell: usize) -> u32 {
        debug_assert_eq!(bits_a_cell, self.layer_type.width_bits() as usize, "a plane read at its own width");
        let tile_shift_bits = self.layer_type.tile_shift();
        let Some(bucket_start) = self.bucket_offset(chunk, (WORDS * bits_a_cell) >> tile_shift_bits) else {
            return 0;
        };
        let bit_in_bucket = (cell >> tile_shift_bits) * bits_a_cell;
        (self.buckets[bucket_start + bit_in_bucket / BITS_PER_WORD] >> (bit_in_bucket % BITS_PER_WORD) & ((1u64 << bits_a_cell) - 1)) as u32
    }

    /// Makes `value` the number at `cell`, in Morton order, of the hot
    /// bucket at `chunk`, if it is not already: the bucket is then
    /// dirty, and the counts -- of the cells whose number is not 0 --
    /// move if the cell came to 0 or left it. Whether it changed.
    pub(crate) fn write_value(&mut self, chunk: usize, cell: usize, value: u32) -> bool {
        let (bits_a_cell, tile_shift_bits) = (self.layer_type.width_bits() as usize, self.layer_type.tile_shift());
        let (bit_in_bucket, words) = ((cell >> tile_shift_bits) * bits_a_cell, (WORDS * bits_a_cell) >> tile_shift_bits);
        // A chunk with no bucket holds 0 everywhere: it is given one only for another number.
        let bucket_start = match self.bucket_offset(chunk, words) {
            Some(existing_start) => existing_start,
            None if value as u64 & ((1u64 << bits_a_cell) - 1) == 0 => return false,
            None => {
                self.bucket_or_insert(chunk);
                self.bucket_offset(chunk, words).expect("kept now")
            }
        };
        let (word, mask) = (&mut self.buckets[bucket_start + bit_in_bucket / BITS_PER_WORD], (1u64 << bits_a_cell) - 1);
        let (old_value, value) = (*word >> (bit_in_bucket % BITS_PER_WORD) & mask, value as u64 & mask);
        if old_value == value {
            return false;
        }
        *word ^= (old_value ^ value) << (bit_in_bucket % BITS_PER_WORD);
        set_membership(&mut self.chunk_flags.dirty, chunk, true);
        if (old_value == 0) != (value == 0) {
            let (count, tile_count) = (self.count(chunk), &mut self.tile_counts[chunk][cell / COUNT_TILE_CELLS]);
            if old_value == 0 {
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
        self.bucket_offset(chunk, WORDS).is_some_and(|bucket_start| self.buckets[bucket_start + cell / BITS_PER_WORD] >> (cell % BITS_PER_WORD) & 1 == 1)
    }

    /// Makes the cell at `cell`, in Morton order, of the hot bucket at
    /// `chunk` set or clear, if it is not already: the bucket is then
    /// dirty, and its count and the hot count move by one. Whether it
    /// changed.
    pub(crate) fn write_cell(&mut self, chunk: usize, cell: usize, make_set: bool) -> bool {
        // A chunk with no bucket has no cell set: it is given one only to set one.
        let bucket_start = match self.bucket_offset(chunk, WORDS) {
            Some(existing_start) => existing_start,
            None if !make_set => return false,
            None => {
                self.bucket_or_insert(chunk);
                self.bucket_offset(chunk, WORDS).expect("kept now")
            }
        };
        // The one word read and written, found once.
        let (word, cell_bit) = (&mut self.buckets[bucket_start + cell / BITS_PER_WORD], 1 << (cell % BITS_PER_WORD));
        if (*word & cell_bit != 0) == make_set {
            return false;
        }
        *word ^= cell_bit;
        set_membership(&mut self.chunk_flags.dirty, chunk, true);
        let (count, tile_count) = (self.count(chunk), &mut self.tile_counts[chunk][cell / COUNT_TILE_CELLS]);
        if make_set {
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
