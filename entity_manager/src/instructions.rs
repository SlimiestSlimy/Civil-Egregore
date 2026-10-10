//! Instructions: changes to entities -- put, move, edit, remove --
//! queued in a tick's first phase and applied in its second
//! (`docs/entity_manager.md`, "Instructions").

use crate::bucket::Put;
use crate::attributes::{push_attribute, Attribute, AttributeBlock, AttributeType, Layout};
use crate::entity::{EntityId, Header};
use crate::store::SuperchunkEntities;
use coordinates::CellIndex;
use std::ops::AddAssign;

/// One instruction.
#[derive(Clone, Copy, Debug)]
enum Instruction {
    /// Puts an entity -- in place of the one with its ID where it stood,
    /// or new -- with the attributes' blocks `first..first + count` of
    /// the queue's list.
    Put {
        /// Its fixed part.
        header: Header,
        /// The cell it stood on, in its cell's superchunk: its own, if
        /// it has not moved or is new.
        from: CellIndex,
        /// The cell of another superchunk it left, if it is crossing:
        /// put here, it is removed from there once the tick's
        /// instructions are all applied.
        left: Option<CellIndex>,
        /// Its attributes' first block.
        first: u32,
        /// How many blocks they are.
        count: u32,
    },
    /// Puts a new entity on its cell or, that taken, on the first free
    /// of the cells `first_cell..first_cell + cells` of the queue's
    /// list: one that must be made is not lost to a cell taken first.
    PutOnTheFirstFree {
        /// Its fixed part, its cell the one wanted.
        header: Header,
        /// Its attributes' first block.
        first: u32,
        /// How many blocks they are.
        count: u32,
        /// The first of the cells to try after its own.
        first_cell: u32,
        /// How many there are.
        cells: u32,
    },
    /// Moves an entity to `header`'s cell, to wake at its tick, its
    /// attributes as they are -- or, the cell being the one it stands
    /// on, only sets when it next wakes.
    Move {
        /// Its fixed part, as it is to be.
        header: Header,
        /// The cell it stands on, in its cell's superchunk.
        from: CellIndex,
    },
    /// Sets one attribute of the entity whose ID is `id` standing on
    /// `at` to the blocks `first..first + count` of the queue's list,
    /// or with none removes it.
    Edit {
        /// Its ID.
        id: EntityId,
        /// Its cell.
        at: CellIndex,
        /// The attribute's type.
        kind: AttributeType,
        /// The attribute's first block.
        first: u32,
        /// How many blocks it is: none to remove it.
        count: u32,
    },
    /// Removes the entity whose ID is `id` standing on `at`.
    Remove {
        /// Its ID.
        id: EntityId,
        /// Its cell.
        at: CellIndex,
    },
}

/// Instructions queued for one superchunk, in order, and the
/// attributes' blocks they carry.
#[derive(Default)]
pub struct Instructions {
    /// The instructions.
    instructions: Vec<Instruction>,
    /// The blocks the puts and the edits carry.
    attributes: Vec<AttributeBlock>,
    /// The cells a new entity is put on the first free of.
    cells: Vec<CellIndex>,
}

impl Instructions {
    /// Queues putting `header`'s entity, with `attributes`: in place of
    /// the one with its ID standing on `from`, a cell of its cell's
    /// superchunk -- its cell itself, if it has not moved or is new.
    pub fn put(&mut self, header: Header, from: CellIndex, attributes: &[AttributeBlock]) {
        self.push(header, from, None, attributes);
    }

    /// Queues putting `header`'s entity, with `attributes`, on its cell,
    /// crossing from `left`, a cell of another superchunk: put, it is
    /// removed from `left` once the tick's instructions are all applied
    /// ([`SuperchunkEntities::settle_leavers`](crate::SuperchunkEntities::settle_leavers)).
    pub fn cross(&mut self, header: Header, left: CellIndex, attributes: &[AttributeBlock]) {
        self.push(header, header.at, Some(left), attributes);
    }

    /// Queues a put.
    fn push(&mut self, header: Header, from: CellIndex, left: Option<CellIndex>, attributes: &[AttributeBlock]) {
        debug_assert_eq!(header.at.superchunk(), from.superchunk(), "an entity put from another superchunk: a crossing");
        self.instructions.push(Instruction::Put { header, from, left, first: self.attributes.len() as u32, count: attributes.len() as u32 });
        self.attributes.extend_from_slice(attributes);
    }

    /// Queues putting `header`'s entity, new, with `attributes`, on its
    /// cell or, that taken by then, on the first free of `others` --
    /// those of them in its cell's superchunk, in the order given
    /// (`docs/entity_manager.md`, "Instructions", Tolerating a cell
    /// taken).
    pub fn put_on_the_first_free(&mut self, header: Header, others: &[CellIndex], attributes: &[AttributeBlock]) {
        let (first, first_cell) = (self.attributes.len() as u32, self.cells.len() as u32);
        self.instructions.push(Instruction::PutOnTheFirstFree { header, first, count: attributes.len() as u32, first_cell, cells: others.len() as u32 });
        self.attributes.extend_from_slice(attributes);
        self.cells.extend_from_slice(others);
    }

    /// Queues moving `header`'s entity, standing on `from` -- a cell of
    /// its cell's superchunk -- to its cell, to wake at its tick, with
    /// the attributes it has: none are carried. Its cell `from` itself,
    /// it only sleeps until then.
    pub fn move_entity(&mut self, header: Header, from: CellIndex) {
        debug_assert_eq!(header.at.superchunk(), from.superchunk(), "an entity moved from another superchunk: a crossing");
        self.instructions.push(Instruction::Move { header, from });
    }

    /// Queues setting `attribute` of the entity whose ID is `id`
    /// standing on `at` to `value`.
    pub fn set_attribute<L: Layout>(&mut self, id: EntityId, at: CellIndex, attribute: Attribute<L>, value: L) {
        let first = self.attributes.len() as u32;
        push_attribute(&mut self.attributes, attribute, value);
        self.instructions.push(Instruction::Edit { id, at, kind: attribute.attribute_type(), first, count: L::BLOCKS as u32 });
    }

    /// Queues setting the attribute `attribute` is the blocks of, of
    /// the entity whose ID is `id` standing on `at`: how one whose
    /// size varies is set.
    pub fn set_attribute_blocks(&mut self, id: EntityId, at: CellIndex, attribute: &[AttributeBlock]) {
        debug_assert!(attribute.first().is_some_and(|first| first.blocks() == attribute.len()), "an attribute as long as its type or its block length says");
        self.instructions.push(Instruction::Edit { id, at, kind: attribute[0].kind(), first: self.attributes.len() as u32, count: attribute.len() as u32 });
        self.attributes.extend_from_slice(attribute);
    }

    /// Queues removing the attribute of type `kind` of the entity
    /// whose ID is `id` standing on `at`.
    pub fn unset_attribute(&mut self, id: EntityId, at: CellIndex, kind: AttributeType) {
        self.instructions.push(Instruction::Edit { id, at, kind, first: 0, count: 0 });
    }

    /// Queues removing the entity whose ID is `id` standing on `at`.
    pub fn remove(&mut self, id: EntityId, at: CellIndex) {
        self.instructions.push(Instruction::Remove { id, at });
    }

    /// How many instructions are queued.
    pub fn len(&self) -> usize {
        self.instructions.len()
    }

    /// Whether none is.
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }

    /// Empties the queue, keeping its room.
    pub fn clear(&mut self) {
        self.instructions.clear();
        self.attributes.clear();
        self.cells.clear();
    }

    /// Applies the instructions, in order, each to the superchunk among
    /// `superchunks` -- sorted by superchunk index -- its cell is in,
    /// every wake filed no earlier than `earliest`; into `applied`
    /// (`docs/entity_manager.md`, "Instructions").
    pub fn apply(&self, superchunks: &mut [SuperchunkEntities], earliest: u64, applied: &mut InstructionsApplied) {
        self.apply_some(0..self.instructions.len(), superchunks, earliest, applied);
    }

    /// [`Instructions::apply`], of those at `some` alone, counted from
    /// the first queued: how those under a compare refused are left out
    /// (`docs/entity_manager.md`, "Instructions").
    pub fn apply_some(&self, some: std::ops::Range<usize>, superchunks: &mut [SuperchunkEntities], earliest: u64, applied: &mut InstructionsApplied) {
        for &instruction in &self.instructions[some] {
            let at = match instruction {
                Instruction::Put { header, .. } | Instruction::PutOnTheFirstFree { header, .. } | Instruction::Move { header, .. } => header.at,
                Instruction::Edit { at, .. } | Instruction::Remove { at, .. } => at,
            };
            let superchunk = at.superchunk();
            let found = match superchunks {
                [only] if only.index() == superchunk => Some(0),
                _ => superchunks.binary_search_by_key(&superchunk, SuperchunkEntities::index).ok(),
            };
            let Some(found) = found else {
                applied.lost += matches!(instruction, Instruction::Put { .. } | Instruction::PutOnTheFirstFree { .. }) as usize;
                continue;
            };
            let superchunk = &mut superchunks[found];
            match instruction {
                Instruction::Put { header, from, left, first, count } => {
                    let put = superchunk.put(earliest, header, from, Some(&self.attributes[first as usize..(first + count) as usize]));
                    match put {
                        Put::New | Put::InPlace | Put::Moved => applied.puts += 1,
                        Put::Stayed => (applied.puts, applied.stayed) = (applied.puts + 1, applied.stayed + 1),
                        Put::Refused => applied.refused += 1,
                        Put::PassedOver => applied.passed_over += 1,
                    }
                    if let (Some(left), Put::New) = (left, put) {
                        let attributes = crate::blocks_sum(&self.attributes[first as usize..(first + count) as usize]);
                        superchunk.arrived(crate::Arrival { id: header.id, left, at: header.at, attributes });
                        applied.crossed += 1;
                    }
                }
                Instruction::PutOnTheFirstFree { header, first, count, first_cell, cells } => {
                    let attributes = &self.attributes[first as usize..(first + count) as usize];
                    let others = self.cells[first_cell as usize..(first_cell + cells) as usize].iter().filter(|other| other.superchunk() == at.superchunk());
                    // Its own cell, then each other: the first it is new on.
                    let put = std::iter::once(&at).chain(others).position(|&cell| superchunk.put(earliest, Header { at: cell, ..header }, cell, Some(attributes)) == Put::New);
                    match put {
                        Some(0) => applied.puts += 1,
                        Some(_) => (applied.puts, applied.beside) = (applied.puts + 1, applied.beside + 1),
                        None => applied.refused += 1,
                    }
                }
                Instruction::Move { header, from } => match superchunk.put(earliest, header, from, None) {
                    Put::Stayed => (applied.moves, applied.stayed) = (applied.moves + 1, applied.stayed + 1),
                    Put::PassedOver => applied.passed_over += 1,
                    _ => applied.moves += 1,
                },
                Instruction::Edit { id, at, kind, first, count } => match superchunk.edit(id, at, kind, &self.attributes[first as usize..(first + count) as usize]) {
                    true => applied.edits += 1,
                    false => applied.passed_over += 1,
                },
                Instruction::Remove { id, at } => match superchunk.remove(id, at) {
                    true => applied.removes += 1,
                    false => applied.passed_over += 1,
                },
            }
        }
    }

    /// Counts the puts as lost: their superchunk holds no entities.
    pub fn count_lost(&self, applied: &mut InstructionsApplied) {
        applied.lost += self.instructions.iter().filter(|instruction| matches!(instruction, Instruction::Put { .. } | Instruction::PutOnTheFirstFree { .. })).count();
    }
}

/// What applying the instructions did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InstructionsApplied {
    /// Entities put: made, or made anew whole.
    pub puts: usize,
    /// Entities moved, or set to wake at another tick, their attributes
    /// as they were.
    pub moves: usize,
    /// Attributes set or removed, one at a time.
    pub edits: usize,
    /// Entities removed: died, or moved out of their chunk.
    pub removes: usize,
    /// Entities put in a superchunk holding no entities, so lost.
    pub lost: usize,
    /// Of the entities put or moved, those whose cell was taken: left
    /// where they stood.
    pub stayed: usize,
    /// New entities whose cell was taken: not put.
    pub refused: usize,
    /// Of the entities put, those that crossed from another superchunk.
    pub crossed: usize,
    /// Of the new entities put, those whose cell was taken and that
    /// were put on another given with it.
    pub beside: usize,
    /// Instructions for an entity no longer where it stood -- moved on,
    /// or removed, earlier in the tick: not applied.
    pub passed_over: usize,
    /// Of the entities that crossed, those turned back: the one left
    /// behind was changed by another, or removed, in the same tick.
    pub turned_back: usize,
}

impl AddAssign for InstructionsApplied {
    /// Both added up.
    fn add_assign(&mut self, other: Self) {
        self.puts += other.puts;
        self.moves += other.moves;
        self.edits += other.edits;
        self.removes += other.removes;
        self.lost += other.lost;
        self.stayed += other.stayed;
        self.refused += other.refused;
        self.crossed += other.crossed;
        self.beside += other.beside;
        self.passed_over += other.passed_over;
        self.turned_back += other.turned_back;
    }
}
