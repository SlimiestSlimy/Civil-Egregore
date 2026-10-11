//! Aligned tiles read as they lie: a tile whose side is a power of two
//! and whose corner is a multiple of it is one run of bits of its
//! chunk's bitmap, in Morton order -- nothing turned, nothing cut
//! (`docs/bitplane_manager.md`, "The arena", Aligned tiles).

use super::Reader;
use bitmap::BITS_PER_WORD;
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

/// Four small tiles, two by two, of one layer type: 8x8 cells in
/// Morton order from the top left of the first -- the cell `x` across
/// and `y` down at the bit whose index is their bits interleaved --
/// so each small tile is 16 bits of it, as it lies in its bitmap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SmallTiles {
    /// The cells the type holds at: hot ones only.
    pub set: u64,
    /// The cells in hot bitmaps: in the world, and read.
    pub hot: u64,
}

impl<'a> Reader<'a> {
    /// The four small tiles, two by two, the first of which begins at
    /// `first` -- an aligned small tile's first cell -- of each of
    /// `types`: those of them `wanted` has a cell in, the rest not
    /// looked up and left clear, as every cell out of `wanted` is.
    /// Where they lie in one bitmap, it is looked up once.
    #[inline]
    pub fn small_tiles<const N: usize>(&self, types: [LayerType; N], first: CellIndex, wanted: u64) -> [SmallTiles; N] {
        debug_assert!(first.0 & SMALL_TILE == 0, "small tiles read from a cell that begins none");
        let mut tiles = [SmallTiles::default(); N];
        let side = SMALL_TILE_SIDE as i32;
        for tile in 0..SMALL_TILES_TOGETHER {
            let shift = SMALL_TILE_CELLS * tile;
            if wanted >> shift & SMALL_TILE == 0 {
                continue;
            }
            // Past the world's edge there is no tile: left clear, and not hot.
            let Some(begins) = first.offset(side * (tile & 1) as i32, side * (tile >> 1) as i32) else {
                continue;
            };
            let place = begins.place();
            for (tiles, layer_type) in tiles.iter_mut().zip(types) {
                if let Some(cells) = self.lookup.bucket(self.superchunks, layer_type, begins) {
                    tiles.set |= (cells[place / BITS_PER_WORD] >> (place % BITS_PER_WORD) & SMALL_TILE & wanted >> shift) << shift;
                    tiles.hot |= (SMALL_TILE & wanted >> shift) << shift;
                }
            }
        }
        tiles
    }
}
