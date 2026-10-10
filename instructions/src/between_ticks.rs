//! Entities put on the world between two ticks, on no superchunk's
//! turn: how a rule gives a world its first entities of a kind.

use entity_manager::{Attribute, Entities, Header};

/// The world's entities between two ticks: where a rule puts the ones a
/// world starts with. Whoever runs the world lends it, and places what
/// was put before the next tick.
pub struct EntitiesBetweenTicks<'a> {
    /// The world's entities.
    entities: &'a mut Entities,
}

impl<'a> EntitiesBetweenTicks<'a> {
    /// Those of a world whose entities are `entities`: made by whoever
    /// runs the world, never by a rule.
    pub fn of(entities: &'a mut Entities) -> Self {
        Self { entities }
    }

    /// The tick the world is at.
    pub fn now(&self) -> u64 {
        self.entities.now()
    }

    /// Queues a put of an entity whole, header and attributes: in the
    /// world once whoever runs it places what was put.
    pub fn put(&mut self, header: Header, attributes: &[Attribute]) {
        self.entities.queue_put(header, attributes);
    }
}
