//! Where entities are kept: a superchunk's ([`SuperchunkEntities`]),
//! every superchunk's ([`Entities`]), read in a tick by an
//! [`EntityReader`] (`docs/entity_manager.md`, "The store").

mod entity_reader;
mod world_entities;

pub use entity_reader::{EntityReader, OCCUPIED_SIDE};
pub use world_entities::Entities;

use crate::bucket::{place, Bucket, Put};
use crate::attributes::{sorted, AttributeBlock, AttributeType};
use crate::entity::{EntityId, EntityRef, Header, NEVER};
use crate::wheel::{Wake, Wheel};
use coordinates::{CellIndex, SuperchunkIndex, CHUNKS_IN_SUPERCHUNK};

/// How many wakes ahead an entity is asked of memory.
const ENTITY_AHEAD: usize = 8;
/// How many wakes ahead its attributes are: after the entity.
const ATTRIBUTES_AHEAD: usize = 4;

/// A superchunk's entities: a bucket a chunk, in the chunks' Morton
/// order, and when each wakes.
pub struct SuperchunkEntities {
    /// Which superchunk it is.
    index: SuperchunkIndex,
    /// The buckets, by chunk place ([`ChunkIndex::place`]).
    chunks: [Bucket; CHUNKS_IN_SUPERCHUNK],
    /// When each entity wakes.
    wheel: Wheel,
    /// The entities that crossed into it this tick, each with the cell
    /// it left in another superchunk: to be removed from there once
    /// the second phase is over ([`SuperchunkEntities::settle_leavers`]).
    arrived: Vec<(EntityId, CellIndex)>,
    /// Room for the attributes of an entity moving, with those it has,
    /// from one chunk's bucket to another's.
    carried: Vec<AttributeBlock>,
}

impl SuperchunkEntities {
    /// No entities, in `superchunk`.
    pub fn new(superchunk: SuperchunkIndex) -> Self {
        Self { index: superchunk, chunks: Default::default(), wheel: Wheel::default(), arrived: Vec::new(), carried: Vec::new() }
    }

    /// Which superchunk it is.
    pub fn index(&self) -> SuperchunkIndex {
        self.index
    }

    /// How many entities it holds.
    pub fn len(&self) -> usize {
        self.chunks.iter().map(Bucket::len).sum()
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The entity whose ID is `id`, standing on `at`.
    #[inline]
    pub fn get(&self, id: EntityId, at: CellIndex) -> Option<EntityRef<'_>> {
        debug_assert_eq!(at.superchunk(), self.index);
        self.chunks[at.chunk().place()].get(id, at)
    }

    /// Every entity it holds, in Morton order by cell, then by ID.
    pub fn iter(&self) -> impl Iterator<Item = EntityRef<'_>> {
        self.chunks.iter().flat_map(Bucket::iter)
    }

    /// The entities on the chunk at `place` ([`ChunkIndex::place`]), in
    /// Morton order by cell, then by ID.
    pub fn chunk(&self, place: usize) -> impl Iterator<Item = EntityRef<'_>> {
        self.chunks[place].iter()
    }

    /// The entities waking at `tick`, which is in reach of the wheel: in
    /// Morton order by cell, then by ID, once every wake for it is filed
    /// and sorted -- as the tick sees them -- those no longer due passed
    /// over. Those to come are asked of memory ahead ([`ENTITY_AHEAD`]).
    pub fn woken(&self, tick: u64) -> impl Iterator<Item = EntityRef<'_>> {
        self.woken_prefetching(tick, |_| {})
    }

    /// [`SuperchunkEntities::woken`], `prefetch` called with the cell of
    /// each entity [`ENTITY_AHEAD`] wakes before it is given: for
    /// whoever will read the cells about it to ask memory for them.
    pub fn woken_prefetching<'a>(&'a self, tick: u64, prefetch: impl Fn(CellIndex) + 'a) -> impl Iterator<Item = EntityRef<'a>> {
        let due = self.wheel.due(tick);
        due[..due.len().min(ENTITY_AHEAD)].iter().for_each(|ahead| prefetch(ahead.at));
        // The first have none before them to be asked for from.
        for ahead in &due[..due.len().min(ENTITY_AHEAD)] {
            self.chunks[ahead.at.chunk().place()].prefetch_entity(ahead.at);
        }
        for ahead in &due[..due.len().min(ATTRIBUTES_AHEAD)] {
            self.chunks[ahead.at.chunk().place()].prefetch_attributes(ahead.at);
        }
        due.iter().enumerate().filter_map(move |(at, wake)| {
            if let Some(ahead) = due.get(at + ENTITY_AHEAD) {
                prefetch(ahead.at);
                self.chunks[ahead.at.chunk().place()].prefetch_entity(ahead.at);
            }
            if let Some(ahead) = due.get(at + ATTRIBUTES_AHEAD) {
                self.chunks[ahead.at.chunk().place()].prefetch_attributes(ahead.at);
            }
            self.get(wake.id, wake.at).filter(|entity| entity.header.wake == tick)
        })
    }

    /// Puts `header`'s entity, which stood on `from`, with `attributes`
    /// sorted by type -- or, with none given, those it has -- and files
    /// its wake, no earlier than `earliest`: what came of it
    /// (`docs/entity_manager.md`, "A chunk's bucket", Putting).
    pub(crate) fn put(&mut self, earliest: u64, header: Header, from: CellIndex, attributes: Option<&[AttributeBlock]>) -> Put {
        debug_assert_eq!(header.at.superchunk(), self.index, "an entity put in a superchunk it is not in");
        debug_assert_eq!(from.superchunk(), self.index, "an entity put from another superchunk: a crossing");
        debug_assert!(attributes.is_none_or(sorted), "attributes whole, sorted by type, each type once");
        let (origin, target) = (from.chunk().place(), header.at.chunk().place());
        let put = if origin == target {
            self.chunks[target].put(header, place(from), attributes)
        } else if let Some(stood) = self.chunks[origin].get(header.id, from) {
            // Those it has go with it to the other bucket.
            self.carried.clear();
            self.carried.extend_from_slice(attributes.unwrap_or(stood.attributes));
            self.move_between(origin, target, header, from)
        } else {
            Put::PassedOver
        };
        let stands = if put == Put::Stayed { from } else { header.at };
        if !matches!(put, Put::Refused | Put::PassedOver) && header.wake != NEVER {
            self.wheel.file(earliest, header.wake, Wake { id: header.id, at: stands });
        }
        put
    }

    /// Moves `header`'s entity, standing on `from` in the chunk at
    /// `origin`, to its cell in the chunk at `target`, with the
    /// attributes `carried` -- unless an entity stands there, when it
    /// stays, changed all the same.
    fn move_between(&mut self, origin: usize, target: usize, header: Header, from: CellIndex) -> Put {
        if self.chunks[target].occupied(place(header.at)) {
            self.chunks[origin].put(Header { at: from, ..header }, place(from), Some(&self.carried));
            Put::Stayed
        } else {
            self.chunks[origin].remove(header.id, from);
            self.chunks[target].put(header, place(header.at), Some(&self.carried));
            Put::Moved
        }
    }

    /// Sets the attribute of type `kind` of the entity whose ID is `id`
    /// standing on `at` to `blocks`, or with none removes it: whether
    /// the entity is there.
    pub(crate) fn edit(&mut self, id: EntityId, at: CellIndex, kind: AttributeType, blocks: &[AttributeBlock]) -> bool {
        debug_assert_eq!(at.superchunk(), self.index);
        self.chunks[at.chunk().place()].edit(id, place(at), kind, blocks)
    }

    /// The places, in the chunk at `chunk`, of the entities standing on
    /// the word tile whose first cell is at `first` there: a run of the
    /// bucket's places, a word tile being a run of cells in Morton order.
    pub(crate) fn in_word_tile(&self, chunk: usize, first: u16) -> &[u16] {
        self.chunks[chunk].in_word_tile(first)
    }

    /// Removes the entity whose ID is `id` standing on `at`: whether it
    /// was there.
    pub(crate) fn remove(&mut self, id: EntityId, at: CellIndex) -> bool {
        debug_assert_eq!(at.superchunk(), self.index);
        self.chunks[at.chunk().place()].remove(id, at)
    }

    /// Notes that the entity whose ID is `id` crossed into this
    /// superchunk from `left`, a cell of another.
    pub(crate) fn arrived(&mut self, id: EntityId, left: CellIndex) {
        self.arrived.push((id, left));
    }

    /// Swaps the entities that crossed into it this tick, each with the
    /// cell it left, for `arrived` -- empty, kept for its room.
    pub fn take_arrived(&mut self, arrived: &mut Vec<(EntityId, CellIndex)>) {
        std::mem::swap(&mut self.arrived, arrived);
    }

    /// Removes from the cells they left those of `arrived` -- the
    /// entities that crossed into a neighbour this tick -- that left
    /// this superchunk (`docs/entity_manager.md`, "Instructions",
    /// Crossing a border).
    pub fn settle_leavers(&mut self, arrived: &[(EntityId, CellIndex)]) {
        let here = self.index;
        for &(id, left) in arrived.iter().filter(|(_, left)| left.superchunk() == here) {
            self.remove(id, left);
        }
    }

    /// Passes `tick`, just run, on the wheel.
    pub fn pass(&mut self, tick: u64) {
        self.wheel.pass(tick);
    }

    /// Sorts the wakes of `tick`, every one filed, into Morton order.
    pub fn sort_wakes(&mut self, tick: u64) {
        self.wheel.sort(tick);
    }

    /// Attributes' blocks in use, those left as garbage, and wakes filed.
    pub(crate) fn counts(&self) -> (usize, usize, usize) {
        let (used, garbage) = self.chunks.iter().map(Bucket::attribute_counts).fold((0, 0), |(a, b), (c, d)| (a + c, b + d));
        (used, garbage, self.wheel.len())
    }
}
