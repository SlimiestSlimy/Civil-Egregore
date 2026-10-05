//! The entities in a turn: those woken, any in reach read, and the
//! instructions queued for them.

use super::{slot, Turn};
use entity_manager::{Attribute, AttributeType, EntityEdit, EntityId, EntityRef, EntityType, Header, SuperchunkEntities, OCCUPIED_SIDE};
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
    /// (each up to [`OCCUPIED_SIDE`]) whose top left cell is `origin` --
    /// in any hot superchunk -- as the tick found them, a row a word:
    /// cell `(x, y)` from `origin` at bit `x` of row `y`. Asked of the
    /// entities themselves, a few of them read, so it costs what it
    /// costs only when asked: a step onto a cell an entity stands on is
    /// turned back as it is applied, asked or not.
    pub fn occupied(&self, origin: CellIndex, width: u32, height: u32) -> [u16; OCCUPIED_SIDE] {
        self.entity_reader.occupied(origin, width, height)
    }

    /// A new entity's ID, drawn from the superchunk's random numbers.
    pub fn new_id(&mut self) -> EntityId {
        EntityId(self.random.draw())
    }

    /// Queues putting `header`'s entity -- made, or changed where it
    /// stands -- with `attributes`, sorted by type, in the superchunk
    /// its cell is in. It wakes at its wake tick, which is after this
    /// one. A new one whose cell another entity stands on by then is
    /// not put: entities never overlap. One that moves is
    /// [`Turn::update`]d.
    pub fn put(&mut self, header: Header, attributes: &[Attribute]) {
        debug_assert!(header.wake > self.now, "an entity put to wake at tick {}, not after {}", header.wake, self.now);
        let slot = self.slot_of(header.at.superchunk());
        self.outbox.instructions[slot].put(header, header.at, attributes);
    }

    /// Queues making an entity of type `kind` on `at`, with
    /// `attributes` sorted by type, to wake at `wake`: its ID, drawn
    /// here. It is not made if an entity stands on the cell by then.
    pub fn spawn(&mut self, kind: EntityType, at: CellIndex, wake: u64, attributes: &[Attribute]) -> EntityId {
        let id = self.new_id();
        self.put(Header { id, kind, at, wake }, attributes);
        id
    }

    /// Queues `entity` sleeping where it stands until `wake`: its
    /// attributes as they are, none carried.
    pub fn sleep(&mut self, entity: &Header, wake: u64) {
        self.step(entity, entity.at, wake);
    }

    /// Queues `entity` stepping to `to`, to wake at `wake`, its
    /// attributes as they are: none are carried, unless it crosses to
    /// another superchunk, where it goes whole
    /// ([`Turn::update`]). If an entity stands on `to` by then
    /// it stays where it stood, and wakes at `wake` all the same.
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

    /// Queues setting the attribute of type `kind` of `entity` -- any
    /// entity in reach, the rule's own or another -- to `value`. An
    /// entity changing itself whole does so by [`Turn::commit`];
    /// this is one entity acting on another: only the one attribute is
    /// written, so two acting on one in a tick do not undo each other.
    pub fn set_attribute(&mut self, entity: &Header, kind: AttributeType, value: u64) {
        let slot = self.slot_of(entity.at.superchunk());
        self.outbox.instructions[slot].edit(entity.id, entity.at, kind, Some(value));
    }

    /// Queues removing the attribute of type `kind` of `entity`, any in
    /// reach.
    pub fn unset_attribute(&mut self, entity: &Header, kind: AttributeType) {
        let slot = self.slot_of(entity.at.superchunk());
        self.outbox.instructions[slot].edit(entity.id, entity.at, kind, None);
    }

    /// Queues what `edit`'s entity came to: on `to`, to wake at `wake`,
    /// by the instruction that carries least -- moved or put to sleep
    /// with the attributes it has if none was changed, else put whole.
    pub fn commit(&mut self, edit: EntityEdit, to: CellIndex, wake: u64) {
        let before = *edit.header();
        if edit.edited() {
            self.update(&before, Header { at: to, wake, ..before }, edit.attributes());
        } else {
            self.step(&before, to, wake);
        }
    }

    /// Queues `before`'s entity becoming `after`, with `attributes`:
    /// changed, and moved to its cell if that is another -- unless an
    /// entity stands on it by then, when it stays where it stood,
    /// changed all the same: entities never overlap. One no longer
    /// where the tick found it is passed over.
    ///
    /// Moving to a cell of another superchunk, it crosses: it is put
    /// there as new, and changed here too, as if its cell were taken.
    /// Once the second phase is over, the one here is removed if the
    /// other was put ([`SuperchunkEntities::settle_leavers`]) -- so its cell
    /// is never left for one that cannot be had, and between ticks it
    /// stands on one cell.
    pub fn update(&mut self, before: &Header, after: Header, attributes: &[Attribute]) {
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
