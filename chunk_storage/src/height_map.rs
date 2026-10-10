//! A superchunk's heights: a floor a chunk and a byte a cell over it,
//! or a whole [`Height`] a cell where a chunk is tall
//! (`docs/chunk_storage.md`, "The height map").

use coordinates::{CELLS_IN_CHUNK, CHUNKS_IN_SUPERCHUNK};

/// A cell's height.
pub type Height = u16;

/// Floors a word holds, and a tall chunk's heights.
const HEIGHTS_IN_WORD: usize = (u64::BITS / Height::BITS) as usize;
/// Bytes a word holds: cells' heights over their floors.
const BYTES_IN_WORD: usize = 8;
/// Words the floors take.
const FLOOR_WORDS: usize = CHUNKS_IN_SUPERCHUNK / HEIGHTS_IN_WORD;
/// The word saying which chunks are tall.
const TALL_WORD: usize = FLOOR_WORDS;
/// Where the cells' bytes start.
const BYTES_START: usize = TALL_WORD + 1;
/// Words a superchunk's heights take with no chunk tall.
pub const HEIGHT_WORDS: usize = BYTES_START + CELLS_IN_CHUNK * CHUNKS_IN_SUPERCHUNK / BYTES_IN_WORD;
/// Words a tall chunk's map takes.
pub const TALL_WORDS: usize = CELLS_IN_CHUNK / HEIGHTS_IN_WORD;
/// The most a chunk spans, lowest to highest, with a byte a cell.
const SPAN: Height = u8::MAX as Height;

/// Words a superchunk's heights take, the first of them being `words`:
/// [`HEIGHT_WORDS`], and [`TALL_WORDS`] more a tall chunk.
pub fn words_of(words: &[u64]) -> usize {
    HEIGHT_WORDS + (words[TALL_WORD] & ((1 << CHUNKS_IN_SUPERCHUNK) - 1)).count_ones() as usize * TALL_WORDS
}

/// The floor of the chunk at `chunk` in the superchunk, from the
/// superchunk's height words.
fn floor_in(words: &[u64], chunk: usize) -> Height {
    (words[chunk / HEIGHTS_IN_WORD] >> ((chunk % HEIGHTS_IN_WORD) as u32 * Height::BITS)) as Height
}

/// The height of the cell at `place` in the superchunk
/// ([`coordinates::CellIndex::place_in_superchunk`]), from the
/// superchunk's height words: its chunk's floor and its byte, or what
/// its chunk's own map says if the chunk is tall.
pub fn height_in(words: &[u64], place: usize) -> Height {
    let (chunk, cell) = (place / CELLS_IN_CHUNK, place % CELLS_IN_CHUNK);
    let tall = words[TALL_WORD];
    if tall >> chunk & 1 == 1 {
        // Its map: after those of the tall chunks before it.
        let map = HEIGHT_WORDS + (tall & ((1 << chunk) - 1)).count_ones() as usize * TALL_WORDS;
        return (words[map + cell / HEIGHTS_IN_WORD] >> ((cell % HEIGHTS_IN_WORD) as u32 * Height::BITS)) as Height;
    }
    let byte = (words[BYTES_START + place / BYTES_IN_WORD] >> ((place % BYTES_IN_WORD) as u32 * u8::BITS)) as u8;
    floor_in(words, chunk) + byte as Height
}

/// A superchunk's heights, one a cell, in Morton order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeightMap {
    /// The floors, which chunks are tall, every cell's byte, and the
    /// tall chunks' maps: as an image holds them.
    words: Box<[u64]>,
}

impl HeightMap {
    /// Every cell at `height`.
    pub fn filled(height: Height) -> Self {
        Self::from_heights(|_| height)
    }

    /// The heights `height_at` gives, by a cell's place in the
    /// superchunk: each chunk's floor its lowest, and a chunk spanning
    /// more than a byte over it tall.
    pub fn from_heights(height_at: impl Fn(usize) -> Height) -> Self {
        let mut words = vec![0u64; HEIGHT_WORDS];
        for chunk in 0..CHUNKS_IN_SUPERCHUNK {
            let cells = chunk * CELLS_IN_CHUNK..(chunk + 1) * CELLS_IN_CHUNK;
            let (floor, top) = cells.clone().map(&height_at).fold((Height::MAX, 0), |(floor, top), height| (floor.min(height), top.max(height)));
            words[chunk / HEIGHTS_IN_WORD] |= (floor as u64) << ((chunk % HEIGHTS_IN_WORD) as u32 * Height::BITS);
            if top - floor <= SPAN {
                for place in cells {
                    words[BYTES_START + place / BYTES_IN_WORD] |= ((height_at(place) - floor) as u64) << ((place % BYTES_IN_WORD) as u32 * u8::BITS);
                }
                continue;
            }
            words[TALL_WORD] |= 1 << chunk;
            let map = words.len();
            words.resize(map + TALL_WORDS, 0);
            for (cell, place) in cells.enumerate() {
                words[map + cell / HEIGHTS_IN_WORD] |= (height_at(place) as u64) << ((cell % HEIGHTS_IN_WORD) as u32 * Height::BITS);
            }
        }
        Self { words: words.into_boxed_slice() }
    }

    /// The heights in `words`, as a superchunk image holds them.
    pub fn from_words(words: &[u64]) -> Self {
        assert!(words.len() >= HEIGHT_WORDS && words.len() == words_of(words), "a superchunk's heights");
        Self { words: words.into() }
    }

    /// The height of the cell at `place` in the superchunk.
    pub fn get(&self, place: usize) -> Height {
        height_in(&self.words, place)
    }

    /// The floor of the chunk at `chunk` in the superchunk: its lowest height.
    pub fn floor(&self, chunk: usize) -> Height {
        floor_in(&self.words, chunk)
    }

    /// Whether the chunk at `chunk` in the superchunk is tall: spans
    /// more than a byte, and has a map of whole heights.
    pub fn tall(&self, chunk: usize) -> bool {
        self.words[TALL_WORD] >> chunk & 1 == 1
    }

    /// Sets the height of the cell at `place` in the superchunk, if its
    /// chunk is not tall: to no less than the chunk's floor and no more
    /// than a byte over it.
    pub fn set(&mut self, place: usize, height: Height) {
        let chunk = place / CELLS_IN_CHUNK;
        let floor = self.floor(chunk);
        assert!(!self.tall(chunk) && (floor..=floor + SPAN).contains(&height), "a height its chunk cannot hold as it is");
        let (word, shift) = (BYTES_START + place / BYTES_IN_WORD, (place % BYTES_IN_WORD) as u32 * u8::BITS);
        self.words[word] = self.words[word] & !((u8::MAX as u64) << shift) | ((height - floor) as u64) << shift;
    }

    /// Every word, as an image holds them.
    pub fn words(&self) -> &[u64] {
        &self.words
    }
}

impl Default for HeightMap {
    /// Every cell at height 0.
    fn default() -> Self {
        Self { words: vec![0; HEIGHT_WORDS].into_boxed_slice() }
    }
}
