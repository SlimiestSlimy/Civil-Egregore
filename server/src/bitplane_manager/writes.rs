//! Writes into the hot bitplanes, batched: the only way cells change.
//! A [`Write`] is queued for a layer type ([`WriteQueues`]) and changes
//! nothing until applied (`docs/bitplane_manager.md`, "Writes").

use crate::bitplane_manager::superchunk_layer::SuperchunkLayer;
use crate::bitplane_manager::{contains, BitmapArena};
use utilities::bitmap::morton::morton_index;
use crate::chunk_storage::{LayerType};
use crate::bitplane_manager::{WidePlane, Width};
use utilities::coordinates::{CellCartesian, CellIndex, SuperchunkIndex, CHUNK_SIDE, SUPERCHUNK_SIDE_CELLS};

/// What a write does to each cell it covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteOp {
    /// Makes the cell set.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    SetCell,
    /// Makes the cell clear.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    ClearCell,
    /// Makes the cell set if it was clear, clear if it was set.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    FlipCell,
    /// Makes the cell's number this: for a layer more than a bit a cell
    /// wide, its low bits as many as the layer has. On a layer of a bit
    /// a cell, set if it is not 0.
    WriteValue(u32),
}

/// The cells a write covers, from its anchor cell; the parts past the
/// world's edges are cut off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteShape {
    /// The anchor cell alone.
    Cell,
    /// `width` by `height` cells, the anchor the top left one; no cells
    /// if either is 0.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    RectangleOfCells {
        /// Cells across.
        width: u8,
        /// Cells down.
        height: u8,
    },
    /// Every cell no farther than `radius` from the anchor, centre to
    /// centre: the anchor alone at radius 0.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    DiscOfCells {
        /// The farthest a cell may be, in cells.
        radius_in_cells: u8,
    },
}

/// One write: `op` over every cell of `shape`, from `at`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C, packed(4))]
pub struct Write {
    /// The anchor cell: the cell, the rectangle's top left, the disc's
    /// centre.
    pub anchor_cell: CellIndex,
    /// What is done to each cell.
    pub write_op: WriteOp,
    /// Which cells.
    pub write_shape: WriteShape,
}

const _: () = assert!(size_of::<Write>() == 20, "a write is fixed in size, 20 bytes: a number of up to 32 bits in it");

impl Write {
    /// `write_op` on the cell `anchor_cell`.
    pub fn cell(anchor_cell: CellIndex, write_op: WriteOp) -> Self {
        Self { anchor_cell, write_op, write_shape: WriteShape::Cell }
    }

    /// `value` made the number of the cell `anchor_cell`, of a wide plane: no
    /// more than a plane of that width holds.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn value<W: Width>(wide_plane: WidePlane<W>, anchor_cell: CellIndex, value: u32) -> Self {
        debug_assert!(value <= wide_plane.most_value(), "{value} in a plane of {} bits a cell", W::BITS_A_CELL);
        Self { anchor_cell, write_op: WriteOp::WriteValue(value), write_shape: WriteShape::Cell }
    }

    /// The smallest rectangle holding the write's cells: its first and
    /// last columns and rows, if it has any cell.
    fn cartesian_bounds(self) -> Option<([u32; 2], [u32; 2])> {
        let (anchor_cartesian, write_shape) = ({ self.anchor_cell }.cartesian(), self.write_shape);
        match write_shape {
            WriteShape::Cell => Some(([anchor_cartesian.x, anchor_cartesian.x], [anchor_cartesian.y, anchor_cartesian.y])),
            WriteShape::RectangleOfCells { width, height } => (width > 0 && height > 0)
                .then(|| ([anchor_cartesian.x, anchor_cartesian.x.saturating_add(width as u32 - 1)], [anchor_cartesian.y, anchor_cartesian.y.saturating_add(height as u32 - 1)])),
            WriteShape::DiscOfCells { radius_in_cells: radius } => {
                let unsigned_radius = radius as u32;
                Some(([anchor_cartesian.x.saturating_sub(unsigned_radius), anchor_cartesian.x.saturating_add(unsigned_radius)], [anchor_cartesian.y.saturating_sub(unsigned_radius), anchor_cartesian.y.saturating_add(unsigned_radius)]))
            }
        }
    }

    /// Whether the write covers the cell at `(x, y)`, which is inside its
    /// bounds.
    fn covers_cartesian(self, x: u32, y: u32) -> bool {
        match self.write_shape {
            WriteShape::Cell | WriteShape::RectangleOfCells { .. } => true,
            WriteShape::DiscOfCells { radius_in_cells: radius } => {
                let anchor_cartesian = { self.anchor_cell }.cartesian();
                let (dx, dy) = (x.abs_diff(anchor_cartesian.x) as u64, y.abs_diff(anchor_cartesian.y) as u64);
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
    pub cells_changed: u64,
    /// Cells covered in bitmaps that were not hot, so left unwritten.
    pub missed: u64,
    /// Writes that said what their rule saw at the cell, and were not
    /// applied: the cell no longer held it.
    pub writes_refused: u64,
}

impl std::ops::AddAssign for WritesApplied {
    /// Both added up.
    fn add_assign(&mut self, other_applied: Self) {
        self.writes += other_applied.writes;
        self.cells_changed += other_applied.cells_changed;
        self.missed += other_applied.missed;
        self.writes_refused += other_applied.writes_refused;
    }
}

/// Writes queued, a queue a layer type, sorted by type, each in the
/// order queued: a rule writes to few types, so finding one's queue is
/// a search of a few.
#[derive(Default)]
pub struct WriteQueues {
    /// The queues, by type.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    queues_by_type: Vec<(LayerType, Vec<Write>)>,
}

impl WriteQueues {
    /// Queues `write` into `layer_type`'s queue.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn push(&mut self, layer_type: LayerType, write: Write) {
        let queue_index = match self.queues_by_type.binary_search_by_key(&layer_type, |(queued_type, _)| *queued_type) {
            Ok(found_index) => found_index,
            Err(insert_index) => {
                self.queues_by_type.insert(insert_index, (layer_type, Vec::new()));
                insert_index
            }
        };
        self.queues_by_type[queue_index].1.push(write);
    }

    /// How many writes are queued.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn len(&self) -> usize {
        self.queues_by_type.iter().map(|(_, writes)| writes.len()).sum()
    }

    /// Every queue, by type.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn iter(&self) -> impl Iterator<Item = (LayerType, &[Write])> {
        self.queues_by_type.iter().map(|(layer_type, writes)| (*layer_type, writes.as_slice()))
    }

    /// Empties every queue, keeping its room.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn clear(&mut self) {
        self.queues_by_type.iter_mut().for_each(|(_, writes)| writes.clear());
    }
}

/// Cells along a chunk's side, as a coordinate.
const CHUNK_SIDE_U32: u32 = CHUNK_SIDE as u32;

impl Write {
    /// The superchunks the write's cells lie in: one, or for a shape
    /// across a border up to four.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn superchunks(self) -> impl Iterator<Item = SuperchunkIndex> {
        let ([left_column, right_column], [top_row, bottom_row]) = self.cartesian_bounds().unwrap_or(([1, 0], [1, 0]));
        let superchunk_of = |cell_coordinate: u32| cell_coordinate / SUPERCHUNK_SIDE_CELLS;
        let (superchunk_columns, superchunk_rows) = (superchunk_of(left_column)..=superchunk_of(right_column), superchunk_of(top_row)..=superchunk_of(bottom_row));
        let no_cells = left_column > right_column;
        superchunk_rows.flat_map(move |y| superchunk_columns.clone().map(move |x| (x, y)))
            .filter(move |_| !no_cells)
            .map(|(x, y)| SuperchunkIndex::from_cartesian(x, y))
    }
}

/// Applies the part of `write`, to `layer_type`'s bitplane, that lies in
/// `superchunk`, whose allocations are `layers` -- `None` if it has none
/// in use: a cell straight from its cell index, a shape chunk by chunk
/// over its bounds there.
pub(crate) fn apply_in(layers: Option<&mut [SuperchunkLayer]>, superchunk: SuperchunkIndex, layer_type: LayerType, write: Write, writes_applied: &mut WritesApplied) {
    let layer = layers.and_then(|layers| {
        let layer_index = layers.binary_search_by_key(&layer_type, |layer| layer.layer_type).ok()?;
        Some(&mut layers[layer_index])
    });
    // What the write does to one cell of a hot bucket: whether it changed.
    let write_one_cell = |layer: &mut SuperchunkLayer, chunk, cell| match (write.write_op, layer.layer_type.width_bits()) {
        (WriteOp::SetCell, 1) => layer.write_cell(chunk, cell, true),
        (WriteOp::ClearCell, 1) => layer.write_cell(chunk, cell, false),
        (WriteOp::FlipCell, 1) => layer.write_cell(chunk, cell, !layer.get(chunk, cell)),
        (WriteOp::WriteValue(value), 1) => layer.write_cell(chunk, cell, value != 0),
        (WriteOp::WriteValue(value), _) => layer.write_value(chunk, cell, value),
        // A wide cell is given a number: set is 1, unset 0, and a flip sets a cell at 0 and clears any other.
        (WriteOp::SetCell, _) => layer.write_value(chunk, cell, 1),
        (WriteOp::ClearCell, _) => layer.write_value(chunk, cell, 0),
        (WriteOp::FlipCell, _) => layer.write_value(chunk, cell, (layer.value(chunk, cell) == 0) as u32),
    };
    let anchor_cell = { write.anchor_cell };
    if write.write_shape == WriteShape::Cell {
        if anchor_cell.superchunk() != superchunk {
            return;
        }
        let chunk = anchor_cell.chunk().place();
        match layer {
            Some(layer) if contains(layer.chunk_flags.hot, chunk) => {
                writes_applied.cells_changed += write_one_cell(layer, chunk, anchor_cell.place()) as u64;
            }
            _ => writes_applied.missed += 1,
        }
        return;
    }
    let Some(([left_column, right_column], [top_row, bottom_row])) = write.cartesian_bounds() else {
        return;
    };
    // The bounds' part inside this superchunk.
    let CellCartesian { x: first_x, y: first_y } = superchunk.top_left().cartesian();
    let (left_in_superchunk, top_in_superchunk) = (left_column.max(first_x), top_row.max(first_y));
    let (right_in_superchunk, bottom_in_superchunk) = (right_column.min(first_x + (SUPERCHUNK_SIDE_CELLS - 1)), bottom_row.min(first_y + (SUPERCHUNK_SIDE_CELLS - 1)));
    if left_in_superchunk > right_in_superchunk || top_in_superchunk > bottom_in_superchunk {
        return;
    }
    let mut layer = layer;
    let chunk_of = |cell_coordinate: u32| cell_coordinate / CHUNK_SIDE_U32;
    for chunk_y in chunk_of(top_in_superchunk)..=chunk_of(bottom_in_superchunk) {
        for chunk_x in chunk_of(left_in_superchunk)..=chunk_of(right_in_superchunk) {
            // The bounds' part inside this chunk.
            let (chunk_left_x, chunk_top_y) = (left_in_superchunk.max(chunk_x * CHUNK_SIDE_U32), top_in_superchunk.max(chunk_y * CHUNK_SIDE_U32));
            let (chunk_right_x, chunk_bottom_y) = (right_in_superchunk.min(chunk_x * CHUNK_SIDE_U32 + (CHUNK_SIDE_U32 - 1)), bottom_in_superchunk.min(chunk_y * CHUNK_SIDE_U32 + (CHUNK_SIDE_U32 - 1)));
            let covered_cartesian = (chunk_top_y..=chunk_bottom_y).flat_map(|y| (chunk_left_x..=chunk_right_x).map(move |x| (x, y))).filter(|&(x, y)| write.covers_cartesian(x, y));
            let chunk = CellIndex::from(CellCartesian { x: chunk_left_x, y: chunk_top_y }).chunk().place();
            match layer.as_deref_mut() {
                Some(layer) if contains(layer.chunk_flags.hot, chunk) => {
                    for (x, y) in covered_cartesian {
                        let cell = morton_index(x as u8, y as u8);
                        writes_applied.cells_changed += write_one_cell(layer, chunk, cell) as u64;
                    }
                }
                _ => writes_applied.missed += covered_cartesian.count() as u64,
            }
        }
    }
}

impl BitmapArena {
    /// Queues `write` into `layer_type`'s queue, to be applied with every
    /// other queued, in order, by [`BitmapArena::apply`]: until then no
    /// cell changes. For writes from outside a tick -- setting up, say;
    /// a simulation's rules queue theirs through its own queues.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn queue_write(&mut self, layer_type: LayerType, write: Write) {
        self.queued_writes.push(layer_type, write);
    }

    /// How many writes are queued, over every layer type.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn queued_write_count(&self) -> usize {
        self.queued_writes.len()
    }

    /// Applies every queue, type by type, each write in the order queued,
    /// and empties them: where writes to one bitplane overlap, the latest
    /// wins. A write covering cells of bitmaps that are not hot leaves
    /// those cells out.
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn apply(&mut self) -> WritesApplied {
        let mut writes_applied = WritesApplied::default();
        let taken_queues = std::mem::take(&mut self.queued_writes);
        for (layer_type, writes) in taken_queues.iter() {
            writes_applied.writes += writes.len();
            for &write in writes {
                for superchunk in write.superchunks() {
                    let entry_index = self.lookup.superchunk(&self.directory, superchunk).ok();
                    let layers = entry_index.map(|found_entry_index| self.directory[found_entry_index].layers.as_mut_slice());
                    apply_in(layers, superchunk, layer_type, write, &mut writes_applied);
                }
            }
        }
        self.queued_writes = taken_queues;
        self.queued_writes.clear();
        writes_applied
    }
}
