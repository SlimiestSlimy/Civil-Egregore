//! Where instructions land as they are applied: one superchunk, a
//! slot's queue; or several, an act over a border, where an entity
//! is found in whichever it stands in by then
//! (`docs/simulation.md`, "An entity over a border").

use super::instruction::Instruction;
use bitplane_manager::Superchunk;
use coordinates::{CellIndex, SuperchunkIndex};
use entity_manager::SuperchunkEntities;

/// Where instructions land as they are applied: the superchunk of
/// an outbox slot and its entities, if it is hot.
pub(crate) trait Landing {
    /// Whether it lands in several superchunks: only then is an
    /// entity found where it crossed to, or had cross. Known as the
    /// code is built, so a slot's queue asks neither.
    const SEVERAL: bool;

    /// The superchunk of the slot `slot` of the outbox the
    /// instructions were queued in, and its entities.
    fn of_slot(&mut self, slot: usize) -> Option<(&mut Superchunk, &mut SuperchunkEntities)>;

    /// Where `instruction`, which names an entity, lands: the
    /// superchunk the entity stands in now, its entities, and the
    /// cell that names it. Its slot's, unless the entity crossed a
    /// border this tick and the superchunk it came to is held.
    #[inline]
    fn of_named(&mut self, instruction: &Instruction) -> Option<(&mut Superchunk, &mut SuperchunkEntities, CellIndex)> {
        let (superchunk, entities) = self.of_slot(instruction.slot())?;
        let at = instruction.at(superchunk.index());
        Some((superchunk, entities, at))
    }

    /// What the crossing `instruction` is applied to: the cell the
    /// entity is named by, the cell it comes to, the superchunk it
    /// leaves and the one it comes to. None unless both are held.
    fn of_crossing(&mut self, _: &Instruction) -> Option<(CellIndex, CellIndex, Held<'_>, Held<'_>)> {
        None
    }

    /// Notes that the entity named by `stood` crossed to `to`.
    fn crossed(&mut self, _stood: CellIndex, _to: CellIndex) {}
}

/// A superchunk held as instructions are applied to it, and its
/// entities.
pub(crate) type Held<'a> = (&'a mut Superchunk, &'a mut SuperchunkEntities);

/// One superchunk, all a queue's instructions landing in it: the
/// queue of a slot.
pub(crate) struct LandingInOne<'a> {
    /// The superchunk.
    pub(crate) superchunk: &'a mut Superchunk,
    /// Its entities.
    pub(crate) entities: &'a mut SuperchunkEntities,
}

impl Landing for LandingInOne<'_> {
    const SEVERAL: bool = false;

    #[inline]
    fn of_slot(&mut self, _: usize) -> Option<(&mut Superchunk, &mut SuperchunkEntities)> {
        Some((self.superchunk, self.entities))
    }
}

/// The superchunks an act over a border lands in, by outbox slot:
/// none at a slot whose superchunk is not hot, or that the act does
/// not reach.
pub(crate) struct LandingInSeveral<'a> {
    /// The superchunk whose outbox the instructions were queued in:
    /// what the slots are told from.
    pub(crate) source: SuperchunkIndex,
    /// The superchunk of each slot, and its entities.
    pub(crate) lands: [Option<Held<'a>>; super::SLOTS],
    /// The superchunks held besides: those entities the act names
    /// crossed to, which may be no neighbour of `source`.
    pub(crate) also: Vec<Held<'a>>,
    /// The entities the act had cross a border, each by the cell
    /// that names it, with the cell it came to.
    pub(crate) crossed: Vec<(CellIndex, CellIndex)>,
}

impl Landing for LandingInSeveral<'_> {
    const SEVERAL: bool = true;

    #[inline]
    fn of_slot(&mut self, slot: usize) -> Option<(&mut Superchunk, &mut SuperchunkEntities)> {
        self.lands[slot].as_mut().map(|(superchunk, entities)| (&mut **superchunk, &mut **entities))
    }

    fn of_named(&mut self, instruction: &Instruction) -> Option<(&mut Superchunk, &mut SuperchunkEntities, CellIndex)> {
        let slot = instruction.slot();
        let at = instruction.at(self.source.offset(slot as i32 % 3 - 1, slot as i32 / 3 - 1)?);
        // Where the tick found it, unless it crossed a border since: then whichever superchunk held it came to.
        let stayed = self.lands[slot].as_ref().is_some_and(|(_, entities)| entities.crossed_to(at).is_none());
        if !stayed {
            let came_to = |held: &Held| held.1.named_from_abroad(at);
            if let Some(found) = self.lands.iter().position(|held| held.as_ref().is_some_and(came_to)) {
                return self.lands[found].as_mut().map(|(superchunk, entities)| (&mut **superchunk, &mut **entities, at));
            }
            if let Some((superchunk, entities)) = self.also.iter_mut().find(|held| came_to(held)) {
                return Some((&mut **superchunk, &mut **entities, at));
            }
        }
        self.lands[slot].as_mut().map(|(superchunk, entities)| (&mut **superchunk, &mut **entities, at))
    }

    fn of_crossing(&mut self, instruction: &Instruction) -> Option<(CellIndex, CellIndex, Held<'_>, Held<'_>)> {
        let (slot, to) = (instruction.slot(), CellIndex(instruction.first));
        let came_to = self.lands.iter().position(|held| held.as_ref().is_some_and(|held| held.0.index() == to.superchunk()))?;
        let [from, into] = self.lands.get_disjoint_mut([slot, came_to]).ok()?;
        let (from, into) = (from.as_mut()?, into.as_mut()?);
        Some((instruction.at(from.0.index()), to, (&mut *from.0, &mut *from.1), (&mut *into.0, &mut *into.1)))
    }

    fn crossed(&mut self, stood: CellIndex, to: CellIndex) {
        self.crossed.push((stood, to));
    }
}
