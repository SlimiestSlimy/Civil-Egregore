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

/// One instruction: the opcode and the tile's index in word 0, the
/// plane's type ID in word 1, the value in words 2 and 3.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instruction {
    /// What it does.
    opcode: Opcode,
    /// The tile's index: its corner cell's Morton index in the chunk.
    tile_index: u32,
    /// The type ID of the plane, as the proc named it.
    plane_type_id: u64,
    /// The value: a plane's, the low 32 bits of the first word.
    data: [u64; 2],
}

const _: () = assert!(size_of::<Instruction>() == 32);

impl Instruction {
    /// The plane of type ID `plane_type_id` holds `value` at the tile of
    /// index `tile_index`.
    pub fn compare(plane_type_id: u64, tile_index: u32, value: u32) -> Self {
        Self { opcode: Opcode::Compare, tile_index, plane_type_id, data: [u64::from(value), 0] }
    }

    /// The plane of type ID `plane_type_id` made `value` at the tile of
    /// index `tile_index`.
    pub fn write(plane_type_id: u64, tile_index: u32, value: u32) -> Self {
        Self { opcode: Opcode::Write, tile_index, plane_type_id, data: [u64::from(value), 0] }
    }

    /// A skip tag.
    pub fn skip_tag() -> Self {
        Self { opcode: Opcode::SkipTag, tile_index: 0, plane_type_id: 0, data: [0; 2] }
    }

    /// What it does.
    pub(crate) fn opcode(&self) -> Opcode {
        self.opcode
    }

    /// Its tile's index: its corner cell's Morton index in the chunk.
    pub(crate) fn tile_index(&self) -> usize {
        self.tile_index as usize
    }

    /// Its plane's type ID.
    pub(crate) fn plane_type_id(&self) -> u64 {
        self.plane_type_id
    }

    /// Its value.
    pub(crate) fn value(&self) -> u32 {
        self.data[0] as u32
    }
}
