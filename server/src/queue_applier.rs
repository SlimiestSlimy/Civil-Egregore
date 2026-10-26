//! The queue applier ([`apply_queue`]): a chunk's queue of instructions applied to
//! it, in order, each as it is come to -- so a compare holds against what
//! the writes before it made. A compare that fails skips past the next
//! skip tag. The queue is trusted: its planes are planes, its values fit
//! them, its writes are under compares; what queued it checked.

use crate::hot_chunk::HotChunk;
use crate::queued_instruction::{Instruction, Opcode};
use crate::plane_shapes::PlaneShapes;
use utilities::coordinates::CellIndex;

/// `instructions`, the queue of `chunk`, applied to it in order.
pub fn apply_queue(chunk: &mut HotChunk, instructions: &[Instruction], plane_shapes: &PlaneShapes) {
    let mut next = 0;
    while let Some(instruction) = instructions.get(next) {
        next += 1;
        let cell = CellIndex::from_chunk_place(chunk.chunk_index(), instruction.tile_index());
        match instruction.opcode() {
            Opcode::Compare if chunk.read_tile(instruction.plane_type_id(), cell) != instruction.value() => {
                let to_the_tag = instructions[next..].iter().position(|skipped| skipped.opcode() == Opcode::SkipTag);
                debug_assert!(to_the_tag.is_some(), "a compare with no skip tag after it");
                next += to_the_tag.map_or(instructions.len() - next, |skipped| skipped + 1);
            }
            Opcode::Compare | Opcode::SkipTag => {}
            Opcode::Write => {
                let shape = plane_shapes.of(instruction.plane_type_id()).expect("a plane's type ID");
                chunk.write_tile(instruction.plane_type_id(), shape, cell, instruction.value());
            }
        }
    }
}
