//! A world's hash ([`world_hash`]): everything two worlds the same
//! hold the same, folded into a few words by nothing a machine could
//! do its own way (`docs/server.md`, "The same on every machine").

use crate::World;
use utilities::hash::{fold, fold_all};

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
    pub fn whole(&self, seed: u64) -> u64 {
        [self.tick, self.cells, self.entities, self.random_streams, self.halos, self.cold].into_iter().fold(fold(0, seed), fold)
    }
}

/// The hash of `world`. Its hot superchunks are written back and the
/// writeback ring flushed first (`World::write_back_and_flush_all`),
/// as a save does: a cold superchunk's image is behind its cells until
/// then. That changes nothing a tick reads.
pub fn world_hash(world: &mut World) -> WorldHash {
    world.write_back_and_flush_all();
    let mut cells = 0;
    for &layer_type in &world.info.layers {
        cells = fold(cells, layer_type.0);
        for (chunk, bucket) in world.arena.run(layer_type) {
            cells = fold_all(fold(fold(cells, chunk.0), u64::from(bucket.count())), bucket.words());
        }
    }
    let mut entities = fold(0, world.entities.len() as u64);
    for entity in world.entities.iter() {
        let header = entity.header;
        entities = [header.id.0, header.kind.0, header.at.0, header.wake, entity.attributes.len() as u64].into_iter().fold(entities, fold);
        entities = entity.attributes.iter().fold(entities, |hash, attribute| fold(fold(hash, attribute.kind.0), attribute.value));
    }
    let random_streams = world.simulation.random_states().fold(0, |hash, (superchunk, state)| fold(fold(hash, superchunk.0), state));
    let due = |hash: u64, (superchunk, tick): (coordinates::SuperchunkIndex, u64)| fold(fold(hash, superchunk.0), tick);
    let hot = world.arena.superchunk_indices().iter().fold(0, |hash, superchunk| fold(hash, superchunk.0));
    let halos = world.cooling().fold(fold(world.warming().fold(fold(hot, 0), due), 0), due);
    let mut cold = fold(0, world.cold.len() as u64);
    for (&superchunk, state) in &world.cold {
        cold = fold_all(fold(cold, superchunk.0), state);
        cold = fold_all(cold, world.storage.image(superchunk).map_or(&[][..], |image| image.words()));
    }
    WorldHash { tick: world.entities.now(), cells, entities, random_streams, halos, cold }
}
