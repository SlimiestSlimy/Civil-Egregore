//! What the last pass is to do: the floor tiles to code cell by cell,
//! and those copied from others.

use crate::tile::{FLOOR_LEVEL, Tile, copy_offset, tiles_across, tiles_in_level};
use super::{FLOOR_SET_WORDS, FLOOR_TILES, FLOOR_TILE_CELLS, FloorIndex, FloorSet, NO_SOURCE, each_floor_tile, insert};

/// The last pass's input: the 4x4 floor tiles the tree leaves unsaid -- each
/// floor tile a copy covers, and its source floor tile, and the residual floor tiles.
/// Gathered by the quadtree writer and reader as they walk the tree,
/// so encoding and decoding gather the same.
pub struct FloorPlan {
    /// Each floor tile's source while a copy covers it and it is not copied
    /// yet; [`NO_SOURCE`] otherwise.
    pub(crate) sources: Box<[FloorIndex; FLOOR_TILES]>,
    /// The floor tiles the tree leaves unsaid: copied or residual.
    pub(crate) unsaid: FloorSet,
    /// The residual floor tiles not yet coded.
    pub(crate) residual: FloorSet,
}

impl FloorPlan {
    /// No floor tile planned.
    pub fn new() -> Self {
        Self { sources: Box::new([NO_SOURCE; FLOOR_TILES]), unsaid: [0; FLOOR_SET_WORDS], residual: [0; FLOOR_SET_WORDS] }
    }

    /// Forgets every floor tile planned: before the tree is walked.
    pub fn clear(&mut self) {
        self.sources.fill(NO_SOURCE);
        self.unsaid = [0; FLOOR_SET_WORDS];
        self.residual = [0; FLOOR_SET_WORDS];
    }

    /// Adds the residual floor tile `floor tile`.
    pub fn add_residual_floor_tile(&mut self, floor_tile: Tile) {
        insert(&mut self.residual, floor_tile.index());
        insert(&mut self.unsaid, floor_tile.index());
    }

    /// The residual floor tiles, by Morton index, in that order.
    pub fn residual_floor_tiles(&self, mut visit: impl FnMut(usize)) {
        each_floor_tile(|word_index| self.residual[word_index], &mut visit);
    }

    /// Adds the floor tiles of `part` -- the copy at `copy`, or a child of it
    /// the copy copies -- copied from the tile `far` and `direction`
    /// name, counted in the copy's own sides: each floor tile from the floor tile
    /// at the same place in the same-size tile that far away.
    pub fn add_copied_floor_tiles(&mut self, copy: Tile, part: Tile, far: bool, direction: u8) {
        let (dx, dy) = copy_offset(far, direction);
        let reach = tiles_across(part.level - copy.level) as isize;
        let source = Tile { level: part.level, x: (part.x as isize + dx * reach) as u8, y: (part.y as isize + dy * reach) as u8 };
        let (first, source_first) = (part.first_cell() / FLOOR_TILE_CELLS, source.first_cell() / FLOOR_TILE_CELLS);
        for place in 0..tiles_in_level(FLOOR_LEVEL - part.level) {
            self.sources[first + place] = (source_first + place) as FloorIndex;
            insert(&mut self.unsaid, first + place);
        }
    }
}
