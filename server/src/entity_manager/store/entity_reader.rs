//! Entities read wherever they stand: over the superchunks, by the
//! cells they occupy.

use crate::entity_manager::entity::EntityRef;
use super::SuperchunkEntities;
use utilities::bitmap::window::{PLACE_IN_WORD_TILE, in_word_tile};
use utilities::coordinates::{CellIndex, ChunkIndex, SuperchunkIndex};

/// Cells along the side of the most [`EntityReader::occupied`] reads at
/// once: a row's bits.
#[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
pub const OCCUPIED_SIDE: usize = 16;

/// Reads entities from superchunks in a tick's first phase, across
/// superchunks, as they were when the tick began: the entities' side of
/// the bitplanes' reader.
pub struct EntityReader<'a> {
    /// The superchunks read, sorted by Morton index.
    superchunks: &'a [SuperchunkEntities],
}

impl<'a> EntityReader<'a> {
    /// A reader of `superchunks`: an [`Entities`]'s.
    pub fn new(superchunks: &'a [SuperchunkEntities]) -> Self {
        Self { superchunks }
    }

    /// `superchunk`'s entities, if read.
    pub(crate) fn superchunk(&self, superchunk: SuperchunkIndex) -> Option<&'a SuperchunkEntities> {
        let found_index = self.superchunks.binary_search_by_key(&superchunk, SuperchunkEntities::superchunk_index).ok()?;
        Some(&self.superchunks[found_index])
    }

    /// The entity standing on `stood_cell`, if one does and it is read.
    pub fn get(&self, stood_cell: CellIndex) -> Option<EntityRef<'a>> {
        self.superchunk(stood_cell.superchunk())?.get(stood_cell)
    }

    /// The cells entities stand on among the `width` by `height` cells
    /// (each up to 16) whose top left cell is `origin`, a row a word:
    /// cell `(x, y)` from `origin` at bit `x` of row `y`
    /// (`docs/entity_manager.md`, "The store").
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub fn occupied(&self, origin: CellIndex, width: u32, height: u32) -> [u16; OCCUPIED_SIDE] {
        debug_assert!(width as usize <= OCCUPIED_SIDE && height as usize <= OCCUPIED_SIDE, "more cells than a row's bits");
        let mut occupied_rows = [0; OCCUPIED_SIDE];
        let (origin_x_in_tile, origin_y_in_tile) = in_word_tile(origin.0);
        let first_tile_cell = CellIndex(origin.0 & !PLACE_IN_WORD_TILE);
        let mut last_superchunk: Option<&SuperchunkEntities> = None;
        for (tile_x, tile_y) in (0..3).flat_map(|tile_y| (0..3).map(move |tile_x| (tile_x, tile_y))) {
            // Where the word tile's first cell is among the cells asked for: before them, by up to 7.
            let (tile_left, tile_top) = (8 * tile_x - origin_x_in_tile as i32, 8 * tile_y - origin_y_in_tile as i32);
            if tile_left >= width as i32 || tile_top >= height as i32 {
                continue;
            }
            let Some(tile) = first_tile_cell.at_cartesian_offset(8 * tile_x, 8 * tile_y) else {
                continue;
            };
            if last_superchunk.is_none_or(|previous_superchunk| previous_superchunk.superchunk_index != tile.superchunk()) {
                last_superchunk = self.superchunk(tile.superchunk());
            }
            let Some(superchunk) = last_superchunk else {
                continue;
            };
            for &place in superchunk.in_word_tile(tile.chunk().place(), tile.place() as u16) {
                let (x, y) = in_word_tile(place as u64);
                let (x, y) = (tile_left + x as i32, tile_top + y as i32);
                if x >= 0 && y >= 0 && x < width as i32 && y < height as i32 {
                    occupied_rows[y as usize] |= 1 << x;
                }
            }
        }
        occupied_rows
    }

    /// The entities on `chunk`, in Morton order by cell, if
    /// its superchunk is read.
    pub fn chunk(&self, chunk: ChunkIndex) -> Option<impl Iterator<Item = EntityRef<'a>> + 'a> {
        Some(self.superchunk(chunk.superchunk())?.chunk(chunk.place()))
    }
}
