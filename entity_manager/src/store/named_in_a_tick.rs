//! An entity's name in a tick: the cell the tick found it on. No two
//! stand on one cell, so the cell names it, and goes on naming it
//! until the tick is over, wherever it moves meanwhile -- so every
//! instruction of the tick finds it, whatever was applied before
//! (`docs/entity_manager.md`, "Instructions", An entity's name in a
//! tick).

use super::SuperchunkEntities;
use crate::attributes::{AttributeBlock, AttributeType};
use crate::bucket::{place, Put};
use crate::entity::NEVER_WAKES;
use crate::wheel::Wake;
use crate::entity::{EntityRef, Header};
use coordinates::CellIndex;

/// The collision plane as entities keep it: whoever applies their
/// instructions lends it, a superchunk's
/// (`docs/entity_manager.md`, "Instructions", Entities in the
/// collision plane). `()` is none: nothing held, nothing kept.
pub trait CollisionCells {
    /// Whether the plane holds `entity_cell`: something stands there.
    fn holds_cell(&self, entity_cell: CellIndex) -> bool;
    /// Sets `entity_cell`: an entity came to stand there.
    fn hold_cell(&mut self, entity_cell: CellIndex);
    /// Clears `entity_cell`: the entity that stood there is gone from it.
    fn free_cell(&mut self, entity_cell: CellIndex);
}

impl CollisionCells for () {
    fn holds_cell(&self, _: CellIndex) -> bool {
        false
    }

    fn hold_cell(&mut self, _: CellIndex) {}

    fn free_cell(&mut self, _: CellIndex) {}
}

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
    pub fn crossed_to(&self, named_cell: CellIndex) -> Option<coordinates::SuperchunkIndex> {
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
    /// this tick, and stays if `to_cell` is another's this tick or one
    /// `collision_plane`, `destination_entities`'s plane, holds. Crossed, its name finds it
    /// in `destination_entities` for the rest of the tick, `to_cell` is set in `collision_plane`
    /// and the cell it left is cleared when the tick is over
    /// (`docs/entity_manager.md`, "Instructions", Crossing a border).
    pub fn cross_named(&mut self, destination_entities: &mut SuperchunkEntities, earliest_tick: u64, named_cell: CellIndex, to_cell: CellIndex, collision_plane: &mut impl CollisionCells) -> Put {
        debug_assert!(named_cell.superchunk() == self.superchunk_index && to_cell.superchunk() == destination_entities.superchunk_index && self.superchunk_index != destination_entities.superchunk_index, "a crossing within a superchunk: a move");
        if self.left_this_tick.contains_key(&named_cell) {
            return Put::PassedOver;
        }
        let Some(entity) = self.get(named_cell) else {
            return Put::PassedOver;
        };
        if destination_entities.named_or_taken(to_cell) || collision_plane.holds_cell(to_cell) {
            return Put::TurnedBack;
        }
        let header = Header { stood_cell: to_cell, ..entity.header };
        // Those it has go with it as this superchunk kept them.
        let mut carried_blocks = std::mem::take(&mut destination_entities.carried_blocks);
        carried_blocks.clear();
        carried_blocks.extend_from_slice(entity.kept_blocks);
        let put = destination_entities.chunks[to_cell.chunk().place()].make_entity(header, &carried_blocks);
        destination_entities.carried_blocks = carried_blocks;
        debug_assert_eq!(put, Put::NewlyPut, "a cell seen free");
        if header.wake != NEVER_WAKES && header.wake >= earliest_tick {
            destination_entities.wheel.file_wake(earliest_tick, header.wake, Wake { stood_cell: to_cell });
        }
        self.remove(named_cell);
        self.left_this_tick.insert(named_cell, Some(to_cell));
        self.vacated_cells.push(named_cell);
        destination_entities.left_this_tick.insert(named_cell, Some(to_cell));
        collision_plane.hold_cell(to_cell);
        Put::MovedToCell
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
    /// or refused, the cell another's this tick, or one `collision_plane`
    /// holds, whatever stands there. Made, its cell is set in
    /// `collision_plane`.
    pub fn make_named(&mut self, earliest_tick: u64, header: Header, attributes: &[AttributeBlock], collision_plane: &mut impl CollisionCells) -> Put {
        if self.named_or_taken(header.stood_cell) || collision_plane.holds_cell(header.stood_cell) {
            return Put::RefusedOccupied;
        }
        let put = self.make_entity(earliest_tick, header, attributes);
        if put == Put::NewlyPut {
            collision_plane.hold_cell(header.stood_cell);
        }
        put
    }

    /// Moves the entity the tick found on `named_cell` to `header`'s cell,
    /// to wake at its tick, with the attributes it has, its cell being `named_cell` if it is to stay where
    /// it is. Where it stands is written as anything is, by
    /// compare-and-write: one that already left `named_cell` this tick is
    /// passed over -- of two moving one entity, the first applied
    /// does. And it comes to no cell that is another's this tick, nor
    /// to one `collision_plane` holds, whatever stands there: it stays. The
    /// cell it comes to is set in `collision_plane`, and the one it left is
    /// cleared when the tick is over.
    pub fn put_named(&mut self, earliest_tick: u64, header: Header, named_cell: CellIndex, collision_plane: &mut impl CollisionCells) -> Put {
        if self.left_this_tick.contains_key(&named_cell) || !self.occupied(named_cell) {
            return Put::PassedOver;
        }
        if header.stood_cell != named_cell && (self.named_or_taken(header.stood_cell) || collision_plane.holds_cell(header.stood_cell)) {
            return self.put(earliest_tick, Header { stood_cell: named_cell, ..header }, named_cell).as_turned_back();
        }
        let (to_cell, put) = (header.stood_cell, self.put(earliest_tick, header, named_cell));
        if put == Put::MovedToCell {
            self.left_this_tick.insert(named_cell, Some(to_cell));
            collision_plane.hold_cell(to_cell);
            self.vacated_cells.push(named_cell);
        }
        put
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
    pub fn move_named(&mut self, earliest_tick: u64, named_cell: CellIndex, to_cell: CellIndex, collision_plane: &mut impl CollisionCells) -> Put {
        let Some(header) = self.get(named_cell).map(|entity| entity.header) else {
            return Put::PassedOver;
        };
        self.put_named(earliest_tick, Header { stood_cell: to_cell, ..header }, named_cell, collision_plane)
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
        self.now_on(named_cell).is_some_and(|now_on| self.chunks[now_on.chunk().place()].write_bytes(place(now_on), attribute_type, byte_offset, byte_count, value))
    }

    /// Whether `checked_cell` is free for an entity to be made on: no entity
    /// stands on it, it is no entity's name this tick, and
    /// `collision_plane` does not hold it
    /// ([`SuperchunkEntities::make_named`] refuses on any other).
    pub fn free_for_a_new_entity(&self, checked_cell: CellIndex, collision_plane: &impl CollisionCells) -> bool {
        !self.named_or_taken(checked_cell) && !collision_plane.holds_cell(checked_cell)
    }

    /// Removes the entity the tick found on `named_cell`: whether it was
    /// there. Its cell is its name until the tick is over, and no
    /// other's.
    pub fn remove_named(&mut self, named_cell: CellIndex) -> bool {
        let Some(now_on) = self.now_on(named_cell).filter(|&now_on| self.remove(now_on)) else {
            return false;
        };
        self.left_this_tick.insert(named_cell, None);
        self.vacated_cells.push(now_on);
        true
    }

    /// The tick is over: every entity is named by the cell it stands
    /// on now, and each cell an entity left or was removed from in it
    /// is cleared in `collision_plane` -- held until now, as the cell was
    /// that entity's name.
    pub fn names_anew(&mut self, collision_plane: &mut impl CollisionCells) {
        self.left_this_tick.clear();
        for vacated_cell in self.vacated_cells.drain(..) {
            if !self.chunks[vacated_cell.chunk().place()].occupied(crate::bucket::place(vacated_cell)) {
                collision_plane.free_cell(vacated_cell);
            }
        }
    }
}
