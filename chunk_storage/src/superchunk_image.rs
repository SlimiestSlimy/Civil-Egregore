//! A superchunk as chunk storage holds it, in the cold pool and on disk
//! alike: one run of 64-bit words, written to disk as it is in memory.
//!
//! | words | what they hold |
//! |---|---|
//! | 16 | the chunk table: each chunk's offset in the image, in Morton order |
//! | [`HEIGHT_WORDS`], and more if a chunk is tall | the superchunk's height map, raw ([`HeightMap`]) |
//! | 1, and a map for each chunk with water | the water's depths ([`ChunkMaps`]): a byte a cell in the chunks that have any, 16 bits in those deeper than 255 |
//! | the rest | each chunk in Morton order, its data together: its layer count, its layer table -- a type and an offset per layer, sorted by type -- then its encoded layers |
//!
//! An encoded layer's offset counts from its chunk's start, so a chunk
//! moves whole. Encoded layers start on a word and lie in no particular
//! order. No length is kept, since a Tessera stream ends itself: an
//! encoded layer runs from its offset to the next offset of its chunk,
//! or the chunk's end. A chunk runs to the next chunk's offset, the last
//! to the image's end.
//!
//! An image is never changed in place: changes to it make a new one
//! ([`SuperchunkImage::rewritten`]).

use coordinates::CHUNKS_IN_SUPERCHUNK;
use crate::chunk_maps::{self, number_in, ChunkMaps};
use crate::height_map::{height_in, words_of, Height, HeightMap, HEIGHT_WORDS};
use crate::layer_codec::LayerType;

/// Where the height map starts: after the chunk table.
const HEIGHTS_START: usize = CHUNKS_IN_SUPERCHUNK;
/// Where the first chunk starts at the least: after a height map with
/// no chunk tall, and the water's one word with no water.
const CHUNKS_START: usize = HEIGHTS_START + HEIGHT_WORDS + 1;
/// Words a layer table entry takes: its type and its offset.
const ENTRY_WORDS: usize = 2;

/// Why words are not a superchunk image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidImage(pub &'static str);

/// A change to an image: the layer of `layer_type` in the chunk at
/// `place` is now `encoded`, or is gone if `encoded` is empty.
#[derive(Clone, Copy, Debug)]
pub struct LayerChange<'a> {
    /// The chunk's place in its superchunk.
    pub place: usize,
    /// The layer's type.
    pub layer_type: LayerType,
    /// The layer, encoded; empty for no layer.
    pub encoded: &'a [u64],
}

/// A superchunk's words: its chunk table, its heights, its water's
/// depths, its chunks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuperchunkImage {
    /// Every word, as on disk.
    words: Box<[u64]>,
}

impl SuperchunkImage {
    /// A superchunk with `heights` and no layers.
    pub fn new(heights: &HeightMap) -> Self {
        Self { words: build(heights.words(), ChunkMaps::default().words(), &Default::default()) }
    }

    /// This image with `depths` its water's, its heights and layers
    /// as they are.
    pub fn with_water(&self, depths: &ChunkMaps) -> Self {
        let chunks: [Vec<(LayerType, &[u64])>; CHUNKS_IN_SUPERCHUNK] = std::array::from_fn(|place| exact_layers(self.chunk(place)));
        Self { words: build(self.height_words(), depths.words(), &chunks) }
    }

    /// `words` as an image, if they are one: every offset inside its
    /// chunk, and each chunk's layer types sorted, each once.
    pub fn from_words(words: Box<[u64]>) -> Result<Self, InvalidImage> {
        if words.len() < CHUNKS_START {
            return Err(InvalidImage("shorter than its chunk table and height map"));
        }
        // The height map ends past its tall chunks' maps.
        let mut previous_end = HEIGHTS_START + words_of(&words[HEIGHTS_START..]);
        if previous_end > words.len() {
            return Err(InvalidImage("shorter than its height map"));
        }
        // Then the water's depths, as long as their first word says: the first chunk starts where they end.
        if words.len() <= previous_end || words.len() < previous_end + chunk_maps::words_of(&words[previous_end..]) {
            return Err(InvalidImage("shorter than its water's depths"));
        }
        previous_end += chunk_maps::words_of(&words[previous_end..]);
        for index in 0..CHUNKS_IN_SUPERCHUNK {
            let start = words[index] as usize;
            if start != previous_end {
                return Err(InvalidImage("a chunk does not start where the one before ends"));
            }
            let end = if index + 1 < CHUNKS_IN_SUPERCHUNK { words[index + 1] as usize } else { words.len() };
            if end <= start || end > words.len() {
                return Err(InvalidImage("a chunk outside the image"));
            }
            check_chunk(&words[start..end])?;
            previous_end = end;
        }
        Ok(Self { words })
    }

    /// This image with `heights` its heights, its layers as they are.
    pub fn with_heights(&self, heights: &HeightMap) -> Self {
        let chunks: [Vec<(LayerType, &[u64])>; CHUNKS_IN_SUPERCHUNK] = std::array::from_fn(|place| exact_layers(self.chunk(place)));
        Self { words: build(heights.words(), self.water_words(), &chunks) }
    }

    /// Every word, as on disk.
    pub fn words(&self) -> &[u64] {
        &self.words
    }

    /// The superchunk's height map's words ([`HeightMap`]).
    pub fn height_words(&self) -> &[u64] {
        &self.words[HEIGHTS_START..][..words_of(&self.words[HEIGHTS_START..])]
    }

    /// The words of the water's depths ([`ChunkMaps`]): from the
    /// height map's end to the first chunk.
    pub fn water_words(&self) -> &[u64] {
        &self.words[HEIGHTS_START + words_of(&self.words[HEIGHTS_START..])..self.words[0] as usize]
    }

    /// How deep the water is over the cell at `place` in the
    /// superchunk: 0 where there is none.
    pub fn depth(&self, place: usize) -> Height {
        number_in(self.water_words(), place)
    }

    /// The height of the cell at `place` in the superchunk
    /// ([`coordinates::CellIndex::place_in_superchunk`]).
    pub fn height(&self, place: usize) -> Height {
        height_in(self.height_words(), place)
    }

    /// The words of the chunk at `place`, its layer count first.
    fn chunk(&self, place: usize) -> &[u64] {
        let end = if place + 1 < CHUNKS_IN_SUPERCHUNK { self.words[place + 1] as usize } else { self.words.len() };
        &self.words[self.words[place] as usize..end]
    }

    /// The encoded layer of `layer_type` in the chunk at `place`, if it
    /// has one: its words from its first to its chunk's end, since its own
    /// end is where its stream ends.
    pub fn layer(&self, place: usize, layer_type: LayerType) -> Option<&[u64]> {
        let words = self.chunk(place);
        let table = layer_table(words);
        let entry = table.binary_search_by_key(&layer_type.0, |entry| entry[0]).ok()?;
        Some(&words[table[entry][1] as usize..])
    }

    /// The types of the layers of the chunk at `place`, sorted.
    pub fn layer_types(&self, place: usize) -> impl Iterator<Item = LayerType> + '_ {
        layer_table(self.chunk(place)).iter().map(|entry| LayerType(entry[0]))
    }

    /// The image with `changes` made, in order: a later change to a
    /// layer replaces an earlier one. Every encoded layer is copied; this
    /// image is not changed.
    pub fn rewritten(&self, changes: &[LayerChange]) -> Self {
        let mut chunks: [Vec<(LayerType, &[u64])>; CHUNKS_IN_SUPERCHUNK] = Default::default();
        for (place, layers) in chunks.iter_mut().enumerate() {
            *layers = exact_layers(self.chunk(place));
        }
        for change in changes {
            let layers = &mut chunks[change.place];
            match (layers.binary_search_by_key(&change.layer_type, |&(layer_type, _)| layer_type), change.encoded.is_empty()) {
                (Ok(at), false) => layers[at].1 = change.encoded,
                (Ok(at), true) => drop(layers.remove(at)),
                (Err(at), false) => layers.insert(at, (change.layer_type, change.encoded)),
                (Err(_), true) => {}
            }
        }
        Self { words: build(self.height_words(), self.water_words(), &chunks) }
    }
}

/// A chunk's layer table: a type and an offset per entry, sorted by
/// type.
fn layer_table(chunk: &[u64]) -> &[[u64; ENTRY_WORDS]] {
    let layers = chunk[0] as usize;
    let (table, _) = chunk[1..][..layers * ENTRY_WORDS].as_chunks::<ENTRY_WORDS>();
    table
}

/// Whether a chunk's words hold a layer table that fits, and encoded
/// layers inside the chunk, each at least a word.
fn check_chunk(chunk: &[u64]) -> Result<(), InvalidImage> {
    let layers = chunk[0] as usize;
    let header = layers.checked_mul(ENTRY_WORDS).and_then(|words| words.checked_add(1)).filter(|&words| words <= chunk.len());
    let header = header.ok_or(InvalidImage("a layer table longer than its chunk"))?;
    let table = layer_table(chunk);
    if !table.windows(2).all(|pair| pair[0][0] < pair[1][0]) {
        return Err(InvalidImage("a layer table not sorted by type, each type once"));
    }
    let mut offsets: Vec<u64> = table.iter().map(|entry| entry[1]).collect();
    offsets.sort_unstable();
    if offsets.first().is_some_and(|&first| (first as usize) < header)
        || offsets.last().is_some_and(|&last| last as usize >= chunk.len())
        || !offsets.windows(2).all(|pair| pair[0] < pair[1])
    {
        return Err(InvalidImage("an encoded layer outside its chunk, or two at one offset"));
    }
    Ok(())
}

/// A chunk's encoded layers, sorted by type, each one's words up to the
/// next one's start, or the chunk's end.
fn exact_layers(chunk: &[u64]) -> Vec<(LayerType, &[u64])> {
    let table = layer_table(chunk);
    let mut starts: Vec<usize> = table.iter().map(|entry| entry[1] as usize).collect();
    starts.sort_unstable();
    table
        .iter()
        .map(|&[layer_type, offset]| {
            let start = offset as usize;
            let next = starts.partition_point(|&other| other <= start);
            let end = starts.get(next).copied().unwrap_or(chunk.len());
            (LayerType(layer_type), &chunk[start..end])
        })
        .collect()
}

/// An image's words: `heights`, the `water`'s depths if any, then each
/// chunk's encoded layers, sorted by type.
fn build(heights: &[u64], water: &[u64], chunks: &[Vec<(LayerType, &[u64])>; CHUNKS_IN_SUPERCHUNK]) -> Box<[u64]> {
    let chunk_words = |layers: &Vec<(LayerType, &[u64])>| 1 + layers.len() * ENTRY_WORDS + layers.iter().map(|(_, words)| words.len()).sum::<usize>();
    let mut start = HEIGHTS_START + heights.len() + water.len();
    let mut words = Vec::with_capacity(start + chunks.iter().map(chunk_words).sum::<usize>());
    for layers in chunks {
        words.push(start as u64);
        start += chunk_words(layers);
    }
    words.extend_from_slice(heights);
    words.extend_from_slice(water);
    for layers in chunks {
        words.push(layers.len() as u64);
        let mut offset = 1 + layers.len() * ENTRY_WORDS;
        for &(layer_type, encoded) in layers {
            words.extend([layer_type.0, offset as u64]);
            offset += encoded.len();
        }
        for (_, encoded) in layers {
            words.extend_from_slice(encoded);
        }
    }
    words.into_boxed_slice()
}
