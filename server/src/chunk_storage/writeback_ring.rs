//! The writeback ring: changed layers, encoded, on their way to the
//! cold pool (`docs/chunk_storage.md`, "The ring's words").

use utilities::coordinates::{ChunkIndex, SuperchunkIndex};
use protocol::type_registry::LayerType;
use std::ops::Range;

/// Words an entry's header takes: its chunk, its type, its length.
const HEADER_WORDS: usize = 3;
/// A header's first word that is no chunk's Morton index (a chunk's
/// takes 48 bits): the ring goes on at its start.
const WRAP_MARKER: u64 = u64::MAX;
/// The length word's bit saying an entry is dead.
const DEAD_BIT: u64 = 1;

/// A live entry of the ring.
#[derive(Clone, Debug)]
pub struct RingEntry {
    /// Its chunk's place in its superchunk.
    pub place: usize,
    /// Its layer's type.
    pub layer_type: LayerType,
    /// Where its encoded layer's words are in the ring.
    pub words: Range<usize>,
}

/// Changed layers, encoded, in the order written.
pub struct WritebackRing {
    /// The ring's words.
    words: Box<[u64]>,
    /// Where the next entry goes.
    head_word: usize,
    /// Where the oldest entry starts.
    tail_word: usize,
    /// How many entries, live or dead, are held.
    entry_count: usize,
}

impl WritebackRing {
    /// An empty ring of `capacity_in_words` words.
    pub fn new(capacity_in_words: usize) -> Self {
        Self { words: vec![0; capacity_in_words].into_boxed_slice(), head_word: 0, tail_word: 0, entry_count: 0 }
    }

    /// How many words the ring holds.
    pub fn capacity_in_words(&self) -> usize {
        self.words.len()
    }

    /// Whether no entry is held.
    pub fn is_empty(&self) -> bool {
        self.entry_count == 0
    }

    /// Where an entry of `entry_words` words would go, if it fits now.
    fn room_for(&self, entry_words: usize) -> Option<usize> {
        let capacity_in_words = self.words.len();
        if self.entry_count == 0 {
            (entry_words <= capacity_in_words).then_some(0)
        } else if self.head_word > self.tail_word {
            if self.head_word + entry_words <= capacity_in_words {
                Some(self.head_word)
            } else {
                (entry_words <= self.tail_word).then_some(0)
            }
        } else {
            (self.head_word + entry_words <= self.tail_word).then_some(self.head_word)
        }
    }

    /// Appends `encoded_layer` as the layer of `layer_type` in `chunk` (no
    /// words: no layer), if it fits. Returns whether it did.
    pub fn push(&mut self, chunk: ChunkIndex, layer_type: LayerType, encoded_layer: &[u64]) -> bool {
        let entry_words = HEADER_WORDS + encoded_layer.len();
        let Some(entry_at) = self.room_for(entry_words) else {
            return false;
        };
        if self.entry_count == 0 {
            self.tail_word = 0;
        } else if entry_at < self.head_word && self.head_word < self.words.len() {
            self.words[self.head_word] = WRAP_MARKER;
        }
        self.words[entry_at..entry_at + HEADER_WORDS].copy_from_slice(&[chunk.0, layer_type.type_id(), (encoded_layer.len() as u64) << 1]);
        self.words[entry_at + HEADER_WORDS..entry_at + entry_words].copy_from_slice(encoded_layer);
        self.head_word = entry_at + entry_words;
        self.entry_count += 1;
        true
    }

    /// Makes the ring empty and at least `capacity_in_words` words: only an empty
    /// ring grows, so nothing moves.
    pub fn grow_empty(&mut self, capacity_in_words: usize) {
        assert!(self.is_empty(), "only an empty ring grows");
        if capacity_in_words > self.words.len() {
            *self = Self::new(capacity_in_words);
        }
    }

    /// Where the entry at `entry_at` starts, past a wrap marker.
    fn entry_start(&self, entry_at: usize) -> usize {
        if entry_at == self.words.len() || self.words[entry_at] == WRAP_MARKER {
            0
        } else {
            entry_at
        }
    }

    /// Every entry, live or dead, oldest first: where each starts, its
    /// chunk, its type, its length and whether it is dead.
    fn all_entries(&self) -> impl Iterator<Item = (usize, ChunkIndex, LayerType, usize, bool)> + '_ {
        let mut next_at = self.tail_word;
        (0..self.entry_count).map(move |_| {
            let start_word = self.entry_start(next_at);
            let [chunk, layer_type, length_word] = [self.words[start_word], self.words[start_word + 1], self.words[start_word + 2]];
            let words = (length_word >> 1) as usize;
            next_at = start_word + HEADER_WORDS + words;
            (start_word, ChunkIndex(chunk), LayerType::bitplane_of_id(layer_type), words, length_word & DEAD_BIT != 0)
        })
    }

    /// The superchunk of the oldest live entry, if any.
    pub fn tail_superchunk(&self) -> Option<SuperchunkIndex> {
        let (_, chunk, ..) = self.all_entries().find(|&(.., is_dead)| !is_dead)?;
        Some(chunk.superchunk())
    }

    /// The live entries of `superchunk`, oldest first.
    pub fn entries_of(&self, superchunk: SuperchunkIndex) -> Vec<RingEntry> {
        self.all_entries()
            .filter(|&(_, chunk, .., is_dead)| !is_dead && chunk.superchunk() == superchunk)
            .map(|(start_word, chunk, layer_type, words, _)| RingEntry {
                place: chunk.place(),
                layer_type,
                words: start_word + HEADER_WORDS..start_word + HEADER_WORDS + words,
            })
            .collect()
    }

    /// An entry's encoded layer.
    pub fn encoded_layer(&self, ring_entry: &RingEntry) -> &[u64] {
        &self.words[ring_entry.words.clone()]
    }

    /// Frees every entry of `superchunk`: marks them dead, and moves the
    /// tail past the dead entries at it.
    pub fn release_superchunk(&mut self, superchunk: SuperchunkIndex) {
        let start_words: Vec<usize> = self.all_entries().filter(|&(_, chunk, ..)| chunk.superchunk() == superchunk).map(|(dead_start, ..)| dead_start).collect();
        for released_start in start_words {
            self.words[released_start + 2] |= DEAD_BIT;
        }
        while self.entry_count > 0 {
            let tail_start = self.entry_start(self.tail_word);
            let length_word = self.words[tail_start + 2];
            if length_word & DEAD_BIT == 0 {
                self.tail_word = tail_start;
                break;
            }
            self.tail_word = tail_start + HEADER_WORDS + (length_word >> 1) as usize;
            self.entry_count -= 1;
        }
        if self.entry_count == 0 {
            (self.head_word, self.tail_word) = (0, 0);
        }
    }
}
