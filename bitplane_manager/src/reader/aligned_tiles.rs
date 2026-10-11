//! Aligned tiles read as they lie: a tile whose side is a power of two
//! and whose corner is a multiple of it is one run of bits of its
//! chunk's bitmap, in Morton order -- nothing turned, nothing cut
//! (`docs/bitplane_manager.md`, "The arena", Aligned tiles).

use super::Reader;
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
const BESIDE: usize = SMALL_TILE_CELLS as usize;
/// ...and to its y.
const BELOW: usize = 2 * SMALL_TILE_CELLS as usize;

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
    /// Where they lie in one chunk -- all but at its far edges -- its
    /// bitmap is looked up once, the tiles beside and below stepped to
    /// on the place in it, and all four read.
    #[inline]
    pub fn small_tiles<const N: usize>(&self, types: [LayerType; N], first: CellIndex, wanted: u64) -> [SmallTiles; N] {
        debug_assert!(first.0 & SMALL_TILE == 0, "small tiles read from a cell that begins none");
        let place = first.place();
        // The tiles beside and below, in the chunk: a carry through the other coordinate's bits, from the tile's own up.
        let (beside, below) = (((place | PLACE_Y) + BESIDE) & PLACE_X, ((place | PLACE_X) + BELOW) & PLACE_Y);
        if beside > place & PLACE_X && below > place & PLACE_Y {
            let places = [place, beside | place & PLACE_Y, place & PLACE_X | below, beside | below];
            return types.map(|layer_type| match self.lookup.bucket(self.superchunks, layer_type, first) {
                Some(cells) => {
                    let set = places.iter().enumerate().fold(0, |set, (tile, &place)| set | (cells[place / BITS_PER_WORD] >> (place % BITS_PER_WORD) & SMALL_TILE) << (SMALL_TILE_CELLS * tile as u32));
                    SmallTiles { set: set & wanted, hot: wanted }
                }
                None => SmallTiles::default(),
            });
        }
        self.small_tiles_over_chunks(types, first, wanted)
    }

    /// The words of the aligned tile `side` cells a side -- a power of
    /// two from a word tile's 8 to a chunk's -- that `cell` is in, of
    /// `layer_type`: its cells in Morton order, as they lie in the
    /// bitmap. `None` if its bitmap is not hot.
    pub fn aligned_tile(&self, layer_type: LayerType, cell: CellIndex, side: u32) -> Option<&'a [u64]> {
        debug_assert!(side.is_power_of_two() && (8..=WIDTH as u32).contains(&side), "an aligned tile {side} cells a side");
        let cells = (side * side) as usize;
        let first = cell.place() & !(cells - 1);
        self.lookup.bucket(self.superchunks, layer_type, cell).map(|words| &words[first / BITS_PER_WORD..][..cells / BITS_PER_WORD])
    }

    /// The four aligned tiles `side` cells a side (8 to half a
    /// chunk's), two by two, the first of which begins at `first`, of
    /// each of `types`: each tile's words as they lie, or none where
    /// its bitmap is not hot or it is past the world's edge. Where
    /// they lie in one chunk its bitmap is looked up once, the tiles
    /// beside and below stepped to on the place in it.
    #[inline]
    pub fn aligned_tiles_two_by_two<const N: usize>(&self, types: [LayerType; N], first: CellIndex, side: u32) -> [[Option<&'a [u64]>; 4]; N] {
        debug_assert!(side.is_power_of_two() && (8..WIDTH as u32).contains(&side), "aligned tiles {side} cells a side, two by two");
        let cells = (side * side) as usize;
        let place = first.place();
        debug_assert!(place & (cells - 1) == 0, "aligned tiles read from a cell that begins none");
        let words = cells / BITS_PER_WORD;
        let (beside, below) = (((place | PLACE_Y) + cells) & PLACE_X, ((place | PLACE_X) + 2 * cells) & PLACE_Y);
        if beside > place & PLACE_X && below > place & PLACE_Y {
            let places = [place, beside | place & PLACE_Y, place & PLACE_X | below, beside | below];
            return types.map(|layer_type| {
                let bucket = self.lookup.bucket(self.superchunks, layer_type, first);
                places.map(|place| bucket.map(|cells| &cells[place / BITS_PER_WORD..][..words]))
            });
        }
        types.map(|layer_type| {
            std::array::from_fn(|tile| {
                let begins = first.offset((side * (tile as u32 & 1)) as i32, (side * (tile as u32 >> 1)) as i32)?;
                self.lookup.bucket(self.superchunks, layer_type, begins).map(|cells| &cells[begins.place() / BITS_PER_WORD..][..words])
            })
        })
    }

    /// [`Reader::small_tiles`], where the four do not lie in one
    /// chunk: each tile `wanted` has a cell in looked up by itself.
    #[cold]
    fn small_tiles_over_chunks<const N: usize>(&self, types: [LayerType; N], first: CellIndex, wanted: u64) -> [SmallTiles; N] {
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
