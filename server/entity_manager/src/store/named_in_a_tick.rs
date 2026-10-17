//! An entity's name in a tick: the cell the tick found it on. No two
//! stand on one cell, so the cell names it, and goes on naming it
//! until the tick is over, wherever it moves meanwhile -- so every
//! instruction of the tick finds it, whatever was applied before
//! (`docs/entity_manager.md`, "Instructions", An entity's name in a
//! tick).

use super::SuperchunkEntities;
use crate::attributes::{AttributeBlock, AttributeType};
use crate::bucket::{place, EntityWritten};
use crate::entity::NEVER_WAKES;
use crate::wheel::Wake;
use crate::entity::{EntityRef, Header};
use utilities::coordinates::CellIndex;

impl SuperchunkEntities {
    /// Where the entity the tick found on `named_cell` stands now: there
    /// still, on the cell it moved to, or -- removed -- nowhere.
    fn now_on(&self, named_cell: CellIndex) -> Option<CellIndex> {
        match self.left_this_tick.get(&named_cell) {
            // Where it went, if that is here: one that crossed a border is the other superchunk's.
            Some(now_on) => now_on.filter(|now_on| now_on.superchunk() == self.superchunk_index),
            None => {
                debug_assert_eq!(named_cell.superchunk(), self.superchunk_index, "an entity named in a superchunk it neither stood in nor came to");
                Some(named_cell)
            }
        }
    }

    /// The superchunk the entity the tick found on `named_cell` crossed
    /// to, if it left this one over a border this tick: what is
    /// queued for it lands there
    /// (`docs/entity_manager.md`, "Instructions", Crossing a border).
    pub fn crossed_to(&self, named_cell: CellIndex) -> Option<utilities::coordinates::SuperchunkIndex> {
        let now_on = (*self.left_this_tick.get(&named_cell)?)?;
        Some(now_on.superchunk()).filter(|&superchunk| superchunk != self.superchunk_index)
    }

    /// Whether the entity the tick found on `named_cell`, a cell of
    /// another superchunk, crossed into this one this tick and is
    /// named here still -- standing, or removed since.
    pub fn named_from_abroad(&self, named_cell: CellIndex) -> bool {
        named_cell.superchunk() != self.superchunk_index && self.left_this_tick.contains_key(&named_cell)
    }

    /// Moves the entity the tick found on `named_cell`, standing there
    /// still, to `to_cell`, a cell of `destination_entities`, another superchunk: all it
    /// has goes with it, as it is now, and its wake, if still to come,
    /// is filed there. It is passed over if it already left `named_cell`
    /// this tick, and stays if `to_cell` is another's this tick. Crossed,
    /// its name finds it in `destination_entities` for the rest of the tick
    /// (`docs/entity_manager.md`, "Instructions", Crossing a border).
    pub fn cross_named(&mut self, destination_entities: &mut SuperchunkEntities, earliest_tick: u64, named_cell: CellIndex, to_cell: CellIndex) -> EntityWritten {
        debug_assert!(named_cell.superchunk() == self.superchunk_index && to_cell.superchunk() == destination_entities.superchunk_index && self.superchunk_index != destination_entities.superchunk_index, "a crossing within a superchunk: a move");
        if self.left_this_tick.contains_key(&named_cell) {
            return EntityWritten::PassedOver;
        }
        let Some(entity) = self.get(named_cell) else {
            return EntityWritten::PassedOver;
        };
        if destination_entities.named_or_taken(to_cell) {
            return EntityWritten::TurnedBack;
        }
        let header = Header { stood_cell: to_cell, ..entity.header };
        // Those it has go with it as this superchunk kept them.
        let mut carried_blocks = std::mem::take(&mut destination_entities.carried_blocks);
        carried_blocks.clear();
        carried_blocks.extend_from_slice(entity.kept_blocks);
        let made = destination_entities.chunks[to_cell.chunk().place()].make_entity(header, &carried_blocks, &destination_entities.default_attributes);
        destination_entities.carried_blocks = carried_blocks;
        debug_assert_eq!(made, EntityWritten::Added, "a cell seen free");
        if header.wake != NEVER_WAKES && header.wake >= earliest_tick {
            destination_entities.wheel.file_wake(earliest_tick, header.wake, Wake { stood_cell: to_cell });
        }
        self.remove(named_cell);
        self.left_this_tick.insert(named_cell, Some(to_cell));
        destination_entities.left_this_tick.insert(named_cell, Some(to_cell));
        EntityWritten::MovedToCell
    }

    /// Whether `checked_cell` is another's this tick: an entity stands on it, or
    /// stood on it as the tick began -- its name still, though it left.
    fn named_or_taken(&self, checked_cell: CellIndex) -> bool {
        self.occupied(checked_cell) || self.left_this_tick.contains_key(&checked_cell)
    }

    /// The entity the tick found on `named_cell`, wherever it stands now.
    pub fn get_named(&self, named_cell: CellIndex) -> Option<EntityRef<'_>> {
        self.get(self.now_on(named_cell)?)
    }

    /// Makes `header`'s entity on its cell, with `attributes`: new --
    /// or refused, the cell another's this tick.
    pub fn make_named(&mut self, earliest_tick: u64, header: Header, attributes: &[AttributeBlock]) -> EntityWritten {
        if self.named_or_taken(header.stood_cell) {
            return EntityWritten::RefusedOccupied;
        }
        self.make_entity(earliest_tick, header, attributes)
    }

    /// Moves the entity the tick found on `named_cell` to `header`'s cell,
    /// to wake at its tick, with the attributes it has, its cell being `named_cell` if it is to stay where
    /// it is. Where it stands is written as anything is, by
    /// compare-and-write: one that already left `named_cell` this tick is
    /// passed over -- of two moving one entity, the first applied
    /// does. And it comes to no cell that is another's this tick: it
    /// stays.
    pub fn write_named(&mut self, earliest_tick: u64, header: Header, named_cell: CellIndex) -> EntityWritten {
        if self.left_this_tick.contains_key(&named_cell) || !self.occupied(named_cell) {
            return EntityWritten::PassedOver;
        }
        if header.stood_cell != named_cell && self.named_or_taken(header.stood_cell) {
            return self.write_entity(earliest_tick, Header { stood_cell: named_cell, ..header }, named_cell).as_turned_back();
        }
        let (to_cell, written) = (header.stood_cell, self.write_entity(earliest_tick, header, named_cell));
        if written == EntityWritten::MovedToCell {
            self.left_this_tick.insert(named_cell, Some(to_cell));
        }
        written
    }

    /// Sets the attribute of type `attribute_type` of the entity the tick found
    /// on `named_cell` to `whole_attribute`, or with none removes it: whether the
    /// entity is there.
    pub fn edit_named(&mut self, named_cell: CellIndex, attribute_type: AttributeType, whole_attribute: &[AttributeBlock]) -> bool {
        self.now_on(named_cell).is_some_and(|now_on| self.edit(now_on, attribute_type, whole_attribute))
    }

    /// Moves the entity the tick found on `named_cell` to `to_cell`, a cell of
    /// its superchunk, all else of it as it is
    /// ([`SuperchunkEntities::put_named`]): its wake, if it is still
    /// to come, is filed where it now stands -- one already passed is
    /// not, whatever writes its next one files that.
    pub fn move_named(&mut self, earliest_tick: u64, named_cell: CellIndex, to_cell: CellIndex) -> EntityWritten {
        let Some(header) = self.get(named_cell).map(|entity| entity.header) else {
            return EntityWritten::PassedOver;
        };
        self.write_named(earliest_tick, Header { stood_cell: to_cell, ..header }, named_cell)
    }

    /// Sets the tick the entity the tick found on `named_cell` next wakes
    /// at to `wake`, no earlier than `earliest_tick`, and files it on the
    /// wheel where the entity stands now: a write to the wheel, as
    /// another is to an attribute. Whether the entity is there.
    pub fn wake_named(&mut self, earliest_tick: u64, named_cell: CellIndex, wake: u64) -> bool {
        let Some(now_on) = self.now_on(named_cell).filter(|&now_on| self.chunks[now_on.chunk().place()].set_wake(place(now_on), wake)) else {
            return false;
        };
        if wake != NEVER_WAKES {
            self.wheel.file_wake(earliest_tick, wake, Wake { stood_cell: now_on });
        }
        true
    }

    /// Writes the low `byte_count` bytes of `value` over the member from
    /// the byte `byte_offset` of the attribute of type `attribute_type`, of the
    /// entity the tick found on `named_cell`: whether it was written --
    /// the entity there, the attribute its, the member within it.
    pub fn write_attribute_bytes_named(&mut self, named_cell: CellIndex, attribute_type: AttributeType, byte_offset: usize, byte_count: usize, value: u64) -> bool {
        self.now_on(named_cell).is_some_and(|now_on| self.chunks[now_on.chunk().place()].write_bytes(place(now_on), attribute_type, byte_offset, byte_count, value, &self.default_attributes))
    }

    /// Whether `checked_cell` is free for an entity to be made on: no entity
    /// stands on it, and it is no entity's name this tick
    /// ([`SuperchunkEntities::make_named`] refuses on any other).
    pub fn free_for_a_new_entity(&self, checked_cell: CellIndex) -> bool {
        !self.named_or_taken(checked_cell)
    }

    /// Removes the entity the tick found on `named_cell`: whether it was
    /// there. Its cell is its name until the tick is over, and no
    /// other's.
    pub fn remove_named(&mut self, named_cell: CellIndex) -> bool {
        if !self.now_on(named_cell).is_some_and(|now_on| self.remove(now_on)) {
            return false;
        }
        self.left_this_tick.insert(named_cell, None);
        true
    }

    /// The tick is over: every entity is named by the cell it stands
    /// on now.
    pub fn names_anew(&mut self) {
        self.left_this_tick.clear();
    }
}
