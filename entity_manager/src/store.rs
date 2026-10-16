//! Where entities are kept: a superchunk's ([`SuperchunkEntities`]),
//! every superchunk's ([`Entities`]), read in a tick by an
//! [`EntityReader`] (`docs/entity_manager.md`, "The store").

mod entity_reader;
mod named_in_a_tick;
mod world_entities;

pub use entity_reader::{EntityReader, OCCUPIED_SIDE};
pub use named_in_a_tick::CollisionCells;
pub use world_entities::WorldEntities;

use crate::bucket::{place, Bucket};
pub use crate::bucket::Put;
use crate::attributes::{append_kept, may_be_given, AttributeBlock, AttributeType};
use crate::entity::{EntityRef, Header, NEVER_WAKES};
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
    superchunk_index: SuperchunkIndex,
    /// The buckets, by chunk place ([`ChunkIndex::place`]).
    chunks: [Bucket; CHUNKS_IN_SUPERCHUNK],
    /// When each entity wakes.
    wheel: Wheel,
    /// Where each entity that left the cell the tick found it on
    /// stands now, by that cell -- its name for the tick: the cell it
    /// moved to, or none, removed. One that crossed a border is in
    /// both superchunks': where it left, the cell being another's,
    /// says it is no longer here; where it came to, its name -- a
    /// cell of the other -- finds it. Emptied when the tick is over
    /// ([`SuperchunkEntities::names_anew`]).
    left_this_tick: std::collections::HashMap<CellIndex, Option<CellIndex>>,
    /// The cells entities left, or were removed from, this tick:
    /// cleared in the collision plane when the tick is over
    /// ([`SuperchunkEntities::names_anew`]).
    vacated_cells: Vec<CellIndex>,
    /// Room for the attributes of an entity moving, with those it has,
    /// from one chunk's bucket to another's.
    carried_blocks: Vec<AttributeBlock>,
}

impl SuperchunkEntities {
    /// No entities, in `superchunk`.
    pub fn new(superchunk: SuperchunkIndex) -> Self {
        Self { superchunk_index: superchunk, chunks: Default::default(), wheel: Wheel::default(), left_this_tick: Default::default(), vacated_cells: Vec::new(), carried_blocks: Vec::new() }
    }

    /// Which superchunk it is.
    pub fn superchunk_index(&self) -> SuperchunkIndex {
        self.superchunk_index
    }

    /// How many entities it holds.
    pub fn len(&self) -> usize {
        self.chunks.iter().map(Bucket::len).sum()
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The entity standing on `stood_cell`, if one does.
    #[inline]
    pub fn get(&self, stood_cell: CellIndex) -> Option<EntityRef<'_>> {
        debug_assert_eq!(stood_cell.superchunk(), self.superchunk_index);
        self.chunks[stood_cell.chunk().place()].get(stood_cell)
    }

    /// Every entity it holds, in Morton order by cell.
    pub fn iter(&self) -> impl Iterator<Item = EntityRef<'_>> {
        self.chunks.iter().flat_map(Bucket::iter)
    }

    /// The entities on the chunk at `place` ([`ChunkIndex::place`]), in
    /// Morton order by cell.
    pub fn chunk(&self, place: usize) -> impl Iterator<Item = EntityRef<'_>> {
        self.chunks[place].iter()
    }

    /// The entities waking at `tick`, which is in reach of the wheel: in
    /// Morton order by cell, once every wake for it is filed
    /// and sorted -- as the tick sees them -- those no longer due passed
    /// over. Those to come are asked of memory ahead ([`ENTITY_AHEAD`]).
    pub fn woken_entities(&self, tick: u64) -> impl Iterator<Item = EntityRef<'_>> {
        self.woken_prefetching(tick, |_| {})
    }

    /// [`SuperchunkEntities::woken_entities`], `prefetch_cell` called with the cell of
    /// each entity [`ENTITY_AHEAD`] wakes before it is given: for
    /// whoever will read the cells about it to ask memory for them.
    pub fn woken_prefetching<'a>(&'a self, tick: u64, prefetch_cell: impl Fn(CellIndex) + 'a) -> impl Iterator<Item = EntityRef<'a>> {
        let due_wakes = self.wheel.due_wakes(tick);
        due_wakes[..due_wakes.len().min(ENTITY_AHEAD)].iter().for_each(|ahead_wake| prefetch_cell(ahead_wake.stood_cell));
        // The first have none before them to be asked for from.
        for ahead_wake in &due_wakes[..due_wakes.len().min(ENTITY_AHEAD)] {
            self.chunks[ahead_wake.stood_cell.chunk().place()].prefetch_entity(ahead_wake.stood_cell);
        }
        for ahead_wake in &due_wakes[..due_wakes.len().min(ATTRIBUTES_AHEAD)] {
            self.chunks[ahead_wake.stood_cell.chunk().place()].prefetch_attributes(ahead_wake.stood_cell);
        }
        due_wakes.iter().enumerate().filter_map(move |(wake_index, wake)| {
            if let Some(ahead_wake) = due_wakes.get(wake_index + ENTITY_AHEAD) {
                prefetch_cell(ahead_wake.stood_cell);
                self.chunks[ahead_wake.stood_cell.chunk().place()].prefetch_entity(ahead_wake.stood_cell);
            }
            if let Some(ahead_wake) = due_wakes.get(wake_index + ATTRIBUTES_AHEAD) {
                self.chunks[ahead_wake.stood_cell.chunk().place()].prefetch_attributes(ahead_wake.stood_cell);
            }
            self.get(wake.stood_cell).filter(|entity| entity.header.wake == tick)
        })
    }

    /// Makes `header`'s entity on its cell, with `attributes` sorted
    /// by type -- its type's default attribute among them or, not
    /// given, zeroes -- and files its wake, no earlier than
    /// `earliest_tick`: new, or refused, an entity standing there.
    pub(crate) fn make_entity(&mut self, earliest_tick: u64, header: Header, attributes: &[AttributeBlock]) -> Put {
        debug_assert_eq!(header.stood_cell.superchunk(), self.superchunk_index, "an entity made in a superchunk it is not in");
        debug_assert!(may_be_given(header.entity_type, attributes), "attributes whole, sorted by type, each type once");
        self.carried_blocks.clear();
        append_kept(header.entity_type, attributes, &mut self.carried_blocks);
        let put = self.chunks[header.stood_cell.chunk().place()].make_entity(header, &self.carried_blocks);
        if put == Put::NewlyPut && header.wake != NEVER_WAKES {
            self.wheel.file_wake(earliest_tick, header.wake, Wake { stood_cell: header.stood_cell });
        }
        put
    }

    /// Gives the entity standing on `from_cell` `header` -- its cell, when
    /// it wakes -- leaving it the attributes it has; and files its
    /// wake, if it is `earliest_tick` or later: what came of it
    /// (`docs/entity_manager.md`, "A chunk's bucket", Putting).
    pub(crate) fn put(&mut self, earliest_tick: u64, header: Header, from_cell: CellIndex) -> Put {
        debug_assert_eq!(header.stood_cell.superchunk(), self.superchunk_index, "an entity put in a superchunk it is not in");
        debug_assert_eq!(from_cell.superchunk(), self.superchunk_index, "an entity put from another superchunk: a crossing");
        let (origin, target_chunk_place) = (from_cell.chunk().place(), header.stood_cell.chunk().place());
        let put = if origin == target_chunk_place {
            self.chunks[target_chunk_place].put(header, place(from_cell), None)
        } else if let Some(stood_entity) = self.chunks[origin].get(from_cell) {
            // Those it has go with it to the other bucket, as that one kept them.
            self.carried_blocks.clear();
            self.carried_blocks.extend_from_slice(stood_entity.kept_blocks);
            self.move_between(origin, target_chunk_place, header, from_cell)
        } else {
            Put::PassedOver
        };
        let standing_cell = if put == Put::TurnedBack { from_cell } else { header.stood_cell };
        // A wake already passed is not filed again: whatever sets its next one files that.
        if !matches!(put, Put::RefusedOccupied | Put::PassedOver) && header.wake != NEVER_WAKES && header.wake >= earliest_tick {
            self.wheel.file_wake(earliest_tick, header.wake, Wake { stood_cell: standing_cell });
        }
        put
    }

    /// Moves `header`'s entity, standing on `from_cell` in the chunk at
    /// `origin`, to its cell in the chunk at `target_chunk_place`, with the
    /// attributes `carried` -- unless an entity stands there, when it
    /// stays, changed all the same.
    fn move_between(&mut self, origin: usize, target_chunk_place: usize, header: Header, from_cell: CellIndex) -> Put {
        if self.chunks[target_chunk_place].occupied(place(header.stood_cell)) {
            self.chunks[origin].put(Header { stood_cell: from_cell, ..header }, place(from_cell), Some(&self.carried_blocks));
            Put::TurnedBack
        } else {
            self.chunks[origin].remove(from_cell);
            self.chunks[target_chunk_place].make_entity(header, &self.carried_blocks);
            Put::MovedToCell
        }
    }

    /// Sets the attribute of type `attribute_type` of the entity standing on
    /// `stood_cell` to `whole_attribute`, or with none removes it: whether one stands
    /// there.
    pub(crate) fn edit(&mut self, stood_cell: CellIndex, attribute_type: AttributeType, whole_attribute: &[AttributeBlock]) -> bool {
        debug_assert_eq!(stood_cell.superchunk(), self.superchunk_index);
        self.chunks[stood_cell.chunk().place()].edit(place(stood_cell), attribute_type, whole_attribute)
    }

    /// The places, in the chunk at `chunk`, of the entities standing on
    /// the word tile whose first cell is at `first_place` there: a run of the
    /// bucket's places, a word tile being a run of cells in Morton order.
    pub(crate) fn in_word_tile(&self, chunk: usize, first_place: u16) -> &[u16] {
        self.chunks[chunk].in_word_tile(first_place)
    }

    /// Removes the entity standing on `stood_cell`: whether one did.
    pub(crate) fn remove(&mut self, stood_cell: CellIndex) -> bool {
        debug_assert_eq!(stood_cell.superchunk(), self.superchunk_index);
        self.chunks[stood_cell.chunk().place()].remove(stood_cell)
    }

    /// Whether an entity stands on `stood_cell`.
    pub fn occupied(&self, stood_cell: CellIndex) -> bool {
        debug_assert_eq!(stood_cell.superchunk(), self.superchunk_index);
        self.chunks[stood_cell.chunk().place()].occupied(place(stood_cell))
    }

    /// Passes `tick`, just run, on the wheel.
    pub fn pass_tick(&mut self, tick: u64) {
        self.wheel.pass_tick(tick);
    }

    /// Sorts the wakes of `tick`, every one filed, into Morton order.
    pub fn sort_wakes(&mut self, tick: u64) {
        self.wheel.sort_wakes(tick);
    }

    /// Attributes' blocks in use, those left as garbage, and wakes filed.
    pub(crate) fn counts(&self) -> (usize, usize, usize) {
        let (used_blocks, garbage_blocks) = self.chunks.iter().map(Bucket::attribute_counts).fold((0, 0), |(used_sum, garbage_sum), (bucket_used, bucket_garbage)| (used_sum + bucket_used, garbage_sum + bucket_garbage));
        (used_blocks, garbage_blocks, self.wheel.len())
    }
}
