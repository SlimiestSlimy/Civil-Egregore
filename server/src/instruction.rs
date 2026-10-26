//! An instruction ([`Instruction`]): one step of what a chunk's queue
//! asks of it, 32 bytes -- a compare, a write, or a skip tag, what a
//! compare that fails skips past: what a proc queues. The atomic compare
//! and the fence, the server's own, come with sets spanning chunks,
//! inserted as instructions are queued.

/// What an instruction does.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opcode {
    /// The tile holds the value, or what follows, up to the next skip
    /// tag, is skipped.
    Compare,
    /// The tile is made the value.
    Write,
    /// Where a compare that fails skips past: nothing done.
    SkipTag,
}

/// One instruction: the opcode and the tile in word 0, the type in word
/// 1, the value in words 2 and 3.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instruction {
    /// What it does.
    opcode: Opcode,
    /// The tile's corner cell: its place in the chunk, in Morton order.
    tile: u32,
    /// The type ID of the plane, as the proc named it.
    type_id: u64,
    /// The value: a plane's, the low 32 bits of the first word.
    data: [u64; 2],
}

const _: () = assert!(size_of::<Instruction>() == 32);

impl Instruction {
    /// The plane of ID `plane` holds `value` at the tile holding the
    /// cell at `place` in the chunk.
    pub fn compare(plane: u64, place: u32, value: u32) -> Self {
        Self { opcode: Opcode::Compare, tile: place, type_id: plane, data: [u64::from(value), 0] }
    }

    /// The plane of ID `plane` made `value` at the tile holding the cell
    /// at `place` in the chunk.
    pub fn write(plane: u64, place: u32, value: u32) -> Self {
        Self { opcode: Opcode::Write, tile: place, type_id: plane, data: [u64::from(value), 0] }
    }

    /// A skip tag.
    pub fn skip_tag() -> Self {
        Self { opcode: Opcode::SkipTag, tile: 0, type_id: 0, data: [0; 2] }
    }

    /// What it does.
    pub(crate) fn opcode(&self) -> Opcode {
        self.opcode
    }

    /// The place in the chunk of its tile's corner cell.
    pub(crate) fn place(&self) -> usize {
        self.tile as usize
    }

    /// Its plane's type ID.
    pub(crate) fn plane(&self) -> u64 {
        self.type_id
    }

    /// Its plane's value.
    pub(crate) fn plane_value(&self) -> u32 {
        self.data[0] as u32
    }
}
