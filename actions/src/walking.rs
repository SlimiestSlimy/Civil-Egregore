//! Walking: the steps an entity may take over the terrain, and the way
//! to what it seeks -- the area about it read (`area`), the
//! terrain's walls among them (`worldgen`), and paths found over them
//! (`pathfinding`): what a rule that walks asks.

use chunk_storage::LayerType;
use coordinates::CellIndex;
use pathfinding::{a_star, Cell, Rows, Walls};
use crate::area::{self, AREA_CENTRE, AREA_SIDE, FARTHEST_SCALE};
use crate::around::{self, TilesAbout, Cells};
use crate::tiles_about;
use crate::cells;
use simulation::Turn;
use type_registry::{COLLISION, WALL_EAST, WALL_NORTH, WALL_SOUTH, WALL_WEST};

// The area a turn reads is the area paths are found over.
const _: () = assert!(pathfinding::AREA_SIDE == AREA_SIDE);

/// Where an entity stands in the area about it.
const AREA_CENTRE_CELL: Cell = Cell { x: AREA_CENTRE as u8, y: AREA_CENTRE as u8 };

/// The cell of the world `cell` of the area about `entity_cell` is: none off
/// the world.
fn of_the_area(entity_cell: CellIndex, cell: Cell) -> Option<CellIndex> {
    entity_cell.at_cartesian_offset(cell.x as i32 - AREA_CENTRE as i32, cell.y as i32 - AREA_CENTRE as i32)
}

/// What is in the way in the area about `entity_cell`, read at once: the
/// cells the collision plane holds -- what stands there, entities
/// among it -- and the terrain's walls.
fn in_the_way(turn: &Turn, entity_cell: CellIndex) -> (Rows, Walls) {
    let [collision_rows, north_walls, west_walls, south_walls, east_walls] = area::read_layers(turn, [COLLISION, WALL_NORTH, WALL_WEST, WALL_SOUTH, WALL_EAST], entity_cell).map(|area| area.set_rows);
    (collision_rows, Walls::one_way(north_walls, west_walls, south_walls, east_walls))
}

/// `cell_rows` less the cells of `collision_rows`.
fn rows_less_collisions(cell_rows: &Rows, collision_rows: &Rows) -> Rows {
    std::array::from_fn(|row_index| cell_rows[row_index] & !collision_rows[row_index])
}

/// A step found by [`seek`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoughtStep {
    /// The cell to step to.
    pub step_cell: CellIndex,
    /// How far off it had to look: 0 in the area around, else the scale
    /// of the tiles it looked over, `2^scale` cells a side.
    pub scale: u32,
}

/// Which of the cells `about_tiles` a cell may be stepped to from it, for
/// the terrain's walls: those none is before -- its own among them.
/// A wall is one way's, the cell's it is stepped from: a drop lets a
/// step down and none back. A diagonal is barred unless both ways
/// round it are open. Where the wall layers are not hot, none bars;
/// the walls' tiles are asked a cell at a time as they lie, and not
/// at all where they hold nothing.
pub fn around_unwalled(turn: &Turn, about_tiles: &TilesAbout) -> Cells {
    let [north_walls, west_walls, south_walls, east_walls] = tiles_about::read_layers(turn, [WALL_NORTH, WALL_WEST, WALL_SOUTH, WALL_EAST], about_tiles, about_tiles.nine_word()).map(|tiles| tiles.cells_set);
    if north_walls | west_walls | south_walls | east_walls == 0 {
        return about_tiles.nine_cells();
    }
    use around::{EAST, HERE, NORTH, NORTH_EAST, NORTH_WEST, SOUTH, SOUTH_EAST, SOUTH_WEST, WEST};
    // Whether no step leaves the cell `way` from it to the north, the west, the south, the east.
    let (north_of, west_of) = (|way: u32| north_walls & about_tiles.way_word(way) != 0, |way: u32| west_walls & about_tiles.way_word(way) != 0);
    let (south_of, east_of) = (|way: u32| south_walls & about_tiles.way_word(way) != 0, |way: u32| east_walls & about_tiles.way_word(way) != 0);
    let (to_east, to_west, to_south, to_north) = (east_of(HERE), west_of(HERE), south_of(HERE), north_of(HERE));
    // A diagonal is barred unless both ways round it are open: each a step out of the centre, and one out of the cell it comes to.
    let to_south_east = to_east | to_south | south_of(EAST) | east_of(SOUTH);
    let to_south_west = to_west | to_south | south_of(WEST) | west_of(SOUTH);
    let to_north_east = to_east | to_north | north_of(EAST) | east_of(NORTH);
    let to_north_west = to_west | to_north | north_of(WEST) | west_of(NORTH);
    let barred_ways = [(to_north_west, NORTH_WEST), (to_north, NORTH), (to_north_east, NORTH_EAST), (to_west, WEST), (to_east, EAST), (to_south_west, SOUTH_WEST), (to_south, SOUTH), (to_south_east, SOUTH_EAST)];
    Cells(barred_ways.iter().fold(about_tiles.nine_word(), |open_word, &(way_barred, way)| if way_barred { open_word & !about_tiles.way_word(way) } else { open_word }))
}

/// Which of the neighbours `about_tiles` a cell may be stepped to from it:
/// those the collision plane does not hold -- nothing stands there
/// that bars a step -- and no wall of the terrain is before
/// ([`around_unwalled`]). The plane is asked first, the ring about
/// the cell ANDed with its tiles: hemmed in, there is no step, and no
/// wall is read.
pub fn around_steppable(turn: &Turn, about_tiles: &TilesAbout) -> Cells {
    let free_ring = about_tiles.ring_word() & !tiles_about::read_layer(turn, COLLISION, about_tiles, about_tiles.ring_word()).cells_set;
    if free_ring == 0 {
        return Cells::default();
    }
    Cells(free_ring) & around_unwalled(turn, about_tiles)
}

/// The terrain's walls among the [`AREA_SIDE`] by [`AREA_SIDE`]
/// cells around `centre_cell`, laid out as an [`crate::area::Area`] is: what paths
/// are found round.
pub fn area_walls(turn: &Turn, centre_cell: CellIndex) -> Walls {
    let [north_walls, west_walls, south_walls, east_walls] = area::read_layers(turn, [WALL_NORTH, WALL_WEST, WALL_SOUTH, WALL_EAST], centre_cell).map(|area| area.set_rows);
    Walls::one_way(north_walls, west_walls, south_walls, east_walls)
}

/// The cell to step to from `entity_cell` for the nearest of `goal_rows` -- cells
/// of the area around `entity_cell` -- over the cells `passable`, round walls
/// and what the collision plane holds; none if no goal can be come to. One step: no route
/// is kept.
pub fn step_towards(turn: &mut Turn, entity_cell: CellIndex, goal_rows: &Rows, passable: &Rows) -> Option<CellIndex> {
    let (collision_rows, walls) = in_the_way(turn, entity_cell);
    let first_step = pathfinding::step_towards(&rows_less_collisions(passable, &collision_rows), &walls, &rows_less_collisions(goal_rows, &collision_rows), AREA_CENTRE_CELL, turn.random_stream().next_draw())?.first_cell;
    of_the_area(entity_cell, first_step)
}

/// The step from `entity_cell` towards the nearest cell `layer_type` holds
/// at: in the area around it, else over tiles by scale, as far as
/// an entity reaches. None if there is none in reach.
pub fn seek(turn: &mut Turn, entity_cell: CellIndex, layer_type: LayerType) -> Option<SoughtStep> {
    let near_area = area::read_layer(turn, layer_type, entity_cell);
    if let Some(near_step_cell) = step_towards(turn, entity_cell, &near_area.set_rows, &near_area.hot) {
        return Some(SoughtStep { step_cell: near_step_cell, scale: 0 });
    }
    let mut tiles = area::of_tiles(turn, layer_type, entity_cell, FARTHEST_SCALE);
    let no_walls = Walls::default();
    let mut path = pathfinding::step_towards(&tiles.hot, &no_walls, &tiles.set_rows, AREA_CENTRE_CELL, turn.random_stream().next_draw());
    // In its own tile alone, there is no tile to step towards: a finer scale sees where in it.
    let in_own_tile = tiles.set_rows[AREA_CENTRE] >> AREA_CENTRE & 1 == 1;
    // The nearest is at least this many cells off: a scale's tiles reach under nine tiles.
    let least_cells_off = match path {
        _ if in_own_tile => 0,
        Some(path) => (path.steps as u32 - 1) << FARTHEST_SCALE,
        None => return None,
    };
    let mut scale = 1;
    while scale < FARTHEST_SCALE {
        if 9 << scale > least_cells_off + 1 {
            tiles = area::of_tiles(turn, layer_type, entity_cell, scale);
            if let Some(nearer_path) = pathfinding::step_towards(&tiles.hot, &no_walls, &tiles.set_rows, AREA_CENTRE_CELL, turn.random_stream().next_draw()) {
                path = Some(nearer_path);
                break;
            }
        }
        scale += 1;
    }
    let path = path?;
    // The cell beside it the way the tile is: stepped to if it is in the world hot.
    let step_cell = of_the_area(entity_cell, path.first_cell)?;
    // From far off no wall is seen, nor what stands in the way: a step either bars is not taken.
    let about_tiles = around::tiles_about_cell(entity_cell);
    let step_open = !(around_steppable(turn, &about_tiles) & about_tiles.cell_towards(around::way_of(entity_cell, step_cell))).is_empty();
    (step_open && cells::hot(turn, layer_type, step_cell)).then_some(SoughtStep { step_cell, scale })
}

/// The cell to step to from `entity_cell` to come, by the shortest way, to
/// `target_cell` -- a cell of the area around `entity_cell` -- over the cells
/// `passable`, no cell the collision plane holds walked on. None if `target_cell` is out of
/// the area, or cannot be come to.
pub fn step_to(turn: &mut Turn, entity_cell: CellIndex, target_cell: CellIndex, passable: &Rows) -> Option<CellIndex> {
    let (entity_cartesian, target_cartesian) = (entity_cell.cartesian(), target_cell.cartesian());
    let across_in_area = target_cartesian.x as i64 - entity_cartesian.x as i64 + AREA_CENTRE as i64;
    let down_in_area = target_cartesian.y as i64 - entity_cartesian.y as i64 + AREA_CENTRE as i64;
    if !(0..AREA_SIDE as i64).contains(&across_in_area) || !(0..AREA_SIDE as i64).contains(&down_in_area) {
        return None;
    }
    let (collision_rows, walls) = in_the_way(turn, entity_cell);
    let first_step = a_star(&rows_less_collisions(passable, &collision_rows), &walls, AREA_CENTRE_CELL, Cell { x: across_in_area as u8, y: down_in_area as u8 })?.first_cell;
    of_the_area(entity_cell, first_step)
}
