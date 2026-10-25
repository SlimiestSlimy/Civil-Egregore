//! A plane over one superchunk ([`HotPlane`]), kept sparse: only
//! the chunks with a cell set are kept, one after another in Morton
//! order, each with its count tiles.

use super::{contains, members, ChunkSet, COUNT_TILES_IN_CHUNK, COUNT_TILE_CELLS};
use crate::cold_storage::plane_chunk_encoding::numbers_set;
use protocol::type_registry::PlaneType;
use utilities::bitmap::{CellWords, BITS_PER_WORD, WORDS};

/// The most bits a number of a plane takes.
const MOST_BITS_A_CELL: usize = 32;

/// A chunk with no cell set, however wide its plane: what a chunk not
/// kept is read as.
static NO_CELLS: [u64; WORDS * MOST_BITS_A_CELL] = [0; WORDS * MOST_BITS_A_CELL];

/// The count tiles of a chunk with no cell set.
static NO_COUNTS: [u16; COUNT_TILES_IN_CHUNK] = [0; COUNT_TILES_IN_CHUNK];

/// One plane type's numbers over one superchunk: the chunks with a cell
/// set kept, the others read as all clear and costing nothing. A chunk
/// emptied stays kept until it is written back and flushed, and nothing
/// of the superchunk is on its way to the ring.
pub(crate) struct HotPlane {
    /// The plane's type: its width and its tile size.
    plane_type: PlaneType,
    /// The chunks kept, a bit each by place.
    kept: ChunkSet,
    /// The kept chunks' numbers, one chunk after another in Morton
    /// order, [`PlaneType::chunk_words`] words each.
    words: Vec<u64>,
    /// How many cells of each count tile read a number that is not 0 --
    /// of a plane of tiles, its tiles' cells -- a chunk's 64 a kept chunk,
    /// in the same order: what sampling walks by.
    tile_counts: Vec<[u16; COUNT_TILES_IN_CHUNK]>,
    /// The chunks changed since they were made hot or last written back.
    pub(crate) dirty: ChunkSet,
    /// The chunks written back and not yet flushed into their cold superchunk.
    pub(crate) in_ring: ChunkSet,
    /// How many cells over the superchunk read a number that is not 0.
    cells_set: u32,
}

impl HotPlane {
    /// A plane of `plane_type` with no chunk kept: no cell set.
    pub(crate) fn new(plane_type: PlaneType) -> Self {
        Self { plane_type, kept: 0, words: Vec::new(), tile_counts: Vec::new(), dirty: 0, in_ring: 0, cells_set: 0 }
    }

    /// Its type.
    pub(crate) fn plane_type(&self) -> PlaneType {
        self.plane_type
    }

    /// How many cells over the superchunk read a number that is not 0.
    pub(crate) fn cells_set(&self) -> u32 {
        self.cells_set
    }

    /// The chunks kept.
    pub(crate) fn kept(&self) -> ChunkSet {
        self.kept
    }

    /// Where the chunk at `chunk` is among those kept, if it is.
    #[inline]
    fn kept_rank(&self, chunk: usize) -> Option<usize> {
        contains(self.kept, chunk).then(|| (self.kept & ((1 << chunk) - 1)).count_ones() as usize)
    }

    /// The numbers of the chunk at `chunk`, however wide: all clear if it
    /// is not kept.
    #[inline]
    pub(crate) fn chunk(&self, chunk: usize) -> &[u64] {
        let words = self.plane_type.chunk_words();
        match self.kept_rank(chunk) {
            Some(rank) => &self.words[rank * words..][..words],
            None => &NO_CELLS[..words],
        }
    }

    /// The cells of the chunk at `chunk`, of a plane of a bit a cell.
    #[inline]
    pub(crate) fn cells(&self, chunk: usize) -> &CellWords {
        self.chunk(chunk).try_into().expect("a plane of a bit a cell is a bitmap's words a chunk")
    }

    /// How many cells of each count tile of the chunk at `chunk` read a
    /// number that is not 0.
    #[inline]
    pub(crate) fn tile_counts(&self, chunk: usize) -> &[u16; COUNT_TILES_IN_CHUNK] {
        self.kept_rank(chunk).map_or(&NO_COUNTS, |rank| &self.tile_counts[rank])
    }

    /// How many cells of the chunk at `chunk` read a number that is not 0.
    pub(crate) fn count(&self, chunk: usize) -> u32 {
        self.tile_counts(chunk).iter().map(|&count| u32::from(count)).sum()
    }

    /// The number at `cell`, a cell's place in its chunk in Morton order,
    /// of the chunk at `chunk` -- of a plane of tiles, its tile's.
    #[inline]
    pub(crate) fn value(&self, chunk: usize, cell: usize) -> u32 {
        let Some(rank) = self.kept_rank(chunk) else {
            return 0;
        };
        let (width, bit) = self.number_bit(cell);
        let word = self.words[rank * self.plane_type.chunk_words() + bit / BITS_PER_WORD];
        (word >> (bit % BITS_PER_WORD) & ((1 << width) - 1)) as u32
    }

    /// The width of a number, and the first bit of `cell`'s in its chunk's
    /// words.
    #[inline]
    fn number_bit(&self, cell: usize) -> (usize, usize) {
        let width = self.plane_type.width().bits() as usize;
        (width, (cell >> self.plane_type.tile_size().shift()) * width)
    }

    /// Makes `value` the number at `cell` of the chunk at `chunk`, keeping
    /// the chunk if it was not: whether it changed. Changed, the chunk is
    /// dirty and the counts follow.
    pub(crate) fn set_value(&mut self, chunk: usize, cell: usize, value: u32) -> bool {
        let (width, bit) = self.number_bit(cell);
        let mask = (1u64 << width) - 1;
        debug_assert!(u64::from(value) <= mask, "{value} in a plane of {width} bits a number");
        let value = u64::from(value) & mask;
        let rank = match self.kept_rank(chunk) {
            Some(rank) => rank,
            None if value == 0 => return false,
            None => self.keep_chunk(chunk),
        };
        let word = &mut self.words[rank * self.plane_type.chunk_words() + bit / BITS_PER_WORD];
        let old_value = *word >> (bit % BITS_PER_WORD) & mask;
        if old_value == value {
            return false;
        }
        *word ^= (old_value ^ value) << (bit % BITS_PER_WORD);
        self.dirty |= 1 << chunk;
        if (old_value == 0) != (value == 0) {
            let number = cell >> self.plane_type.tile_size().shift();
            let counts = &mut self.tile_counts[rank];
            for (count_tile, cells) in count_tiles_under(self.plane_type, number) {
                if old_value == 0 {
                    counts[count_tile] += cells;
                } else {
                    counts[count_tile] -= cells;
                }
            }
            let tile_cells = 1u32 << self.plane_type.tile_size().shift();
            self.cells_set = if old_value == 0 { self.cells_set + tile_cells } else { self.cells_set - tile_cells };
        }
        true
    }

    /// Fills the chunk at `chunk`, kept now if it was not, by `fill` -- an
    /// encoded chunk decoded, say -- and counts it. Not dirty: it holds
    /// what its cold superchunk holds.
    pub(crate) fn fill_chunk(&mut self, chunk: usize, fill: impl FnOnce(&mut [u64])) {
        let rank = self.kept_rank(chunk).unwrap_or_else(|| self.keep_chunk(chunk));
        let words = self.plane_type.chunk_words();
        fill(&mut self.words[rank * words..][..words]);
        let mut tile_counts = [0; COUNT_TILES_IN_CHUNK];
        let numbers_a_count_tile = COUNT_TILE_CELLS >> self.plane_type.tile_size().shift();
        let count_tile_words = numbers_a_count_tile * self.plane_type.width().bits() as usize / BITS_PER_WORD;
        if count_tile_words > 0 {
            // Each count tile whole words of numbers: counted a word at a time.
            let tile_cells = 1u16 << self.plane_type.tile_size().shift();
            for (count_tile, tile_words) in self.words[rank * words..][..words].chunks(count_tile_words).enumerate() {
                tile_counts[count_tile] = numbers_set(self.plane_type, tile_words) as u16 * tile_cells;
            }
        } else {
            // Tiles too big for that: a number at a time.
            for number in 0..self.plane_type.numbers_in_chunk() {
                if self.value(chunk, number << self.plane_type.tile_size().shift()) != 0 {
                    count_tiles_under(self.plane_type, number).for_each(|(count_tile, cells)| tile_counts[count_tile] += cells);
                }
            }
        }
        let (old_count, new_count) = (self.tile_counts[rank].iter().map(|&count| u32::from(count)).sum::<u32>(), tile_counts.iter().map(|&count| u32::from(count)).sum::<u32>());
        self.cells_set = self.cells_set - old_count + new_count;
        self.tile_counts[rank] = tile_counts;
    }

    /// Keeps the chunk at `chunk`, all clear, in its place among those
    /// kept: where it is among them.
    fn keep_chunk(&mut self, chunk: usize) -> usize {
        self.kept |= 1 << chunk;
        let (rank, words) = (self.kept_rank(chunk).expect("kept now"), self.plane_type.chunk_words());
        self.words.splice(rank * words..rank * words, std::iter::repeat_n(0, words));
        self.tile_counts.insert(rank, [0; COUNT_TILES_IN_CHUNK]);
        rank
    }

    /// Lets go every chunk kept with no cell set, unless it is dirty or
    /// in the ring: its emptiness is not in its cold superchunk yet.
    pub(crate) fn let_empty_chunks_go(&mut self) {
        let words = self.plane_type.chunk_words();
        for chunk in members(self.kept & !(self.dirty | self.in_ring)).collect::<Vec<_>>().into_iter().rev() {
            let rank = self.kept_rank(chunk).expect("kept");
            if self.tile_counts[rank].iter().all(|&count| count == 0) {
                self.words.drain(rank * words..(rank + 1) * words);
                self.tile_counts.remove(rank);
                self.kept &= !(1 << chunk);
            }
        }
    }

    /// Whether it keeps nothing: no chunk kept, none in the ring.
    pub(crate) fn is_unused(&self) -> bool {
        self.kept == 0 && self.in_ring == 0
    }
}

/// The count tiles the number `number` of a chunk of `plane_type` lies over,
/// each with how many of its cells it covers: one, if a tile is no bigger
/// than a count tile; else every count tile in it, whole.
fn count_tiles_under(plane_type: PlaneType, number: usize) -> impl Iterator<Item = (usize, u16)> {
    let shift = plane_type.tile_size().shift();
    let (first_cell, cells) = (number << shift, 1usize << shift);
    let (first_count_tile, count_tiles) = (first_cell / COUNT_TILE_CELLS, cells.div_ceil(COUNT_TILE_CELLS));
    (first_count_tile..first_count_tile + count_tiles).map(move |count_tile| (count_tile, cells.min(COUNT_TILE_CELLS) as u16))
}
