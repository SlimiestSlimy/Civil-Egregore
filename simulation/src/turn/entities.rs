//! The entities in a turn: those woken, any in reach read, and the
//! instructions queued for them.

use super::conditional::Compare;
use super::{slot, Turn};
use entity_manager::{attribute_blocks, each_attribute, push_attribute, Attribute, AttributeBlock, EntityId, EntityRef, Header, Layout, SuperchunkEntities, OCCUPIED_SIDE};
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
    /// it stands -- to wake at `wake`, its attributes as they are. If
    /// `to` is taken by then it stays, and wakes at `wake` all the
    /// same. To another superchunk it crosses ([`Turn::update`]).
    pub fn step(&mut self, entity: &Header, to: CellIndex, wake: u64) {
        debug_assert!(wake > self.now, "an entity put to wake at tick {wake}, not after {}", self.now);
        let after = Header { at: to, wake, ..*entity };
        if entity.at.superchunk() == to.superchunk() {
            let slot = self.slot_of(to.superchunk());
            self.outbox.instructions[slot].move_entity(after, entity.at);
        } else if let Some(whole) = self.entity_reader.get(entity.id, entity.at) {
            self.cross(entity, after, whole.attributes);
        }
    }

    /// Queues the attribute `seen` of `entity` -- any in reach, the
    /// rule's own or another -- becoming `value`: both the blocks of
    /// one attribute, either none -- not there before, or removed. A
    /// compare-and-write: applied only if the attribute is still as
    /// seen, and the compare the rule's instructions are under holds
    /// too. So of two writing one attribute of one entity in a tick the
    /// first applied does and the other is refused, and two writing two
    /// attributes both do: an entity is written an attribute at a
    /// time, never whole, and no write lands on another's.
    pub fn set_attribute_blocks(&mut self, entity: &Header, seen: Option<&[AttributeBlock]>, value: Option<&[AttributeBlock]>) {
        let kind = value.or(seen).expect("an attribute seen or set")[0].kind();
        let compare = Compare::attribute(entity.id, entity.at, kind, seen);
        self.queue_instruction_if(compare, entity.at, |instructions| match value {
            Some(value) => instructions.set_attribute_blocks(entity.id, entity.at, value),
            None => instructions.unset_attribute(entity.id, entity.at, kind),
        });
    }

    /// [`Turn::set_attribute_blocks`], of an attribute by its layout:
    /// `attribute` of `entity` set to `value`, if it is still the
    /// `seen` the rule read of it, or still has none.
    pub fn set_attribute<L: Layout>(&mut self, entity: &Header, attribute: Attribute<L>, seen: Option<L>, value: L) {
        let (mut was, mut is) = (Vec::new(), Vec::new());
        seen.into_iter().for_each(|seen| push_attribute(&mut was, attribute, seen));
        push_attribute(&mut is, attribute, value);
        self.set_attribute_blocks(entity, seen.map(|_| &was[..]), Some(&is));
    }

    /// Queues removing `attribute` of `entity`, if it is still the
    /// `seen` the rule read of it.
    pub fn unset_attribute<L: Layout>(&mut self, entity: &Header, attribute: Attribute<L>, seen: L) {
        let mut was = Vec::new();
        push_attribute(&mut was, attribute, seen);
        self.set_attribute_blocks(entity, Some(&was), None);
    }

    /// Queues every attribute that differs between `seen` and
    /// `attributes` -- both sorted by type -- of `entity` becoming as
    /// in `attributes`, each a compare-and-write of its own.
    fn set_attributes_changed(&mut self, entity: &Header, seen: &[AttributeBlock], attributes: &[AttributeBlock]) {
        for value in each_attribute(attributes) {
            let was = attribute_blocks(seen, value[0].kind());
            if was != Some(value) {
                self.set_attribute_blocks(entity, was, Some(value));
            }
        }
        for was in each_attribute(seen).filter(|was| attribute_blocks(attributes, was[0].kind()).is_none()) {
            self.set_attribute_blocks(entity, Some(was), None);
        }
    }

    /// Queues `before`'s entity becoming `after`, with `attributes`:
    /// each attribute that differs from what the tick found a
    /// compare-and-write of its own ([`Turn::set_attribute_blocks`]),
    /// then moved to its cell unless that is taken by then. It is
    /// never put whole over itself, so what another entity writes to
    /// it in the same tick is not lost. To another superchunk it
    /// crosses (`docs/simulation.md`, "Entities").
    pub fn update(&mut self, before: &Header, after: Header, attributes: &[AttributeBlock]) {
        debug_assert!(after.wake > self.now, "an entity put to wake at tick {}, not after {}", after.wake, self.now);
        let seen = self.entity_reader.get(before.id, before.at).map_or(&[][..], |seen| seen.attributes);
        self.set_attributes_changed(before, seen, attributes);
        if before.at.superchunk() == after.at.superchunk() {
            let slot = self.slot_of(after.at.superchunk());
            self.outbox.instructions[slot].move_entity(after, before.at);
        } else {
            self.cross(before, after, attributes);
        }
    }

    /// Queues `before`'s entity crossing to `after`'s cell, in another
    /// superchunk, with `attributes` -- what it is to have by then,
    /// its changes queued where it stands: put there whole, and here
    /// put to sleep where it stands until its wake. The crossing
    /// stands only if it ends the tick here with those attributes --
    /// else another changed it meanwhile, and it stays, asleep, with
    /// every change made to it (`SuperchunkEntities::settle_leavers`).
    fn cross(&mut self, before: &Header, after: Header, attributes: &[AttributeBlock]) {
        let there = self.slot_of(after.at.superchunk());
        self.outbox.instructions[there].cross(after, before.at, attributes);
        self.outbox.instructions[slot(0, 0)].move_entity(Header { at: before.at, ..after }, before.at);
    }

    /// Queues removing `header`'s entity.
    pub fn remove(&mut self, header: &Header) {
        let slot = self.slot_of(header.at.superchunk());
        self.outbox.instructions[slot].remove(header.id, header.at);
    }
}
