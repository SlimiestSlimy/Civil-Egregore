//! A world's hash ([`world_hash`]): everything two worlds the same
//! hold the same, folded into a few words by nothing a machine could
//! do its own way (`docs/server.md`, "The same on every machine").

use crate::World;
use utilities::hash::{fold_word, fold_all};

/// A world's hash, a part at a time: what differs between two worlds
/// says where they part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldHash {
    /// The tick it is at.
    pub tick: u64,
    /// Every hot bitmap: each layer type's, a chunk at a time, in
    /// Morton order -- the chunk, its count of cells set, its words.
    pub cells: u64,
    /// Every entity, in Morton order by cell, then by ID: its header
    /// and its attributes.
    pub entities: u64,
    /// Every hot superchunk's random stream.
    pub random_streams: u64,
    /// The superchunks hot, warming and cooling, with the tick each
    /// warming or cooling one is due.
    pub halos: u64,
    /// Every cold superchunk: its kept state and its image.
    pub cold: u64,
}

impl WorldHash {
    /// Every part folded into one word, the seed and the tick with
    /// them.
    pub fn whole_hash(&self, seed: u64) -> u64 {
        [self.tick, self.cells, self.entities, self.random_streams, self.halos, self.cold].into_iter().fold(fold_word(0, seed), fold_word)
    }
}

/// The hash of `world`. What went cold is flushed first
/// (`World::flush_what_went_cold`): a cold superchunk's image is
/// behind its cells until its write-back, encoded off the tick, is
/// in. The hot superchunks are hashed as they are, their cells
/// themselves, and not written back for it -- encoding them is a
/// save's work, and was most of what a hash cost. That changes
/// nothing a tick reads.
pub fn world_hash(world: &mut World) -> WorldHash {
    world.flush_what_went_cold();
    let mut cells = 0;
    for &layer_type in &world.world_info.layers {
        cells = fold_word(cells, layer_type.0);
        for (chunk, bucket) in world.bitmap_arena().buckets_of(layer_type) {
            cells = fold_all(fold_word(fold_word(cells, chunk.0), u64::from(bucket.count())), bucket.words());
        }
    }
    let mut entities_hash = fold_word(0, world.entity_store().len() as u64);
    for entity in world.entity_store().iter() {
        let header = entity.header;
        entities_hash = [header.entity_type.0, header.stood_cell.0, header.wake, entity.attribute_list().len() as u64].into_iter().fold(entities_hash, fold_word);
        entities_hash = entity.attribute_list().iter().flat_map(|attribute_block| attribute_block.0).fold(entities_hash, fold_word);
    }
    let random_streams = world.world_simulation().random_states().fold(0, |streams_hash, (superchunk, random_state)| fold_word(fold_word(streams_hash, superchunk.0), random_state));
    let fold_due_tick = |halos_hash: u64, (superchunk, tick): (utilities::coordinates::SuperchunkIndex, u64)| fold_word(fold_word(halos_hash, superchunk.0), tick);
    let hot = world.bitmap_arena().superchunk_indices().iter().fold(0, |hot_hash, superchunk| fold_word(hot_hash, superchunk.0));
    let halos = world.cooling().fold(fold_word(world.warming().fold(fold_word(hot, 0), fold_due_tick), 0), fold_due_tick);
    let mut cold = fold_word(0, world.cold().len() as u64);
    for (&superchunk, cold_state) in world.cold() {
        cold = fold_all(fold_word(cold, superchunk.0), cold_state);
        cold = fold_all(cold, world.chunk_storage().shared_image(superchunk).as_ref().map_or(&[][..], |image| image.words()));
    }
    WorldHash { tick: world.entity_store().current_tick(), cells, entities: entities_hash, random_streams, halos, cold }
}
