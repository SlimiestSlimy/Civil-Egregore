//! The cells about a cell, as a turn reads them: the 3x3 around it,
//! the area of 16x16, the tiles further off, and the cells entities
//! stand on among them.

use super::Turn;
use crate::around::{squeeze, Around};
use entity_manager::OCCUPIED_SIDE;
use bitplane_manager::COARSEST_TILES_IN_CHUNK;
use chunk_storage::LayerType;
use coordinates::{CellCartesian, CellIndex};

/// Cells along the side of an [`Area`].
pub const AREA_SIDE: usize = 16;
/// The column and the row of an [`Area`] its centre is at.
pub const AREA_CENTRE: usize = AREA_SIDE / 2;

/// The cells of one layer type around a cell ([`Turn::area`]),
/// a row a word: cell `(x, y)` from the area's top left at bit `x` of
/// row `y`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Area {
    /// The cells the type holds at: hot ones only.
    pub set: [u16; AREA_SIDE],
    /// The cells in hot bitmaps: in the world, and read.
    pub hot: [u16; AREA_SIDE],
}

impl Area {
    /// How many of its cells the type holds at.
    pub fn count(&self) -> u32 {
        self.set.iter().map(|row| row.count_ones()).sum()
    }
}

/// The coarsest scale [`Turn::area_of_tiles`] looks over: tiles `2^6`
/// cells a side, [`AREA_SIDE`] of them 1,024 cells -- an entity's reach.
pub const FARTHEST_SCALE: u32 = bitplane_manager::COARSEST_SCALE;

// The cells entities stand on are asked for over the area a turn reads.
const _: () = assert!(OCCUPIED_SIDE == AREA_SIDE);

impl Turn<'_> {
    /// The [`AREA_SIDE`] by [`AREA_SIDE`] cells around `centre` -- it at
    /// `(AREA_CENTRE, AREA_CENTRE)` of them -- of `layer_type`, as the
    /// tick found them: four windows, a row a word. What an entity sees
    /// of the world about it at once: where to find a path over, say.
    pub fn area(&self, layer_type: LayerType, centre: CellIndex) -> Area {
        let [area] = self.areas([layer_type], centre);
        area
    }

    /// [`Turn::area`], of each of `types` at once.
    pub fn areas<const N: usize>(&self, types: [LayerType; N], centre: CellIndex) -> [Area; N] {
        let mut areas = [Area::default(); N];
        let (reach, half) = (AREA_CENTRE as i32, AREA_SIDE as u32 / 2);
        for quarter in 0..4u32 {
            let (across, down) = (quarter % 2 * half, quarter / 2 * half);
            // A quarter off the world is left clear, and not hot.
            let Some(origin) = centre.offset(across as i32 - reach, down as i32 - reach) else {
                continue;
            };
            let windows = self.reader.windows(types, origin, half, half);
            for (area, window) in areas.iter_mut().zip(windows) {
                for row in 0..half {
                    let at = (down + row) as usize;
                    area.set[at] |= ((window.set >> (8 * row) & 0xff) as u16) << across;
                    area.hot[at] |= ((window.hot >> (8 * row) & 0xff) as u16) << across;
                }
            }
        }
        areas
    }

    /// The 3x3 cells around `at`, of `layer_type`, as the tick found
    /// them, nine bits ([`crate::around`]): one window read. At the
    /// world's edge, none.
    pub fn around(&self, layer_type: LayerType, at: CellIndex) -> Around {
        let Some(corner) = at.offset(-1, -1) else {
            return Around::default();
        };
        let window = self.reader.window(layer_type, corner, 3, 3);
        Around { set: squeeze(window.set), hot: squeeze(window.hot) }
    }

    /// Which of the 3x3 cells around `at` an entity stands on, as the
    /// tick found them, nine bits ([`crate::around`]) -- `at`'s own
    /// among them, if one stands there. Asked when a cell must be had,
    /// not before a step, which is turned back if its cell is taken.
    pub fn around_occupied(&self, at: CellIndex) -> u16 {
        at.offset(-1, -1).map_or(0, |corner| {
            let rows = self.occupied(corner, 3, 3);
            rows[0] | rows[1] << 3 | rows[2] << 6
        })
    }

    /// One of the neighbours of `at` among `open` -- nine bits
    /// ([`crate::around`]) -- that no entity stood on as the tick found
    /// them, drawn at random: where to make an entity, which must have
    /// its cell. None if every one is taken.
    pub fn free_beside(&mut self, at: CellIndex, open: u16) -> Option<u32> {
        let free = open & !self.around_occupied(at);
        crate::around::pick(&mut self.random, free)
    }

    /// The cells entities stand on among the [`AREA_SIDE`] by
    /// [`AREA_SIDE`] around `centre`, laid out as an [`Area`] is. Off
    /// the world's edge, none.
    pub fn area_occupied(&self, centre: CellIndex) -> [u16; AREA_SIDE] {
        let reach = AREA_CENTRE as i32;
        centre.offset(-reach, -reach).map_or([0; AREA_SIDE], |corner| self.occupied(corner, AREA_SIDE as u32, AREA_SIDE as u32))
    }

    /// [`Turn::area`] from further off: the tiles of `scale` around
    /// `centre`, one set if `layer_type` holds at any of its cells. Up
    /// to [`FARTHEST_SCALE`].
    pub fn area_of_tiles(&self, layer_type: LayerType, centre: CellIndex, scale: u32) -> Area {
        let mut area = Area::default();
        let centre = centre.cartesian();
        // The area's top left tile, in tiles from the world's: before the world, where the centre is near its edge.
        let (left, top) = ((centre.x >> scale) as i64 - AREA_CENTRE as i64, (centre.y >> scale) as i64 - AREA_CENTRE as i64);
        let world = (1i64 << u32::BITS) >> scale;
        if scale == FARTHEST_SCALE {
            // The chunks its tiles are in, each read at once.
            let chunk = COARSEST_TILES_IN_CHUNK.trailing_zeros() / 2;
            for (down, across) in ((top >> chunk)..=((top + AREA_SIDE as i64 - 1) >> chunk)).flat_map(|down| ((left >> chunk)..=((left + AREA_SIDE as i64 - 1) >> chunk)).map(move |across| (down, across))) {
                if across < 0 || down < 0 || across << chunk >= world || down << chunk >= world {
                    continue;
                }
                let first = CellCartesian { x: (across << (chunk + scale)) as u32, y: (down << (chunk + scale)) as u32 };
                let Some(holding) = self.reader.tiles_holding(layer_type, first.into()) else {
                    continue;
                };
                // Where the chunk's tiles are in the area: up to three before it, across and down.
                let (x, y) = ((across << chunk) - left, (down << chunk) - top);
                let side = 1i64 << chunk;
                for row in (0..side).filter(|row| (0..AREA_SIDE as i64).contains(&(y + row))) {
                    area.hot[(y + row) as usize] |= ((((1u64 << side) - 1) << (x + side)) >> side) as u16;
                }
                let mut left_to_place = holding;
                while left_to_place != 0 {
                    // A tile's index in its chunk is a Morton index: across in its even bits, down in its odd.
                    let tile = left_to_place.trailing_zeros() as i64;
                    let (x, y) = (x + (tile & 1 | tile >> 1 & 2), y + (tile >> 1 & 1 | tile >> 2 & 2));
                    if (0..AREA_SIDE as i64).contains(&x) && (0..AREA_SIDE as i64).contains(&y) {
                        area.set[y as usize] |= 1 << x;
                    }
                    left_to_place &= left_to_place - 1;
                }
            }
            return area;
        }
        for (y, x) in (0..AREA_SIDE).flat_map(|y| (0..AREA_SIDE).map(move |x| (y, x))) {
            let (across, down) = (left + x as i64, top + y as i64);
            if across < 0 || down < 0 || across >= world || down >= world {
                continue;
            }
            let cell = CellCartesian { x: (across << scale) as u32, y: (down << scale) as u32 };
            let holds = self.reader.any_in_tile(layer_type, cell.into(), scale);
            area.set[y] |= ((holds == Some(true)) as u16) << x;
            area.hot[y] |= (holds.is_some() as u16) << x;
        }
        area
    }
}
