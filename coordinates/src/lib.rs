//! TileSim's coordinates: where things are.
//!
//! Every place in the world is a Morton index -- its coordinates' bits
//! interleaved, `x` in the even bits -- one type a size: a
//! [`SuperchunkIndex`] (44 bits), a [`ChunkIndex`] (48) and a
//! [`CellIndex`] (64). Each is the next one's top bits: a cell's index
//! is its chunk's and then 16 bits for its **place** in the chunk, a
//! chunk's is its superchunk's and then 4 bits for its place in the
//! superchunk. A place is a Morton index inside the thing it is in: a
//! `usize`, the index of its bit in a bitmap's words, or of its chunk
//! among a superchunk's.
//!
//! Morton indices are what everything is stored and worked in. A cell's
//! cartesian coordinates -- its `x` and `y` -- are a [`CartesianCell`],
//! kept for geometry and drawing: whatever is cartesian says so.
//!
//! Every coordinate is a non-negative integer, counted from the world's
//! top left corner: x grows to the right and y downwards, as in a bitmap.
//! The world starts roughly in the middle of both.
//!
//! The design: `docs/coordinates.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

/// Cells along a chunk's side: a chunk's layers are bitmaps, and a
/// bitmap is this wide.
pub const CHUNK_SIDE: usize = bitmap::WIDTH;
/// Chunks along a superchunk's side.
pub const SUPERCHUNK_SIDE: usize = 4;
/// Chunks in a superchunk.
pub const CHUNKS_IN_SUPERCHUNK: usize = SUPERCHUNK_SIDE * SUPERCHUNK_SIDE;
/// Cells along a superchunk's side.
pub const SUPERCHUNK_SIDE_CELLS: u32 = (CHUNK_SIDE * SUPERCHUNK_SIDE) as u32;
/// Cells in a chunk.
pub const CELLS_IN_CHUNK: usize = CHUNK_SIDE * CHUNK_SIDE;
/// Superchunks along the world's side: as many as leave a cell's
/// coordinates a `u32` each.
pub const WORLD_SIDE_SUPERCHUNKS: u32 = 1 << (u32::BITS - SUPERCHUNK_SIDE_CELLS.trailing_zeros());

/// The superchunk roughly in the middle of the world, where it starts:
/// as far from every edge as a superchunk can be.
pub const WORLD_MIDDLE: SuperchunkIndex = SuperchunkIndex::from_cartesian(WORLD_SIDE_SUPERCHUNKS / 2, WORLD_SIDE_SUPERCHUNKS / 2);

/// Superchunks along the side of the square `count` of them make.
pub fn square_side(count: u32) -> u32 {
    (count as f64).sqrt().ceil() as u32
}

/// `count` superchunks in a square from [`WORLD_MIDDLE`], row by row
/// from its top left: how a world of that many is laid out.
pub fn square_from_middle(count: u32) -> impl Iterator<Item = SuperchunkIndex> {
    let (side, (x, y)) = (square_side(count), WORLD_MIDDLE.cartesian());
    (0..count).map(move |index| SuperchunkIndex::from_cartesian(x + index % side, y + index / side))
}

/// A cell's eight neighbours, as offsets `(dx, dy)`, row by row from the
/// top left.
pub const NEIGHBOURS: [(i32, i32); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// The bits of a cell's index giving its place in its chunk.
const CELL_PLACE_BITS: u32 = CELLS_IN_CHUNK.trailing_zeros();
/// The bits of a chunk's index giving its place in its superchunk.
const CHUNK_PLACE_BITS: u32 = CHUNKS_IN_SUPERCHUNK.trailing_zeros();
/// The bits of a cell's index below its superchunk's index: its chunk's
/// place and its own.
const CELL_PLACE_IN_SUPERCHUNK_BITS: u32 = CELL_PLACE_BITS + CHUNK_PLACE_BITS;
/// The bits a superchunk index takes.
const SUPERCHUNK_INDEX_BITS: u32 = u64::BITS - CELL_PLACE_IN_SUPERCHUNK_BITS;

/// A superchunk anywhere in the world, as its Morton index -- the top 44
/// bits of its cells' -- which is what identifies a superchunk: what
/// the directory, the cold pool and saves are sorted and named by.
/// Neighbouring superchunks mostly get near indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SuperchunkIndex(pub u64);

impl SuperchunkIndex {
    /// The superchunk `x` superchunks from the world's left edge and `y`
    /// from its top -- cartesian, each under [`WORLD_SIDE_SUPERCHUNKS`].
    pub const fn from_cartesian(x: u32, y: u32) -> Self {
        debug_assert!(x < WORLD_SIDE_SUPERCHUNKS && y < WORLD_SIDE_SUPERCHUNKS, "a superchunk outside the world");
        Self(interleave(x, y))
    }

    /// Its cartesian coordinates, in superchunks:
    /// [`SuperchunkIndex::from_cartesian`] undone.
    pub fn cartesian(self) -> (u32, u32) {
        (gather(self.0), gather(self.0 >> 1))
    }

    /// Its top left cell.
    pub fn top_left(self) -> CellIndex {
        CellIndex(self.0 << CELL_PLACE_IN_SUPERCHUNK_BITS)
    }

    /// Its chunks, in Morton order.
    pub fn chunks(self) -> impl Iterator<Item = ChunkIndex> {
        (0..CHUNKS_IN_SUPERCHUNK).map(move |place| ChunkIndex::of(self, place))
    }

    /// The superchunk `dx` across and `dy` down, if it is in the world:
    /// stepped on the index, as [`CellIndex::offset`] steps a cell.
    pub fn offset(self, dx: i32, dy: i32) -> Option<Self> {
        let x = step(self.0 & X_BITS, dx, X_BITS)?;
        let y = step(self.0 & Y_BITS, dy, Y_BITS)?;
        let moved = x | y;
        (moved >> SUPERCHUNK_INDEX_BITS == 0).then_some(Self(moved))
    }
}

/// A chunk anywhere in the world, as its Morton index: its superchunk's
/// index, then 4 bits for its place in the superchunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChunkIndex(pub u64);

impl ChunkIndex {
    /// The chunk at `place` (0 to 15, in Morton order) in `superchunk`.
    pub fn of(superchunk: SuperchunkIndex, place: usize) -> Self {
        debug_assert!(place < CHUNKS_IN_SUPERCHUNK, "chunk place {place} is outside a superchunk");
        Self(superchunk.0 << CHUNK_PLACE_BITS | place as u64)
    }

    /// The superchunk it is in.
    pub fn superchunk(self) -> SuperchunkIndex {
        SuperchunkIndex(self.0 >> CHUNK_PLACE_BITS)
    }

    /// Its place in its superchunk: 0 to 15, in Morton order.
    pub fn place(self) -> usize {
        (self.0 as usize) & (CHUNKS_IN_SUPERCHUNK - 1)
    }

    /// Its top left cell.
    pub fn top_left(self) -> CellIndex {
        CellIndex(self.0 << CELL_PLACE_BITS)
    }
}

/// A `u64` whose bits alternate, `run` set then `run` clear, from the
/// lowest: the mask that keeps each half of a spread step.
const fn alternating_runs(run: u32) -> u64 {
    let mut mask = 0u64;
    let mut bit = 0;
    while bit < u64::BITS {
        if (bit / run).is_multiple_of(2) {
            mask |= 1 << bit;
        }
        bit += 1;
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
const fn spread(value: u32) -> u64 {
    let (mut spread, mut step) = (value as u64, 0);
    while step < SPREAD_STEPS.len() {
        let (shift, mask) = SPREAD_STEPS[step];
        spread = (spread | spread << shift) & mask;
        step += 1;
    }
    spread
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

/// The even bits of `value` gathered into the low half: [`spread`]
/// undone.
fn gather(value: u64) -> u32 {
    GATHER_STEPS.iter().fold(value & alternating_runs(1), |gathered, &(shift, mask)| (gathered | gathered >> shift) & mask) as u32
}

/// `x` and `y`'s bits interleaved, `x` in the even bits.
const fn interleave(x: u32, y: u32) -> u64 {
    spread(x) | spread(y) << 1
}

/// A cell anywhere in the world, as its Morton index: what locates it
/// alone, and what the bitplanes are laid out by. From the lowest bit,
/// 16 for its place in its chunk -- its bit's index in the chunk's
/// bitmap words -- 4 for its chunk's place in its superchunk, 44 for its
/// superchunk's index. The parts are bit fields, so finding a cell's
/// superchunk, chunk and bit takes shifts and masks; [`CartesianCell`]
/// is the same cell as cartesian coordinates, for geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellIndex(pub u64);

impl CellIndex {
    /// The cell at `place` (in Morton order) in `chunk`.
    pub fn of(chunk: ChunkIndex, place: usize) -> Self {
        debug_assert!(place < CELLS_IN_CHUNK, "cell place {place} is outside a chunk");
        Self(chunk.0 << CELL_PLACE_BITS | place as u64)
    }

    /// The superchunk the cell is in.
    #[inline]
    pub fn superchunk(self) -> SuperchunkIndex {
        SuperchunkIndex(self.0 >> CELL_PLACE_IN_SUPERCHUNK_BITS)
    }

    /// The chunk the cell is in.
    #[inline]
    pub fn chunk(self) -> ChunkIndex {
        ChunkIndex(self.0 >> CELL_PLACE_BITS)
    }

    /// The cell's place in its chunk, in Morton order: its bit's index in
    /// the chunk's bitmap words.
    #[inline]
    pub fn place(self) -> usize {
        (self.0 as usize) & (CELLS_IN_CHUNK - 1)
    }

    /// The cell's place in its superchunk, in Morton order: its chunk's
    /// place, then its own.
    #[inline]
    pub fn place_in_superchunk(self) -> usize {
        (self.0 as usize) & ((1 << CELL_PLACE_IN_SUPERCHUNK_BITS) - 1)
    }

    /// The same cell as cartesian coordinates.
    pub fn cartesian(self) -> CartesianCell {
        CartesianCell { x: gather(self.0), y: gather(self.0 >> 1) }
    }

    /// The cell `dx` across and `dy` down from this one, if it is in the
    /// world. Added on the Morton index itself, one coordinate's bits at a
    /// time -- the other coordinate's bits filled with ones so a carry
    /// passes over them, or cleared so a borrow does -- with no cartesian
    /// coordinates made.
    pub fn offset(self, dx: i32, dy: i32) -> Option<Self> {
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
pub struct CartesianCell {
    /// Cells from the world's left edge.
    pub x: u32,
    /// Cells from the world's top edge.
    pub y: u32,
}

/// The place in its superchunk ([`CellIndex::place_in_superchunk`]) of
/// the cell `x` across and `y` down from the superchunk's top left.
pub fn place_from_cartesian(x: u32, y: u32) -> usize {
    debug_assert!(x < SUPERCHUNK_SIDE_CELLS && y < SUPERCHUNK_SIDE_CELLS, "a cell outside its superchunk");
    interleave(x, y) as usize
}

/// How far across and down from its superchunk's top left the cell at
/// `place` in it is: [`place_from_cartesian`] undone.
pub fn cartesian_from_place(place: usize) -> (u32, u32) {
    (gather(place as u64), gather(place as u64 >> 1))
}

/// The bits of a Morton index holding x: the even ones.
const X_BITS: u64 = alternating_runs(1);
/// The bits of a Morton index holding y: the odd ones.
const Y_BITS: u64 = !X_BITS;

/// `coordinate` -- one coordinate's bits of a Morton index, those in
/// `lane` -- moved by `by`, unless it passes the top or bottom of the
/// `u64`. A power of two (a step to a neighbour, to the next word tile
/// or chunk: by far the most common) spreads to a single bit, with no
/// spreading steps.
fn step(coordinate: u64, by: i32, lane: u64) -> Option<u64> {
    let magnitude = by.unsigned_abs();
    if magnitude == 0 {
        return Some(coordinate);
    }
    let spread_by = if magnitude.is_power_of_two() { 1 << (2 * magnitude.trailing_zeros()) } else { spread(magnitude) };
    // The y lane is the odd bits: its distance one bit higher.
    let distance = spread_by << (lane & 1 ^ 1);
    let moved = if by >= 0 { (coordinate | !lane).wrapping_add(distance) & lane } else { (coordinate.wrapping_sub(distance)) & lane };
    let wrapped = if by >= 0 { moved < coordinate } else { moved > coordinate };
    (!wrapped).then_some(moved)
}

impl From<CartesianCell> for CellIndex {
    /// The cell's Morton index.
    fn from(cell: CartesianCell) -> Self {
        Self(interleave(cell.x, cell.y))
    }
}
