//! The entities in a turn: those woken, any in reach read, and the
//! instructions queued for them.

use super::conditional::Compare;
use super::{slot, Turn};
use entity_manager::{Attribute, AttributeBlock, EntityId, EntityRef, Header, Layout, SuperchunkEntities, OCCUPIED_SIDE};
use bitplane_manager::Reader;
use chunk_storage::LayerType;
use coordinates::{CellIndex, ChunkIndex};

impl<'a> Turn<'a> {
    /// The superchunk's entities waking this tick, as the tick found
    /// them, in Morton order by cell, then by ID -- the order their
    /// buckets hold them in. Each that is to wake
    /// again must be put back with a later wake tick.
    pub fn woken(&self) -> impl Iterator<Item = EntityRef<'a>> + 'a {
        let entities: &'a SuperchunkEntities = self.entities;
        entities.woken(self.now)
    }

    /// [`Turn::woken`], for a rule that reads the cells of
    /// `layers` about each entity woken: they are asked of memory a few
    /// wakes ahead.
    pub fn woken_reading<const N: usize>(&self, layers: [LayerType; N]) -> impl Iterator<Item = EntityRef<'a>> + 'a {
        let (entities, reader): (&'a SuperchunkEntities, &'a Reader<'a>) = (self.entities, self.reader);
        entities.woken_prefetching(self.now, move |cell| layers.iter().for_each(|&layer_type| reader.prefetch(layer_type, cell)))
    }

    /// The entity whose ID is `id`, standing on `at` -- in any hot
    /// superchunk -- as the tick found it.
    pub fn entity(&self, id: EntityId, at: CellIndex) -> Option<EntityRef<'a>> {
        self.entity_reader.get(id, at)
    }

    /// The entities on `chunk` -- in any hot superchunk -- as the tick
    /// found them, in Morton order by cell, then by ID; `None` if its
    /// superchunk is not hot.
    pub fn entities_in(&self, chunk: ChunkIndex) -> Option<impl Iterator<Item = EntityRef<'a>> + 'a> {
        self.entity_reader.chunk(chunk)
    }

    /// The cells entities stand on among the `width` by `height` cells
    /// (each up to [`OCCUPIED_SIDE`]) from `origin`, as the tick found
    /// them: cell `(x, y)` at bit `x` of row `y`
    /// (`docs/simulation.md`, "Entities").
    pub fn occupied(&self, origin: CellIndex, width: u32, height: u32) -> [u16; OCCUPIED_SIDE] {
        self.entity_reader.occupied(origin, width, height)
    }

    /// A new entity's ID, drawn from the superchunk's random numbers.
    pub fn new_id(&mut self) -> EntityId {
        EntityId(self.random.draw())
    }

    /// Queues putting `header`'s entity -- made, or changed where it
    /// stands -- with `attributes`, sorted by type, to wake after this
    /// tick. A new one whose cell is taken by then is not put.
    pub fn put(&mut self, header: Header, attributes: &[AttributeBlock]) {
        debug_assert!(header.wake > self.now, "an entity put to wake at tick {}, not after {}", header.wake, self.now);
        let slot = self.slot_of(header.at.superchunk());
        self.outbox.instructions[slot].put(header, header.at, attributes);
    }

    /// Queues putting `header`'s entity, new, with `attributes`, on its
    /// cell or, that taken by then, on the first free of `others` in
    /// its cell's superchunk: not put only if every one is taken.
    pub fn put_on_the_first_free(&mut self, header: Header, others: &[CellIndex], attributes: &[AttributeBlock]) {
        debug_assert!(header.wake > self.now, "an entity put to wake at tick {}, not after {}", header.wake, self.now);
        let slot = self.slot_of(header.at.superchunk());
        self.outbox.instructions[slot].put_on_the_first_free(header, others, attributes);
    }

    /// Queues `entity` stepping to `to` -- its own cell to sleep where
    /// it stands -- to wake at `wake`, no attribute carried unless it
    /// crosses to another superchunk. If `to` is taken by then it
    /// stays, and wakes at `wake` all the same.
    pub fn step(&mut self, entity: &Header, to: CellIndex, wake: u64) {
        debug_assert!(wake > self.now, "an entity put to wake at tick {wake}, not after {}", self.now);
        let after = Header { at: to, wake, ..*entity };
        if entity.at.superchunk() == to.superchunk() {
            let slot = self.slot_of(to.superchunk());
            self.outbox.instructions[slot].move_entity(after, entity.at);
        } else if let Some(whole) = self.entity_reader.get(entity.id, entity.at) {
            self.update(entity, after, whole.attributes);
        }
    }

    /// Queues setting `attribute` of `entity` -- another than the
    /// rule's own, any in reach -- to `value`, if it is still the
    /// `seen` the rule read of it, or it still has none: a
    /// compare-and-write, so of two setting one attribute in a tick
    /// the first applied does and the other is refused. Whether it was
    /// queued: not if the entity wakes this tick, for then it writes
    /// itself whole ([`Turn::update`]) from what it was, and one of the
    /// two would be lost -- it is asked again another tick. Of an
    /// attribute one block long.
    pub fn set_attribute<L: Layout>(&mut self, entity: &Header, attribute: Attribute<L>, seen: Option<L>, value: L) -> bool {
        if entity.wake <= self.now {
            return false;
        }
        let compare = Compare::Attribute { id: entity.id, at: entity.at, kind: attribute.attribute_type(), seen: seen.map(|seen| AttributeBlock::holding(attribute, seen)) };
        self.queue_instruction_if(compare, entity.at, |instructions| instructions.set_attribute(entity.id, entity.at, attribute, value));
        true
    }

    /// Queues removing `attribute` of `entity`, another in reach, if
    /// it is still the `seen` the rule read of it: whether it was
    /// queued, as [`Turn::set_attribute`] is.
    pub fn unset_attribute<L: Layout>(&mut self, entity: &Header, attribute: Attribute<L>, seen: L) -> bool {
        if entity.wake <= self.now {
            return false;
        }
        let kind = attribute.attribute_type();
        let compare = Compare::Attribute { id: entity.id, at: entity.at, kind, seen: Some(AttributeBlock::holding(attribute, seen)) };
        self.queue_instruction_if(compare, entity.at, |instructions| instructions.unset_attribute(entity.id, entity.at, kind));
        true
    }

    /// Queues `before`'s entity becoming `after`, with `attributes`:
    /// changed, and moved to its cell unless that is taken by then. To
    /// another superchunk it crosses (`docs/simulation.md`,
    /// "Entities").
    pub fn update(&mut self, before: &Header, after: Header, attributes: &[AttributeBlock]) {
        debug_assert!(after.wake > self.now, "an entity put to wake at tick {}, not after {}", after.wake, self.now);
        let there = self.slot_of(after.at.superchunk());
        if before.at.superchunk() == after.at.superchunk() {
            self.outbox.instructions[there].put(after, before.at, attributes);
        } else {
            self.outbox.instructions[there].cross(after, before.at, attributes);
            self.outbox.instructions[slot(0, 0)].put(Header { at: before.at, ..after }, before.at, attributes);
        }
    }

    /// Queues removing `header`'s entity.
    pub fn remove(&mut self, header: &Header) {
        let slot = self.slot_of(header.at.superchunk());
        self.outbox.instructions[slot].remove(header.id, header.at);
    }
}
