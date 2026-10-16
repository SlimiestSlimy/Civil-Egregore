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
    pub(crate) copy_sources: Box<[FloorIndex; FLOOR_TILES]>,
    /// The floor tiles the tree leaves unsaid: copied or residual.
    pub(crate) unsaid_floor_tiles: FloorSet,
    /// The residual floor tiles not yet coded.
    pub(crate) residual_floor_tiles_left: FloorSet,
}

impl FloorPlan {
    /// No floor tile planned.
    pub fn new() -> Self {
        Self { copy_sources: Box::new([NO_SOURCE; FLOOR_TILES]), unsaid_floor_tiles: [0; FLOOR_SET_WORDS], residual_floor_tiles_left: [0; FLOOR_SET_WORDS] }
    }

    /// Forgets every floor tile planned: before the tree is walked.
    pub fn clear(&mut self) {
        self.copy_sources.fill(NO_SOURCE);
        self.unsaid_floor_tiles = [0; FLOOR_SET_WORDS];
        self.residual_floor_tiles_left = [0; FLOOR_SET_WORDS];
    }

    /// Adds the residual floor tile `floor tile`.
    pub fn add_residual_floor_tile(&mut self, floor_tile: Tile) {
        insert(&mut self.residual_floor_tiles_left, floor_tile.place_in_level());
        insert(&mut self.unsaid_floor_tiles, floor_tile.place_in_level());
    }

    /// The residual floor tiles, by Morton index, in that order.
    pub fn residual_floor_tiles(&self, mut visit_floor_tile: impl FnMut(usize)) {
        each_floor_tile(|word_index| self.residual_floor_tiles_left[word_index], &mut visit_floor_tile);
    }

    /// Adds the floor tiles of `part` -- the copy at `copy_tile`, or a child of it
    /// the copy copies -- copied from the tile `is_far_copy` and `copy_direction`
    /// name, counted in the copy's own sides: each floor tile from the floor tile
    /// at the same place in the same-size tile that far away.
    pub fn add_copied_floor_tiles(&mut self, copy_tile: Tile, part: Tile, is_far_copy: bool, copy_direction: u8) {
        let (dx, dy) = copy_offset(is_far_copy, copy_direction);
        let reach = tiles_across(part.tile_level - copy_tile.tile_level) as isize;
        let source_tile = Tile { tile_level: part.tile_level, x: (part.x as isize + dx * reach) as u8, y: (part.y as isize + dy * reach) as u8 };
        let (first_floor_tile, source_first) = (part.first_cell() / FLOOR_TILE_CELLS, source_tile.first_cell() / FLOOR_TILE_CELLS);
        for place in 0..tiles_in_level(FLOOR_LEVEL - part.tile_level) {
            self.copy_sources[first_floor_tile + place] = (source_first + place) as FloorIndex;
            insert(&mut self.unsaid_floor_tiles, first_floor_tile + place);
        }
    }
}
