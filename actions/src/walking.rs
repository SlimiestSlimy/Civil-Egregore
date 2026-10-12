//! Walking: the steps an entity may take over the terrain, and the way
//! to what it seeks -- the area about it read (`area`), the
//! terrain's walls among them (`worldgen`), and paths found over them
//! (`pathfinding`): what a rule that walks asks.

use chunk_storage::LayerType;
use coordinates::CellIndex;
use pathfinding::{a_star, Cell, Rows, Walls};
use crate::area::{self, AREA_CENTRE, AREA_SIDE, FARTHEST_SCALE};
use crate::around::{self, About, Cells};
use crate::tiles_about;
use crate::cells;
use simulation::Turn;
use type_registry::{COLLISION, WALL_EAST, WALL_NORTH, WALL_SOUTH, WALL_WEST};

// The area a turn reads is the area paths are found over.
const _: () = assert!(pathfinding::SIDE == AREA_SIDE);

/// Where an entity stands in the area about it.
const HERE: Cell = Cell { x: AREA_CENTRE as u8, y: AREA_CENTRE as u8 };

/// The cell of the world `cell` of the area about `at` is: none off
/// the world.
fn of_the_area(at: CellIndex, cell: Cell) -> Option<CellIndex> {
    at.offset(cell.x as i32 - AREA_CENTRE as i32, cell.y as i32 - AREA_CENTRE as i32)
}

/// What is in the way in the area about `at`, read at once: the
/// cells the collision plane holds -- what stands there, entities
/// among it -- and the terrain's walls.
fn in_the_way(turn: &Turn, at: CellIndex) -> (Rows, Walls) {
    let [held, north, west, south, east] = area::layers(turn, [COLLISION, WALL_NORTH, WALL_WEST, WALL_SOUTH, WALL_EAST], at).map(|area| area.set);
    (held, Walls::one_way(north, west, south, east))
}

/// `rows` less the cells of `held`.
fn less(rows: &Rows, held: &Rows) -> Rows {
    std::array::from_fn(|row| rows[row] & !held[row])
}

/// A step found by [`seek`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoughtStep {
    /// The cell to step to.
    pub to: CellIndex,
    /// How far off it had to look: 0 in the area around, else the scale
    /// of the tiles it looked over, `2^scale` cells a side.
    pub scale: u32,
}

/// Which of the cells `about` a cell may be stepped to from it, for
/// the terrain's walls: those none is before -- its own among them.
/// A wall is one way's, the cell's it is stepped from: a drop lets a
/// step down and none back. A diagonal is barred unless both ways
/// round it are open. Where the wall layers are not hot, none bars;
/// the walls' tiles are asked a cell at a time as they lie, and not
/// at all where they hold nothing.
pub fn around_unwalled(turn: &Turn, about: &About) -> Cells {
    let [north, west, south, east] = tiles_about::layers(turn, [WALL_NORTH, WALL_WEST, WALL_SOUTH, WALL_EAST], about, about.nine_word()).map(|tiles| tiles.set);
    if north | west | south | east == 0 {
        return about.nine();
    }
    use around::{EAST, HERE, NORTH, NORTH_EAST, NORTH_WEST, SOUTH, SOUTH_EAST, SOUTH_WEST, WEST};
    // Whether no step leaves the cell `way` from it to the north, the west, the south, the east.
    let (north_of, west_of) = (|way: u32| north & about.way_word(way) != 0, |way: u32| west & about.way_word(way) != 0);
    let (south_of, east_of) = (|way: u32| south & about.way_word(way) != 0, |way: u32| east & about.way_word(way) != 0);
    let (to_east, to_west, to_south, to_north) = (east_of(HERE), west_of(HERE), south_of(HERE), north_of(HERE));
    // A diagonal is barred unless both ways round it are open: each a step out of the centre, and one out of the cell it comes to.
    let to_south_east = to_east | to_south | south_of(EAST) | east_of(SOUTH);
    let to_south_west = to_west | to_south | south_of(WEST) | west_of(SOUTH);
    let to_north_east = to_east | to_north | north_of(EAST) | east_of(NORTH);
    let to_north_west = to_west | to_north | north_of(WEST) | west_of(NORTH);
    let barred = [(to_north_west, NORTH_WEST), (to_north, NORTH), (to_north_east, NORTH_EAST), (to_west, WEST), (to_east, EAST), (to_south_west, SOUTH_WEST), (to_south, SOUTH), (to_south_east, SOUTH_EAST)];
    Cells(barred.iter().fold(about.nine_word(), |open, &(barred, way)| if barred { open & !about.way_word(way) } else { open }))
}

/// Which of the neighbours `about` a cell may be stepped to from it:
/// those the collision plane does not hold -- nothing stands there
/// that bars a step -- and no wall of the terrain is before
/// ([`around_unwalled`]). The plane is asked first, the ring about
/// the cell ANDed with its tiles: hemmed in, there is no step, and no
/// wall is read.
pub fn around_steppable(turn: &Turn, about: &About) -> Cells {
    let free = about.ring_word() & !tiles_about::layer(turn, COLLISION, about, about.ring_word()).set;
    if free == 0 {
        return Cells::default();
    }
    Cells(free) & around_unwalled(turn, about)
}

/// The terrain's walls among the [`AREA_SIDE`] by [`AREA_SIDE`]
/// cells around `centre`, laid out as an [`crate::area::Area`] is: what paths
/// are found round.
pub fn area_walls(turn: &Turn, centre: CellIndex) -> Walls {
    let [north, west, south, east] = area::layers(turn, [WALL_NORTH, WALL_WEST, WALL_SOUTH, WALL_EAST], centre).map(|area| area.set);
    Walls::one_way(north, west, south, east)
}

/// The cell to step to from `at` for the nearest of `goals` -- cells
/// of the area around `at` -- over the cells `passable`, round walls
/// and what the collision plane holds; none if no goal can be come to. One step: no route
/// is kept.
pub fn step_towards(turn: &mut Turn, at: CellIndex, goals: &Rows, passable: &Rows) -> Option<CellIndex> {
    let (held, walls) = in_the_way(turn, at);
    let first = pathfinding::step_towards(&less(passable, &held), &walls, &less(goals, &held), HERE, turn.random().draw())?.first;
    of_the_area(at, first)
}

/// The step from `at` towards the nearest cell `layer_type` holds
/// at: in the area around it, else over tiles by scale, as far as
/// an entity reaches. None if there is none in reach.
pub fn seek(turn: &mut Turn, at: CellIndex, layer_type: LayerType) -> Option<SoughtStep> {
    let near = area::layer(turn, layer_type, at);
    if let Some(to) = step_towards(turn, at, &near.set, &near.hot) {
        return Some(SoughtStep { to, scale: 0 });
    }
    let mut tiles = area::of_tiles(turn, layer_type, at, FARTHEST_SCALE);
    let no_walls = Walls::default();
    let mut path = pathfinding::step_towards(&tiles.hot, &no_walls, &tiles.set, HERE, turn.random().draw());
    // In its own tile alone, there is no tile to step towards: a finer scale sees where in it.
    let own = tiles.set[AREA_CENTRE] >> AREA_CENTRE & 1 == 1;
    // The nearest is at least this many cells off: a scale's tiles reach under nine tiles.
    let least = match path {
        _ if own => 0,
        Some(path) => (path.steps as u32 - 1) << FARTHEST_SCALE,
        None => return None,
    };
    let mut scale = 1;
    while scale < FARTHEST_SCALE {
        if 9 << scale > least + 1 {
            tiles = area::of_tiles(turn, layer_type, at, scale);
            if let Some(nearer) = pathfinding::step_towards(&tiles.hot, &no_walls, &tiles.set, HERE, turn.random().draw()) {
                path = Some(nearer);
                break;
            }
        }
        scale += 1;
    }
    let path = path?;
    // The cell beside it the way the tile is: stepped to if it is in the world hot.
    let to = of_the_area(at, path.first)?;
    // From far off no wall is seen, nor what stands in the way: a step either bars is not taken.
    let about = around::about(at);
    let open = !(around_steppable(turn, &about) & about.towards(around::way_of(at, to))).is_empty();
    (open && cells::hot(turn, layer_type, to)).then_some(SoughtStep { to, scale })
}

/// The cell to step to from `at` to come, by the shortest way, to
/// `to` -- a cell of the area around `at` -- over the cells
/// `passable`, no cell the collision plane holds walked on. None if `to` is out of
/// the area, or cannot be come to.
pub fn step_to(turn: &mut Turn, at: CellIndex, to: CellIndex, passable: &Rows) -> Option<CellIndex> {
    let (from, target) = (at.cartesian(), to.cartesian());
    let across = target.x as i64 - from.x as i64 + AREA_CENTRE as i64;
    let down = target.y as i64 - from.y as i64 + AREA_CENTRE as i64;
    if !(0..AREA_SIDE as i64).contains(&across) || !(0..AREA_SIDE as i64).contains(&down) {
        return None;
    }
    let (held, walls) = in_the_way(turn, at);
    let first = a_star(&less(passable, &held), &walls, HERE, Cell { x: across as u8, y: down as u8 })?.first;
    of_the_area(at, first)
}
