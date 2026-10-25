//! Planes read wherever their cells lie, across the hot superchunks
//! ([`HotPlaneReader`]): a cell's number, the windows about a cell, aligned
//! tiles as they lie in their chunk, and whether a tile holds anything.
//! A cell of a superchunk not hot is [`NotHot`]; a plane or a chunk a
//! hot superchunk does not keep reads as all clear.

use super::hot_plane::HotPlane;
use super::{NotHot, COARSEST_SCALE, COUNTS_IN_COARSEST, COUNT_TILE_CELLS};
use crate::hot_superchunks::hot_superchunk::HotSuperchunk;
use protocol::type_registry::PlaneType;
use std::cell::Cell;
use utilities::bitmap::window::{in_word_tile, left_columns, rows_from_morton, top_rows, window, PLACE_IN_WORD_TILE, WORD_TILE_SIDE};
use utilities::bitmap::{CellWords, BITS_PER_WORD, WIDTH, WORDS};
use utilities::cache::prefetch_cache_line;
use utilities::coordinates::{CellIndex, SuperchunkIndex};

/// A chunk with no cell set, of a plane of a bit a cell.
static NO_CELLS: CellWords = [0; WORDS];

/// Up to 8x8 cells of one plane, row by row: cell `(x, y)` from the
/// window's top left at bit `y * 8 + x` ([`utilities::bitmap::window`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Window {
    /// The cells the plane holds at: of hot superchunks only.
    pub cells_set: u64,
    /// The cells of hot superchunks: in the world, and in the window.
    pub hot: u64,
}

/// Cells along a small tile's side: the smallest aligned tile read.
pub const SMALL_TILE_SIDE: u32 = 4;

/// Cells in a small tile, and the bits of one.
pub const SMALL_TILE_CELLS: u32 = SMALL_TILE_SIDE * SMALL_TILE_SIDE;

/// Small tiles read together: two by two, a word of bits.
const SMALL_TILES_TOGETHER: u32 = u64::BITS / SMALL_TILE_CELLS;

/// The bits of one small tile, at the foot of a word.
const SMALL_TILE: u64 = (1 << SMALL_TILE_CELLS) - 1;

/// Four small tiles, two by two, of one plane: 8x8 cells in Morton order
/// from the top left of the first, so each small tile is 16 bits of it,
/// as it lies in its chunk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SmallTiles {
    /// The cells the plane holds at: of hot superchunks only.
    pub cells_set: u64,
    /// The cells of hot superchunks, among those asked for.
    pub hot: u64,
}

/// The bits of a cell's place in its chunk holding x: the even ones...
const PLACE_X: usize = 0x5555;

/// ...and holding y: the odd ones.
const PLACE_Y: usize = 0xaaaa;

/// A word tile's index in its chunk -- its word's index -- is a Morton
/// index over the chunk's 32x32 word tiles: its x in the even bits...
const WORD_TILE_X: usize = 0x155;

/// ...and its y in the odd ones.
const WORD_TILE_Y: usize = 0x2aa;

/// The word tile at `tile` of `cells`, row by row; nothing hot if none.
fn word_tile(cells: Option<&CellWords>, tile: usize) -> Window {
    cells.map_or_else(Window::default, |cells| Window { cells_set: rows_from_morton(cells[tile]), hot: u64::MAX })
}

/// Reads planes across the hot superchunks, remembering the superchunk
/// it found last -- one a thread -- so runs of reads in one superchunk
/// search nothing.
pub(crate) struct HotPlaneReader<'a> {
    /// The hot superchunks, sorted by superchunk index.
    superchunks: &'a [HotSuperchunk],
    /// The superchunk found last, and where it is among them.
    last_found: Cell<Option<(SuperchunkIndex, usize)>>,
}

impl<'a> HotPlaneReader<'a> {
    /// A reader of `superchunks`, sorted by superchunk index.
    pub(crate) fn new(superchunks: &'a [HotSuperchunk]) -> Self {
        Self { superchunks, last_found: Cell::new(None) }
    }

    /// The hot superchunk `superchunk`, if it is hot.
    #[inline]
    fn hot_superchunk(&self, superchunk: SuperchunkIndex) -> Option<&'a HotSuperchunk> {
        if let Some((last_superchunk, last_place)) = self.last_found.get()
            && last_superchunk == superchunk
        {
            return Some(&self.superchunks[last_place]);
        }
        let place = self.superchunks.binary_search_by_key(&superchunk, HotSuperchunk::superchunk_index).ok()?;
        self.last_found.set(Some((superchunk, place)));
        Some(&self.superchunks[place])
    }

    /// The plane of `plane_type` over `cell`'s superchunk: none if it keeps
    /// none; [`NotHot`] if the superchunk is not hot.
    #[inline]
    fn plane(&self, plane_type: PlaneType, cell: CellIndex) -> Result<Option<&'a HotPlane>, NotHot> {
        let hot_superchunk = self.hot_superchunk(cell.superchunk()).ok_or(NotHot(cell.superchunk()))?;
        Ok(hot_superchunk.plane(plane_type))
    }

    /// The cells of `cell`'s chunk of `plane_type`, a plane of a bit a
    /// cell: all clear if not kept; none if its superchunk is not hot.
    #[inline]
    fn chunk_cells(&self, plane_type: PlaneType, cell: CellIndex) -> Option<&'a CellWords> {
        match self.plane(plane_type, cell) {
            Ok(Some(plane)) => Some(plane.cells(cell.chunk().place())),
            Ok(None) => Some(&NO_CELLS),
            Err(_) => None,
        }
    }

    /// Whether `plane_type` holds at `cell`: its number is not 0.
    pub(crate) fn holds(&self, plane_type: PlaneType, cell: CellIndex) -> Result<bool, NotHot> {
        Ok(self.plane(plane_type, cell)?.is_some_and(|plane| plane.value(cell.chunk().place(), cell.place()) != 0))
    }

    /// The number `plane_type` holds at `cell` -- of a plane of tiles, its
    /// tile's: one read, however wide.
    #[inline]
    pub(crate) fn value(&self, plane_type: PlaneType, cell: CellIndex) -> Result<u32, NotHot> {
        Ok(self.plane(plane_type, cell)?.map_or(0, |plane| plane.value(cell.chunk().place(), cell.place())))
    }

    /// Asks memory for the word `cell` is in, of `plane_type`, ahead of its
    /// being read.
    pub(crate) fn prefetch_cell(&self, plane_type: PlaneType, cell: CellIndex) {
        if let Some(cells) = self.chunk_cells(plane_type, cell) {
            prefetch_cache_line(&cells[cell.place() / BITS_PER_WORD]);
        }
    }

    /// The window of `width` by `height` cells (each up to 8) whose top
    /// left cell is `origin`, of `plane_type`, row by row: one to four
    /// words read, turned and cut.
    pub(crate) fn window(&self, plane_type: PlaneType, origin: CellIndex, width: u32, height: u32) -> Window {
        let [read_window] = self.windows([plane_type], origin, width, height);
        read_window
    }

    /// [`HotPlaneReader::window`], of each of `plane_types` at once: where the
    /// window lies worked out once.
    #[inline]
    pub(crate) fn windows<const N: usize>(&self, plane_types: [PlaneType; N], origin: CellIndex, width: u32, height: u32) -> [Window; N] {
        // The window's top left in its word tile.
        let (across_in_word_tile, down_in_word_tile) = in_word_tile(origin.0);
        let tile_first_cell = CellIndex(origin.0 & !PLACE_IN_WORD_TILE);
        let tile = tile_first_cell.place() / BITS_PER_WORD;
        let (x, y) = (tile & WORD_TILE_X, tile & WORD_TILE_Y);
        // The word tiles beside and below, in the chunk: a carry through the other coordinate's bits.
        let (tile_beside, tile_below) = (((x | WORD_TILE_Y) + 1) & WORD_TILE_X, ((y | WORD_TILE_X) + 2) & WORD_TILE_Y);
        let (past_tile_across, past_tile_down) = (across_in_word_tile + width > WORD_TILE_SIDE, down_in_word_tile + height > WORD_TILE_SIDE);
        let word_tile_side = WORD_TILE_SIDE as i32;
        let window_mask = left_columns(width) & top_rows(height);
        // Built by index: an array mapped by a closure is drained through calls the optimizer leaves standing.
        std::array::from_fn(|type_index| {
            let plane_type = plane_types[type_index];
            let cells = self.chunk_cells(plane_type, tile_first_cell);
            let top_left = word_tile(cells, tile);
            let (mut top_right, mut bottom_left, mut bottom_right) = (Window::default(), Window::default(), Window::default());
            if past_tile_across {
                top_right = if x != WORD_TILE_X { word_tile(cells, tile_beside | y) } else { self.word_tile_at(plane_type, tile_first_cell.at_cartesian_offset(word_tile_side, 0)) };
            }
            if past_tile_down {
                bottom_left = if y != WORD_TILE_Y { word_tile(cells, x | tile_below) } else { self.word_tile_at(plane_type, tile_first_cell.at_cartesian_offset(0, word_tile_side)) };
            }
            if past_tile_across && past_tile_down {
                bottom_right = if x != WORD_TILE_X && y != WORD_TILE_Y {
                    word_tile(cells, tile_beside | tile_below)
                } else {
                    self.word_tile_at(plane_type, tile_first_cell.at_cartesian_offset(word_tile_side, word_tile_side))
                };
            }
            Window {
                cells_set: window([[top_left.cells_set, top_right.cells_set], [bottom_left.cells_set, bottom_right.cells_set]], across_in_word_tile, down_in_word_tile) & window_mask,
                hot: window([[top_left.hot, top_right.hot], [bottom_left.hot, bottom_right.hot]], across_in_word_tile, down_in_word_tile) & window_mask,
            }
        })
    }

    /// The word tile whose first cell is `tile_first_cell`, if in the
    /// world, of `plane_type`.
    fn word_tile_at(&self, plane_type: PlaneType, tile_first_cell: Option<CellIndex>) -> Window {
        tile_first_cell.map_or_else(Window::default, |first_cell| word_tile(self.chunk_cells(plane_type, first_cell), first_cell.place() / BITS_PER_WORD))
    }

    /// Whether `plane_type` holds at any cell of the tile of `scale` that
    /// `cell` is in, up to [`COARSEST_SCALE`]: passed over by its count
    /// tiles where they are 0. `None` if its superchunk is not hot.
    pub(crate) fn any_in_tile(&self, plane_type: PlaneType, cell: CellIndex, scale: u32) -> Option<bool> {
        debug_assert!(scale <= COARSEST_SCALE, "a tile coarser than the coarsest scale");
        let Some(plane) = self.plane(plane_type, cell).ok()? else {
            return Some(false);
        };
        let chunk = cell.chunk().place();
        // The tile's cells are a run of this many bits, in Morton order.
        let cells_in_tile = 1usize << (2 * scale);
        let tile_first_place = cell.place() & !(cells_in_tile - 1);
        let covering_tile_counts = &plane.tile_counts(chunk)[tile_first_place / COUNT_TILE_CELLS..][..cells_in_tile.div_ceil(COUNT_TILE_CELLS)];
        if covering_tile_counts.iter().all(|&count| count == 0) {
            return Some(false);
        }
        let cells = plane.cells(chunk);
        Some(if cells_in_tile < BITS_PER_WORD {
            cells[tile_first_place / BITS_PER_WORD] >> (tile_first_place % BITS_PER_WORD) & ((1 << cells_in_tile) - 1) != 0
        } else {
            cells_in_tile >= COUNT_TILE_CELLS || cells[tile_first_place / BITS_PER_WORD..(tile_first_place + cells_in_tile) / BITS_PER_WORD].iter().any(|&word| word != 0)
        })
    }

    /// Which tiles of the coarsest scale in `cell`'s chunk `plane_type`
    /// holds at any cell of, a bit each in Morton order: read off the
    /// counts. `None` if its superchunk is not hot.
    pub(crate) fn tiles_holding(&self, plane_type: PlaneType, cell: CellIndex) -> Option<u16> {
        let Some(plane) = self.plane(plane_type, cell).ok()? else {
            return Some(0);
        };
        let tile_counts = plane.tile_counts(cell.chunk().place());
        Some(tile_counts.as_chunks::<COUNTS_IN_COARSEST>().0.iter().enumerate().fold(0, |holding_tiles, (tile, counts)| holding_tiles | u16::from(counts.iter().any(|&count| count != 0)) << tile))
    }

    /// The four small tiles, two by two, the first of which begins at
    /// `first_cell`, of each of `plane_types`: those of them `wanted_cells`
    /// has a cell in, the rest left clear. Where they lie in one chunk, its
    /// cells are found once and all four read.
    #[inline]
    pub(crate) fn small_tiles<const N: usize>(&self, plane_types: [PlaneType; N], first_cell: CellIndex, wanted_cells: u64) -> [SmallTiles; N] {
        debug_assert!(first_cell.0 & u64::from(SMALL_TILE_CELLS - 1) == 0, "small tiles read from a cell that begins none");
        let place = first_cell.place();
        // The tiles beside and below, in the chunk: a carry through the other coordinate's bits, from the tile's own up.
        let (tile_beside_bits, tile_below_bits) = (((place | PLACE_Y) + SMALL_TILE_CELLS as usize) & PLACE_X, ((place | PLACE_X) + 2 * SMALL_TILE_CELLS as usize) & PLACE_Y);
        if tile_beside_bits > place & PLACE_X && tile_below_bits > place & PLACE_Y {
            let places = [place, tile_beside_bits | place & PLACE_Y, place & PLACE_X | tile_below_bits, tile_beside_bits | tile_below_bits];
            return plane_types.map(|plane_type| match self.chunk_cells(plane_type, first_cell) {
                Some(cells) => {
                    let four_tiles_set = places.iter().enumerate().fold(0, |tiles_set_so_far, (tile, &place)| tiles_set_so_far | (cells[place / BITS_PER_WORD] >> (place % BITS_PER_WORD) & SMALL_TILE) << (SMALL_TILE_CELLS * tile as u32));
                    SmallTiles { cells_set: four_tiles_set & wanted_cells, hot: wanted_cells }
                }
                None => SmallTiles::default(),
            });
        }
        self.small_tiles_over_chunks(plane_types, first_cell, wanted_cells)
    }

    /// [`HotPlaneReader::small_tiles`], where the four do not lie in one
    /// chunk: each wanted tile found by itself.
    #[cold]
    fn small_tiles_over_chunks<const N: usize>(&self, plane_types: [PlaneType; N], first_cell: CellIndex, wanted_cells: u64) -> [SmallTiles; N] {
        let mut tiles = [SmallTiles::default(); N];
        let side_in_cells = SMALL_TILE_SIDE as i32;
        for tile in 0..SMALL_TILES_TOGETHER {
            let tile_bit_shift = SMALL_TILE_CELLS * tile;
            let wanted_in_tile = SMALL_TILE & wanted_cells >> tile_bit_shift;
            // Past the world's edge there is no tile: left clear, and not hot.
            let Some(tile_first_cell) = first_cell.at_cartesian_offset(side_in_cells * (tile & 1) as i32, side_in_cells * (tile >> 1) as i32).filter(|_| wanted_in_tile != 0) else {
                continue;
            };
            let place = tile_first_cell.place();
            for (tiles_read, plane_type) in tiles.iter_mut().zip(plane_types) {
                if let Some(cells) = self.chunk_cells(plane_type, tile_first_cell) {
                    tiles_read.cells_set |= (cells[place / BITS_PER_WORD] >> (place % BITS_PER_WORD) & wanted_in_tile) << tile_bit_shift;
                    tiles_read.hot |= wanted_in_tile << tile_bit_shift;
                }
            }
        }
        tiles
    }

    /// The words of the aligned tile `side_in_cells` cells a side -- a power
    /// of two from a word tile's 8 to a chunk's -- that `cell` is in, of
    /// `plane_type`: its cells in Morton order, as they lie. `None` if its
    /// superchunk is not hot.
    pub(crate) fn aligned_tile(&self, plane_type: PlaneType, cell: CellIndex, side_in_cells: u32) -> Option<&'a [u64]> {
        debug_assert!(side_in_cells.is_power_of_two() && (8..=WIDTH as u32).contains(&side_in_cells), "an aligned tile {side_in_cells} cells a side");
        let cells = (side_in_cells * side_in_cells) as usize;
        let first_place = cell.place() & !(cells - 1);
        self.chunk_cells(plane_type, cell).map(|words| &words[first_place / BITS_PER_WORD..][..cells / BITS_PER_WORD])
    }

    /// The four aligned tiles `side_in_cells` cells a side (8 to half a
    /// chunk's), two by two, the first of which begins at `first_cell`, of
    /// each of `plane_types`: each tile's words as they lie, or none where
    /// its superchunk is not hot or it is past the world's edge.
    #[inline]
    pub(crate) fn aligned_tiles_two_by_two<const N: usize>(&self, plane_types: [PlaneType; N], first_cell: CellIndex, side_in_cells: u32) -> [[Option<&'a [u64]>; 4]; N] {
        debug_assert!(side_in_cells.is_power_of_two() && (8..WIDTH as u32).contains(&side_in_cells), "aligned tiles {side_in_cells} cells a side, two by two");
        let cells = (side_in_cells * side_in_cells) as usize;
        let place = first_cell.place();
        debug_assert!(place & (cells - 1) == 0, "aligned tiles read from a cell that begins none");
        let words = cells / BITS_PER_WORD;
        let (tile_beside_bits, tile_below_bits) = (((place | PLACE_Y) + cells) & PLACE_X, ((place | PLACE_X) + 2 * cells) & PLACE_Y);
        if tile_beside_bits > place & PLACE_X && tile_below_bits > place & PLACE_Y {
            let places = [place, tile_beside_bits | place & PLACE_Y, place & PLACE_X | tile_below_bits, tile_beside_bits | tile_below_bits];
            return plane_types.map(|plane_type| {
                let chunk_cells = self.chunk_cells(plane_type, first_cell);
                places.map(|place| chunk_cells.map(|cells| &cells[place / BITS_PER_WORD..][..words]))
            });
        }
        plane_types.map(|plane_type| {
            std::array::from_fn(|tile| {
                let tile_first_cell = first_cell.at_cartesian_offset((side_in_cells * (tile as u32 & 1)) as i32, (side_in_cells * (tile as u32 >> 1)) as i32)?;
                self.chunk_cells(plane_type, tile_first_cell).map(|cells| &cells[tile_first_cell.place() / BITS_PER_WORD..][..words])
            })
        })
    }
}
