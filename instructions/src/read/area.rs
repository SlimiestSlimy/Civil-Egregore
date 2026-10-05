//! The area about a cell read ([`crate::area`]): a layer's 16x16
//! cells, several layers' at once, the cells entities stand on among
//! them, and the same from further off, a tile a bit.

use crate::area::{Area, AREA_CENTRE, AREA_SIDE, FARTHEST_SCALE};
use bitplane_manager::COARSEST_TILES_IN_CHUNK;
use chunk_storage::LayerType;
use coordinates::{CellCartesian, CellIndex};
use entity_manager::OCCUPIED_SIDE;
use simulation::Turn;

// The cells entities stand on are asked for over the area a turn reads.
const _: () = assert!(OCCUPIED_SIDE == AREA_SIDE);

/// The [`AREA_SIDE`] by [`AREA_SIDE`] cells around `centre` -- it at
/// `(AREA_CENTRE, AREA_CENTRE)` of them -- of `layer_type`, as the
/// tick found them: four windows, a row a word.
pub fn layer(turn: &Turn, layer_type: LayerType, centre: CellIndex) -> Area {
    let [area] = layers(turn, [layer_type], centre);
    area
}

/// [`layer`], of each of `types` at once.
pub fn layers<const N: usize>(turn: &Turn, types: [LayerType; N], centre: CellIndex) -> [Area; N] {
    let mut areas = [Area::default(); N];
    let (reach, half) = (AREA_CENTRE as i32, AREA_SIDE as u32 / 2);
    for quarter in 0..4u32 {
        let (across, down) = (quarter % 2 * half, quarter / 2 * half);
        // A quarter off the world is left clear, and not hot.
        let Some(origin) = centre.offset(across as i32 - reach, down as i32 - reach) else {
            continue;
        };
        let windows = turn.windows(types, origin, half, half);
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

/// The cells entities stand on among the [`AREA_SIDE`] by
/// [`AREA_SIDE`] around `centre`, laid out as an [`Area`] is. Off
/// the world's edge, none.
pub fn occupied(turn: &Turn, centre: CellIndex) -> [u16; AREA_SIDE] {
    let reach = AREA_CENTRE as i32;
    centre.offset(-reach, -reach).map_or([0; AREA_SIDE], |corner| turn.occupied(corner, AREA_SIDE as u32, AREA_SIDE as u32))
}

/// [`layer`] from further off: the tiles of `scale` around `centre`,
/// one set if `layer_type` holds at any of its cells. Up to
/// [`FARTHEST_SCALE`].
pub fn of_tiles(turn: &Turn, layer_type: LayerType, centre: CellIndex, scale: u32) -> Area {
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
            let Some(holding) = turn.tiles_holding(layer_type, first.into()) else {
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
        let holds = turn.any_in_tile(layer_type, cell.into(), scale);
        area.set[y] |= ((holds == Some(true)) as u16) << x;
        area.hot[y] |= (holds.is_some() as u16) << x;
    }
    area
}
