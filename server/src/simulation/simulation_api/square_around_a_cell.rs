//! The area about a cell: the 16x16 cells around it, a row a word --
//! what an entity sees of the world about it at once, and what paths
//! are found over -- a layer's, several layers' at once, and the same
//! from further off, a tile a bit.

use crate::simulation::simulation_api::cell_reads_and_writes::corner_about;
use utilities::bitmap::window::{rows_from_morton, WORD_TILE_SIDE};
use crate::bitplane_manager::COARSEST_TILES_IN_CHUNK;
use crate::chunk_storage::LayerType;
use utilities::coordinates::{cartesian_from_place, CellCartesian, CellIndex};
use crate::simulation::SimulationTickState;

/// Cells along the side of an [`Area`].
pub const AREA_SIDE: usize = 16;
/// The column and the row of an [`Area`] its centre is at.
pub const AREA_CENTRE: usize = AREA_SIDE / 2;
/// Cells in an aligned tile of an area's side: the bits of a cell's
/// index that place it in one.
const TILE_CELLS: u64 = (AREA_SIDE * AREA_SIDE) as u64;

/// The cells of one layer type around a cell ([`read_layer`]),
/// a row a word: cell `(x, y)` from the area's top left at bit `x` of
/// row `y`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Area {
    /// The cells the type holds at: hot ones only.
    pub set_rows: [u16; AREA_SIDE],
    /// The cells in hot bitmaps: in the world, and read.
    pub hot: [u16; AREA_SIDE],
}

impl Area {
    /// How many of its cells the type holds at.
    pub fn count(&self) -> u32 {
        self.set_rows.iter().map(|set_row| set_row.count_ones()).sum()
    }
}

/// The coarsest scale [`of_tiles`] looks over: tiles `2^6`
/// cells a side, [`AREA_SIDE`] of them 1,024 cells -- an entity's reach.
pub const FARTHEST_SCALE: u32 = crate::bitplane_manager::COARSEST_SCALE;

/// The [`AREA_SIDE`] by [`AREA_SIDE`] cells around `centre_cell` -- it at
/// `(AREA_CENTRE, AREA_CENTRE)` of them -- of `layer_type`, as the
/// tick found them: four windows, a row a word.
pub fn read_layer(tick_state: &SimulationTickState, layer_type: LayerType, centre_cell: CellIndex) -> Area {
    let [area] = read_layers(tick_state, [layer_type], centre_cell);
    area
}

/// [`read_layer`], of each of `layer_types` at once: the four aligned tiles of
/// [`AREA_SIDE`] the area lies in, two by two, taken as they lie --
/// a lookup a layer -- and the words of them the area reaches turned
/// into rows, for the paths found over them.
pub fn read_layers<const N: usize>(tick_state: &SimulationTickState, layer_types: [LayerType; N], centre_cell: CellIndex) -> [Area; N] {
    let Some(area_corner) = corner_about(centre_cell, AREA_SIDE as u32) else {
        return layers_at_the_edge(tick_state, layer_types, centre_cell);
    };
    // The aligned tile the area's corner is in, and how far into it the corner is.
    let in_tile = area_corner.0 & (TILE_CELLS - 1);
    let (corner_across, corner_down) = cartesian_from_place(in_tile as usize);
    let tiles = tick_state.aligned_tiles_two_by_two(layer_types, CellIndex(area_corner.0 - in_tile), AREA_SIDE as u32);
    std::array::from_fn(|layer| {
        // The two by two tiles row by row, the rows the area does not reach left clear.
        let (mut wide_set_rows, mut hot) = ([0u32; 2 * AREA_SIDE], [0u32; 2 * AREA_SIDE]);
        for (tile, words) in tiles[layer].iter().enumerate() {
            let (tile_across, tile_down) = (AREA_SIDE as u32 * (tile as u32 & 1), AREA_SIDE as u32 * (tile as u32 >> 1));
            // A tile beside or below the corner's is reached only if the corner is into its own.
            let Some(words) = words.filter(|_| (tile_across == 0 || corner_across > 0) && (tile_down == 0 || corner_down > 0)) else {
                continue;
            };
            for (word_tile, &word) in words.iter().enumerate() {
                let (word_left, word_top) = (tile_across + WORD_TILE_SIDE * (word_tile as u32 & 1), tile_down + WORD_TILE_SIDE * (word_tile as u32 >> 1));
                if word_left + WORD_TILE_SIDE <= corner_across || word_left >= corner_across + AREA_SIDE as u32 || word_top + WORD_TILE_SIDE <= corner_down || word_top >= corner_down + AREA_SIDE as u32 {
                    continue;
                }
                let word_rows = rows_from_morton(word);
                for word_row in 0..WORD_TILE_SIDE {
                    wide_set_rows[(word_top + word_row) as usize] |= ((word_rows >> (WORD_TILE_SIDE * word_row) & 0xff) as u32) << word_left;
                    hot[(word_top + word_row) as usize] |= 0xff << word_left;
                }
            }
        }
        let mut area = Area::default();
        for area_row in 0..AREA_SIDE {
            area.set_rows[area_row] = (wide_set_rows[corner_down as usize + area_row] >> corner_across) as u16;
            area.hot[area_row] = (hot[corner_down as usize + area_row] >> corner_across) as u16;
        }
        area
    })
}

/// [`read_layers`], of a centre so near the world's edge that the area's
/// corner is off it: a window a quarter, those off the world left
/// clear and not hot.
#[cold]
fn layers_at_the_edge<const N: usize>(tick_state: &SimulationTickState, layer_types: [LayerType; N], centre_cell: CellIndex) -> [Area; N] {
    let mut areas = [Area::default(); N];
    let half_side = AREA_SIDE as u32 / 2;
    let centre_cartesian = centre_cell.cartesian();
    for quarter_index in 0..4u32 {
        let (quarter_across, quarter_down) = (quarter_index % 2 * half_side, quarter_index / 2 * half_side);
        let (Some(x), Some(y)) = ((centre_cartesian.x + quarter_across).checked_sub(half_side), (centre_cartesian.y + quarter_down).checked_sub(half_side)) else {
            continue;
        };
        let windows = tick_state.windows(layer_types, CellCartesian { x, y }.into(), half_side, half_side);
        for (area, window) in areas.iter_mut().zip(windows) {
            for window_row in 0..half_side {
                let area_row = (quarter_down + window_row) as usize;
                area.set_rows[area_row] |= ((window.cells_set >> (8 * window_row) & 0xff) as u16) << quarter_across;
                area.hot[area_row] |= ((window.hot >> (8 * window_row) & 0xff) as u16) << quarter_across;
            }
        }
    }
    areas
}

/// [`read_layer`] from further off: the tiles of `scale` around `centre_cell`,
/// one set if `layer_type` holds at any of its cells. Up to
/// [`FARTHEST_SCALE`].
pub fn of_tiles(tick_state: &SimulationTickState, layer_type: LayerType, centre_cell: CellIndex, scale: u32) -> Area {
    let mut area = Area::default();
    let centre_cartesian = centre_cell.cartesian();
    // The area's top left tile, in tiles from the world's: before the world, where the centre is near its edge.
    let (left_tile, top_tile) = ((centre_cartesian.x >> scale) as i64 - AREA_CENTRE as i64, (centre_cartesian.y >> scale) as i64 - AREA_CENTRE as i64);
    let world = (1i64 << u32::BITS) >> scale;
    if scale == FARTHEST_SCALE {
        // The chunks its tiles are in, each read at once.
        let chunk = COARSEST_TILES_IN_CHUNK.trailing_zeros() / 2;
        for (chunk_down, chunk_across) in ((top_tile >> chunk)..=((top_tile + AREA_SIDE as i64 - 1) >> chunk)).flat_map(|down_chunk| ((left_tile >> chunk)..=((left_tile + AREA_SIDE as i64 - 1) >> chunk)).map(move |across_chunk| (down_chunk, across_chunk))) {
            if chunk_across < 0 || chunk_down < 0 || chunk_across << chunk >= world || chunk_down << chunk >= world {
                continue;
            }
            let chunk_first_cell = CellCartesian { x: (chunk_across << (chunk + scale)) as u32, y: (chunk_down << (chunk + scale)) as u32 };
            let Some(holding_tiles) = tick_state.tiles_holding(layer_type, chunk_first_cell.into()) else {
                continue;
            };
            // Where the chunk's tiles are in the area: up to three before it, across and down.
            let (x, y) = ((chunk_across << chunk) - left_tile, (chunk_down << chunk) - top_tile);
            let chunk_side_tiles = 1i64 << chunk;
            for tile_row in (0..chunk_side_tiles).filter(|candidate_row| (0..AREA_SIDE as i64).contains(&(y + candidate_row))) {
                area.hot[(y + tile_row) as usize] |= ((((1u64 << chunk_side_tiles) - 1) << (x + chunk_side_tiles)) >> chunk_side_tiles) as u16;
            }
            let mut left_to_place = holding_tiles;
            while left_to_place != 0 {
                // A tile's index in its chunk is a Morton index: across in its even bits, down in its odd.
                let tile = left_to_place.trailing_zeros() as i64;
                let (x, y) = (x + (tile & 1 | tile >> 1 & 2), y + (tile >> 1 & 1 | tile >> 2 & 2));
                if (0..AREA_SIDE as i64).contains(&x) && (0..AREA_SIDE as i64).contains(&y) {
                    area.set_rows[y as usize] |= 1 << x;
                }
                left_to_place &= left_to_place - 1;
            }
        }
        return area;
    }
    for (y, x) in (0..AREA_SIDE).flat_map(|y| (0..AREA_SIDE).map(move |x| (y, x))) {
        let (tile_across, tile_down) = (left_tile + x as i64, top_tile + y as i64);
        if tile_across < 0 || tile_down < 0 || tile_across >= world || tile_down >= world {
            continue;
        }
        let cell = CellCartesian { x: (tile_across << scale) as u32, y: (tile_down << scale) as u32 };
        let tile_holds = tick_state.any_in_tile(layer_type, cell.into(), scale);
        area.set_rows[y] |= ((tile_holds == Some(true)) as u16) << x;
        area.hot[y] |= (tile_holds.is_some() as u16) << x;
    }
    area
}
