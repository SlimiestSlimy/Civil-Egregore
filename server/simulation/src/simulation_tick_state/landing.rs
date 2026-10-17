//! Where instructions land as they are applied: one superchunk, a
//! slot's queue; or several, an act over a border, where an entity
//! is found in whichever it stands in by then
//! (`docs/simulation.md`, "An entity over a border").

use super::instruction::Instruction;
use bitplane_manager::Superchunk;
use utilities::coordinates::{CellIndex, SuperchunkIndex};
use entity_manager::SuperchunkEntities;

/// Where instructions land as they are applied: the superchunk of
/// an outbox slot and its entities, if it is hot.
pub(crate) trait InstructionLanding {
    /// Whether it lands in several superchunks: only then is an
    /// entity found where it crossed to, or had cross. Known as the
    /// code is built, so a slot's queue asks neither.
    const LANDS_IN_SEVERAL: bool;

    /// The superchunk of the slot `slot` of the outbox the
    /// instructions were queued in, and its entities.
    fn of_slot(&mut self, slot: usize) -> Option<(&mut Superchunk, &mut SuperchunkEntities)>;

    /// Where `instruction`, which names an entity, lands: the
    /// superchunk the entity stands in now, its entities, and the
    /// cell that names it. Its slot's, unless the entity crossed a
    /// border this tick and the superchunk it came to is held.
    #[inline]
    fn of_named(&mut self, instruction: &Instruction) -> Option<(&mut Superchunk, &mut SuperchunkEntities, CellIndex)> {
        let (superchunk, slot_entities) = self.of_slot(instruction.outbox_slot())?;
        let named_cell = instruction.cell_in(superchunk.superchunk_index());
        Some((superchunk, slot_entities, named_cell))
    }

    /// What the crossing `instruction` is applied to: the cell the
    /// entity is named by, the cell it comes to, the superchunk it
    /// leaves and the one it comes to. None unless both are held.
    fn of_crossing(&mut self, _: &Instruction) -> Option<(CellIndex, CellIndex, SuperchunkAndEntities<'_>, SuperchunkAndEntities<'_>)> {
        None
    }

    /// Notes that the entity named by `stood` crossed to `to`.
    fn note_crossed(&mut self, _stood: CellIndex, _to: CellIndex) {}
}

/// A superchunk held as instructions are applied to it, and its
/// entities.
pub(crate) type SuperchunkAndEntities<'a> = (&'a mut Superchunk, &'a mut SuperchunkEntities);

/// One superchunk, all a queue's instructions landing in it: the
/// queue of a slot.
pub(crate) struct LandingInOne<'a> {
    /// The superchunk.
    pub(crate) superchunk: &'a mut Superchunk,
    /// Its entities.
    pub(crate) superchunk_entities: &'a mut SuperchunkEntities,
}

impl InstructionLanding for LandingInOne<'_> {
    const LANDS_IN_SEVERAL: bool = false;

    #[inline]
    fn of_slot(&mut self, _: usize) -> Option<(&mut Superchunk, &mut SuperchunkEntities)> {
        Some((self.superchunk, self.superchunk_entities))
    }
}

/// The superchunks an act over a border lands in, by outbox slot:
/// none at a slot whose superchunk is not hot, or that the act does
/// not reach.
pub(crate) struct LandingInSeveral<'a> {
    /// The superchunk whose outbox the instructions were queued in:
    /// what the slots are told from.
    pub(crate) source_superchunk: SuperchunkIndex,
    /// The superchunk of each slot, and its entities.
    pub(crate) slot_superchunks: [Option<SuperchunkAndEntities<'a>>; super::SLOTS],
    /// The superchunks held besides: those entities the act names
    /// crossed to, which may be no neighbour of `source`.
    pub(crate) also_superchunks: Vec<SuperchunkAndEntities<'a>>,
    /// The entities the act had cross a border, each by the cell
    /// that names it, with the cell it came to.
    pub(crate) crossed_entities: Vec<(CellIndex, CellIndex)>,
}

impl InstructionLanding for LandingInSeveral<'_> {
    const LANDS_IN_SEVERAL: bool = true;

    #[inline]
    fn of_slot(&mut self, slot: usize) -> Option<(&mut Superchunk, &mut SuperchunkEntities)> {
        self.slot_superchunks[slot].as_mut().map(|(superchunk, slot_entities)| (&mut **superchunk, &mut **slot_entities))
    }

    fn of_named(&mut self, instruction: &Instruction) -> Option<(&mut Superchunk, &mut SuperchunkEntities, CellIndex)> {
        let slot = instruction.outbox_slot();
        let named_cell = instruction.cell_in(self.source_superchunk.at_cartesian_offset(slot as i32 % 3 - 1, slot as i32 / 3 - 1)?);
        // Where the tick found it, unless it crossed a border since: then whichever superchunk held it came to.
        let stayed_in_slot = self.slot_superchunks[slot].as_ref().is_some_and(|(_, stayed_entities)| stayed_entities.crossed_to(named_cell).is_none());
        if !stayed_in_slot {
            let came_to = |candidate_superchunk: &SuperchunkAndEntities| candidate_superchunk.1.named_from_abroad(named_cell);
            if let Some(found_slot) = self.slot_superchunks.iter().position(|slot_superchunk| slot_superchunk.as_ref().is_some_and(came_to)) {
                return self.slot_superchunks[found_slot].as_mut().map(|(superchunk, found_entities)| (&mut **superchunk, &mut **found_entities, named_cell));
            }
            if let Some((superchunk, also_entities)) = self.also_superchunks.iter_mut().find(|also_superchunk| came_to(also_superchunk)) {
                return Some((&mut **superchunk, &mut **also_entities, named_cell));
            }
        }
        self.slot_superchunks[slot].as_mut().map(|(superchunk, slot_entities)| (&mut **superchunk, &mut **slot_entities, named_cell))
    }

    fn of_crossing(&mut self, instruction: &Instruction) -> Option<(CellIndex, CellIndex, SuperchunkAndEntities<'_>, SuperchunkAndEntities<'_>)> {
        let (slot, to_cell) = (instruction.outbox_slot(), CellIndex(instruction.first_operand));
        let came_to = self.slot_superchunks.iter().position(|slot_superchunk| slot_superchunk.as_ref().is_some_and(|hot_superchunk| hot_superchunk.0.superchunk_index() == to_cell.superchunk()))?;
        let [from_slot_entry, into_slot_entry] = self.slot_superchunks.get_disjoint_mut([slot, came_to]).ok()?;
        let (from_superchunk, into_superchunk) = (from_slot_entry.as_mut()?, into_slot_entry.as_mut()?);
        Some((instruction.cell_in(from_superchunk.0.superchunk_index()), to_cell, (&mut *from_superchunk.0, &mut *from_superchunk.1), (&mut *into_superchunk.0, &mut *into_superchunk.1)))
    }

    fn note_crossed(&mut self, stood_cell: CellIndex, to_cell: CellIndex) {
        self.crossed_entities.push((stood_cell, to_cell));
    }
}
