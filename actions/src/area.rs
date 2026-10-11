//! The area about a cell: the 16x16 cells around it, a row a word --
//! what an entity sees of the world about it at once, and what paths
//! are found over -- a layer's, several layers' at once, and the same
//! from further off, a tile a bit.

use crate::cells::corner_about as about;
use bitmap::window::{rows_from_morton, WORD_TILE_SIDE};
use bitplane_manager::COARSEST_TILES_IN_CHUNK;
use chunk_storage::LayerType;
use coordinates::{cartesian_from_place, CellCartesian, CellIndex};
use simulation::Turn;

/// Cells along the side of an [`Area`].
pub const AREA_SIDE: usize = 16;
/// The column and the row of an [`Area`] its centre is at.
pub const AREA_CENTRE: usize = AREA_SIDE / 2;
/// Cells in an aligned tile of an area's side: the bits of a cell's
/// index that place it in one.
const TILE_CELLS: u64 = (AREA_SIDE * AREA_SIDE) as u64;

/// The cells of one layer type around a cell ([`layer`]),
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

/// The coarsest scale [`of_tiles`] looks over: tiles `2^6`
/// cells a side, [`AREA_SIDE`] of them 1,024 cells -- an entity's reach.
pub const FARTHEST_SCALE: u32 = bitplane_manager::COARSEST_SCALE;

/// The [`AREA_SIDE`] by [`AREA_SIDE`] cells around `centre` -- it at
/// `(AREA_CENTRE, AREA_CENTRE)` of them -- of `layer_type`, as the
/// tick found them: four windows, a row a word.
pub fn layer(turn: &Turn, layer_type: LayerType, centre: CellIndex) -> Area {
    let [area] = layers(turn, [layer_type], centre);
    area
}

/// [`layer`], of each of `types` at once: the four aligned tiles of
/// [`AREA_SIDE`] the area lies in, two by two, taken as they lie --
/// a lookup a layer -- and the words of them the area reaches turned
/// into rows, for the paths found over them.
pub fn layers<const N: usize>(turn: &Turn, types: [LayerType; N], centre: CellIndex) -> [Area; N] {
    let Some(corner) = about(centre, AREA_SIDE as u32) else {
        return layers_at_the_edge(turn, types, centre);
    };
    // The aligned tile the area's corner is in, and how far into it the corner is.
    let in_tile = corner.0 & (TILE_CELLS - 1);
    let (across, down) = cartesian_from_place(in_tile as usize);
    let tiles = turn.aligned_tiles_two_by_two(types, CellIndex(corner.0 - in_tile), AREA_SIDE as u32);
    std::array::from_fn(|layer| {
        // The two by two tiles row by row, the rows the area does not reach left clear.
        let (mut set, mut hot) = ([0u32; 2 * AREA_SIDE], [0u32; 2 * AREA_SIDE]);
        for (tile, words) in tiles[layer].iter().enumerate() {
            let (tile_across, tile_down) = (AREA_SIDE as u32 * (tile as u32 & 1), AREA_SIDE as u32 * (tile as u32 >> 1));
            // A tile beside or below the corner's is reached only if the corner is into its own.
            let Some(words) = words.filter(|_| (tile_across == 0 || across > 0) && (tile_down == 0 || down > 0)) else {
                continue;
            };
            for (word_tile, &word) in words.iter().enumerate() {
                let (left, top) = (tile_across + WORD_TILE_SIDE * (word_tile as u32 & 1), tile_down + WORD_TILE_SIDE * (word_tile as u32 >> 1));
                if left + WORD_TILE_SIDE <= across || left >= across + AREA_SIDE as u32 || top + WORD_TILE_SIDE <= down || top >= down + AREA_SIDE as u32 {
                    continue;
                }
                let rows = rows_from_morton(word);
                for row in 0..WORD_TILE_SIDE {
                    set[(top + row) as usize] |= ((rows >> (WORD_TILE_SIDE * row) & 0xff) as u32) << left;
                    hot[(top + row) as usize] |= 0xff << left;
                }
            }
        }
        let mut area = Area::default();
        for row in 0..AREA_SIDE {
            area.set[row] = (set[down as usize + row] >> across) as u16;
            area.hot[row] = (hot[down as usize + row] >> across) as u16;
        }
        area
    })
}

/// [`layers`], of a centre so near the world's edge that the area's
/// corner is off it: a window a quarter, those off the world left
/// clear and not hot.
#[cold]
fn layers_at_the_edge<const N: usize>(turn: &Turn, types: [LayerType; N], centre: CellIndex) -> [Area; N] {
    let mut areas = [Area::default(); N];
    let half = AREA_SIDE as u32 / 2;
    let centre = centre.cartesian();
    for quarter in 0..4u32 {
        let (across, down) = (quarter % 2 * half, quarter / 2 * half);
        let (Some(x), Some(y)) = ((centre.x + across).checked_sub(half), (centre.y + down).checked_sub(half)) else {
            continue;
        };
        let windows = turn.windows(types, CellCartesian { x, y }.into(), half, half);
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
