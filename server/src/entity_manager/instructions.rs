//! Instructions: changes to entities -- make, move, edit, remove --
//! queued in a tick's first phase and applied in its second
//! (`docs/entity_manager.md`, "Instructions").

use crate::entity_manager::bucket::EntityWritten;
use crate::entity_manager::attributes::{push_attribute, AttributeType};
use protocol::server_types::attribute_layouts::{Attribute, Layout};
use protocol::server_types::attributes::AttributeBlock;
use protocol::server_types::entity::EntityBaseData;
use crate::entity_manager::store::SuperchunkEntities;
use utilities::coordinates::CellIndex;
use std::ops::AddAssign;

/// One instruction.
#[derive(Clone, Copy, Debug)]
enum Instruction {
    /// Makes an entity on its cell, with the attributes' blocks
    /// `first..first_block + count` of the queue's list: not made, an
    /// entity standing there.
    Add {
        /// Its fixed part.
        base_data: EntityBaseData,
        /// Its attributes' first block.
        first_block: u32,
        /// How many blocks they are.
        count: u32,
    },
    /// Moves the entity the tick found on `from` to `base_data`'s cell,
    /// to wake at its tick, its attributes as they are -- or, the
    /// cell being the one it stands on, only sets when it next wakes.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    Move {
        /// Its fixed part, as it is to be.
        base_data: EntityBaseData,
        /// The cell it stands on, in its cell's superchunk.
        named_cell: CellIndex,
    },
    /// Sets one attribute of the entity the tick found on `at` to the
    /// blocks `first..first_block + count` of the queue's list, or with
    /// none removes it.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    Edit {
        /// Its cell, as the tick found it.
        named_cell: CellIndex,
        /// The attribute's type.
        attribute_type: AttributeType,
        /// The attribute's first block.
        first_block: u32,
        /// How many blocks it is: none to remove it.
        count: u32,
    },
    /// Removes the entity the tick found on `at`.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    RemoveEntity {
        /// Its cell, as the tick found it.
        named_cell: CellIndex,
    },
}

impl Instruction {
    /// The cell it lands on: its superchunk applies it.
    fn landing_cell(&self) -> CellIndex {
        match *self {
            Self::Add { base_data, .. } | Self::Move { base_data, .. } => base_data.stood_cell,
            Self::Edit { named_cell: at, .. } | Self::RemoveEntity { named_cell: at, .. } => at,
        }
    }
}

/// Instructions queued for one superchunk, in order, and the
/// attributes' blocks they carry.
#[derive(Default)]
pub struct Instructions {
    /// The instructions.
    instructions: Vec<Instruction>,
    /// The blocks the puts and the edits carry.
    attributes: Vec<AttributeBlock>,
}

impl Instructions {
    /// Queues making `base_data`'s entity on its cell, with
    /// `attributes`: not made, an entity standing there by then.
    pub fn add(&mut self, base_data: EntityBaseData, attributes: &[AttributeBlock]) {
        self.push(base_data, attributes);
    }

    /// Queues an entity added.
    fn push(&mut self, base_data: EntityBaseData, attributes: &[AttributeBlock]) {
        self.instructions.push(Instruction::Add { base_data, first_block: self.attributes.len() as u32, count: attributes.len() as u32 });
        self.attributes.extend_from_slice(attributes);
    }

    /// Queues moving the entity the tick found on `named_cell` -- a cell of
    /// its cell's superchunk -- to its cell, to wake at its tick, with
    /// the attributes it has: none are carried. Its cell `named_cell` itself,
    /// it only sleeps until then.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn move_entity(&mut self, base_data: EntityBaseData, named_cell: CellIndex) {
        debug_assert_eq!(base_data.stood_cell.superchunk(), named_cell.superchunk(), "an entity moved from another superchunk: a crossing");
        self.instructions.push(Instruction::Move { base_data, named_cell });
    }

    /// Queues setting `attribute` of the entity the tick found on
    /// `named_cell` to `value`.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn set_attribute<L: Layout>(&mut self, named_cell: CellIndex, attribute: Attribute<L>, value: L) {
        let first_block = self.attributes.len() as u32;
        push_attribute(&mut self.attributes, attribute, value);
        self.instructions.push(Instruction::Edit { named_cell, attribute_type: attribute.attribute_type(), first_block, count: L::BLOCK_COUNT as u32 });
    }

    /// Queues setting the attribute `attribute` is the blocks of, of
    /// the entity the tick found on `named_cell`: how one whose size varies
    /// is set.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn set_attribute_blocks(&mut self, named_cell: CellIndex, attribute: &[AttributeBlock]) {
        debug_assert!(attribute.first().is_some_and(|first_block| first_block.block_count() == attribute.len()), "an attribute as long as its type or its block length says");
        self.instructions.push(Instruction::Edit { named_cell, attribute_type: attribute[0].attribute_type(), first_block: self.attributes.len() as u32, count: attribute.len() as u32 });
        self.attributes.extend_from_slice(attribute);
    }

    /// Queues removing the attribute of type `attribute_type` of the entity
    /// the tick found on `named_cell`.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn unset_attribute(&mut self, named_cell: CellIndex, attribute_type: AttributeType) {
        self.instructions.push(Instruction::Edit { named_cell, attribute_type, first_block: 0, count: 0 });
    }

    /// Queues removing the entity the tick found on `named_cell`.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn remove(&mut self, named_cell: CellIndex) {
        self.instructions.push(Instruction::RemoveEntity { named_cell });
    }

    /// How many instructions are queued.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn len(&self) -> usize {
        self.instructions.len()
    }

    /// Empties the queue, keeping its room.
    pub fn clear(&mut self) {
        self.instructions.clear();
        self.attributes.clear();
    }

    /// Applies the instructions, in order, each to the superchunk among
    /// `superchunks` -- sorted by superchunk index -- its cell is in,
    /// every wake filed no earlier than `earliest_tick`; into `instructions_applied`
    /// (`docs/entity_manager.md`, "Instructions").
    pub fn apply(&self, superchunks: &mut [SuperchunkEntities], earliest_tick: u64, instructions_applied: &mut InstructionsApplied) {
        self.apply_some(0..self.instructions.len(), superchunks, earliest_tick, instructions_applied);
    }

    /// [`Instructions::apply`], of those at `instruction_range` alone, counted from
    /// the first queued: how those under a compare refused are left out
    /// (`docs/entity_manager.md`, "Instructions"). Each as it is come
    /// to, whatever it does -- none before another for its kind: the
    /// entity it names is found wherever those before it moved it
    /// (`SuperchunkEntities::get_named`).
    pub fn apply_some(&self, instruction_range: std::ops::Range<usize>, superchunks: &mut [SuperchunkEntities], earliest_tick: u64, instructions_applied: &mut InstructionsApplied) {
        for &instruction in &self.instructions[instruction_range] {
            let superchunk = instruction.landing_cell().superchunk();
            let maybe_found = match superchunks {
                [only_superchunk] if only_superchunk.superchunk_index() == superchunk => Some(0),
                _ => superchunks.binary_search_by_key(&superchunk, SuperchunkEntities::superchunk_index).ok(),
            };
            let Some(found_index) = maybe_found else {
                instructions_applied.lost += matches!(instruction, Instruction::Add { .. }) as usize;
                continue;
            };
            let superchunk = &mut superchunks[found_index];
            match instruction {
                Instruction::Add { base_data, first_block: first, count } => {
                    let attributes = &self.attributes[first as usize..(first + count) as usize];
                    let made = superchunk.make_named(earliest_tick, base_data, attributes);
                    match made {
                        EntityWritten::Added | EntityWritten::InPlace | EntityWritten::MovedToCell => instructions_applied.added += 1,
                        EntityWritten::TurnedBack => (instructions_applied.added, instructions_applied.turned_back) = (instructions_applied.added + 1, instructions_applied.turned_back + 1),
                        EntityWritten::RefusedOccupied => instructions_applied.adds_refused += 1,
                        EntityWritten::PassedOver => instructions_applied.passed_over += 1,
                    }
                }
                Instruction::Move { base_data, named_cell: from } => match superchunk.write_named(earliest_tick, base_data, from) {
                    EntityWritten::TurnedBack => (instructions_applied.moves, instructions_applied.turned_back) = (instructions_applied.moves + 1, instructions_applied.turned_back + 1),
                    EntityWritten::PassedOver => instructions_applied.passed_over += 1,
                    _ => instructions_applied.moves += 1,
                },
                Instruction::Edit { named_cell: at, attribute_type: kind, first_block: first, count } => match superchunk.edit_named(at, kind, &self.attributes[first as usize..(first + count) as usize]) {
                    true => instructions_applied.edits += 1,
                    false => instructions_applied.passed_over += 1,
                },
                Instruction::RemoveEntity { named_cell: at } => match superchunk.remove_named(at) {
                    true => instructions_applied.entities_removed += 1,
                    false => instructions_applied.passed_over += 1,
                },
            }
        }
    }
}

/// What applying the instructions did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InstructionsApplied {
    /// Entities made.
    pub added: usize,
    /// Entities moved, or set to wake at another tick, their attributes
    /// as they were.
    pub moves: usize,
    /// Writes to attributes: words written, an attribute given or
    /// taken, each an instruction.
    pub edits: usize,
    /// Wakes written to the wheel.
    pub wakes: usize,
    /// Entities removed: died, or moved out of their chunk.
    pub entities_removed: usize,
    /// Entities put in a superchunk holding no entities, so lost.
    pub lost: usize,
    /// Of the entities put or moved, those whose cell was taken: left
    /// where they stood.
    pub turned_back: usize,
    /// New entities whose cell was taken: not added.
    pub adds_refused: usize,
    /// Of the entities moved, those that crossed into another superchunk.
    pub border_crossings: usize,
    /// Instructions for an entity no longer where it stood -- moved on,
    /// or removed, earlier in the tick: not applied.
    pub passed_over: usize,
}

impl AddAssign for InstructionsApplied {
    /// Both added up.
    fn add_assign(&mut self, other_applied: Self) {
        self.added += other_applied.added;
        self.moves += other_applied.moves;
        self.edits += other_applied.edits;
        self.wakes += other_applied.wakes;
        self.entities_removed += other_applied.entities_removed;
        self.lost += other_applied.lost;
        self.turned_back += other_applied.turned_back;
        self.adds_refused += other_applied.adds_refused;
        self.border_crossings += other_applied.border_crossings;
        self.passed_over += other_applied.passed_over;
    }
}
