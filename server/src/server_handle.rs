//! What a proc sees of the server ([`ServerHandle`]): the hot chunks'
//! tiles read, its chunk's random stream, and compare-and-write sets queued --
//! each built whole ([`ServerHandle::set`]) and queued at its end, its
//! compares before its writes, never a write without a compare.

use crate::queued_instruction::Instruction;
use crate::server_manager::ServerManager;
use protocol::type_kinds::{Kind, PlaneWidth, TileSide};
use utilities::coordinates::{CellIndex, ChunkIndex};
use utilities::rng::RandomSource;

/// A tile of a chunk not hot, so not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotHot;

/// The server, as a proc's call sees it: one chunk's tick.
pub struct ServerHandle<'tick> {
    /// The server's world, as the tick began: read alone.
    server_manager: &'tick ServerManager,
    /// The chunk ticking.
    chunk_index: ChunkIndex,
    /// The chunk's random stream, taken for the tick.
    random_stream: &'tick mut RandomSource,
    /// The chunk's queue: every set ended.
    queue: &'tick mut Vec<Instruction>,
    /// The set being built: queued at its end, left behind if never
    /// ended.
    set_being_built: Vec<Instruction>,
}

impl<'tick> ServerHandle<'tick> {
    /// The tick of the chunk of index `chunk_index`, of the world
    /// `server_manager`, drawing from `random_stream`, its sets queued in
    /// `queue`.
    pub fn new(server_manager: &'tick ServerManager, chunk_index: ChunkIndex, random_stream: &'tick mut RandomSource, queue: &'tick mut Vec<Instruction>) -> Self {
        Self { server_manager, chunk_index, random_stream, queue, set_being_built: Vec::new() }
    }

    /// The number the plane of type ID `plane_type_id` holds at the tile
    /// holding `cell`: or the chunk is not hot. A type ID of no plane, a
    /// bug of the proc's, panics.
    pub fn read_tile(&self, plane_type_id: u64, cell: CellIndex) -> Result<u32, NotHot> {
        self.plane_of(plane_type_id);
        self.server_manager.hot_chunk(cell.chunk()).map(|chunk| chunk.read_tile(plane_type_id, cell)).ok_or(NotHot)
    }

    /// The chunk's random stream.
    pub fn random_stream(&mut self) -> &mut RandomSource {
        self.random_stream
    }

    /// A compare-and-write set begun: its compares, then its writes, then
    /// its end, where it is queued.
    pub fn set(&mut self) -> SetWithoutCompare<'_, 'tick> {
        self.set_being_built.clear();
        SetWithoutCompare { handle: self }
    }

    /// The width and the tile side of the plane of type ID
    /// `plane_type_id`: a type ID of no plane panics.
    fn plane_of(&self, plane_type_id: u64) -> (PlaneWidth, TileSide) {
        let &Kind::Plane { width, tile_side } = self.server_manager.type_registry().type_of(plane_type_id).kind() else { panic!("{plane_type_id:#x} is no plane's type ID") };
        (width, tile_side)
    }

    /// The instruction `make` makes -- a compare or a write -- of the
    /// plane of type ID `plane_type_id`, the tile holding `cell`, and
    /// `value`, put in the set being built: a type ID of no plane, a
    /// value wider than the plane's numbers, or a cell of another chunk
    /// panics.
    fn put_in_set(&mut self, make: fn(u64, u32, u32) -> Instruction, plane_type_id: u64, cell: CellIndex, value: u32) {
        let (width, tile_side) = self.plane_of(plane_type_id);
        assert!(u64::from(value) >> width.bits() == 0, "{value} is wider than the {} bits of {plane_type_id:#x}", width.bits());
        assert_eq!(cell.chunk(), self.chunk_index, "a cell of another chunk: sets across chunks come later");
        let tile_corner = cell.place() - cell.place() % tile_side.tile_cells() as usize;
        self.set_being_built.push(make(plane_type_id, tile_corner as u32, value));
    }
}

/// A set begun, no compare in it yet: a compare first.
#[must_use = "a set never ended is never queued"]
pub struct SetWithoutCompare<'set, 'tick> {
    /// The handle building it.
    handle: &'set mut ServerHandle<'tick>,
}

impl<'set, 'tick> SetWithoutCompare<'set, 'tick> {
    /// The set holds only if the plane of type ID `plane_type_id` holds
    /// `value` at the tile holding `cell`.
    pub fn compare(self, plane_type_id: u64, cell: CellIndex, value: u32) -> SetComparing<'set, 'tick> {
        self.handle.put_in_set(Instruction::compare, plane_type_id, cell, value);
        SetComparing { handle: self.handle }
    }
}

/// A set comparing: more compares, or its first write.
#[must_use = "a set never ended is never queued"]
pub struct SetComparing<'set, 'tick> {
    /// The handle building it.
    handle: &'set mut ServerHandle<'tick>,
}

impl<'set, 'tick> SetComparing<'set, 'tick> {
    /// The set holds only if the plane of type ID `plane_type_id` holds
    /// `value` at the tile holding `cell` too.
    pub fn compare(self, plane_type_id: u64, cell: CellIndex, value: u32) -> Self {
        self.handle.put_in_set(Instruction::compare, plane_type_id, cell, value);
        self
    }

    /// If the set holds, the plane of type ID `plane_type_id` is made
    /// `value` at the tile holding `cell`.
    pub fn write(self, plane_type_id: u64, cell: CellIndex, value: u32) -> SetWriting<'set, 'tick> {
        self.handle.put_in_set(Instruction::write, plane_type_id, cell, value);
        SetWriting { handle: self.handle }
    }
}

/// A set writing: more writes, or its end.
#[must_use = "a set never ended is never queued"]
pub struct SetWriting<'set, 'tick> {
    /// The handle building it.
    handle: &'set mut ServerHandle<'tick>,
}

impl SetWriting<'_, '_> {
    /// If the set holds, the plane of type ID `plane_type_id` is made
    /// `value` at the tile holding `cell` too.
    pub fn write(self, plane_type_id: u64, cell: CellIndex, value: u32) -> Self {
        self.handle.put_in_set(Instruction::write, plane_type_id, cell, value);
        self
    }

    /// The set ended: queued whole, a skip tag after it.
    pub fn end(self) {
        self.handle.queue.extend_from_slice(&self.handle.set_being_built);
        self.handle.queue.push(Instruction::skip_tag());
    }
}
