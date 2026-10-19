//! A superchunk as chunk storage holds it, in the cold pool and on disk
//! alike: one run of 64-bit words (`docs/chunk_storage.md`, "The
//! superchunk image").

use utilities::coordinates::CHUNKS_IN_SUPERCHUNK;
use protocol::type_registry::LayerType;

/// Where the first chunk starts: after the chunk table.
const CHUNKS_START: usize = CHUNKS_IN_SUPERCHUNK;
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
    pub encoded_layer: &'a [u64],
}

/// A superchunk's words: its chunk table, and its chunks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuperchunkImage {
    /// Every word, as on disk.
    words: Box<[u64]>,
}

impl SuperchunkImage {
    /// A superchunk with no layers.
    pub fn empty() -> Self {
        Self { words: build_image_words(&Default::default()) }
    }

    /// `words` as an image, if they are one: every offset inside its
    /// chunk, and each chunk's layer types sorted, each once.
    pub fn from_words(words: Box<[u64]>) -> Result<Self, InvalidImage> {
        if words.len() < CHUNKS_START {
            return Err(InvalidImage("shorter than its chunk table"));
        }
        let mut previous_end = CHUNKS_START;
        for chunk_place in 0..CHUNKS_IN_SUPERCHUNK {
            let chunk_start = words[chunk_place] as usize;
            if chunk_start != previous_end {
                return Err(InvalidImage("a chunk does not start where the one before ends"));
            }
            let chunk_end = if chunk_place + 1 < CHUNKS_IN_SUPERCHUNK { words[chunk_place + 1] as usize } else { words.len() };
            if chunk_end <= chunk_start || chunk_end > words.len() {
                return Err(InvalidImage("a chunk outside the image"));
            }
            check_chunk(&words[chunk_start..chunk_end])?;
            previous_end = chunk_end;
        }
        Ok(Self { words })
    }

    /// Every word, as on disk.
    pub fn words(&self) -> &[u64] {
        &self.words
    }

    /// The words of the chunk at `place`, its layer count first.
    fn chunk(&self, place: usize) -> &[u64] {
        let chunk_end = if place + 1 < CHUNKS_IN_SUPERCHUNK { self.words[place + 1] as usize } else { self.words.len() };
        &self.words[self.words[place] as usize..chunk_end]
    }

    /// The encoded layer of `layer_type` in the chunk at `place`, if it
    /// has one: its words from its first to its chunk's end, since its own
    /// end is where its stream ends.
    pub fn layer(&self, place: usize, layer_type: LayerType) -> Option<&[u64]> {
        let words = self.chunk(place);
        let table = layer_table(words);
        let entry_index = table.binary_search_by_key(&layer_type.type_id(), |table_entry| table_entry[0]).ok()?;
        Some(&words[table[entry_index][1] as usize..])
    }

    /// The types of the layers of the chunk at `place`, sorted.
    pub fn layer_types(&self, place: usize) -> impl Iterator<Item = LayerType> + '_ {
        layer_table(self.chunk(place)).iter().map(|table_entry| LayerType::bitplane_of_id(table_entry[0]))
    }

    /// The image with `layer_changes` made, in order: a later change to a
    /// layer replaces an earlier one. Every encoded layer is copied; this
    /// image is not changed.
    pub fn rewritten_with(&self, layer_changes: &[LayerChange]) -> Self {
        let mut chunks: [Vec<(LayerType, &[u64])>; CHUNKS_IN_SUPERCHUNK] = Default::default();
        for (place, layers) in chunks.iter_mut().enumerate() {
            *layers = exact_layers(self.chunk(place));
        }
        for layer_change in layer_changes {
            let layers = &mut chunks[layer_change.place];
            match (layers.binary_search_by_key(&layer_change.layer_type, |&(layer_type, _)| layer_type), layer_change.encoded_layer.is_empty()) {
                (Ok(found_index), false) => layers[found_index].1 = layer_change.encoded_layer,
                (Ok(removed_index), true) => drop(layers.remove(removed_index)),
                (Err(insert_index), false) => layers.insert(insert_index, (layer_change.layer_type, layer_change.encoded_layer)),
                (Err(_), true) => {}
            }
        }
        Self { words: build_image_words(&chunks) }
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
    if !table.windows(2).all(|type_pair| type_pair[0][0] < type_pair[1][0]) {
        return Err(InvalidImage("a layer table not sorted by type, each type once"));
    }
    let mut layer_offsets: Vec<u64> = table.iter().map(|table_entry| table_entry[1]).collect();
    layer_offsets.sort_unstable();
    if layer_offsets.first().is_some_and(|&first_offset| (first_offset as usize) < header)
        || layer_offsets.last().is_some_and(|&last_offset| last_offset as usize >= chunk.len())
        || !layer_offsets.windows(2).all(|offset_pair| offset_pair[0] < offset_pair[1])
    {
        return Err(InvalidImage("an encoded layer outside its chunk, or two at one offset"));
    }
    Ok(())
}

/// A chunk's encoded layers, sorted by type, each one's words up to the
/// next one's start, or the chunk's end.
fn exact_layers(chunk: &[u64]) -> Vec<(LayerType, &[u64])> {
    let table = layer_table(chunk);
    let mut layer_starts: Vec<usize> = table.iter().map(|table_entry| table_entry[1] as usize).collect();
    layer_starts.sort_unstable();
    table
        .iter()
        .map(|&[layer_type, layer_offset]| {
            let layer_start = layer_offset as usize;
            let next_index = layer_starts.partition_point(|&other_start| other_start <= layer_start);
            let layer_end = layer_starts.get(next_index).copied().unwrap_or(chunk.len());
            (LayerType::bitplane_of_id(layer_type), &chunk[layer_start..layer_end])
        })
        .collect()
}

/// An image's words: the chunk table, then each chunk's encoded
/// layers, sorted by type.
fn build_image_words(chunks: &[Vec<(LayerType, &[u64])>; CHUNKS_IN_SUPERCHUNK]) -> Box<[u64]> {
    let chunk_words = |layers: &Vec<(LayerType, &[u64])>| 1 + layers.len() * ENTRY_WORDS + layers.iter().map(|(_, words)| words.len()).sum::<usize>();
    let mut chunk_start = CHUNKS_START;
    let mut words = Vec::with_capacity(chunk_start + chunks.iter().map(chunk_words).sum::<usize>());
    for layers in chunks {
        words.push(chunk_start as u64);
        chunk_start += chunk_words(layers);
    }
    for layers in chunks {
        words.push(layers.len() as u64);
        let mut layer_offset = 1 + layers.len() * ENTRY_WORDS;
        for &(layer_type, encoded_layer) in layers {
            words.extend([layer_type.type_id(), layer_offset as u64]);
            layer_offset += encoded_layer.len();
        }
        for (_, layer_words) in layers {
            words.extend_from_slice(layer_words);
        }
    }
    words.into_boxed_slice()
}
