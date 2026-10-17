//! Aligned tiles read as they lie: a tile whose side is a power of two
//! and whose corner is a multiple of it is one run of bits of its
//! chunk's bitmap, in Morton order -- nothing turned, nothing cut
//! (`docs/bitplane_manager.md`, "The arena", Aligned tiles).

use super::CellReader;
use bitmap::{BITS_PER_WORD, WIDTH};
use chunk_storage::LayerType;
use coordinates::CellIndex;

/// Cells along a small tile's side: the smallest aligned tile read.
pub const SMALL_TILE_SIDE: u32 = 4;
/// Cells in a small tile, and the bits of one.
pub const SMALL_TILE_CELLS: u32 = SMALL_TILE_SIDE * SMALL_TILE_SIDE;
/// Small tiles read together: two by two, a word of bits.
pub const SMALL_TILES_TOGETHER: u32 = u64::BITS / SMALL_TILE_CELLS;
/// The bits of one small tile, at the foot of a word.
const SMALL_TILE: u64 = (1 << SMALL_TILE_CELLS) - 1;

/// The bits of a cell's place in its chunk holding x: the even ones.
const PLACE_X: usize = 0x5555;
/// The bits of a cell's place in its chunk holding y: the odd ones.
const PLACE_Y: usize = 0xaaaa;
/// A small tile's side added to a place's x...
const SMALL_TILE_STEP_ACROSS: usize = SMALL_TILE_CELLS as usize;
/// ...and to its y.
const SMALL_TILE_STEP_DOWN: usize = 2 * SMALL_TILE_CELLS as usize;

/// Four small tiles, two by two, of one layer type: 8x8 cells in
/// Morton order from the top left of the first -- the cell `x` across
/// and `y` down at the bit whose index is their bits interleaved --
/// so each small tile is 16 bits of it, as it lies in its bitmap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SmallTiles {
    /// The cells the type holds at: hot ones only.
    pub cells_set: u64,
    /// The cells in hot bitmaps: in the world, and read.
    pub hot: u64,
}

impl<'a> CellReader<'a> {
    /// The four small tiles, two by two, the first of which begins at
    /// `first_cell` -- an aligned small tile's first cell -- of each of
    /// `layer_types`: those of them `wanted_cells` has a cell in, the rest not
    /// looked up and left clear, as every cell out of `wanted_cells` is.
    /// Where they lie in one chunk -- all but at its far edges -- its
    /// bitmap is looked up once, the tiles beside and below stepped to
    /// on the place in it, and all four read.
    #[inline]
    pub fn small_tiles<const N: usize>(&self, layer_types: [LayerType; N], first_cell: CellIndex, wanted_cells: u64) -> [SmallTiles; N] {
        debug_assert!(first_cell.0 & SMALL_TILE == 0, "small tiles read from a cell that begins none");
        let place = first_cell.place();
        // The tiles beside and below, in the chunk: a carry through the other coordinate's bits, from the tile's own up.
        let (tile_beside_bits, tile_below_bits) = (((place | PLACE_Y) + SMALL_TILE_STEP_ACROSS) & PLACE_X, ((place | PLACE_X) + SMALL_TILE_STEP_DOWN) & PLACE_Y);
        if tile_beside_bits > place & PLACE_X && tile_below_bits > place & PLACE_Y {
            let places = [place, tile_beside_bits | place & PLACE_Y, place & PLACE_X | tile_below_bits, tile_beside_bits | tile_below_bits];
            return layer_types.map(|layer_type| match self.lookup.bucket(self.superchunks, layer_type, first_cell) {
                Some(cells) => {
                    let four_tiles_set = places.iter().enumerate().fold(0, |tiles_set_so_far, (tile, &place)| tiles_set_so_far | (cells[place / BITS_PER_WORD] >> (place % BITS_PER_WORD) & SMALL_TILE) << (SMALL_TILE_CELLS * tile as u32));
                    SmallTiles { cells_set: four_tiles_set & wanted_cells, hot: wanted_cells }
                }
                None => SmallTiles::default(),
            });
        }
        self.small_tiles_over_chunks(layer_types, first_cell, wanted_cells)
    }

    /// The words of the aligned tile `side_in_cells` cells a side -- a power of
    /// two from a word tile's 8 to a chunk's -- that `cell` is in, of
    /// `layer_type`: its cells in Morton order, as they lie in the
    /// bitmap. `None` if its bitmap is not hot.
    pub fn aligned_tile(&self, layer_type: LayerType, cell: CellIndex, side_in_cells: u32) -> Option<&'a [u64]> {
        debug_assert!(side_in_cells.is_power_of_two() && (8..=WIDTH as u32).contains(&side_in_cells), "an aligned tile {side_in_cells} cells a side");
        let cells = (side_in_cells * side_in_cells) as usize;
        let first_place = cell.place() & !(cells - 1);
        self.lookup.bucket(self.superchunks, layer_type, cell).map(|words| &words[first_place / BITS_PER_WORD..][..cells / BITS_PER_WORD])
    }

    /// The four aligned tiles `side_in_cells` cells a side (8 to half a
    /// chunk's), two by two, the first of which begins at `first_cell`, of
    /// each of `layer_types`: each tile's words as they lie, or none where
    /// its bitmap is not hot or it is past the world's edge. Where
    /// they lie in one chunk its bitmap is looked up once, the tiles
    /// beside and below stepped to on the place in it.
    #[inline]
    pub fn aligned_tiles_two_by_two<const N: usize>(&self, layer_types: [LayerType; N], first_cell: CellIndex, side_in_cells: u32) -> [[Option<&'a [u64]>; 4]; N] {
        debug_assert!(side_in_cells.is_power_of_two() && (8..WIDTH as u32).contains(&side_in_cells), "aligned tiles {side_in_cells} cells a side, two by two");
        let cells = (side_in_cells * side_in_cells) as usize;
        let place = first_cell.place();
        debug_assert!(place & (cells - 1) == 0, "aligned tiles read from a cell that begins none");
        let words = cells / BITS_PER_WORD;
        let (tile_beside_bits, tile_below_bits) = (((place | PLACE_Y) + cells) & PLACE_X, ((place | PLACE_X) + 2 * cells) & PLACE_Y);
        if tile_beside_bits > place & PLACE_X && tile_below_bits > place & PLACE_Y {
            let places = [place, tile_beside_bits | place & PLACE_Y, place & PLACE_X | tile_below_bits, tile_beside_bits | tile_below_bits];
            return layer_types.map(|layer_type| {
                let bucket = self.lookup.bucket(self.superchunks, layer_type, first_cell);
                places.map(|place| bucket.map(|cells| &cells[place / BITS_PER_WORD..][..words]))
            });
        }
        layer_types.map(|layer_type| {
            std::array::from_fn(|tile| {
                let tile_first_cell = first_cell.at_cartesian_offset((side_in_cells * (tile as u32 & 1)) as i32, (side_in_cells * (tile as u32 >> 1)) as i32)?;
                self.lookup.bucket(self.superchunks, layer_type, tile_first_cell).map(|cells| &cells[tile_first_cell.place() / BITS_PER_WORD..][..words])
            })
        })
    }

    /// [`Reader::small_tiles`], where the four do not lie in one
    /// chunk: each tile `wanted_cells` has a cell in looked up by itself.
    #[cold]
    fn small_tiles_over_chunks<const N: usize>(&self, layer_types: [LayerType; N], first_cell: CellIndex, wanted_cells: u64) -> [SmallTiles; N] {
        let mut tiles = [SmallTiles::default(); N];
        let side_in_cells = SMALL_TILE_SIDE as i32;
        for tile in 0..SMALL_TILES_TOGETHER {
            let tile_bit_shift = SMALL_TILE_CELLS * tile;
            if wanted_cells >> tile_bit_shift & SMALL_TILE == 0 {
                continue;
            }
            // Past the world's edge there is no tile: left clear, and not hot.
            let Some(tile_first_cell) = first_cell.at_cartesian_offset(side_in_cells * (tile & 1) as i32, side_in_cells * (tile >> 1) as i32) else {
                continue;
            };
            let place = tile_first_cell.place();
            for (tiles, layer_type) in tiles.iter_mut().zip(layer_types) {
                if let Some(cells) = self.lookup.bucket(self.superchunks, layer_type, tile_first_cell) {
                    tiles.cells_set |= (cells[place / BITS_PER_WORD] >> (place % BITS_PER_WORD) & SMALL_TILE & wanted_cells >> tile_bit_shift) << tile_bit_shift;
                    tiles.hot |= (SMALL_TILE & wanted_cells >> tile_bit_shift) << tile_bit_shift;
                }
            }
        }
        tiles
    }
}
