//! Entities read wherever they stand: over the superchunks, by id and
//! by the cells they occupy.

use crate::entity::{EntityId, EntityRef};
use super::SuperchunkEntities;
use bitmap::window::{PLACE_IN_WORD_TILE, in_word_tile};
use coordinates::{CellIndex, ChunkIndex, SuperchunkIndex};

/// Cells along the side of the most [`EntityReader::occupied`] reads at
/// once: a row's bits.
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
        let at = self.superchunks.binary_search_by_key(&superchunk, SuperchunkEntities::index).ok()?;
        Some(&self.superchunks[at])
    }

    /// The entity whose ID is `id`, standing on `at`, if read.
    pub fn get(&self, id: EntityId, at: CellIndex) -> Option<EntityRef<'a>> {
        self.superchunk(at.superchunk())?.get(id, at)
    }

    /// The cells entities stand on among the `width` by `height` cells
    /// (each up to 16) whose top left cell is `origin`, a row a word:
    /// cell `(x, y)` from `origin` at bit `x` of row `y`. Found from the
    /// buckets, which are sorted by cell: the cells lie on up to nine
    /// word tiles, each a run of a bucket's places, so what is read is
    /// the few entities there, not the cells. Where no superchunk is
    /// read, no entity stands.
    pub fn occupied(&self, origin: CellIndex, width: u32, height: u32) -> [u16; OCCUPIED_SIDE] {
        debug_assert!(width as usize <= OCCUPIED_SIDE && height as usize <= OCCUPIED_SIDE, "more cells than a row's bits");
        let mut rows = [0; OCCUPIED_SIDE];
        let (across, down) = in_word_tile(origin.0);
        let first = CellIndex(origin.0 & !PLACE_IN_WORD_TILE);
        let mut last: Option<&SuperchunkEntities> = None;
        for (tile_x, tile_y) in (0..3).flat_map(|tile_y| (0..3).map(move |tile_x| (tile_x, tile_y))) {
            // Where the word tile's first cell is among the cells asked for: before them, by up to 7.
            let (left, top) = (8 * tile_x - across as i32, 8 * tile_y - down as i32);
            if left >= width as i32 || top >= height as i32 {
                continue;
            }
            let Some(tile) = first.offset(8 * tile_x, 8 * tile_y) else {
                continue;
            };
            if last.is_none_or(|last| last.index != tile.superchunk()) {
                last = self.superchunk(tile.superchunk());
            }
            let Some(superchunk) = last else {
                continue;
            };
            for &place in superchunk.in_word_tile(tile.chunk().place(), tile.place() as u16) {
                let (x, y) = in_word_tile(place as u64);
                let (x, y) = (left + x as i32, top + y as i32);
                if x >= 0 && y >= 0 && x < width as i32 && y < height as i32 {
                    rows[y as usize] |= 1 << x;
                }
            }
        }
        rows
    }

    /// The entities on `chunk`, in Morton order by cell, then by ID, if
    /// its superchunk is read.
    pub fn chunk(&self, chunk: ChunkIndex) -> Option<impl Iterator<Item = EntityRef<'a>> + 'a> {
        Some(self.superchunk(chunk.superchunk())?.chunk(chunk.place()))
    }
}
