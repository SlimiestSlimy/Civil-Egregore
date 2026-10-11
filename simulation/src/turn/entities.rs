//! The entities in a turn: those woken, any in reach read, and the
//! instructions queued for them.

use super::Turn;
use super::instruction::Instruction;
use entity_manager::{attribute_blocks, attribute_bytes, attribute_seen, each_attribute, Attribute, AttributeBlock, AttributeType, EntityRef, Header, Layout, Member, SuperchunkEntities, WholeAttribute, words_sum, ATTRIBUTE_WORDS_MOST, BLOCK_WORDS, NO_ATTRIBUTE, OCCUPIED_SIDE};
use bitplane_manager::Reader;
use chunk_storage::LayerType;
use coordinates::{CellIndex, ChunkIndex};

impl<'a> Turn<'a> {
    /// The superchunk's entities waking this tick, as the tick found
    /// them, in Morton order by cell -- the order their buckets hold
    /// them in. Each that is to wake
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

    /// The entity standing on `at` -- in any hot superchunk -- as the
    /// tick found it: the cell is its name for the tick.
    pub fn entity_on(&self, at: CellIndex) -> Option<EntityRef<'a>> {
        self.entity_reader.get(at)
    }

    /// The entities on `chunk` -- in any hot superchunk -- as the tick
    /// found them, in Morton order by cell; `None` if its
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

    /// Queues making `header`'s entity on its cell, with `attributes`,
    /// sorted by type, to wake after this tick: under a compare of
    /// the cell being free, in the queue of the superchunk it lands
    /// in -- the entity made, each attribute given it whole, a write
    /// in bulk. Its cell taken by then, none of it happens. What the rule queues until
    /// [`Turn::made`] is under that compare too: what is to happen
    /// only if the entity is made.
    pub fn put(&mut self, header: Header, attributes: &[AttributeBlock]) {
        self.make(header, attributes);
        self.made();
    }

    /// [`Turn::put`], the compare of its cell left open: what the
    /// rule queues for that superchunk from here to [`Turn::made`]
    /// happens only if the entity is made.
    pub fn make(&mut self, header: Header, attributes: &[AttributeBlock]) {
        debug_assert!(header.wake > self.now, "an entity put to wake at tick {}, not after {}", header.wake, self.now);
        let at = header.at;
        self.open(at, Instruction::compare_cell_free(at));
        self.queue(at, Instruction::make_entity(at, header.kind.0, header.wake));
        for attribute in each_attribute(attributes) {
            self.queue_attribute_whole(at, attribute);
        }
    }

    /// Queues the entity on `at` given `attribute` whole, its blocks
    /// carried beside the queue: under whatever compare is open.
    fn queue_attribute_whole(&mut self, at: CellIndex, attribute: &[AttributeBlock]) {
        let first = self.carry(at, attribute);
        self.queue(at, Instruction::write_attribute(at, first, attribute.len() as u32));
    }

    /// Closes the compare [`Turn::make`] left open.
    pub fn made(&mut self) {
        self.close_compare();
    }

    /// Queues `entity` stepping to `to` -- its own cell to sleep where
    /// it stands -- to wake at `wake`, its attributes as they are:
    /// under a compare of its being there still, a move, if it is to
    /// another cell, and its wake written to the wheel. If `to` is
    /// taken by then it stays, and wakes at `wake` all the same. To
    /// another superchunk it crosses ([`Turn::update`]).
    pub fn step(&mut self, entity: &Header, to: CellIndex, wake: u64) {
        debug_assert!(wake > self.now, "an entity put to wake at tick {wake}, not after {}", self.now);
        if entity.at.superchunk() == to.superchunk() {
            let (there, sleeps) = (Instruction::compare_entity(entity.at, entity.kind.0), Instruction::write_wake(entity.at, wake));
            if to == entity.at {
                self.queue_under(entity.at, there, &[sleeps]);
            } else {
                self.queue_under(entity.at, there, &[Instruction::move_entity(entity.at, to), sleeps]);
            }
        } else if let Some(whole) = self.entity_reader.get(entity.at) {
            self.cross(entity, Header { at: to, wake, ..*entity }, whole.attributes);
        }
    }

    /// Queues each of `members` that differs between `was` and `is`,
    /// the blocks of one attribute of the entity on `at`, as long as
    /// each other: a write a member, its bytes and no more. Under
    /// whatever compare is open.
    fn queue_members_differing(&mut self, at: CellIndex, members: impl Iterator<Item = Member>, was: &[AttributeBlock], is: &[AttributeBlock]) {
        let kind = is[0].kind();
        for member in members {
            let held = |blocks: &[AttributeBlock]| attribute_bytes(blocks, usize::from(member.offset), usize::from(member.bytes));
            if let Some(value) = held(is).filter(|&value| held(was) != Some(value)) {
                self.queue(at, Instruction::write_attribute_member(at, kind, member, value));
            }
        }
    }

    /// Queues the attribute `seen` of `entity` -- any in reach, the
    /// rule's own or another -- becoming `value`: both the blocks of
    /// one attribute, either none -- not there before, or removed. A
    /// compare of the attribute being still as seen, and under it the
    /// writes: the words that differ, each a member for want of its
    /// layout; or, its size another or it not there, the attribute
    /// whole; or it taken. So of two writing one attribute of one
    /// entity in a tick the first applied does and the other is
    /// refused, and two writing two attributes both do: an entity is
    /// written an attribute at a time, never whole, and no write
    /// lands on another's.
    pub fn set_attribute_blocks(&mut self, entity: &Header, seen: Option<&[AttributeBlock]>, value: Option<&[AttributeBlock]>) {
        let (at, kind) = (entity.at, value.or(seen).expect("an attribute seen or set")[0].kind());
        self.open(at, Instruction::compare_attribute(at, kind, attribute_seen(seen)));
        match (seen, value) {
            (_, None) => self.queue(at, Instruction::remove_attribute(at, kind)),
            (Some(seen), Some(value)) if seen.len() == value.len() => self.queue_members_differing(at, (0..value.len() * BLOCK_WORDS - 1).map(Member::word), seen, value),
            (_, Some(value)) => self.queue_attribute_whole(at, value),
        }
        self.close_compare();
    }

    /// Queues `member` of the attribute of type `kind` of `entity`
    /// becoming `value`, if it is still the `seen` the rule read
    /// there: a compare of that member alone, and the write under it.
    /// So two writing two members of one attribute in a tick both do,
    /// where a compare of the whole attribute would refuse the second.
    pub fn set_attribute_member(&mut self, entity: &Header, kind: AttributeType, member: Member, seen: u64, value: u64) {
        let at = entity.at;
        self.queue_under(at, Instruction::compare_attribute_member(at, kind, member, seen), &[Instruction::write_attribute_member(at, kind, member, value)]);
    }

    /// [`Turn::set_attribute_blocks`], of an attribute by its layout:
    /// `attribute` of `entity` set to `value`, if it is still the
    /// `seen` the rule read of it, or still has none. What is written
    /// is each member of the layout that differs, its bytes and no
    /// more; the attribute whole if it had none. Laid out where it
    /// is queued from: nothing is copied to be changed.
    pub fn set_attribute<L: Layout>(&mut self, entity: &Header, attribute: Attribute<L>, seen: Option<L>, value: L) {
        let Some(seen) = seen else {
            return self.set_attribute_blocks(entity, None, Some(&WholeAttribute::holding(attribute, value)));
        };
        // Both as their words, the type first: all a member is read from, and what the attribute sums to.
        let (at, kind, words) = (entity.at, attribute.attribute_type(), L::BLOCKS * BLOCK_WORDS);
        let (mut was, mut is) = ([0; ATTRIBUTE_WORDS_MOST], [0; ATTRIBUTE_WORDS_MOST]);
        (was[0], is[0]) = (kind.0, kind.0);
        seen.write(&mut was[1..words]);
        value.write(&mut is[1..words]);
        self.open(at, Instruction::compare_attribute(at, kind, words_sum(&was[..words]).max(NO_ATTRIBUTE + 1)));
        for &member in L::MEMBERS {
            let held = member.of(&is[1..words]);
            if member.of(&was[1..words]) != held {
                self.queue(at, Instruction::write_attribute_member(at, kind, member, held));
            }
        }
        self.close_compare();
    }

    /// Queues removing `attribute` of `entity`, if it is still the
    /// `seen` the rule read of it.
    pub fn unset_attribute<L: Layout>(&mut self, entity: &Header, attribute: Attribute<L>, seen: L) {
        self.set_attribute_blocks(entity, Some(&WholeAttribute::holding(attribute, seen)), None);
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
    /// then moved to its cell unless that is taken by then, and its
    /// wake written. It is never put whole over itself, so what
    /// another entity writes to it in the same tick is not lost. To
    /// another superchunk it crosses (`docs/simulation.md`,
    /// "Entities").
    pub fn update(&mut self, before: &Header, after: Header, attributes: &[AttributeBlock]) {
        debug_assert!(after.wake > self.now, "an entity put to wake at tick {}, not after {}", after.wake, self.now);
        let seen = self.entity_reader.get(before.at).map_or(&[][..], |seen| seen.attributes);
        self.set_attributes_changed(before, seen, attributes);
        if before.at.superchunk() == after.at.superchunk() {
            self.step(before, after.at, after.wake);
        } else {
            self.cross(before, after, attributes);
        }
    }

    /// Queues `before`'s entity crossing to `after`'s cell, in another
    /// superchunk, with `attributes` -- what it is to have by then,
    /// its changes queued where it stands: made there whole, under a
    /// compare of that cell being free, in that superchunk's queue;
    /// and here, under a compare of its being there still, put to
    /// sleep where it stands until its wake. The crossing stands
    /// only if it ends the tick here with those attributes -- else
    /// another changed it meanwhile, and it stays, asleep, with every
    /// change made to it (`SuperchunkEntities::settle_leavers`).
    fn cross(&mut self, before: &Header, after: Header, attributes: &[AttributeBlock]) {
        self.open(after.at, Instruction::compare_cell_free(after.at));
        // Its type and its wake in a block before its attributes': an instruction has no room for them beside the cell it left.
        let mut head = AttributeBlock([0; BLOCK_WORDS]);
        (head.0[0], head.0[1]) = (after.kind.0, after.wake);
        let first = self.carry(after.at, &[head]);
        self.carry(after.at, attributes);
        self.queue(after.at, Instruction::cross_entity(after.at, before.at, first, attributes.len() as u32));
        self.close_compare();
        self.queue_under(before.at, Instruction::compare_entity(before.at, before.kind.0), &[Instruction::write_wake(before.at, after.wake)]);
    }

    /// Queues removing `header`'s entity, if it is there still.
    pub fn remove(&mut self, header: &Header) {
        self.queue_under(header.at, Instruction::compare_entity(header.at, header.kind.0), &[Instruction::remove_entity(header.at)]);
    }
}
