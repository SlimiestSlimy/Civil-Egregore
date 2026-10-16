//! What a rule queued, applied: an author's instructions for a
//! superchunk, each as it is come to, a compare that does not hold
//! jumping over what is under it
//! (`docs/simulation.md`, "Compare-and-write").

use super::conditional::{CountedWhenApplied, InstructionQueue, COUNTED_WHEN_APPLIED};
use super::instruction::{Instruction, Opcode, NO_COUNT};
use super::landing::InstructionLanding;
use bitplane_manager::{Superchunk, Write, WriteOp, WritesApplied};
use chunk_storage::LayerType;
use coordinates::CellIndex;
use entity_manager::{AttributeType, CollisionCells, EntityType, Header, InstructionsApplied, Put, SuperchunkEntities};

/// A superchunk's collision plane as its entities keep it while their
/// instructions are applied: `layer` of `superchunk`, or none -- a
/// world with no such plane (`docs/simulation.md`, "Entities").
pub(crate) struct SuperchunkCollisionPlane<'a> {
    /// The superchunk.
    pub(crate) superchunk: &'a mut Superchunk,
    /// The collision plane's layer, if entities are kept in one.
    pub(crate) layer: Option<LayerType>,
}

impl SuperchunkCollisionPlane<'_> {
    /// Writes `write_op` at `target_cell` of the plane, if there is one. Not counted
    /// among the tick's writes: it is part of the entity's instruction.
    fn write(&mut self, target_cell: CellIndex, write_op: WriteOp) {
        if let Some(layer) = self.layer {
            self.superchunk.apply(layer, Write::cell(target_cell, write_op), &mut WritesApplied::default());
        }
    }
}

impl CollisionCells for SuperchunkCollisionPlane<'_> {
    fn holds_cell(&self, collision_cell: CellIndex) -> bool {
        self.layer.is_some_and(|layer| self.superchunk.value_at(layer, collision_cell) == Some(1))
    }

    fn hold_cell(&mut self, collision_cell: CellIndex) {
        self.write(collision_cell, WriteOp::SetCell);
    }

    fn free_cell(&mut self, collision_cell: CellIndex) {
        self.write(collision_cell, WriteOp::ClearCell);
    }
}

/// What a thread applied in a tick's second phase.
pub(crate) struct PartApplied {
    /// What applying the writes did.
    pub(crate) writes: WritesApplied,
    /// What applying the instructions did.
    pub(crate) instructions: InstructionsApplied,
    /// Compares come to that held, and that did not.
    pub(crate) compares_held_and_not: (u64, u64),
    /// What was counted as it was applied.
    pub(crate) counted_when_applied: CountedWhenApplied,
    /// Acts over a border applied.
    pub(crate) acts_over_a_border: u64,
}

impl Default for PartApplied {
    fn default() -> Self {
        Self { writes: Default::default(), instructions: Default::default(), compares_held_and_not: (0, 0), counted_when_applied: [0; COUNTED_WHEN_APPLIED], acts_over_a_border: 0 }
    }
}

/// Whether the compare `instruction`, of the cell `compared_cell`, holds now, in
/// `superchunk` and its `superchunk_entities`: not where the cell is not hot,
/// nor if the entity no longer stands anywhere.
fn compare_holds(instruction: &Instruction, compared_cell: CellIndex, superchunk: &mut Superchunk, superchunk_entities: &mut SuperchunkEntities, collision_plane: Option<LayerType>) -> bool {
    let entities_read = &*superchunk_entities;
    match instruction.opcode {
        Opcode::CompareCell => superchunk.value_at(LayerType(instruction.first_operand), compared_cell) == Some(instruction.second_operand as u32),
        Opcode::CompareCellFree => entities_read.free_for_a_new_entity(compared_cell, &SuperchunkCollisionPlane { superchunk, layer: collision_plane }),
        Opcode::CompareEntity => entities_read.get_named(compared_cell).is_some_and(|entity| entity.header.entity_type.0 == instruction.first_operand),
        // Gone, it holds against nothing; there, against what is seen of the attribute, none seen of one it has not.
        Opcode::CompareAttribute => entities_read.get_named(compared_cell).is_some_and(|entity| entity.attribute_seen(AttributeType(instruction.second_operand)) == instruction.first_operand),
        Opcode::CompareAttributeMember => {
            let (member_first_byte, member_byte_count) = instruction.member();
            entities_read.get_named(compared_cell).and_then(|entity| entity.attribute_member(AttributeType(instruction.first_operand), member_first_byte, member_byte_count)) == Some(instruction.second_operand)
        }
        _ => unreachable!("no compare"),
    }
}

impl InstructionQueue {
    /// Counts every write of a cell as missed, and every entity to be
    /// made as lost: the superchunk they land in is not hot.
    pub(crate) fn count_missed(&self, writes_applied: &mut WritesApplied, instructions: &mut InstructionsApplied) {
        let count = |opcodes: &[Opcode]| self.instructions.iter().filter(|instruction| opcodes.contains(&instruction.opcode)).count();
        let writes = count(&[Opcode::WriteCell]);
        (writes_applied.writes, writes_applied.missed) = (writes_applied.writes + writes, writes_applied.missed + writes as u64);
        instructions.lost += count(&[Opcode::MakeEntity]);
    }

    /// Whether the instructions at `segment_range`, queued for one superchunk,
    /// are of the entity named by `stood_cell` alone: each names it, and
    /// none moves it, or is a count or a compare's end. Such a
    /// segment follows the entity whole, if it crosses a border.
    pub(crate) fn names_alone(&self, segment_range: std::ops::Range<usize>, stood_cell: CellIndex) -> bool {
        self.instructions[segment_range].iter().all(|instruction| match instruction.opcode {
            Opcode::CompareEnd | Opcode::Count => true,
            Opcode::MoveEntity | Opcode::CrossEntity => false,
            _ => instruction.names_an_entity() && instruction.cell_in(stood_cell.superchunk()) == stood_cell,
        })
    }

    /// Jumps over all that is under the compare `instruction`, which
    /// does not hold -- to its end, or the end of what the author
    /// queued -- `next_instruction` being the instruction after it and `compare_index`
    /// its place among the compares: the writes under it refused.
    #[inline]
    fn jump_over(&self, instruction: &Instruction, next_instruction: &mut usize, compare_index: &mut usize, part_applied: &mut PartApplied) {
        let gated_by_compare = self.gated_by_compares[*compare_index];
        part_applied.compares_held_and_not.1 += 1;
        (*next_instruction, *compare_index) = (*next_instruction + gated_by_compare.instructions as usize, *compare_index + 1 + gated_by_compare.compares_within as usize);
        (part_applied.writes.writes, part_applied.writes.writes_refused) = (part_applied.writes.writes + gated_by_compare.writes as usize, part_applied.writes.writes_refused + u64::from(gated_by_compare.writes));
        // The cell not free is the entity not made.
        part_applied.instructions.puts_refused += usize::from(instruction.opcode == Opcode::CompareCellFree);
    }

    /// Applies the instructions at `segment_range` -- one author's
    /// ([`super::Segment`]), whose compares are those from `compare_index`
    /// on -- each to the superchunk it lands in, of `instruction_landing`, and its
    /// entities, in the order queued, each as it is come to. A
    /// compare that does not hold has what is under it jumped over,
    /// writes of cells, of entities and counts alike, wherever each
    /// lands: so what one applied changed, the next compare is held
    /// against. One landing in a superchunk that is not hot does not
    /// hold, and a write landing in one is missed. Every wake is
    /// filed no earlier than `earliest_wake`; `collision_plane` is the layer
    /// entities are kept in, if the world has one ([`SuperchunkCollisionPlane`]).
    pub(crate) fn apply<L: InstructionLanding>(&self, segment_range: std::ops::Range<usize>, mut compare_index: usize, instruction_landing: &mut L, earliest_wake: u64, collision_plane: Option<LayerType>, part_applied: &mut PartApplied) {
        let (mut next_instruction, segment_end) = (segment_range.start, segment_range.end);
        while next_instruction < segment_end {
            let instruction = &self.instructions[next_instruction];
            next_instruction += 1;
            if L::LANDS_IN_SEVERAL && instruction.opcode == Opcode::CrossEntity {
                // The one instruction of two superchunks: the entity leaves one and stands in the other at once, with all it has.
                let crossing = instruction_landing.of_crossing(instruction).map(|(crossing_from_cell, crossing_to_cell, left_superchunk, entered_superchunk)| (crossing_from_cell, crossing_to_cell, left_superchunk.1.cross_named(entered_superchunk.1, earliest_wake, crossing_from_cell, crossing_to_cell, &mut SuperchunkCollisionPlane { superchunk: entered_superchunk.0, layer: collision_plane })));
                let instructions_done = &mut part_applied.instructions;
                match crossing {
                    Some((crossed_from_cell, crossed_to_cell, Put::MovedToCell)) => {
                        (instructions_done.moves, instructions_done.border_crossings) = (instructions_done.moves + 1, instructions_done.border_crossings + 1);
                        instruction_landing.note_crossed(crossed_from_cell, crossed_to_cell);
                    }
                    Some((_, _, Put::TurnedBack)) => (instructions_done.moves, instructions_done.turned_back) = (instructions_done.moves + 1, instructions_done.turned_back + 1),
                    Some(_) => instructions_done.passed_over += 1,
                    // The superchunk it would come to is not hot: it stays.
                    None => instructions_done.lost += 1,
                }
                continue;
            }
            // One match an instruction where every slot's superchunk is hot, as in a queue of one slot: all but acts over a border.
            let landed_superchunk = if L::LANDS_IN_SEVERAL && instruction.names_an_entity() {
                instruction_landing.of_named(instruction)
            } else {
                instruction_landing.of_slot(instruction.outbox_slot()).map(|(superchunk, slot_entities)| {
                    let slot_cell = instruction.cell_in(superchunk.superchunk_index());
                    (superchunk, slot_entities, slot_cell)
                })
            };
            let Some((superchunk, landed_entities, landed_cell)) = landed_superchunk else {
                // Its superchunk not hot: a compare does not hold, a cell's write is missed, an entity to be made lost, anything else of nobody.
                match instruction.opcode {
                    Opcode::CompareEnd => {}
                    Opcode::Count => part_applied.counted_when_applied[instruction.first_operand as usize] += 1,
                    Opcode::WriteCell => (part_applied.writes.writes, part_applied.writes.missed) = (part_applied.writes.writes + 1, part_applied.writes.missed + 1),
                    Opcode::MakeEntity => part_applied.instructions.lost += 1,
                    _ if instruction.is_a_compare() => self.jump_over(instruction, &mut next_instruction, &mut compare_index, part_applied),
                    _ => part_applied.instructions.passed_over += 1,
                }
                continue;
            };
            let applied_instructions = &mut part_applied.instructions;
            match instruction.opcode {
                Opcode::CompareEnd => {}
                Opcode::Count => part_applied.counted_when_applied[instruction.first_operand as usize] += 1,
                Opcode::CompareCell | Opcode::CompareCellFree | Opcode::CompareEntity | Opcode::CompareAttribute | Opcode::CompareAttributeMember => {
                    if compare_holds(instruction, landed_cell, superchunk, landed_entities, collision_plane) {
                        part_applied.compares_held_and_not.0 += 1;
                        compare_index += 1;
                        continue;
                    }
                    self.jump_over(instruction, &mut next_instruction, &mut compare_index, part_applied);
                }
                Opcode::WriteCell => {
                    let layer_type = LayerType(instruction.first_operand);
                    part_applied.writes.writes += 1;
                    if superchunk.value_at(layer_type, landed_cell).is_none() {
                        part_applied.writes.missed += 1;
                    } else {
                        superchunk.apply(layer_type, Write::cell(landed_cell, WriteOp::Put(instruction.value_written())), &mut part_applied.writes);
                        if instruction.counted_under() != NO_COUNT {
                            part_applied.counted_when_applied[instruction.counted_under() as usize] += 1;
                        }
                    }
                }
                Opcode::MakeEntity => {
                    let header = Header { entity_type: EntityType(instruction.first_operand), stood_cell: landed_cell, wake: instruction.second_operand };
                    match landed_entities.make_named(earliest_wake, header, &[], &mut SuperchunkCollisionPlane { superchunk, layer: collision_plane }) {
                        Put::NewlyPut => applied_instructions.puts += 1,
                        _ => applied_instructions.puts_refused += 1,
                    }
                }
                // In a slot's queue: its author landed in one superchunk, so it crosses to none.
                Opcode::CrossEntity => applied_instructions.passed_over += 1,
                // The cell it is moved to is one of the superchunk it is named in.
                Opcode::MoveEntity => match landed_entities.move_named(earliest_wake, landed_cell, instruction.destination_cell(landed_cell.superchunk()), &mut SuperchunkCollisionPlane { superchunk, layer: collision_plane }) {
                    Put::TurnedBack => (applied_instructions.moves, applied_instructions.turned_back) = (applied_instructions.moves + 1, applied_instructions.turned_back + 1),
                    Put::PassedOver => applied_instructions.passed_over += 1,
                    _ => applied_instructions.moves += 1,
                },
                Opcode::RemoveEntity => match landed_entities.remove_named(landed_cell) {
                    true => applied_instructions.entities_removed += 1,
                    false => applied_instructions.passed_over += 1,
                },
                Opcode::WriteWake => match landed_entities.wake_named(earliest_wake, landed_cell, instruction.first_operand) {
                    true => applied_instructions.wakes += 1,
                    false => applied_instructions.passed_over += 1,
                },
                Opcode::WriteAttributeMember | Opcode::WriteAttribute | Opcode::RemoveAttribute => {
                    let was_written = match instruction.opcode {
                        Opcode::WriteAttributeMember => {
                            let (member_first_byte, member_byte_count) = instruction.member();
                            landed_entities.write_attribute_bytes_named(landed_cell, AttributeType(instruction.first_operand), member_first_byte, member_byte_count, instruction.second_operand)
                        }
                        Opcode::WriteAttribute => {
                            let (carried_first, count) = ((instruction.second_operand >> 32) as usize, instruction.second_operand as u32 as usize);
                            let attribute = &self.carried_blocks[carried_first..carried_first + count];
                            landed_entities.edit_named(landed_cell, attribute[0].attribute_type(), attribute)
                        }
                        _ => landed_entities.edit_named(landed_cell, AttributeType(instruction.first_operand), &[]),
                    };
                    match was_written {
                        true => applied_instructions.edits += 1,
                        false => applied_instructions.passed_over += 1,
                    }
                }
            }
        }
    }
}
