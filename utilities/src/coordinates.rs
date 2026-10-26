//! Civil Egregore's coordinates: where things are. Every place is a
//! Morton index -- [`ChunkIndex`], [`CellIndex`], the first the second's
//! top bits -- and whatever is cartesian says so ([`CellCartesian`]).

/// Cells along a chunk's side.
pub const CHUNK_SIDE_CELLS: u32 = 1024;
/// Cells in a chunk.
pub const CELLS_IN_CHUNK: usize = (CHUNK_SIDE_CELLS * CHUNK_SIDE_CELLS) as usize;
/// Chunks along the world's side: as many as leave a cell's coordinates
/// a `u32` each.
pub const WORLD_SIDE_CHUNKS: u32 = 1 << (u32::BITS - CHUNK_SIDE_CELLS.trailing_zeros());

/// The chunk roughly in the middle of the world, where it starts: as far
/// from every edge as a chunk can be.
pub const WORLD_MIDDLE: ChunkIndex = ChunkIndex::from_cartesian(WORLD_SIDE_CHUNKS / 2, WORLD_SIDE_CHUNKS / 2);
/// The cell in the world's middle: [`WORLD_MIDDLE`]'s top left.
pub const WORLD_MIDDLE_CELL: CellCartesian = CellCartesian { x: WORLD_SIDE_CHUNKS / 2 * CHUNK_SIDE_CELLS, y: WORLD_SIDE_CHUNKS / 2 * CHUNK_SIDE_CELLS };

/// Chunks along the side of the square `count` of them make.
pub fn square_side(count: u32) -> u32 {
    // The whole root of one less, and one: the least side whose square holds them.
    if count == 0 { 0 } else { (count - 1).isqrt() + 1 }
}

/// A cell's eight neighbours, as offsets `(dx, dy)`, row by row from the
/// top left.
pub const NEIGHBOUR_OFFSETS: [(i32, i32); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// The bits of a cell's index giving its place in its chunk.
const CELL_PLACE_BITS: u32 = CELLS_IN_CHUNK.trailing_zeros();
/// The bits a chunk index takes.
const CHUNK_INDEX_BITS: u32 = u64::BITS - CELL_PLACE_BITS;

/// A chunk anywhere in the world, as its Morton index: the top 44 bits of
/// its cells'.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChunkIndex(pub u64);

impl ChunkIndex {
    /// The chunk `x` chunks from the world's left edge and `y` from its
    /// top -- cartesian, each under [`WORLD_SIDE_CHUNKS`].
    pub const fn from_cartesian(x: u32, y: u32) -> Self {
        debug_assert!(x < WORLD_SIDE_CHUNKS && y < WORLD_SIDE_CHUNKS, "a chunk outside the world");
        Self(morton_index_from_cartesian(x, y))
    }

    /// Its cartesian coordinates, in chunks: [`ChunkIndex::from_cartesian`]
    /// undone.
    pub fn cartesian(self) -> (u32, u32) {
        (gather_even_bits(self.0), gather_even_bits(self.0 >> 1))
    }

    /// Its top left cell.
    pub fn top_left(self) -> CellIndex {
        CellIndex(self.0 << CELL_PLACE_BITS)
    }

    /// The chunk `dx` across and `dy` down, if it is in the world: stepped
    /// on the index, as [`CellIndex::at_cartesian_offset`] steps a cell.
    pub fn at_cartesian_offset(self, dx: i32, dy: i32) -> Option<Self> {
        let x = step(self.0 & X_BITS, dx, X_BITS)?;
        let y = step(self.0 & Y_BITS, dy, Y_BITS)?;
        let moved_index = x | y;
        (moved_index >> CHUNK_INDEX_BITS == 0).then_some(Self(moved_index))
    }
}

/// A `u64` whose bits alternate, `run_length` set then `run_length` clear, from the
/// lowest: the mask that keeps each half of a spread step.
const fn alternating_runs(run_length: u32) -> u64 {
    let mut mask = 0u64;
    let mut bit_index = 0;
    while bit_index < u64::BITS {
        if (bit_index / run_length).is_multiple_of(2) {
            mask |= 1 << bit_index;
        }
        bit_index += 1;
    }
    mask
}

/// The spread steps, widest first: each shifts every other run of bits
/// up by the run's length, halving the runs, until each bit sits alone.
const SPREAD_STEPS: [(u32, u64); 5] = [
    (16, alternating_runs(16)),
    (8, alternating_runs(8)),
    (4, alternating_runs(4)),
    (2, alternating_runs(2)),
    (1, alternating_runs(1)),
];

/// `value`'s bits spread to every other bit: bit `i` to bit `2i`.
const fn spread_to_even_bits(value: u32) -> u64 {
    let (mut spread_value, mut step) = (value as u64, 0);
    while step < SPREAD_STEPS.len() {
        let (run_shift, mask) = SPREAD_STEPS[step];
        spread_value = (spread_value | spread_value << run_shift) & mask;
        step += 1;
    }
    spread_value
}

/// The gather steps, narrowest first: [`SPREAD_STEPS`] undone, each
/// shifting every other run of bits down by the run's length, doubling
/// the runs, until the bits sit together in the low half.
const GATHER_STEPS: [(u32, u64); 5] = [
    (1, alternating_runs(2)),
    (2, alternating_runs(4)),
    (4, alternating_runs(8)),
    (8, alternating_runs(16)),
    (16, alternating_runs(32)),
];

/// The even bits of `value` gathered into the low half: [`spread_to_even_bits`]
/// undone.
fn gather_even_bits(value: u64) -> u32 {
    GATHER_STEPS.iter().fold(value & alternating_runs(1), |gathered_bits, &(run_shift, mask)| (gathered_bits | gathered_bits >> run_shift) & mask) as u32
}

/// `x` and `y`'s bits interleaved, `x` in the even bits.
const fn morton_index_from_cartesian(x: u32, y: u32) -> u64 {
    spread_to_even_bits(x) | spread_to_even_bits(y) << 1
}

/// A cell anywhere in the world, as its Morton index: from the lowest
/// bit, 20 for its place in its chunk, 44 for its chunk's index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellIndex(pub u64);

impl CellIndex {
    /// The cell at `place` (in Morton order) in `chunk`.
    pub fn from_chunk_place(chunk: ChunkIndex, place: usize) -> Self {
        debug_assert!(place < CELLS_IN_CHUNK, "cell place {place} is outside a chunk");
        Self(chunk.0 << CELL_PLACE_BITS | place as u64)
    }

    /// The chunk the cell is in.
    #[inline]
    pub fn chunk(self) -> ChunkIndex {
        ChunkIndex(self.0 >> CELL_PLACE_BITS)
    }

    /// The cell's place in its chunk, in Morton order.
    #[inline]
    pub fn place(self) -> usize {
        (self.0 as usize) & (CELLS_IN_CHUNK - 1)
    }

    /// The same cell as cartesian coordinates.
    pub fn cartesian(self) -> CellCartesian {
        CellCartesian { x: gather_even_bits(self.0), y: gather_even_bits(self.0 >> 1) }
    }

    /// The cell `dx` across and `dy` down from this one, if it is in the
    /// world: stepped on the index itself.
    pub fn at_cartesian_offset(self, dx: i32, dy: i32) -> Option<Self> {
        let x = step(self.0 & X_BITS, dx, X_BITS)?;
        let y = step(self.0 & Y_BITS, dy, Y_BITS)?;
        Some(Self(x | y))
    }
}

/// A cell's cartesian coordinates: counted in cells from the world's top
/// left. Every pair of `u32`s is a cell. For geometry and drawing; the
/// same cell's Morton index, what everything else uses, is a
/// [`CellIndex`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CellCartesian {
    /// Cells from the world's left edge.
    pub x: u32,
    /// Cells from the world's top edge.
    pub y: u32,
}

/// A square of the world's cells: its top left cell and how many cells
/// along its side. Whatever it is kept as is not said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellArea {
    /// Its top left cell.
    pub top_left: CellCartesian,
    /// Cells along its side.
    pub side_cells: u32,
}

impl CellArea {
    /// Whether `cell` is in it.
    pub fn contains(self, cell: CellCartesian) -> bool {
        cell.x.wrapping_sub(self.top_left.x) < self.side_cells && cell.y.wrapping_sub(self.top_left.y) < self.side_cells
    }

    /// Its cells, row by row.
    pub fn cells(self) -> impl Iterator<Item = CellCartesian> {
        (0..self.side_cells).flat_map(move |down| (0..self.side_cells).map(move |across| CellCartesian { x: self.top_left.x + across, y: self.top_left.y + down }))
    }
}

/// The place in its chunk ([`CellIndex::place`]) of the cell `x` across
/// and `y` down from the chunk's top left.
pub fn place_from_cartesian(x: u32, y: u32) -> usize {
    debug_assert!(x < CHUNK_SIDE_CELLS && y < CHUNK_SIDE_CELLS, "a cell outside its chunk");
    morton_index_from_cartesian(x, y) as usize
}

/// How far across and down from its chunk's top left the cell at
/// `place` in it is: [`place_from_cartesian`] undone.
pub fn cartesian_from_place(place: usize) -> (u32, u32) {
    (gather_even_bits(place as u64), gather_even_bits(place as u64 >> 1))
}

/// The bits of a Morton index holding x: the even ones.
const X_BITS: u64 = alternating_runs(1);
/// The bits of a Morton index holding y: the odd ones.
const Y_BITS: u64 = !X_BITS;

/// `coordinate_bits` -- one coordinate's bits of a Morton index, those in
/// `lane_mask` -- moved by `signed_offset`, unless it passes the top or bottom of the
/// `u64`.
fn step(coordinate_bits: u64, signed_offset: i32, lane_mask: u64) -> Option<u64> {
    let offset_magnitude = signed_offset.unsigned_abs();
    if offset_magnitude == 0 {
        return Some(coordinate_bits);
    }
    let spread_by = if offset_magnitude.is_power_of_two() { 1 << (2 * offset_magnitude.trailing_zeros()) } else { spread_to_even_bits(offset_magnitude) };
    // The y lane is the odd bits: its distance one bit higher.
    let lane_distance = spread_by << (lane_mask & 1 ^ 1);
    let moved_bits = if signed_offset >= 0 { (coordinate_bits | !lane_mask).wrapping_add(lane_distance) & lane_mask } else { (coordinate_bits.wrapping_sub(lane_distance)) & lane_mask };
    let wrapped_past_end = if signed_offset >= 0 { moved_bits < coordinate_bits } else { moved_bits > coordinate_bits };
    (!wrapped_past_end).then_some(moved_bits)
}

impl From<CellCartesian> for CellIndex {
    /// The cell's Morton index.
    fn from(cell: CellCartesian) -> Self {
        Self(morton_index_from_cartesian(cell.x, cell.y))
    }
}
