//! A tile: a size and a place in that size's plane. Its level is its
//! size -- 0 the whole 256x256 bitmap, [`CELL_LEVEL`] one cell -- and its
//! `x` and `y` count tiles of that size, not cells.
//!
//! Function by function: `docs/reference.md`, "`tile.rs`".

use bitmap::morton::{morton_coordinates, morton_index};
use bitmap::{Bitmap, WIDTH};

/// The level of a single cell, the finest there is.
pub const CELL_LEVEL: u8 = 8;
/// The 4x4 floor: the finest tile the tree holds a node at, and the
/// finest a copy reads.
pub const FLOOR_LEVEL: u8 = CELL_LEVEL - 2;
/// A tile's children.
pub const CHILDREN_PER_TILE: u8 = 4;
/// Cells in the bitmap.
pub const CELLS: usize = tiles_in_level(CELL_LEVEL);

/// Where a near copy reads from, by direction, in tiles of its own size:
/// the neighbours reading order puts before it -- top left, above, top
/// right, left.
const NEAR_OFFSETS: [(isize, isize); 4] = [(-1, -1), (0, -1), (1, -1), (-1, 0)];
/// Where a far copy reads from, by direction: found by a search over
/// offsets, where the near offsets doubled lost several percent on
/// cities and more on checkerboards.
const FAR_OFFSETS: [(isize, isize); 4] = [(-2, -2), (0, -4), (4, -4), (-4, 0)];
/// The directions a copy names.
pub const COPY_DIRECTIONS: u8 = NEAR_OFFSETS.len() as u8;

/// Where a near or far copy in `copy_direction` reads from, in tiles of its
/// own size: always before it in reading order, so decoding has every
/// source before what copies it.
pub const fn copy_offset(is_far_copy: bool, copy_direction: u8) -> (isize, isize) {
    if is_far_copy { FAR_OFFSETS[copy_direction as usize] } else { NEAR_OFFSETS[copy_direction as usize] }
}

/// Tiles across one row of a level's plane.
pub const fn tiles_across(tile_level: u8) -> usize {
    1 << tile_level
}

/// Tiles in a level's plane -- and so, too, the tiles filling one tile
/// that many levels finer.
pub const fn tiles_in_level(tile_level: u8) -> usize {
    tiles_across(tile_level) * tiles_across(tile_level)
}

/// How many tiles there are from the whole bitmap down to `tile_level`, both
/// included: `1 + 4 + ... + 4^tile_level`.
pub const fn tiles_down_to(tile_level: u8) -> usize {
    (tiles_in_level(tile_level + 1) - 1) / 3
}

/// How many cells one tile at `tile_level` covers.
pub const fn cells_in_tile(tile_level: u8) -> usize {
    tiles_in_level(CELL_LEVEL - tile_level)
}

/// A square of the bitmap: its size, and its place among tiles of that
/// size.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tile {
    /// Its size: each level half the side of the one before.
    pub tile_level: u8,
    /// Its column among the tiles of its level.
    pub x: u8,
    /// Its row among the tiles of its level.
    pub y: u8,
}

impl Tile {
    /// The whole bitmap as one tile.
    pub const WHOLE_BITMAP: Tile = Tile { tile_level: 0, x: 0, y: 0 };

    /// Its place among its level's tiles, in Morton order.
    pub const fn place_in_level(self) -> usize {
        morton_index(self.x, self.y)
    }

    /// Its side, in cells.
    pub const fn side_in_cells(self) -> usize {
        WIDTH >> self.tile_level
    }

    /// Its top left cell.
    pub const fn top_left_cell(self) -> (u8, u8) {
        let tile_side_cells = self.side_in_cells();
        ((self.x as usize * tile_side_cells) as u8, (self.y as usize * tile_side_cells) as u8)
    }

    /// The Morton index of its first cell: its cells are the run from
    /// there.
    pub const fn first_cell(self) -> usize {
        self.place_in_level() * cells_in_tile(self.tile_level)
    }

    /// Its four children, in reading order -- which, for one 2x2 group,
    /// is Morton order.
    pub fn child_tiles(self) -> [Tile; 4] {
        [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dx, dy)| Tile { tile_level: self.tile_level + 1, x: self.x * 2 + dx, y: self.y * 2 + dy })
    }

    /// The same-size tile `(dx, dy)` tiles away, if it is on the bitmap.
    pub fn offset_by(self, (dx, dy): (isize, isize)) -> Option<Tile> {
        let (x, y) = (self.x as isize + dx, self.y as isize + dy);
        let tiles_per_row = tiles_across(self.tile_level) as isize;
        (x >= 0 && y >= 0 && x < tiles_per_row && y < tiles_per_row).then_some(Tile { tile_level: self.tile_level, x: x as u8, y: y as u8 })
    }

    /// What `bitmap` holds at its top left cell: the tile's value, when
    /// every cell of it agrees.
    pub fn top_left_value(self, bitmap: &Bitmap) -> bool {
        let (x, y) = self.top_left_cell();
        bitmap.get(x, y)
    }

    /// Every tile of one level, in Morton order.
    pub fn all_of_level(tile_level: u8) -> impl Iterator<Item = Tile> {
        Tile::WHOLE_BITMAP.tiles_under(tile_level)
    }

    /// The tiles filling this one `size_offset` levels finer, in Morton
    /// order.
    pub fn tiles_under(self, size_offset: u8) -> impl Iterator<Item = Tile> {
        let count = tiles_in_level(size_offset);
        let finer_level = self.tile_level + size_offset;
        (self.place_in_level() * count..(self.place_in_level() + 1) * count).map(move |finer_place| {
            let (x, y) = morton_coordinates(finer_place);
            Tile { tile_level: finer_level, x, y }
        })
    }
}

/// One element per tile, at every level from the whole bitmap down to
/// `FINEST_LEVEL`, each level's elements in Morton order: a tile's four
/// children are four consecutive elements.
#[derive(Clone)]
pub struct TilePyramid<T, const FINEST_LEVEL: u8> {
    /// Every level's elements, coarsest first.
    elements_by_level: Box<[T]>,
}

impl<T: Copy + Default, const FINEST_LEVEL: u8> TilePyramid<T, FINEST_LEVEL> {
    /// Every element the default.
    pub fn new() -> Self {
        Self { elements_by_level: vec![T::default(); tiles_down_to(FINEST_LEVEL)].into_boxed_slice() }
    }

    /// Where `tile_level`'s elements start.
    const fn level_start(tile_level: u8) -> usize {
        (tiles_in_level(tile_level) - 1) / 3
    }

    /// The element of `tile_level`'s tile at Morton index `tile_place`.
    #[inline]
    fn element_index(tile_level: u8, tile_place: usize) -> usize {
        Self::level_start(tile_level) + tile_place
    }

    /// `tile`'s element.
    #[inline]
    pub fn get(&self, tile: Tile) -> T {
        self.elements_by_level[Self::element_index(tile.tile_level, tile.place_in_level())]
    }

    /// Makes `value` `tile`'s element.
    #[inline]
    pub fn set(&mut self, tile: Tile, value: T) {
        self.set_at(tile.tile_level, tile.place_in_level(), value);
    }

    /// Makes `value` the element of `tile_level`'s tile at Morton index
    /// `tile_place`.
    #[inline]
    pub fn set_at(&mut self, tile_level: u8, tile_place: usize, value: T) {
        self.elements_by_level[Self::element_index(tile_level, tile_place)] = value;
    }

    /// `tile`'s four children's elements, in reading order.
    #[inline]
    pub fn child_elements(&self, tile: Tile) -> [T; 4] {
        self.children_at(tile.tile_level, tile.place_in_level())
    }

    /// The four children's elements of `tile_level`'s tile at Morton index
    /// `tile_place`, in reading order.
    #[inline]
    pub fn children_at(&self, tile_level: u8, tile_place: usize) -> [T; 4] {
        let first_child_element = Self::element_index(tile_level + 1, 4 * tile_place);
        self.elements_by_level[first_child_element..first_child_element + 4].try_into().expect("four children")
    }
}

impl<T: Copy + Default, const FINEST_LEVEL: u8> Default for TilePyramid<T, FINEST_LEVEL> {
    /// The same as [`TilePyramid::new`].
    fn default() -> Self {
        Self::new()
    }
}
