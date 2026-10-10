//! Writes into the hot bitplanes, batched: the only way cells change.
//! A [`Write`] is queued for a layer type ([`WriteQueues`]) and changes
//! nothing until applied (`docs/bitplane_manager.md`, "Writes").

use crate::superchunk_layer::SuperchunkLayer;
use crate::{contains, BitmapArena};
use bitmap::morton::morton_index;
use chunk_storage::{LayerType, Wide, Width};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, CHUNK_SIDE, SUPERCHUNK_SIDE_CELLS};

/// What a write does to each cell it covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteOp {
    /// Makes the cell set.
    Set,
    /// Makes the cell clear.
    Unset,
    /// Makes the cell set if it was clear, clear if it was set.
    Flip,
    /// Makes the cell's number this: for a layer more than a bit a cell
    /// wide, its low bits as many as the layer has. On a layer of a bit
    /// a cell, set if it is not 0.
    Put(u16),
}

/// The cells a write covers, from its anchor cell; the parts past the
/// world's edges are cut off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// The anchor cell alone.
    Cell,
    /// `width` by `height` cells, the anchor the top left one; no cells
    /// if either is 0.
    Rect {
        /// Cells across.
        width: u8,
        /// Cells down.
        height: u8,
    },
    /// Every cell no farther than `radius` from the anchor, centre to
    /// centre: the anchor alone at radius 0.
    Disc {
        /// The farthest a cell may be, in cells.
        radius: u8,
    },
}

/// One write: `op` over every cell of `shape`, from `at`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C, packed(4))]
pub struct Write {
    /// The anchor cell: the cell, the rectangle's top left, the disc's
    /// centre.
    pub at: CellIndex,
    /// What is done to each cell.
    pub op: WriteOp,
    /// Which cells.
    pub shape: Shape,
}

const _: () = assert!(size_of::<Write>() == 16, "a write is fixed in size, 16 bytes");

impl Write {
    /// `op` on the cell `at`.
    pub fn cell(at: CellIndex, op: WriteOp) -> Self {
        Self { at, op, shape: Shape::Cell }
    }

    /// `value` made the number of the cell `at`, of a wide plane: no
    /// more than a plane of that width holds.
    pub fn value<W: Width>(plane: Wide<W>, at: CellIndex, value: u32) -> Self {
        debug_assert!(value <= plane.most(), "{value} in a plane of {} bits a cell", W::BITS);
        Self { at, op: WriteOp::Put(value as u16), shape: Shape::Cell }
    }

    /// The smallest rectangle holding the write's cells: its first and
    /// last columns and rows, if it has any cell.
    fn bounds(self) -> Option<([u32; 2], [u32; 2])> {
        let (at, shape) = ({ self.at }.cartesian(), self.shape);
        match shape {
            Shape::Cell => Some(([at.x, at.x], [at.y, at.y])),
            Shape::Rect { width, height } => (width > 0 && height > 0)
                .then(|| ([at.x, at.x.saturating_add(width as u32 - 1)], [at.y, at.y.saturating_add(height as u32 - 1)])),
            Shape::Disc { radius } => {
                let radius = radius as u32;
                Some(([at.x.saturating_sub(radius), at.x.saturating_add(radius)], [at.y.saturating_sub(radius), at.y.saturating_add(radius)]))
            }
        }
    }

    /// Whether the write covers the cell at `(x, y)`, which is inside its
    /// bounds.
    fn covers(self, x: u32, y: u32) -> bool {
        match self.shape {
            Shape::Cell | Shape::Rect { .. } => true,
            Shape::Disc { radius } => {
                let at = { self.at }.cartesian();
                let (dx, dy) = (x.abs_diff(at.x) as u64, y.abs_diff(at.y) as u64);
                dx * dx + dy * dy <= radius as u64 * radius as u64
            }
        }
    }
}

/// What applying the queued writes did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WritesApplied {
    /// Writes applied.
    pub writes: usize,
    /// Cells that changed.
    pub changed: u64,
    /// Cells covered in bitmaps that were not hot, so left unwritten.
    pub missed: u64,
}

impl std::ops::AddAssign for WritesApplied {
    /// Both added up.
    fn add_assign(&mut self, other: Self) {
        self.writes += other.writes;
        self.changed += other.changed;
        self.missed += other.missed;
    }
}

/// Writes queued, a queue a layer type, sorted by type, each in the
/// order queued: a rule writes to few types, so finding one's queue is
/// a search of a few.
#[derive(Default)]
pub struct WriteQueues {
    /// The queues, by type.
    queues: Vec<(LayerType, Vec<Write>)>,
}

impl WriteQueues {
    /// Queues `write` into `layer_type`'s queue.
    pub fn push(&mut self, layer_type: LayerType, write: Write) {
        let queue = match self.queues.binary_search_by_key(&layer_type, |(queued, _)| *queued) {
            Ok(at) => at,
            Err(at) => {
                self.queues.insert(at, (layer_type, Vec::new()));
                at
            }
        };
        self.queues[queue].1.push(write);
    }

    /// How many writes are queued.
    pub fn len(&self) -> usize {
        self.queues.iter().map(|(_, writes)| writes.len()).sum()
    }

    /// Whether no write is queued.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every queue, by type.
    pub fn iter(&self) -> impl Iterator<Item = (LayerType, &[Write])> {
        self.queues.iter().map(|(layer_type, writes)| (*layer_type, writes.as_slice()))
    }

    /// Empties every queue, keeping its room.
    pub fn clear(&mut self) {
        self.queues.iter_mut().for_each(|(_, writes)| writes.clear());
    }
}

/// Cells along a chunk's side, as a coordinate.
const CHUNK_SIDE_U32: u32 = CHUNK_SIDE as u32;

impl Write {
    /// The superchunks the write's cells lie in: one, or for a shape
    /// across a border up to four.
    pub fn superchunks(self) -> impl Iterator<Item = SuperchunkIndex> {
        let ([left, right], [top, bottom]) = self.bounds().unwrap_or(([1, 0], [1, 0]));
        let superchunk_of = |coordinate: u32| coordinate / SUPERCHUNK_SIDE_CELLS;
        let (columns, rows) = (superchunk_of(left)..=superchunk_of(right), superchunk_of(top)..=superchunk_of(bottom));
        let empty = left > right;
        rows.flat_map(move |y| columns.clone().map(move |x| (x, y)))
            .filter(move |_| !empty)
            .map(|(x, y)| SuperchunkIndex::from_cartesian(x, y))
    }
}

/// Counts the cells of the part of `write` in `superchunk` -- which has
/// no bitmap in use -- as missed.
pub fn count_missed(superchunk: SuperchunkIndex, write: Write, applied: &mut WritesApplied) {
    apply_in(None, superchunk, LayerType(0), write, applied);
}

/// Applies the part of `write`, to `layer_type`'s bitplane, that lies in
/// `superchunk`, whose allocations are `layers` -- `None` if it has none
/// in use: a cell straight from its cell index, a shape chunk by chunk
/// over its bounds there.
pub(crate) fn apply_in(layers: Option<&mut [SuperchunkLayer]>, superchunk: SuperchunkIndex, layer_type: LayerType, write: Write, applied: &mut WritesApplied) {
    let layer = layers.and_then(|layers| {
        let at = layers.binary_search_by_key(&layer_type, |layer| layer.layer_type).ok()?;
        Some(&mut layers[at])
    });
    // What the write does to one cell of a hot bucket: whether it changed.
    let put = |layer: &mut SuperchunkLayer, chunk, cell| match (write.op, layer.layer_type.bits()) {
        (WriteOp::Set, 1) => layer.put_cell(chunk, cell, true),
        (WriteOp::Unset, 1) => layer.put_cell(chunk, cell, false),
        (WriteOp::Flip, 1) => layer.put_cell(chunk, cell, !layer.get(chunk, cell)),
        (WriteOp::Put(value), 1) => layer.put_cell(chunk, cell, value != 0),
        (WriteOp::Put(value), _) => layer.put_value(chunk, cell, value as u32),
        // A wide cell is given a number: set is 1, unset 0, and a flip sets a cell at 0 and clears any other.
        (WriteOp::Set, _) => layer.put_value(chunk, cell, 1),
        (WriteOp::Unset, _) => layer.put_value(chunk, cell, 0),
        (WriteOp::Flip, _) => layer.put_value(chunk, cell, (layer.value(chunk, cell) == 0) as u32),
    };
    let at = { write.at };
    if write.shape == Shape::Cell {
        if at.superchunk() != superchunk {
            return;
        }
        let chunk = at.chunk().place();
        match layer {
            Some(layer) if contains(layer.flags.hot, chunk) => {
                applied.changed += put(layer, chunk, at.place()) as u64;
            }
            _ => applied.missed += 1,
        }
        return;
    }
    let Some(([left, right], [top, bottom])) = write.bounds() else {
        return;
    };
    // The bounds' part inside this superchunk.
    let CellCartesian { x: first_x, y: first_y } = superchunk.top_left().cartesian();
    let (left, top) = (left.max(first_x), top.max(first_y));
    let (right, bottom) = (right.min(first_x + (SUPERCHUNK_SIDE_CELLS - 1)), bottom.min(first_y + (SUPERCHUNK_SIDE_CELLS - 1)));
    if left > right || top > bottom {
        return;
    }
    let mut layer = layer;
    let chunk_of = |coordinate: u32| coordinate / CHUNK_SIDE_U32;
    for chunk_y in chunk_of(top)..=chunk_of(bottom) {
        for chunk_x in chunk_of(left)..=chunk_of(right) {
            // The bounds' part inside this chunk.
            let (x0, y0) = (left.max(chunk_x * CHUNK_SIDE_U32), top.max(chunk_y * CHUNK_SIDE_U32));
            let (x1, y1) = (right.min(chunk_x * CHUNK_SIDE_U32 + (CHUNK_SIDE_U32 - 1)), bottom.min(chunk_y * CHUNK_SIDE_U32 + (CHUNK_SIDE_U32 - 1)));
            let covered = (y0..=y1).flat_map(|y| (x0..=x1).map(move |x| (x, y))).filter(|&(x, y)| write.covers(x, y));
            let chunk = CellIndex::from(CellCartesian { x: x0, y: y0 }).chunk().place();
            match layer.as_deref_mut() {
                Some(layer) if contains(layer.flags.hot, chunk) => {
                    for (x, y) in covered {
                        let cell = morton_index(x as u8, y as u8);
                        applied.changed += put(layer, chunk, cell) as u64;
                    }
                }
                _ => applied.missed += covered.count() as u64,
            }
        }
    }
}

impl BitmapArena {
    /// Queues `write` into `layer_type`'s queue, to be applied with every
    /// other queued, in order, by [`BitmapArena::apply`]: until then no
    /// cell changes. For writes from outside a tick -- setting up, say;
    /// a simulation's rules queue theirs through its own queues.
    pub fn queue(&mut self, layer_type: LayerType, write: Write) {
        self.queued.push(layer_type, write);
    }

    /// How many writes are queued, over every layer type.
    pub fn queued(&self) -> usize {
        self.queued.len()
    }

    /// Applies every queue, type by type, each write in the order queued,
    /// and empties them: where writes to one bitplane overlap, the latest
    /// wins. A write covering cells of bitmaps that are not hot leaves
    /// those cells out.
    pub fn apply(&mut self) -> WritesApplied {
        let mut applied = WritesApplied::default();
        let queued = std::mem::take(&mut self.queued);
        for (layer_type, writes) in queued.iter() {
            applied.writes += writes.len();
            for &write in writes {
                for superchunk in write.superchunks() {
                    let entry = self.lookup.superchunk(&self.directory, superchunk).ok();
                    let layers = entry.map(|entry| self.directory[entry].layers.as_mut_slice());
                    apply_in(layers, superchunk, layer_type, write, &mut applied);
                }
            }
        }
        self.queued = queued;
        self.queued.clear();
        applied
    }
}
