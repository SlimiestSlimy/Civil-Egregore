//! What the entities hold: how many, their attributes -- in use, and
//! left as garbage until swept -- and the wakes filed, good or not.

use crate::WorldEntities;

/// What the entities hold.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntityStats {
    /// Superchunks holding entities' buckets.
    pub superchunks: usize,
    /// Entities.
    pub entity_count: usize,
    /// Blocks of the attributes the entities have.
    pub attributes: usize,
    /// Blocks no entity has any longer, not yet swept out.
    pub garbage_blocks: usize,
    /// Wakes filed, good or not.
    pub wakes: usize,
}

impl EntityStats {
    /// What `world_entities` hold now.
    pub fn measure_entities(world_entities: &WorldEntities) -> Self {
        let mut entity_stats = Self { superchunks: world_entities.superchunks().len(), ..Self::default() };
        for superchunk in world_entities.superchunks() {
            let (attributes, garbage_blocks, wakes) = superchunk.counts();
            entity_stats.entity_count += superchunk.len();
            entity_stats.attributes += attributes;
            entity_stats.garbage_blocks += garbage_blocks;
            entity_stats.wakes += wakes;
        }
        entity_stats
    }
}
