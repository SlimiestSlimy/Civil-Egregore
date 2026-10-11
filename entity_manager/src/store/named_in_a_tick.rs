//! An entity's name in a tick: the cell the tick found it on. No two
//! stand on one cell, so the cell names it, and goes on naming it
//! until the tick is over, wherever it moves meanwhile -- so every
//! instruction of the tick finds it, whatever was applied before
//! (`docs/entity_manager.md`, "Instructions", An entity's name in a
//! tick).

use super::SuperchunkEntities;
use crate::attributes::{AttributeBlock, AttributeType};
use crate::bucket::{place, Put};
use crate::entity::NEVER;
use crate::wheel::Wake;
use crate::entity::{EntityRef, Header};
use coordinates::CellIndex;

/// The collision plane as entities keep it: whoever applies their
/// instructions lends it, a superchunk's
/// (`docs/entity_manager.md`, "Instructions", Entities in the
/// collision plane). `()` is none: nothing held, nothing kept.
pub trait CollisionCells {
    /// Whether the plane holds `at`: something stands there.
    fn held(&self, at: CellIndex) -> bool;
    /// Sets `at`: an entity came to stand there.
    fn hold(&mut self, at: CellIndex);
    /// Clears `at`: the entity that stood there is gone from it.
    fn free(&mut self, at: CellIndex);
}

impl CollisionCells for () {
    fn held(&self, _: CellIndex) -> bool {
        false
    }

    fn hold(&mut self, _: CellIndex) {}

    fn free(&mut self, _: CellIndex) {}
}

impl SuperchunkEntities {
    /// Where the entity the tick found on `stood` stands now: there
    /// still, on the cell it moved to, or -- removed -- nowhere.
    fn now_on(&self, stood: CellIndex) -> Option<CellIndex> {
        match self.left_this_tick.get(&stood) {
            // Where it went, if that is here: one that crossed a border is the other superchunk's.
            Some(now_on) => now_on.filter(|now_on| now_on.superchunk() == self.index),
            None => {
                debug_assert_eq!(stood.superchunk(), self.index, "an entity named in a superchunk it neither stood in nor came to");
                Some(stood)
            }
        }
    }

    /// The superchunk the entity the tick found on `stood` crossed
    /// to, if it left this one over a border this tick: what is
    /// queued for it lands there
    /// (`docs/entity_manager.md`, "Instructions", Crossing a border).
    pub fn crossed_to(&self, stood: CellIndex) -> Option<coordinates::SuperchunkIndex> {
        let now_on = (*self.left_this_tick.get(&stood)?)?;
        Some(now_on.superchunk()).filter(|&superchunk| superchunk != self.index)
    }

    /// Whether the entity the tick found on `stood`, a cell of
    /// another superchunk, crossed into this one this tick and is
    /// named here still -- standing, or removed since.
    pub fn named_from_abroad(&self, stood: CellIndex) -> bool {
        stood.superchunk() != self.index && self.left_this_tick.contains_key(&stood)
    }

    /// Moves the entity the tick found on `stood`, standing there
    /// still, to `to`, a cell of `into`, another superchunk: all it
    /// has goes with it, as it is now, and its wake, if still to come,
    /// is filed there. It is passed over if it already left `stood`
    /// this tick, and stays if `to` is another's this tick or one
    /// `collision`, `into`'s plane, holds. Crossed, its name finds it
    /// in `into` for the rest of the tick, `to` is set in `collision`
    /// and the cell it left is cleared when the tick is over
    /// (`docs/entity_manager.md`, "Instructions", Crossing a border).
    pub fn cross_named(&mut self, into: &mut SuperchunkEntities, earliest: u64, stood: CellIndex, to: CellIndex, collision: &mut impl CollisionCells) -> Put {
        debug_assert!(stood.superchunk() == self.index && to.superchunk() == into.index && self.index != into.index, "a crossing within a superchunk: a move");
        if self.left_this_tick.contains_key(&stood) {
            return Put::PassedOver;
        }
        let Some(entity) = self.get(stood) else {
            return Put::PassedOver;
        };
        if into.named_or_taken(to) || collision.held(to) {
            return Put::Stayed;
        }
        let header = Header { at: to, ..entity.header };
        // Those it has go with it as this superchunk kept them.
        let mut carried = std::mem::take(&mut into.carried);
        carried.clear();
        carried.extend_from_slice(entity.kept);
        let put = into.chunks[to.chunk().place()].make(header, &carried);
        into.carried = carried;
        debug_assert_eq!(put, Put::New, "a cell seen free");
        if header.wake != NEVER && header.wake >= earliest {
            into.wheel.file(earliest, header.wake, Wake { at: to });
        }
        self.remove(stood);
        self.left_this_tick.insert(stood, Some(to));
        self.vacated.push(stood);
        into.left_this_tick.insert(stood, Some(to));
        collision.hold(to);
        Put::Moved
    }

    /// Whether `at` is another's this tick: an entity stands on it, or
    /// stood on it as the tick began -- its name still, though it left.
    fn named_or_taken(&self, at: CellIndex) -> bool {
        self.occupied(at) || self.left_this_tick.contains_key(&at)
    }

    /// The entity the tick found on `stood`, wherever it stands now.
    pub fn get_named(&self, stood: CellIndex) -> Option<EntityRef<'_>> {
        self.get(self.now_on(stood)?)
    }

    /// Makes `header`'s entity on its cell, with `attributes`: new --
    /// or refused, the cell another's this tick, or one `collision`
    /// holds, whatever stands there. Made, its cell is set in
    /// `collision`.
    pub fn make_named(&mut self, earliest: u64, header: Header, attributes: &[AttributeBlock], collision: &mut impl CollisionCells) -> Put {
        if self.named_or_taken(header.at) || collision.held(header.at) {
            return Put::Refused;
        }
        let put = self.make(earliest, header, attributes);
        if put == Put::New {
            collision.hold(header.at);
        }
        put
    }

    /// Moves the entity the tick found on `stood` to `header`'s cell,
    /// to wake at its tick, with the attributes it has, its cell being `stood` if it is to stay where
    /// it is. Where it stands is written as anything is, by
    /// compare-and-write: one that already left `stood` this tick is
    /// passed over -- of two moving one entity, the first applied
    /// does. And it comes to no cell that is another's this tick, nor
    /// to one `collision` holds, whatever stands there: it stays. The
    /// cell it comes to is set in `collision`, and the one it left is
    /// cleared when the tick is over.
    pub fn put_named(&mut self, earliest: u64, header: Header, stood: CellIndex, collision: &mut impl CollisionCells) -> Put {
        if self.left_this_tick.contains_key(&stood) || !self.occupied(stood) {
            return Put::PassedOver;
        }
        if header.at != stood && (self.named_or_taken(header.at) || collision.held(header.at)) {
            return self.put(earliest, Header { at: stood, ..header }, stood).stayed();
        }
        let (to, put) = (header.at, self.put(earliest, header, stood));
        if put == Put::Moved {
            self.left_this_tick.insert(stood, Some(to));
            collision.hold(to);
            self.vacated.push(stood);
        }
        put
    }

    /// Sets the attribute of type `kind` of the entity the tick found
    /// on `stood` to `blocks`, or with none removes it: whether the
    /// entity is there.
    pub fn edit_named(&mut self, stood: CellIndex, kind: AttributeType, blocks: &[AttributeBlock]) -> bool {
        self.now_on(stood).is_some_and(|now_on| self.edit(now_on, kind, blocks))
    }

    /// Moves the entity the tick found on `stood` to `to`, a cell of
    /// its superchunk, all else of it as it is
    /// ([`SuperchunkEntities::put_named`]): its wake, if it is still
    /// to come, is filed where it now stands -- one already passed is
    /// not, whatever writes its next one files that.
    pub fn move_named(&mut self, earliest: u64, stood: CellIndex, to: CellIndex, collision: &mut impl CollisionCells) -> Put {
        let Some(header) = self.get(stood).map(|entity| entity.header) else {
            return Put::PassedOver;
        };
        self.put_named(earliest, Header { at: to, ..header }, stood, collision)
    }

    /// Sets the tick the entity the tick found on `stood` next wakes
    /// at to `wake`, no earlier than `earliest`, and files it on the
    /// wheel where the entity stands now: a write to the wheel, as
    /// another is to an attribute. Whether the entity is there.
    pub fn wake_named(&mut self, earliest: u64, stood: CellIndex, wake: u64) -> bool {
        let Some(now_on) = self.now_on(stood).filter(|&now_on| self.chunks[now_on.chunk().place()].set_wake(place(now_on), wake)) else {
            return false;
        };
        if wake != NEVER {
            self.wheel.file(earliest, wake, Wake { at: now_on });
        }
        true
    }

    /// Writes the low `bytes` bytes of `value` over the member from
    /// the byte `offset` of the attribute of type `kind`, of the
    /// entity the tick found on `stood`: whether it was written --
    /// the entity there, the attribute its, the member within it.
    pub fn write_attribute_bytes_named(&mut self, stood: CellIndex, kind: AttributeType, offset: usize, bytes: usize, value: u64) -> bool {
        self.now_on(stood).is_some_and(|now_on| self.chunks[now_on.chunk().place()].write_bytes(place(now_on), kind, offset, bytes, value))
    }

    /// Whether `at` is free for an entity to be made on: no entity
    /// stands on it, it is no entity's name this tick, and
    /// `collision` does not hold it
    /// ([`SuperchunkEntities::make_named`] refuses on any other).
    pub fn free_for_a_new_entity(&self, at: CellIndex, collision: &impl CollisionCells) -> bool {
        !self.named_or_taken(at) && !collision.held(at)
    }

    /// Removes the entity the tick found on `stood`: whether it was
    /// there. Its cell is its name until the tick is over, and no
    /// other's.
    pub fn remove_named(&mut self, stood: CellIndex) -> bool {
        let Some(now_on) = self.now_on(stood).filter(|&now_on| self.remove(now_on)) else {
            return false;
        };
        self.left_this_tick.insert(stood, None);
        self.vacated.push(now_on);
        true
    }

    /// The tick is over: every entity is named by the cell it stands
    /// on now, and each cell an entity left or was removed from in it
    /// is cleared in `collision` -- held until now, as the cell was
    /// that entity's name.
    pub fn names_anew(&mut self, collision: &mut impl CollisionCells) {
        self.left_this_tick.clear();
        for at in self.vacated.drain(..) {
            if !self.chunks[at.chunk().place()].occupied(crate::bucket::place(at)) {
                collision.free(at);
            }
        }
    }
}
