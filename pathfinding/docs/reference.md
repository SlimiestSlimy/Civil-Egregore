# Pathfinding, function by function

The design is in `pathfinding.md`.

## `lib.rs`

`AREA_SIDE` (16), `CELLS` (256, an area's). **`Rows`**: `[u16; AREA_SIDE]`, an area's cells of one kind,
cell `(x, y)` at bit `x` of row `y`. **`Cell`** `{x, y}`, privately `area_index()`, its place row by row, and `from_index(area_index)`, the cell there. **`Path`**
`{first_cell, steps}`: a path's first cell after its start, and its length.

**`rows_hold(area_rows, cell)`**. **`steps_apart(from_cell, to_cell)`**: the greater of their
distances across and down.

**`Wave::from_goals(goal_rows)`**: a search with the goals alone reached;
**`reached`**; **`spread_one_step(passable, walls)`**: every passable neighbour of a
reached cell that may step onto it reached (**`spread_back`**), whether any was.

**`step_towards(passable, walls, goal_rows, from_cell, pick_number)`**: waves from every goal
until one comes beside `from_goals`; the neighbour reached -- the `pick`-th
of those reached together -- and the steps to the goal. `None` if no
goal can be walked to.

**`nearest_goal(goal_rows, from_cell, pick_number)`**: the cell of `goals` nearest `from_goals`,
nothing in the way counted, the `pick`-th of those equally near
(**`for_each`**).

## `a_star.rs`

**`a_star(passable, walls, from_cell, to_cell)`**: the shortest path, searched
backwards from `to` (**`SearchQueue`**: a binary heap in an array of `QUEUE_ROOM`
entries, `push`, `pop`; `STEPS`, the eight neighbours). `None` if there is no way.

## `morton.rs`

The search in Morton order (`pathfinding.md`, "In Morton order").
`TILES_WORDS` (16), `TILES_SIDE` (32), **`Tiles`**.
**`bit_at(x, y)`**: a cell's word and bit. **`step_up(cells, lane)`**,
**`step_down(cells, lane)`**: a set stepped a cell east or south, west
or north (`lane` 0 for x, 1 for y), those past the edge gone.
**`MortonWalls::new(east, south)`**, walls of both ways, with
**`spread_through(cells)`**: the cells and every neighbour no wall is
before. **`step_towards(passable, walls, goals, (x, y), pick)`**: the
first step to the nearest goal and how far it is, of steps as short
the `pick`-th in `NEIGHBOUR_STEPS`' order. Private:
`bits_whose_index_masks_to(mask, value)`, `coordinate_bits(lane)`,
`STEP_UP_KINDS`, `STEP_DOWN_KINDS`, `KIND_SHIFTS`,
`word_tile_at(x, y)`, `spread_coordinate_bits(of)`,
`WORD_TILE_BESIDE`, `cells_less(cells, other)`,
`cells_of_either(cells, other)`; `MortonWalls`' fields
`no_step_across`, `no_south`, `no_north`, `any_wall`.

## `walls.rs`

**`Walls::one_way(north_walls, west_walls, south_walls, east_walls)`**: the steps that cannot
be taken -- for each of the four ways the cells no step leaves that
way, and the diagonals they bar, worked out from them (a wall on
either way round); **`Walls::new(east_walls, south_walls)`**: walls barring both
ways, east and south of cells; **`bars_step(cell, dx, dy)`**. Private:
the eight ways' places among its masks, `NORTH`, `WEST`, `SOUTH`,
`EAST`, `NORTH_WEST`, `NORTH_EAST`, `SOUTH_WEST`, `SOUTH_EAST`, and
**`way_of(dx, dy)`**; **`spread_forwards(from_rows, walls)`**, the cells a step from
one of `rows` comes to, and **`spread_back(to_rows, walls)`**, the cells
a step from which comes to one. `Wave::spread_one_step`,
`step_towards` and `a_star` each take them after `passable`;
`Walls::default()` is none.

## `diagnostics/`, `transient_data.rs`

The folders every crate has (`../../docs/style_guide.md`, "One shape for
every crate"). `diagnostics/mod.rs` gathers nothing yet.
`transient_data::TRANSIENT_DATA` names the crate's `transient_data/`
folder, where its runs would leave what they make.
