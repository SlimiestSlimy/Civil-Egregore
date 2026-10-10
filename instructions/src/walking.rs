//! Walking: the steps an entity may take over the terrain, and the way
//! to what it seeks -- the area about it read (`area`), the
//! terrain's walls among them (`worldgen`), and paths found over them
//! (`pathfinding`): what a rule that walks asks.

use chunk_storage::LayerType;
use coordinates::CellIndex;
use pathfinding::{a_star, Cell, Rows, Walls};
use crate::area::{self, AREA_CENTRE, AREA_SIDE, FARTHEST_SCALE};
use crate::around::{self, squeeze};
use crate::cells;
use crate::mask::about;
use simulation::Turn;
use type_registry::{COLLISION, WALL_EAST, WALL_SOUTH};

// The area a turn reads is the area paths are found over.
const _: () = assert!(pathfinding::SIDE == AREA_SIDE);

/// Where an entity stands in the area about it.
const HERE: Cell = Cell { x: AREA_CENTRE as u8, y: AREA_CENTRE as u8 };

/// The cell of the world `cell` of the area about `at` is: none off
/// the world.
fn of_the_area(at: CellIndex, cell: Cell) -> Option<CellIndex> {
    at.offset(cell.x as i32 - AREA_CENTRE as i32, cell.y as i32 - AREA_CENTRE as i32)
}

/// `rows` of the area about `at`, less the cells entities stand on
/// and those the collision plane holds.
fn unoccupied(turn: &Turn, at: CellIndex, rows: [&Rows; 2]) -> [Rows; 2] {
    let (occupied, collision) = (area::occupied(turn, at), area::layer(turn, COLLISION, at).set);
    rows.map(|rows| std::array::from_fn(|row| rows[row] & !occupied[row] & !collision[row]))
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

/// Which of the 3x3 cells around `at` may be stepped to from it,
/// nine bits ([`crate::around`]): those no wall of the terrain is
/// before -- its own among them. Where the wall layers are not hot,
/// none bars.
pub fn around_unwalled(turn: &Turn, at: CellIndex) -> u16 {
    let Some(corner) = about(at, around::SIDE) else {
        return around::ALL;
    };
    let [east, south] = turn.windows([WALL_EAST, WALL_SOUTH], corner, around::SIDE, around::SIDE).map(|window| squeeze(window.set));
    // Whether the cell at `bit` of the nine keeps a wall east of it, or south.
    let (east_of, south_of) = (|bit: u16| east >> bit & 1, |bit: u16| south >> bit & 1);
    // A wall is kept by the upper or left cell of the two: the centre's own east and south, its neighbours' west and north.
    let (to_east, to_west, to_south, to_north) = (east_of(4), east_of(3), south_of(4), south_of(1));
    // A diagonal is barred unless both ways round it are open.
    let to_south_east = to_east | to_south | south_of(5) | east_of(7);
    let to_south_west = to_west | to_south | south_of(3) | east_of(6);
    let to_north_east = to_east | to_north | south_of(2) | east_of(1);
    let to_north_west = to_west | to_north | south_of(0) | east_of(0);
    let barred = to_north_west | to_north << 1 | to_north_east << 2 | to_west << 3 | to_east << 5 | to_south_west << 6 | to_south << 7 | to_south_east << 8;
    around::ALL & !barred
}

/// Which of the 3x3 cells around `at` may be stepped to from it,
/// nine bits: those no wall of the terrain is before
/// ([`around_unwalled`]) and the collision plane does not hold --
/// nothing stands there that bars a step. Entities are not looked at:
/// a step onto one is turned back as it is applied.
pub fn around_steppable(turn: &Turn, at: CellIndex) -> u16 {
    around_unwalled(turn, at) & !around::layer(turn, COLLISION, at).set
}

/// The terrain's walls among the [`AREA_SIDE`] by [`AREA_SIDE`]
/// cells around `centre`, laid out as an [`crate::area::Area`] is: what paths
/// are found round.
pub fn area_walls(turn: &Turn, centre: CellIndex) -> Walls {
    let [east, south] = area::layers(turn, [WALL_EAST, WALL_SOUTH], centre).map(|area| area.set);
    Walls::new(east, south)
}

/// The cell to step to from `at` for the nearest of `goals` -- cells
/// of the area around `at` -- over the cells `passable`, round walls
/// and entities; none if no goal can be come to. One step: no route
/// is kept.
pub fn step_towards(turn: &mut Turn, at: CellIndex, goals: &Rows, passable: &Rows) -> Option<CellIndex> {
    let [passable, goals] = unoccupied(turn, at, [passable, goals]);
    let first = pathfinding::step_towards(&passable, &area_walls(turn, at), &goals, HERE, turn.random().draw())?.first;
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
    let open = around_steppable(turn, at) >> around::bit_of(at, to) & 1 == 1;
    (open && cells::hot(turn, layer_type, to)).then_some(SoughtStep { to, scale })
}

/// The cell to step to from `at` to come, by the shortest way, to
/// `to` -- a cell of the area around `at` -- over the cells
/// `passable`, no entity's cell walked on. None if `to` is out of
/// the area, or cannot be come to.
pub fn step_to(turn: &mut Turn, at: CellIndex, to: CellIndex, passable: &Rows) -> Option<CellIndex> {
    let (from, target) = (at.cartesian(), to.cartesian());
    let across = target.x as i64 - from.x as i64 + AREA_CENTRE as i64;
    let down = target.y as i64 - from.y as i64 + AREA_CENTRE as i64;
    if !(0..AREA_SIDE as i64).contains(&across) || !(0..AREA_SIDE as i64).contains(&down) {
        return None;
    }
    let [passable, _] = unoccupied(turn, at, [passable, passable]);
    let first = a_star(&passable, &area_walls(turn, at), HERE, Cell { x: across as u8, y: down as u8 })?.first;
    of_the_area(at, first)
}
