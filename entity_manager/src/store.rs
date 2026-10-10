//! Where entities are kept: a superchunk's ([`SuperchunkEntities`]),
//! every superchunk's ([`Entities`]), read in a tick by an
//! [`EntityReader`] (`docs/entity_manager.md`, "The store").

mod entity_reader;
mod named_in_a_tick;
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

/// An entity that crossed into a superchunk in a tick: put there, and
/// still standing where it left until the crossing is settled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arrival {
    /// Its ID.
    pub id: EntityId,
    /// The cell it left, in another superchunk.
    pub left: CellIndex,
    /// The cell it was put on.
    pub at: CellIndex,
    /// The sum of the attributes it was put with
    /// ([`blocks_sum`](crate::blocks_sum)): the crossing stands only
    /// if the one it left behind ended the tick with the same.
    pub attributes: u64,
}

/// What a superchunk settled of the entities that left it over a
/// border in a tick ([`SuperchunkEntities::settle_leavers`]), for the
/// neighbours they were put in.
#[derive(Default)]
pub struct Settled {
    /// Those removed where they left meanwhile: the crossing does not
    /// stand.
    pub turned_back: Vec<Arrival>,
    /// Those that ended the tick with other attributes than they were
    /// put with: each with its attributes' first block in
    /// `attributes`, and how many.
    pub changed: Vec<(Arrival, u32, u32)>,
    /// The attributes of those changed.
    pub attributes: Vec<AttributeBlock>,
}

impl Settled {
    /// Whether nothing is to be settled.
    pub fn is_empty(&self) -> bool {
        self.turned_back.is_empty() && self.changed.is_empty()
    }

    /// Empties it, keeping its room.
    pub fn clear(&mut self) {
        self.turned_back.clear();
        self.changed.clear();
        self.attributes.clear();
    }
}

/// A superchunk's entities: a bucket a chunk, in the chunks' Morton
/// order, and when each wakes.
pub struct SuperchunkEntities {
    /// Which superchunk it is.
    index: SuperchunkIndex,
    /// The buckets, by chunk place ([`ChunkIndex::place`]).
    chunks: [Bucket; CHUNKS_IN_SUPERCHUNK],
    /// When each entity wakes.
    wheel: Wheel,
    /// The entities that crossed into it this tick: each to be removed
    /// from the cell it left in another superchunk once the second
    /// phase is over, or turned back
    /// ([`SuperchunkEntities::settle_leavers`]).
    arrived: Vec<Arrival>,
    /// Where each entity that left the cell the tick found it on
    /// stands now, by that cell -- its name for the tick: the cell it
    /// moved to, or none, removed. Emptied when the tick is over
    /// ([`SuperchunkEntities::names_anew`]).
    left_this_tick: std::collections::HashMap<CellIndex, Option<CellIndex>>,
    /// Room for the attributes of an entity moving, with those it has,
    /// from one chunk's bucket to another's.
    carried: Vec<AttributeBlock>,
}

impl SuperchunkEntities {
    /// No entities, in `superchunk`.
    pub fn new(superchunk: SuperchunkIndex) -> Self {
        Self { index: superchunk, chunks: Default::default(), wheel: Wheel::default(), arrived: Vec::new(), left_this_tick: Default::default(), carried: Vec::new() }
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

    /// Whether an entity stands on `at`.
    pub fn occupied(&self, at: CellIndex) -> bool {
        debug_assert_eq!(at.superchunk(), self.index);
        self.chunks[at.chunk().place()].occupied(place(at))
    }

    /// Notes that an entity crossed into this superchunk.
    pub(crate) fn arrived(&mut self, arrival: Arrival) {
        self.arrived.push(arrival);
    }

    /// Swaps the entities that crossed into it this tick for `arrived`
    /// -- empty, kept for its room.
    pub fn take_arrived(&mut self, arrived: &mut Vec<Arrival>) {
        std::mem::swap(&mut self.arrived, arrived);
    }

    /// Settles those of `arrived` -- the entities that crossed into a
    /// neighbour this tick -- that left this superchunk, where each
    /// stood, written to, until now: it is removed from the cell it
    /// left, and what it has, if not what it was put there with, goes
    /// into `settled` for the neighbour to give it -- every attribute
    /// written to it in the tick crosses with it. One removed here
    /// meanwhile goes there as turned back, for the neighbour to take
    /// back what it put (`docs/entity_manager.md`, "Instructions",
    /// Crossing a border).
    pub fn settle_leavers(&mut self, arrived: &[Arrival], settled: &mut Settled) {
        let here = self.index;
        for &arrival in arrived.iter().filter(|arrival| arrival.left.superchunk() == here) {
            match self.get(arrival.id, arrival.left) {
                Some(left) => {
                    if crate::blocks_sum(left.attributes) != arrival.attributes {
                        let first = settled.attributes.len() as u32;
                        settled.attributes.extend_from_slice(left.attributes);
                        settled.changed.push((arrival, first, left.attributes.len() as u32));
                    }
                    self.remove(arrival.id, arrival.left);
                }
                None => settled.turned_back.push(arrival),
            }
        }
    }

    /// Settles, of what a neighbour `settled`, those put in this
    /// superchunk: one changed where it left is given what it has; one
    /// turned back is removed ([`SuperchunkEntities::settle_leavers`]).
    /// How many were turned back.
    pub fn settle_arrivals(&mut self, settled: &Settled) -> usize {
        let here = self.index;
        for &(arrival, first, count) in settled.changed.iter().filter(|(arrival, ..)| arrival.at.superchunk() == here) {
            if let Some(header) = self.get(arrival.id, arrival.at).map(|put| put.header) {
                self.chunks[arrival.at.chunk().place()].put(header, place(arrival.at), Some(&settled.attributes[first as usize..(first + count) as usize]));
            }
        }
        settled.turned_back.iter().filter(|arrival| arrival.at.superchunk() == here && self.remove(arrival.id, arrival.at)).count()
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
