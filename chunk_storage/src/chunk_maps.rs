//! A number a cell kept only where there is any: a map a chunk with a
//! cell not 0, a byte a cell or 16 bits -- a superchunk's water
//! (`docs/chunk_storage.md`, "The superchunk image").

use coordinates::{CELLS_IN_CHUNK, CHUNKS_IN_SUPERCHUNK};

/// Words a chunk's map takes, a byte a cell; a wide one twice as many.
pub const MAP_WORDS: usize = CELLS_IN_CHUNK / 8;
/// Where in the first word the wide chunks' bits start.
const WIDE_FROM: u32 = CHUNKS_IN_SUPERCHUNK as u32;

/// The first word's bits for the chunks before `chunk`: those with a
/// map, and those wide.
fn before(first: u64, chunk: usize) -> (u64, u64) {
    let below = (1u64 << chunk) - 1;
    (first & below, first >> WIDE_FROM & below)
}

/// Words a superchunk's maps take, the first of them being `words`.
pub fn words_of(words: &[u64]) -> usize {
    let (kept, wide) = before(words[0], CHUNKS_IN_SUPERCHUNK);
    1 + (kept.count_ones() + wide.count_ones()) as usize * MAP_WORDS
}

/// The number of the cell at `place` in the superchunk
/// ([`coordinates::CellIndex::place_in_superchunk`]), from the
/// superchunk's maps' words: 0 where its chunk has no map.
pub fn number_in(words: &[u64], place: usize) -> u16 {
    let (chunk, cell) = (place / CELLS_IN_CHUNK, place % CELLS_IN_CHUNK);
    if words[0] >> chunk & 1 == 0 {
        return 0;
    }
    let (kept, wide) = before(words[0], chunk);
    let map = &words[1 + (kept.count_ones() + wide.count_ones()) as usize * MAP_WORDS..];
    if words[0] >> (WIDE_FROM + chunk as u32) & 1 == 1 {
        (map[cell / 4] >> (cell % 4 * 16)) as u16
    } else {
        (map[cell / 8] >> (cell % 8 * 8)) as u8 as u16
    }
}

/// A superchunk's numbers, one a cell, a map for each chunk with any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkMaps {
    /// Which chunks have maps and which are wide, then the maps: as an
    /// image holds them.
    words: Box<[u64]>,
}

impl ChunkMaps {
    /// The numbers `number_at` gives, by a cell's place in the
    /// superchunk: a map for each chunk with one not 0, wide if one is
    /// over 255.
    pub fn from_numbers(number_at: impl Fn(usize) -> u16) -> Self {
        let mut words = vec![0u64];
        for chunk in 0..CHUNKS_IN_SUPERCHUNK {
            let cells = chunk * CELLS_IN_CHUNK..(chunk + 1) * CELLS_IN_CHUNK;
            let most = cells.clone().map(&number_at).max().unwrap_or(0);
            if most == 0 {
                continue;
            }
            let (wide, map) = (most > u8::MAX as u16, words.len());
            words[0] |= 1 << chunk | (wide as u64) << (WIDE_FROM + chunk as u32);
            words.resize(map + MAP_WORDS * (1 + wide as usize), 0);
            let (in_word, bits) = if wide { (4, 16) } else { (8, 8) };
            for (cell, place) in cells.enumerate() {
                words[map + cell / in_word] |= (number_at(place) as u64) << (cell % in_word * bits);
            }
        }
        Self { words: words.into_boxed_slice() }
    }

    /// The number of the cell at `place` in the superchunk.
    pub fn get(&self, place: usize) -> u16 {
        number_in(&self.words, place)
    }

    /// Every word, as an image holds them.
    pub fn words(&self) -> &[u64] {
        &self.words
    }
}

impl Default for ChunkMaps {
    /// Every cell 0: no map.
    fn default() -> Self {
        Self { words: Box::new([0]) }
    }
}
