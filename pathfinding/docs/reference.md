# Pathfinding, function by function

The design is in `pathfinding.md`.

## `lib.rs`

`SIDE` (16), `CELLS` (256, an area's). **`Rows`**: `[u16; SIDE]`, an area's cells of one kind,
cell `(x, y)` at bit `x` of row `y`. **`Cell`** `{x, y}`, privately `index()`, its place row by row, and `from_index(index)`, the cell there. **`Path`**
`{first, steps}`: a path's first cell after its start, and its length.

**`holds(rows, cell)`**. **`steps_apart(a, b)`**: the greater of their
distances across and down.

**`Wave::from(goals)`**: a search with the goals alone reached;
**`reached`**; **`advance(passable, walls)`**: every passable neighbour of a
reached cell that may step onto it reached (**`spread_back`**), whether any was.

**`step_towards(passable, walls, goals, from, pick)`**: waves from every goal
until one comes beside `from`; the neighbour reached -- the `pick`-th
of those reached together -- and the steps to the goal. `None` if no
goal can be walked to.

**`nearest(goals, from, pick)`**: the cell of `goals` nearest `from`,
nothing in the way counted, the `pick`-th of those equally near
(**`for_each`**).

## `a_star.rs`

**`a_star(passable, walls, from, to)`**: the shortest path, searched
backwards from `to` (**`Queue`**: a binary heap in an array of `QUEUE`
entries, `push`, `pop`; `STEPS`, the eight neighbours). `None` if there is no way.

## `walls.rs`

**`Walls::one_way(north, west, south, east)`**: the steps that cannot
be taken -- for each of the four ways the cells no step leaves that
way, and the diagonals they bar, worked out from them (a wall on
either way round); **`Walls::new(east, south)`**: walls barring both
ways, east and south of cells; **`bars_step(cell, dx, dy)`**. Private:
the eight ways' places among its masks, `NORTH`, `WEST`, `SOUTH`,
`EAST`, `NORTH_WEST`, `NORTH_EAST`, `SOUTH_WEST`, `SOUTH_EAST`, and
**`way_of(dx, dy)`**; **`spread(rows, walls)`**, the cells a step from
one of `rows` comes to, and **`spread_back(rows, walls)`**, the cells
a step from which comes to one. `Wave::advance`,
`step_towards` and `a_star` each take them after `passable`;
`Walls::default()` is none.

## `diagnostics/`, `transient_data.rs`

The folders every crate has (`../../docs/style_guide.md`, "One shape for
every crate"). `diagnostics/mod.rs` gathers nothing yet.
`transient_data::TRANSIENT_DATA` names the crate's `transient_data/`
folder, where its runs would leave what they make.
