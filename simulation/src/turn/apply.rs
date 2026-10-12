//! What a rule queued, applied: an author's instructions for a
//! superchunk, each as it is come to, a compare that does not hold
//! jumping over what is under it
//! (`docs/simulation.md`, "Compare-and-write").

use super::conditional::{CountedWhenApplied, Queue, COUNTED_WHEN_APPLIED};
use super::instruction::{Instruction, Opcode, NO_COUNT};
use super::landing::Landing;
use bitplane_manager::{Superchunk, Write, WriteOp, WritesApplied};
use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{AttributeType, CollisionCells, EntityType, Header, InstructionsApplied, Put, SuperchunkEntities};

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
    /// Acts over a border applied.
    pub(crate) acts_over_a_border: u64,
}

impl Default for Applied {
    fn default() -> Self {
        Self { writes: Default::default(), instructions: Default::default(), compared: (0, 0), counted: [0; COUNTED_WHEN_APPLIED], acts_over_a_border: 0 }
    }
}

/// Whether the compare `instruction`, of the cell `at`, holds now, in
/// `superchunk` and its `entities`: not where the cell is not hot,
/// nor if the entity no longer stands anywhere.
fn holds(instruction: &Instruction, at: CellIndex, superchunk: &mut Superchunk, entities: &mut SuperchunkEntities, collision: Option<LayerType>) -> bool {
    let entities = &*entities;
    match instruction.opcode {
        Opcode::CompareCell => superchunk.value_at(LayerType(instruction.first), at) == Some(instruction.second as u32),
        Opcode::CompareCellFree => entities.free_for_a_new_entity(at, &Standing { superchunk, layer: collision }),
        Opcode::CompareEntity => entities.get_named(at).is_some_and(|entity| entity.header.kind.0 == instruction.first),
        // Gone, it holds against nothing; there, against what is seen of the attribute, none seen of one it has not.
        Opcode::CompareAttribute => entities.get_named(at).is_some_and(|entity| entity.attribute_seen(AttributeType(instruction.second)) == instruction.first),
        Opcode::CompareAttributeMember => {
            let (offset, bytes) = instruction.member();
            entities.get_named(at).and_then(|entity| entity.attribute_member(AttributeType(instruction.first), offset, bytes)) == Some(instruction.second)
        }
        _ => unreachable!("no compare"),
    }
}

impl Queue {
    /// Counts every write of a cell as missed, and every entity to be
    /// made as lost: the superchunk they land in is not hot.
    pub(crate) fn count_missed(&self, applied: &mut WritesApplied, instructions: &mut InstructionsApplied) {
        let count = |opcodes: &[Opcode]| self.instructions.iter().filter(|instruction| opcodes.contains(&instruction.opcode)).count();
        let writes = count(&[Opcode::WriteCell]);
        (applied.writes, applied.missed) = (applied.writes + writes, applied.missed + writes as u64);
        instructions.lost += count(&[Opcode::MakeEntity]);
    }

    /// Whether the instructions at `some`, queued for one superchunk,
    /// are of the entity named by `stood` alone: each names it, and
    /// none moves it, or is a count or a compare's end. Such a
    /// segment follows the entity whole, if it crosses a border.
    pub(crate) fn names_alone(&self, some: std::ops::Range<usize>, stood: CellIndex) -> bool {
        self.instructions[some].iter().all(|instruction| match instruction.opcode {
            Opcode::CompareEnd | Opcode::Count => true,
            Opcode::MoveEntity | Opcode::CrossEntity => false,
            _ => instruction.names_an_entity() && instruction.at(stood.superchunk()) == stood,
        })
    }

    /// Jumps over all that is under the compare `instruction`, which
    /// does not hold -- to its end, or the end of what the author
    /// queued -- `next` being the instruction after it and `compares`
    /// its place among the compares: the writes under it refused.
    #[inline]
    fn jump_over(&self, instruction: &Instruction, next: &mut usize, compares: &mut usize, applied: &mut Applied) {
        let gated = self.gated[*compares];
        applied.compared.1 += 1;
        (*next, *compares) = (*next + gated.instructions as usize, *compares + 1 + gated.compares_within as usize);
        (applied.writes.writes, applied.writes.refused) = (applied.writes.writes + gated.writes as usize, applied.writes.refused + u64::from(gated.writes));
        // The cell not free is the entity not made.
        applied.instructions.refused += usize::from(instruction.opcode == Opcode::CompareCellFree);
    }

    /// Applies the instructions at `some` -- one author's
    /// ([`super::Segment`]), whose compares are those from `compares`
    /// on -- each to the superchunk it lands in, of `lands`, and its
    /// entities, in the order queued, each as it is come to. A
    /// compare that does not hold has what is under it jumped over,
    /// writes of cells, of entities and counts alike, wherever each
    /// lands: so what one applied changed, the next compare is held
    /// against. One landing in a superchunk that is not hot does not
    /// hold, and a write landing in one is missed. Every wake is
    /// filed no earlier than `earliest`; `collision` is the layer
    /// entities are kept in, if the world has one ([`Standing`]).
    pub(crate) fn apply<L: Landing>(&self, some: std::ops::Range<usize>, mut compares: usize, lands: &mut L, earliest: u64, collision: Option<LayerType>, applied: &mut Applied) {
        let (mut next, end) = (some.start, some.end);
        while next < end {
            let instruction = &self.instructions[next];
            next += 1;
            if L::SEVERAL && instruction.opcode == Opcode::CrossEntity {
                // The one instruction of two superchunks: the entity leaves one and stands in the other at once, with all it has.
                let crossing = lands.of_crossing(instruction).map(|(at, to, from, into)| (at, to, from.1.cross_named(into.1, earliest, at, to, &mut Standing { superchunk: into.0, layer: collision })));
                let done = &mut applied.instructions;
                match crossing {
                    Some((at, to, Put::Moved)) => {
                        (done.moves, done.crossed) = (done.moves + 1, done.crossed + 1);
                        lands.crossed(at, to);
                    }
                    Some((_, _, Put::Stayed)) => (done.moves, done.stayed) = (done.moves + 1, done.stayed + 1),
                    Some(_) => done.passed_over += 1,
                    // The superchunk it would come to is not hot: it stays.
                    None => done.lost += 1,
                }
                continue;
            }
            // One match an instruction where every slot's superchunk is hot, as in a queue of one slot: all but acts over a border.
            let landed = if L::SEVERAL && instruction.names_an_entity() {
                lands.of_named(instruction)
            } else {
                lands.of_slot(instruction.slot()).map(|(superchunk, entities)| {
                    let at = instruction.at(superchunk.index());
                    (superchunk, entities, at)
                })
            };
            let Some((superchunk, entities, at)) = landed else {
                // Its superchunk not hot: a compare does not hold, a cell's write is missed, an entity to be made lost, anything else of nobody.
                match instruction.opcode {
                    Opcode::CompareEnd => {}
                    Opcode::Count => applied.counted[instruction.first as usize] += 1,
                    Opcode::WriteCell => (applied.writes.writes, applied.writes.missed) = (applied.writes.writes + 1, applied.writes.missed + 1),
                    Opcode::MakeEntity => applied.instructions.lost += 1,
                    _ if instruction.is_a_compare() => self.jump_over(instruction, &mut next, &mut compares, applied),
                    _ => applied.instructions.passed_over += 1,
                }
                continue;
            };
            let done = &mut applied.instructions;
            match instruction.opcode {
                Opcode::CompareEnd => {}
                Opcode::Count => applied.counted[instruction.first as usize] += 1,
                Opcode::CompareCell | Opcode::CompareCellFree | Opcode::CompareEntity | Opcode::CompareAttribute | Opcode::CompareAttributeMember => {
                    if holds(instruction, at, superchunk, entities, collision) {
                        applied.compared.0 += 1;
                        compares += 1;
                        continue;
                    }
                    self.jump_over(instruction, &mut next, &mut compares, applied);
                }
                Opcode::WriteCell => {
                    let layer_type = LayerType(instruction.first);
                    applied.writes.writes += 1;
                    if superchunk.value_at(layer_type, at).is_none() {
                        applied.writes.missed += 1;
                    } else {
                        superchunk.apply(layer_type, Write::cell(at, WriteOp::Put(instruction.value_written())), &mut applied.writes);
                        if instruction.counted_under() != NO_COUNT {
                            applied.counted[instruction.counted_under() as usize] += 1;
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
                // In a slot's queue: its author landed in one superchunk, so it crosses to none.
                Opcode::CrossEntity => done.passed_over += 1,
                // The cell it is moved to is one of the superchunk it is named in.
                Opcode::MoveEntity => match entities.move_named(earliest, at, instruction.to(at.superchunk()), &mut Standing { superchunk, layer: collision }) {
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
                Opcode::WriteAttributeMember | Opcode::WriteAttribute | Opcode::RemoveAttribute => {
                    let written = match instruction.opcode {
                        Opcode::WriteAttributeMember => {
                            let (offset, bytes) = instruction.member();
                            entities.write_attribute_bytes_named(at, AttributeType(instruction.first), offset, bytes, instruction.second)
                        }
                        Opcode::WriteAttribute => {
                            let (first, count) = ((instruction.second >> 32) as usize, instruction.second as u32 as usize);
                            let attribute = &self.carried[first..first + count];
                            entities.edit_named(at, attribute[0].kind(), attribute)
                        }
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
