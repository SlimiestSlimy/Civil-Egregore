//! A superchunk as cold storage holds it, in the cold pool and on disk
//! alike: one run of 64-bit words (`docs/chunk_storage.md`, "The
//! superchunk cold superchunk").

use utilities::coordinates::CHUNKS_IN_SUPERCHUNK;

/// Where the first chunk starts: after the chunk table.
const CHUNKS_START: usize = CHUNKS_IN_SUPERCHUNK;
/// Words a plane chunk table entry takes: its type and its offset.
const ENTRY_WORDS: usize = 2;

/// Why words are not a superchunk cold superchunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidColdSuperchunk(pub &'static str);

/// A change to a cold superchunk: the plane chunk of the type of ID `type_id` in the
/// chunk at `place` is now `encoded`, or is gone if `encoded` is empty.
#[derive(Clone, Copy, Debug)]
pub struct ColdPlaneChunkChange<'a> {
    /// The chunk's place in its superchunk.
    pub place: usize,
    /// The ID of the plane's type.
    pub type_id: u64,
    /// The plane chunk, encoded; empty for no plane chunk.
    pub encoded_plane_chunk: &'a [u64],
}

/// A superchunk's words: its chunk table, and its chunks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColdSuperchunk {
    /// Every word, as on disk.
    words: Box<[u64]>,
}

impl ColdSuperchunk {
    /// A superchunk with no plane chunks.
    pub fn empty() -> Self {
        Self { words: build_cold_superchunk_words(&Default::default()) }
    }

    /// `words` as a cold superchunk, if they are one: every offset inside its
    /// chunk, and each chunk's plane types sorted, each once.
    pub fn from_words(words: Box<[u64]>) -> Result<Self, InvalidColdSuperchunk> {
        if words.len() < CHUNKS_START {
            return Err(InvalidColdSuperchunk("shorter than its chunk table"));
        }
        let mut previous_end = CHUNKS_START;
        for chunk_place in 0..CHUNKS_IN_SUPERCHUNK {
            let chunk_start = words[chunk_place] as usize;
            if chunk_start != previous_end {
                return Err(InvalidColdSuperchunk("a chunk does not start where the one before ends"));
            }
            let chunk_end = if chunk_place + 1 < CHUNKS_IN_SUPERCHUNK { words[chunk_place + 1] as usize } else { words.len() };
            if chunk_end <= chunk_start || chunk_end > words.len() {
                return Err(InvalidColdSuperchunk("a chunk outside the cold superchunk"));
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

    /// The words of the chunk at `place`, its plane chunk count first.
    fn chunk(&self, place: usize) -> &[u64] {
        let chunk_end = if place + 1 < CHUNKS_IN_SUPERCHUNK { self.words[place + 1] as usize } else { self.words.len() };
        &self.words[self.words[place] as usize..chunk_end]
    }

    /// The encoded plane chunk of the type of ID `type_id` in the chunk at
    /// `place`, if it has one: its words from its first to its chunk's
    /// end, since its own end is where its stream ends.
    pub fn plane_chunk(&self, place: usize, type_id: u64) -> Option<&[u64]> {
        let words = self.chunk(place);
        let table = plane_chunk_table(words);
        let entry_index = table.binary_search_by_key(&type_id, |table_entry| table_entry[0]).ok()?;
        Some(&words[table[entry_index][1] as usize..])
    }

    /// The IDs of the types of the plane chunks of the chunk at `place`, sorted.
    #[cfg(test)]
    pub fn type_ids(&self, place: usize) -> impl Iterator<Item = u64> + '_ {
        plane_chunk_table(self.chunk(place)).iter().map(|table_entry| table_entry[0])
    }

    /// The cold superchunk with `plane_chunk_changes` made, in order: a later change to a
    /// plane chunk replaces an earlier one. Every encoded plane chunk is copied; this
    /// cold superchunk is not changed.
    pub fn rewritten_with(&self, plane_chunk_changes: &[ColdPlaneChunkChange]) -> Self {
        let mut chunks: [Vec<(u64, &[u64])>; CHUNKS_IN_SUPERCHUNK] = Default::default();
        for (place, plane_chunks) in chunks.iter_mut().enumerate() {
            *plane_chunks = exact_plane_chunks(self.chunk(place));
        }
        for plane_chunk_change in plane_chunk_changes {
            let plane_chunks = &mut chunks[plane_chunk_change.place];
            match (plane_chunks.binary_search_by_key(&plane_chunk_change.type_id, |&(type_id, _)| type_id), plane_chunk_change.encoded_plane_chunk.is_empty()) {
                (Ok(found_index), false) => plane_chunks[found_index].1 = plane_chunk_change.encoded_plane_chunk,
                (Ok(removed_index), true) => drop(plane_chunks.remove(removed_index)),
                (Err(insert_index), false) => plane_chunks.insert(insert_index, (plane_chunk_change.type_id, plane_chunk_change.encoded_plane_chunk)),
                (Err(_), true) => {}
            }
        }
        Self { words: build_cold_superchunk_words(&chunks) }
    }
}

/// A chunk's plane chunk table: a type and an offset per entry, sorted by
/// type.
fn plane_chunk_table(chunk: &[u64]) -> &[[u64; ENTRY_WORDS]] {
    let plane_chunks = chunk[0] as usize;
    let (table, _) = chunk[1..][..plane_chunks * ENTRY_WORDS].as_chunks::<ENTRY_WORDS>();
    table
}

/// Whether a chunk's words hold a plane chunk table that fits, and encoded
/// plane chunks inside the chunk, each at least a word.
fn check_chunk(chunk: &[u64]) -> Result<(), InvalidColdSuperchunk> {
    let plane_chunks = chunk[0] as usize;
    let header = plane_chunks.checked_mul(ENTRY_WORDS).and_then(|words| words.checked_add(1)).filter(|&words| words <= chunk.len());
    let header = header.ok_or(InvalidColdSuperchunk("a plane chunk table longer than its chunk"))?;
    let table = plane_chunk_table(chunk);
    if !table.windows(2).all(|type_pair| type_pair[0][0] < type_pair[1][0]) {
        return Err(InvalidColdSuperchunk("a plane chunk table not sorted by type, each type once"));
    }
    let mut plane_chunk_offsets: Vec<u64> = table.iter().map(|table_entry| table_entry[1]).collect();
    plane_chunk_offsets.sort_unstable();
    if plane_chunk_offsets.first().is_some_and(|&first_offset| (first_offset as usize) < header)
        || plane_chunk_offsets.last().is_some_and(|&last_offset| last_offset as usize >= chunk.len())
        || !plane_chunk_offsets.windows(2).all(|offset_pair| offset_pair[0] < offset_pair[1])
    {
        return Err(InvalidColdSuperchunk("an encoded plane chunk outside its chunk, or two at one offset"));
    }
    Ok(())
}

/// A chunk's encoded plane chunks, sorted by type, each one's words up to the
/// next one's start, or the chunk's end.
fn exact_plane_chunks(chunk: &[u64]) -> Vec<(u64, &[u64])> {
    let table = plane_chunk_table(chunk);
    let mut plane_chunk_starts: Vec<usize> = table.iter().map(|table_entry| table_entry[1] as usize).collect();
    plane_chunk_starts.sort_unstable();
    table
        .iter()
        .map(|&[type_id, plane_chunk_offset]| {
            let plane_chunk_start = plane_chunk_offset as usize;
            let next_index = plane_chunk_starts.partition_point(|&other_start| other_start <= plane_chunk_start);
            let plane_chunk_end = plane_chunk_starts.get(next_index).copied().unwrap_or(chunk.len());
            (type_id, &chunk[plane_chunk_start..plane_chunk_end])
        })
        .collect()
}

/// A cold superchunk's words: the chunk table, then each chunk's encoded
/// plane chunks, sorted by type.
fn build_cold_superchunk_words(chunks: &[Vec<(u64, &[u64])>; CHUNKS_IN_SUPERCHUNK]) -> Box<[u64]> {
    let chunk_words = |plane_chunks: &Vec<(u64, &[u64])>| 1 + plane_chunks.len() * ENTRY_WORDS + plane_chunks.iter().map(|(_, words)| words.len()).sum::<usize>();
    let mut chunk_start = CHUNKS_START;
    let mut words = Vec::with_capacity(chunk_start + chunks.iter().map(chunk_words).sum::<usize>());
    for plane_chunks in chunks {
        words.push(chunk_start as u64);
        chunk_start += chunk_words(plane_chunks);
    }
    for plane_chunks in chunks {
        words.push(plane_chunks.len() as u64);
        let mut plane_chunk_offset = 1 + plane_chunks.len() * ENTRY_WORDS;
        for &(type_id, encoded_plane_chunk) in plane_chunks {
            words.extend([type_id, plane_chunk_offset as u64]);
            plane_chunk_offset += encoded_plane_chunk.len();
        }
        for (_, plane_chunk_words_held) in plane_chunks {
            words.extend_from_slice(plane_chunk_words_held);
        }
    }
    words.into_boxed_slice()
}
