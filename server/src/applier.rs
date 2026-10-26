//! The applier ([`apply`]): a chunk's queue of instructions applied to
//! it, in order, each as it is come to -- so a compare holds against what
//! the writes before it made. A compare that fails skips past the next
//! skip tag. The queue is trusted: its planes are planes, its values fit
//! them, its writes are under compares; what queued it checked.

use crate::hot_chunk::HotChunk;
use crate::instruction::{Instruction, Opcode};
use utilities::coordinates::CellIndex;

/// Each plane's width and tile size, by type ID: what a write making a
/// plane in a chunk gives it.
pub struct PlaneShapes {
    /// Type ID, bits a number, cells a tile's side: sorted by type ID.
    shapes: Vec<(u64, u32, u32)>,
}

impl PlaneShapes {
    /// The shapes of the planes `shapes` -- type ID, width in bits, tile
    /// size -- each type once.
    pub fn new(mut shapes: Vec<(u64, u32, u32)>) -> Self {
        shapes.sort_unstable_by_key(|&(type_id, ..)| type_id);
        Self { shapes }
    }

    /// The width and tile size of the plane of ID `plane`, one of them.
    fn of(&self, plane: u64) -> (u32, u32) {
        let place = self.shapes.binary_search_by_key(&plane, |&(type_id, ..)| type_id).expect("a plane's type ID");
        (self.shapes[place].1, self.shapes[place].2)
    }
}

/// `instructions`, the queue of `chunk`, applied to it in order.
pub fn apply(chunk: &mut HotChunk, instructions: &[Instruction], plane_shapes: &PlaneShapes) {
    let mut next = 0;
    while let Some(instruction) = instructions.get(next) {
        next += 1;
        let cell = CellIndex::from_chunk_place(chunk.chunk_index(), instruction.place());
        match instruction.opcode() {
            Opcode::Compare if chunk.read_tile(instruction.plane(), cell) != instruction.plane_value() => {
                let to_the_tag = instructions[next..].iter().position(|skipped| skipped.opcode() == Opcode::SkipTag);
                debug_assert!(to_the_tag.is_some(), "a compare with no skip tag after it");
                next += to_the_tag.map_or(instructions.len() - next, |skipped| skipped + 1);
            }
            Opcode::Compare | Opcode::SkipTag => {}
            Opcode::Write => {
                let (width_bits, tile_size) = plane_shapes.of(instruction.plane());
                chunk.write_tile(instruction.plane(), width_bits, tile_size, cell, instruction.plane_value());
            }
        }
    }
}
