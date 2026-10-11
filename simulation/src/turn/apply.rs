//! What a rule queued, applied: an author's instructions for a
//! superchunk, each as it is come to, a compare that does not hold
//! jumping over what is under it
//! (`docs/simulation.md`, "Compare-and-write").

use super::conditional::{CountedWhenApplied, Queue, COUNTED_WHEN_APPLIED};
use super::instruction::{Instruction, Opcode, NO_COUNT};
use bitplane_manager::{Superchunk, Write, WriteOp, WritesApplied};
use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{attribute_seen, attribute_words, blocks_sum, find_attribute_numbered, Arrival, AttributeBlock, AttributeType, CollisionCells, EntityType, Header, InstructionsApplied, Put, SuperchunkEntities, BLOCK_WORDS};

/// A superchunk's collision plane as its entities keep it while their
/// instructions are applied: `layer` of `superchunk`, or none -- a
/// world with no such plane (`docs/simulation.md`, "Entities").
pub(crate) struct Standing<'a> {
    /// The superchunk.
    pub(crate) superchunk: &'a mut Superchunk,
    /// The collision plane's layer, if entities are kept in one.
    pub(crate) layer: Option<LayerType>,
}

impl Standing<'_> {
    /// Writes `to` at `at` of the plane, if there is one. Not counted
    /// among the tick's writes: it is part of the entity's instruction.
    fn write(&mut self, at: CellIndex, to: WriteOp) {
        if let Some(layer) = self.layer {
            self.superchunk.apply(layer, Write::cell(at, to), &mut WritesApplied::default());
        }
    }
}

impl CollisionCells for Standing<'_> {
    fn held(&self, at: CellIndex) -> bool {
        self.layer.is_some_and(|layer| self.superchunk.value_at(layer, at) == Some(1))
    }

    fn hold(&mut self, at: CellIndex) {
        self.write(at, WriteOp::Set);
    }

    fn free(&mut self, at: CellIndex) {
        self.write(at, WriteOp::Unset);
    }
}

/// What a thread applied in a tick's second phase.
pub(crate) struct Applied {
    /// What applying the writes did.
    pub(crate) writes: WritesApplied,
    /// What applying the instructions did.
    pub(crate) instructions: InstructionsApplied,
    /// Compares come to that held, and that did not.
    pub(crate) compared: (u64, u64),
    /// What was counted as it was applied.
    pub(crate) counted: CountedWhenApplied,
}

impl Default for Applied {
    fn default() -> Self {
        Self { writes: Default::default(), instructions: Default::default(), compared: (0, 0), counted: [0; COUNTED_WHEN_APPLIED] }
    }
}

/// Whether the compare `instruction`, of the cell `at`, holds now, in
/// `superchunk` and its `entities`: not where the cell is not hot,
/// nor if the entity no longer stands anywhere.
fn holds(instruction: &Instruction, at: CellIndex, superchunk: &mut Superchunk, entities: &SuperchunkEntities, collision: Option<LayerType>) -> bool {
    let attribute = || {
        let entity = entities.get_named(at)?;
        Some(find_attribute_numbered(entity.attributes, u64::from(instruction.small)).map(|found| &entity.attributes[found]))
    };
    match instruction.opcode {
        Opcode::CompareCell => superchunk.value_at(LayerType(instruction.first), at) == Some(u32::from(instruction.small)),
        Opcode::CompareCellFree => entities.free_for_a_new_entity(at, &Standing { superchunk, layer: collision }),
        Opcode::CompareEntity => entities.get_named(at).is_some_and(|entity| entity.header.kind.0 == instruction.first),
        Opcode::CompareAttribute => attribute().is_some_and(|blocks| attribute_seen(blocks) == instruction.first),
        Opcode::CompareAttributeWord => attribute().flatten().is_some_and(|blocks| attribute_words(blocks, instruction.word_offset()).next() == Some(instruction.first)),
        Opcode::CompareAttributeWords => attribute().flatten().is_some_and(|blocks| attribute_words(blocks, instruction.word_offset()).take(2).eq([instruction.first, instruction.second])),
        _ => unreachable!("no compare"),
    }
}

/// `blocks` blocks of the attribute of type `kind`, every word of its
/// data 0: its type in its first word, and, its size varying, how
/// many blocks it is in its second.
fn empty_attribute(kind: AttributeType, blocks: usize) -> Vec<AttributeBlock> {
    let mut empty = vec![AttributeBlock([0; BLOCK_WORDS]); blocks];
    empty[0].0[0] = kind.0;
    if kind.blocks().is_none() {
        empty[0].0[1] = blocks as u64;
    }
    empty
}

impl Queue {
    /// Counts every write of a cell as missed, and every entity to be
    /// made as lost: the superchunk they land in is not hot.
    pub(crate) fn count_missed(&self, applied: &mut WritesApplied, instructions: &mut InstructionsApplied) {
        let count = |opcodes: &[Opcode]| self.instructions.iter().filter(|instruction| opcodes.contains(&instruction.opcode)).count();
        let writes = count(&[Opcode::WriteCell]);
        (applied.writes, applied.missed) = (applied.writes + writes, applied.missed + writes as u64);
        instructions.lost += count(&[Opcode::MakeEntity, Opcode::CrossEntity]);
    }

    /// Applies the instructions at `some` -- one author's
    /// ([`super::Segment`]), whose compares are those from `compares`
    /// on -- to `superchunk` and its `entities`, in the order queued,
    /// each as it is come to. A compare that does not hold has what
    /// is under it jumped over, writes of cells, of entities and
    /// counts alike: so what one applied changed, the next compare is
    /// held against. Every wake is filed no earlier than `earliest`;
    /// `collision` is the layer entities are kept in, if the world
    /// has one ([`Standing`]).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply(&self, some: std::ops::Range<usize>, mut compares: usize, superchunk: &mut Superchunk, entities: &mut SuperchunkEntities, earliest: u64, collision: Option<LayerType>, applied: &mut Applied) {
        let here = superchunk.index();
        let (mut next, end) = (some.start, some.end);
        while next < end {
            let instruction = &self.instructions[next];
            next += 1;
            let at = instruction.at(here);
            let done = &mut applied.instructions;
            match instruction.opcode {
                Opcode::CompareEnd => {}
                Opcode::Count => applied.counted[instruction.first as usize] += 1,
                Opcode::CompareCell | Opcode::CompareCellFree | Opcode::CompareEntity | Opcode::CompareAttribute | Opcode::CompareAttributeWord | Opcode::CompareAttributeWords => {
                    let gated = self.gated[compares];
                    compares += 1;
                    if holds(instruction, at, superchunk, entities, collision) {
                        applied.compared.0 += 1;
                        continue;
                    }
                    applied.compared.1 += 1;
                    // Over all that is under it at once: to its end, or the end of what the author queued.
                    (next, compares) = (next + gated.instructions as usize, compares + gated.compares_within as usize);
                    (applied.writes.writes, applied.writes.refused) = (applied.writes.writes + gated.writes as usize, applied.writes.refused + u64::from(gated.writes));
                    // The cell not free is the entity not made.
                    done.refused += usize::from(instruction.opcode == Opcode::CompareCellFree);
                }
                Opcode::WriteCell => {
                    let layer_type = LayerType(instruction.first);
                    applied.writes.writes += 1;
                    if superchunk.value_at(layer_type, at).is_none() {
                        applied.writes.missed += 1;
                    } else {
                        superchunk.apply(layer_type, Write::cell(at, WriteOp::Put(instruction.small)), &mut applied.writes);
                        if instruction.second != NO_COUNT {
                            applied.counted[instruction.second as usize] += 1;
                        }
                    }
                }
                Opcode::MakeEntity => {
                    let header = Header { kind: EntityType(instruction.first), at, wake: instruction.second };
                    match entities.make_named(earliest, header, &[], &mut Standing { superchunk, layer: collision }) {
                        Put::New => done.puts += 1,
                        _ => done.refused += 1,
                    }
                }
                Opcode::CrossEntity => {
                    let (first, count) = ((instruction.second >> 32) as usize, instruction.second as u32 as usize);
                    let (head, attributes) = (self.carried[first].0, &self.carried[first + 1..first + 1 + count]);
                    let header = Header { kind: EntityType(head[0]), at, wake: head[1] };
                    match entities.make_named(earliest, header, attributes, &mut Standing { superchunk, layer: collision }) {
                        Put::New => {
                            entities.arrived(Arrival { left: CellIndex(instruction.first), at, attributes: blocks_sum(attributes) });
                            (done.puts, done.crossed) = (done.puts + 1, done.crossed + 1);
                        }
                        _ => done.refused += 1,
                    }
                }
                Opcode::MoveEntity => match entities.move_named(earliest, at, instruction.to(here), &mut Standing { superchunk, layer: collision }) {
                    Put::Stayed => (done.moves, done.stayed) = (done.moves + 1, done.stayed + 1),
                    Put::PassedOver => done.passed_over += 1,
                    _ => done.moves += 1,
                },
                Opcode::RemoveEntity => match entities.remove_named(at) {
                    true => done.removes += 1,
                    false => done.passed_over += 1,
                },
                Opcode::WriteWake => match entities.wake_named(earliest, at, instruction.first) {
                    true => done.wakes += 1,
                    false => done.passed_over += 1,
                },
                Opcode::WriteAttributeWord | Opcode::WriteAttributeWords | Opcode::AddAttribute | Opcode::RemoveAttribute => {
                    let written = match instruction.opcode {
                        Opcode::WriteAttributeWord => entities.write_attribute_words_named(at, u64::from(instruction.small), instruction.word_offset(), &[instruction.first]),
                        Opcode::WriteAttributeWords => entities.write_attribute_words_named(at, u64::from(instruction.small), instruction.word_offset(), &[instruction.first, instruction.second]),
                        Opcode::AddAttribute => entities.edit_named(at, AttributeType(instruction.first), &empty_attribute(AttributeType(instruction.first), instruction.second as usize)),
                        _ => entities.edit_named(at, AttributeType(instruction.first), &[]),
                    };
                    match written {
                        true => done.edits += 1,
                        false => done.passed_over += 1,
                    }
                }
            }
        }
    }
}
