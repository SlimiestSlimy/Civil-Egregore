//! Compares, and the writes under them. A write is a method of a
//! skip-tag set open ([`SkipTagSet`]), so none can be queued under no
//! compare: a rule opens a set with its first compare, chains as many
//! as it holds against, writes, and the set's skip tag is placed as
//! it goes out of scope. A compare that does not hold skips to that
//! tag; a set opened within another has a tag of its own, placed
//! first (`docs/simulation_api.md`, "Compare-and-write").

use crate::raw_compares_and_writes::{entity as entity_writes, write as raw_writes};
pub use crate::raw_compares_and_writes::compare::{attribute, attribute_blocks, attribute_member, cell_free, entity, layer_holds, layer_lacks, value, with_writes};
pub use crate::{Compare, Write};
use crate::SimulationTickState;
use chunk_storage::LayerType;
use utilities::coordinates::CellIndex;
use entity_manager::{each_attribute, AttributeBlock, AttributeType, Header, Member};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

/// A skip-tag set open on a turn: all queued while it lives is under
/// every compare of it, and skipped up to its skip tag if one does not
/// hold when it is come to. The writes are its methods. It derefs to
/// the turn, for reads and for actions, which queue under it as well:
/// a set they open is within it. Dropped, it places its tag.
#[must_use = "dropped at once, the tag is placed before anything is written under the set"]
pub struct SkipTagSet<'scope, 'world> {
    /// The turn the set is open on.
    tick_state: &'scope mut SimulationTickState<'world>,
    /// Its depth: how many sets it is within.
    depth: usize,
}

/// The skip tag of a set open, to point a set within it at: a
/// compare there that does not hold skips to it, out of every set
/// between. It lives no longer than its set, so it is always of one
/// enclosing the set pointed at it.
#[derive(Clone, Copy)]
pub struct SkipTag<'scope> {
    /// Its set's depth.
    depth: usize,
    /// Its set's borrow of the turn: nothing outlives the set.
    set_open: PhantomData<&'scope ()>,
}

/// Opens a skip-tag set on `turn` with `first_compare`, within whatever sets
/// are open already: what is queued until the returned set is dropped
/// is under it.
#[inline]
pub fn open_compare<'scope, 'world>(tick_state: &'scope mut SimulationTickState<'world>, first_compare: Compare) -> SkipTagSet<'scope, 'world> {
    tick_state.skip_tag_set(first_compare);
    let depth = tick_state.skip_tag_sets_open() - 1;
    SkipTagSet { tick_state, depth }
}

impl<'scope> SkipTagSet<'scope, '_> {
    /// Its skip tag, to point a set within it at.
    #[inline]
    pub fn skip_tag(&self) -> SkipTag<'scope> {
        SkipTag { depth: self.depth, set_open: PhantomData }
    }

    /// Points the compares chained to it from here at `enclosing_tag` --
    /// its own tag, or one of a set it is within: one that does not hold
    /// skips there, out of every set between.
    #[inline]
    pub fn skip_to(&mut self, enclosing_tag: SkipTag) {
        self.tick_state.existing_skip_tag_set(self.depth - enclosing_tag.depth);
    }

    /// `chained_compare` queued in the set as well: chained to the others,
    /// skipping to the same tag.
    #[inline]
    pub fn and_compare(mut self, chained_compare: Compare) -> Self {
        self.chain_compare(chained_compare);
        self
    }

    /// [`SkipTagSet::and_compare`] on a set handed down: a callee's compare
    /// skipping to its caller's tag. Queued after writes, it holds
    /// those after it alone.
    #[inline]
    pub fn chain_compare(&mut self, chained_compare: Compare) {
        self.tick_state.compare_chained(chained_compare);
    }

    /// Queues the number of `layer_type` at `cell` becoming `value` -- 1 or
    /// 0 on a layer of a bit a cell.
    #[inline]
    pub fn write_cell(&mut self, layer_type: LayerType, cell: CellIndex, value: u32) {
        raw_writes::cell(self.tick_state, layer_type, cell, value, None);
    }

    /// [`SkipTagSet::write_cell`], one added to the rule's count
    /// `count_place` as it is applied.
    #[inline]
    pub fn write_cell_counted(&mut self, layer_type: LayerType, cell: CellIndex, value: u32, count_place: usize) {
        raw_writes::cell(self.tick_state, layer_type, cell, value, Some(count_place));
    }

    /// Queues one added to the rule's count `count_place`.
    #[inline]
    pub fn count(&mut self, count_place: usize) {
        raw_writes::count(self.tick_state, count_place);
    }

    /// Queues `header`'s entity made, with `attributes` sorted by type.
    #[inline]
    pub fn make_entity(&mut self, header: Header, attributes: &[AttributeBlock]) {
        entity_writes::queue_entity_made(self.tick_state, header);
        for attribute in each_attribute(attributes) {
            raw_writes::attribute_whole(self.tick_state, header.stood_cell, attribute);
        }
    }

    /// Queues the entity named `from_cell` this tick moving to `to_cell`, a
    /// cell of its superchunk or of another.
    #[inline]
    pub fn move_entity(&mut self, from_cell: CellIndex, to_cell: CellIndex) {
        entity_writes::move_to(self.tick_state, from_cell, to_cell);
    }

    /// Queues the entity named `named_cell` this tick waking next at `wake`.
    #[inline]
    pub fn wake_entity(&mut self, named_cell: CellIndex, wake: u64) {
        raw_writes::wake(self.tick_state, named_cell, wake);
    }

    /// Queues the entity named `named_cell` this tick removed.
    #[inline]
    pub fn remove_entity(&mut self, named_cell: CellIndex) {
        entity_writes::remove(self.tick_state, named_cell);
    }

    /// Queues `member` of the attribute of type `attribute_type` of the
    /// entity named `named_cell` becoming `value`.
    #[inline]
    pub fn write_attribute_member(&mut self, named_cell: CellIndex, attribute_type: AttributeType, member: Member, value: u64) {
        raw_writes::attribute_member(self.tick_state, named_cell, attribute_type, member, value);
    }

    /// Queues the entity named `named_cell` -- or made on it under the
    /// set -- given `attribute` whole: the blocks of one attribute.
    #[inline]
    pub fn write_attribute(&mut self, named_cell: CellIndex, attribute: &[AttributeBlock]) {
        raw_writes::attribute_whole(self.tick_state, named_cell, attribute);
    }

    /// Queues the attribute of type `attribute_type` taken from the entity
    /// named `named_cell`.
    #[inline]
    pub fn remove_attribute(&mut self, named_cell: CellIndex, attribute_type: AttributeType) {
        raw_writes::attribute_removed(self.tick_state, named_cell, attribute_type);
    }
}

impl<'world> Deref for SkipTagSet<'_, 'world> {
    type Target = SimulationTickState<'world>;

    /// The turn, to read.
    #[inline]
    fn deref(&self) -> &SimulationTickState<'world> {
        self.tick_state
    }
}

impl<'world> DerefMut for SkipTagSet<'_, 'world> {
    /// The turn, to hand to an action: what it queues is under the set
    /// too.
    #[inline]
    fn deref_mut(&mut self) -> &mut SimulationTickState<'world> {
        self.tick_state
    }
}

impl Drop for SkipTagSet<'_, '_> {
    /// Places the set's skip tag.
    #[inline]
    fn drop(&mut self) {
        self.tick_state.skip_tag_placed();
    }
}

/// Queues the number of `layer_type` at `cell` becoming `value` if
/// it is still the `value_seen` the rule read there and `under_compare` holds too
/// when the write is come to: one added to the rule's count `count_place`
/// then, if one is given. The compare is of the cell's superchunk.
#[inline]
pub fn write(tick_state: &mut SimulationTickState, under_compare: Compare, layer_type: LayerType, cell: CellIndex, value_seen: u32, value: u32, count_place: Option<usize>) {
    crate::simulation_api::cell_reads_and_writes::seen_becoming(&mut open_compare(tick_state, under_compare), layer_type, cell, value_seen, value, count_place);
}

/// Adds one to the rule's count `count_place` if `under_compare` holds when it
/// is come to.
#[inline]
pub fn count(tick_state: &mut SimulationTickState, under_compare: Compare, count_place: usize) {
    with_writes(tick_state, under_compare, &[Write::Count(count_place as u32)]);
}
