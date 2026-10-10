# Instructions: reference

What a rule is made of: small pieces of behaviour -- queries of the
simulation and changes queued to it alike -- each a free function over
a superchunk's `Turn`. The design: `instructions.md`. `lib.rs` hands on
**`Turn`**, **`Simulation`** and **`TickReport`**, the simulation's.

Instructions that only read are in `read/`, those that only queue a
change in `write/`; the shapes they answer in are modules beside them.

## `around.rs`, `area.rs`, `mask.rs`: the shapes

**`around`**: the 3x3 cells about a cell as nine bits -- **`CENTRE`**,
**`RING`**, **`ALL`**; **`Around`** `{set, hot}`; **`squeeze`**: a
window's 3x3 into nine bits. **`cell(at, bit)`**, **`bit_of(at,
cell)`**: a bit and its cell. **`pick(random, choices)`**,
**`prefer(random, wanted, open)`**: one drawn.

**`area`**: **`Area`** `{set, hot}`, a row a `u16`, `AREA_SIDE` (16) a
side, its centre at `AREA_CENTRE`; **`Area::count`**; `FARTHEST_SCALE`
(6), the coarsest tiles asked of.

**`mask`**: **`Mask`**, a square of cells a bit each, its side one of
**`SIDES`** (4 to 1,024): **`empty(side)`**, **`full(side)`**,
**`disc(side)`**; **`side`**, **`row(y)`**, **`get(x, y)`**, **`set(x,
y, to)`**, **`clear`**, **`count`**, **`is_empty`**; **`and`**,
**`or`**, **`and_not`** with another of its side; **`cells()`** the set
ones, **`pick(random)`** one drawn. **`about(centre, side)`**: the top
left cell of the square about a cell; **`cell(origin, x, y)`**: a cell
of it.

## `read/cells.rs`

**`each_sampled(turn, type, probability, samples, each)`**: `each` run
on every cell sampled, with its counts. **`hot(turn, type, cell)`**,
**`holds`**, **`lacks`**: the cell read. **`value(turn, plane,
cell)`**: a wide plane's number. **`square(turn, type, cell, side)`**:
up to 8x8 cells about a cell as a `Window`, with their top left cell.

## `read/entities.rs`

**`each_woken(turn, layers, state, each)`**: `each` run on every entity
waking.

## `read/around.rs`

**`layer(turn, type, at)`**: the 3x3 cells of a layer about a cell, one
window squeezed. **`occupied(turn, at)`**: those entities stand on.
**`free_beside(turn, at, open)`**: one of `open` none stands on, drawn.

## `read/area.rs`

**`layer(turn, type, centre)`**, **`layers(turn, types, centre)`**:
the area of one layer, or of several at once, four windows each.
**`occupied(turn, centre)`**: the entities on its cells.
**`of_tiles(turn, type, centre, scale)`**: the tiles of `scale` around
`centre`, set where the type holds at any cell.

## `read/mask.rs`

**`layer(turn, type, origin, set, hot)`**: the square of a layer into
two masks, a window a time; **`layer_under(turn, type, origin, under,
set, hot)`**: the cells `under` has alone, the windows it has none in
not read (**`read_where`**).

## `read/walking.rs`

What an entity that walks asks, of a turn: the simulation's cells, the
terrain's walls and `pathfinding` met here. **`around_unwalled(turn,
at)`**: the 3x3 cells about `at` no wall is before, nine bits.
**`area_walls(turn, centre)`**: the area's walls, for paths.
**`step_towards(turn, at, goals, passable)`**, **`step_to(turn, at, to,
passable)`**: the cell to step to for the nearest goal, or for one
cell, round the entities in the way. **`seek(turn, at, type)`**: the
step to the nearest cell the type holds at, the area first, then tiles
by scale, `FARTHEST_SCALE` first and the finest that reach after -- a
**`SoughtStep`** `{to, scale}`.

## `write/cells.rs`

**`set(turn, type, cell)`**, **`clear`**: a write queued.
**`set_value(turn, plane, cell, value)`**: a wide plane's number put.

## `write/entities.rs`

**`spawn(turn, kind, at, wake, attributes)`**: a new entity, its ID
drawn and returned. **`sleep(turn, entity, wake)`**: a move to where it
stands. **`commit(turn, edit, to, wake)`**: an `EntityEdit`'s entity
moved if no attribute changed, else put whole. **`remove(turn,
entity)`**.

## `write/mask.rs`

**`set(turn, type, origin, mask)`**, **`clear`**: writes queued for
every cell of the mask, as rectangles (**`write`**, **`next`**) -- how
many.

## `between_ticks.rs`

**`EntitiesBetweenTicks`**: the world's entities off any turn, lent by
whoever runs the world (**`of`**). **`now()`**, **`put(header,
attributes)`**: queued, in the world once the runner places them.

## `layers.rs`

`GRASS`, `DIRT`, `WET`, `WALL_EAST`, `WALL_SOUTH`: the layers a world
has before a rule adds its own.

## `mock_world.rs`

**`MockWorld::grass_on_dirt(count, grass_cells)`**: superchunks of grass
on dirt, all hot; **`on_threads`**. **`tick(seed, rule)`**: a
`TickReport`. **`plant_grass(at, width, height)`**, **`put(header,
attributes)`**, **`between_ticks()`** and **`settle()`**. Asked:
**`superchunks()`**, **`count(type)`**, **`words(type)`**,
**`entities()`**, **`count_entities(kind)`**; **`held()`** for whoever
measures what holds a world.

## The root

The words the instructions are asked in: `CellIndex`, `CellCartesian`,
`ChunkIndex`, `SuperchunkIndex`, `LayerType`, `Wide`, `Bits4`,
`Attribute`, `AttributeType`, `EntityType`, `EntityId`, `Header`,
`EntityRef`, `EntityEdit`, `Turn`, `TickReport`, `Rng`.
