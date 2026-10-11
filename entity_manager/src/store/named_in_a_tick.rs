//! An entity's name in a tick: the cell the tick found it on. No two
//! stand on one cell, so the cell names it, and goes on naming it
//! until the tick is over, wherever it moves meanwhile -- so every
//! instruction of the tick finds it, whatever was applied before
//! (`docs/entity_manager.md`, "Instructions", An entity's name in a
//! tick).

use super::SuperchunkEntities;
use crate::attributes::{AttributeBlock, AttributeType};
use crate::bucket::Put;
use crate::entity::{EntityId, EntityRef, Header};
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
        self.left_this_tick.get(&stood).map_or(Some(stood), |now_on| *now_on)
    }

    /// Whether `at` is another's this tick: an entity stands on it, or
    /// stood on it as the tick began -- its name still, though it left.
    fn named_or_taken(&self, at: CellIndex) -> bool {
        self.occupied(at) || self.left_this_tick.contains_key(&at)
    }

    /// The entity whose ID is `id` that the tick found on `stood`,
    /// wherever it stands now.
    pub fn get_named(&self, id: EntityId, stood: CellIndex) -> Option<EntityRef<'_>> {
        self.get(id, self.now_on(stood)?)
    }

    /// Puts `header`'s entity, which the tick found on `stood`, with
    /// `attributes` -- or, with none given, moves it with those it
    /// has -- to its cell, its own being `stood` if it is to stay
    /// where it is; or, no such entity there and attributes given,
    /// makes it. Where it stands is written as anything is, by
    /// compare-and-write: one that already left `stood` this tick is
    /// passed over -- of two moving one entity, the first applied
    /// does. And it comes to no cell that is another's this tick: one
    /// moving stays, one new is refused. Nor to one `collision` holds,
    /// whatever stands there; the cell it comes to is set in it, and
    /// the one it left is cleared when the tick is over.
    pub(crate) fn put_named(&mut self, earliest: u64, header: Header, stood: CellIndex, attributes: Option<&[AttributeBlock]>, collision: &mut impl CollisionCells) -> Put {
        let Some(now_on) = self.now_on(stood).filter(|&now_on| self.get(header.id, now_on).is_some()) else {
            return match attributes {
                Some(_) if header.at == stood && !self.named_or_taken(stood) && !collision.held(stood) => {
                    let put = self.put(earliest, header, stood, attributes);
                    if put == Put::New {
                        collision.hold(stood);
                    }
                    put
                }
                Some(_) if header.at == stood => Put::Refused,
                _ => Put::PassedOver,
            };
        };
        if now_on != stood {
            return Put::PassedOver;
        }
        if header.at != stood && (self.named_or_taken(header.at) || collision.held(header.at)) {
            return self.put(earliest, Header { at: stood, ..header }, stood, attributes).stayed();
        }
        let (to, put) = (header.at, self.put(earliest, header, stood, attributes));
        if put == Put::Moved {
            self.left_this_tick.insert(stood, Some(to));
            collision.hold(to);
            self.vacated.push(stood);
        }
        put
    }

    /// Sets the attribute of type `kind` of the entity whose ID is
    /// `id` that the tick found on `stood` to `blocks`, or with none
    /// removes it: whether the entity is there.
    pub(crate) fn edit_named(&mut self, id: EntityId, stood: CellIndex, kind: AttributeType, blocks: &[AttributeBlock]) -> bool {
        self.now_on(stood).is_some_and(|now_on| self.edit(id, now_on, kind, blocks))
    }

    /// Removes the entity whose ID is `id` that the tick found on
    /// `stood`: whether it was there. Its cell is its name until the
    /// tick is over, and no other's.
    pub(crate) fn remove_named(&mut self, id: EntityId, stood: CellIndex) -> bool {
        let Some(now_on) = self.now_on(stood).filter(|&now_on| self.remove(id, now_on)) else {
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
