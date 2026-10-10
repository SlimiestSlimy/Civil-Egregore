# Instructions: reference

What a rule is made of: small pieces of behaviour -- queries of the
simulation and changes queued to it alike -- each a free function over
a superchunk's `Turn`. The design: `instructions.md`. `lib.rs` hands on
**`Turn`** and **`TickReport`**, the simulation's.

A module a subject: its shape, what reads it and what queues a change
to it together.

## `cells.rs`

**`each_sampled(turn, type, chance, samples, each)`**: `each` run
on every cell sampled -- each set cell of the type chosen with the
`Chance`, independently -- with its counts. **`hot(turn, type, cell)`**,
**`holds`**, **`lacks`**: the cell read. **`value(turn, plane,
cell)`**: a wide plane's number. **`square(turn, type, cell, side)`**:
up to 8x8 cells about a cell as a `Window`, with their top left cell.
**`set(turn, type, cell)`**, **`clear`**: a write queued.
**`set_value(turn, plane, cell, value)`**: a wide plane's number put.

## `entities.rs`

**`each_woken(turn, layers, state, each)`**: `each` run on every entity
waking. **`stands(turn, id, at)`**: whether an entity stood on a
cell; **`stands_beside(turn, id, centre, among)`**: which of the
neighbours it stood on. **`spawn_beside(turn, kind, centre, wanted,
open, wake, attributes)`**: a new entity on a neighbour or, that taken
by then, on the first free of the others open. **`spawn(turn, kind, at, wake, attributes)`**: a new entity,
its ID drawn and returned. **`sleep(turn, entity, wake)`**: a move to
where it stands. **`commit(turn, edit, to, wake)`**: an `EntityEdit`'s
entity moved if no attribute changed, else put whole. **`remove(turn,
entity)`**. **`EntitiesBetweenTicks`**: the world's entities off any
turn, lent by whoever runs the world (**`of`**). **`now()`**,
**`put(header, attributes)`**: queued, in the world once the runner
places them.

## `around.rs`

The 3x3 cells about a cell as nine bits -- `SIDE` (3), **`CENTRE`**,
**`RING`**, **`ALL`**; **`Around`** `{set, hot}`; **`squeeze`**: a
window's 3x3 into nine bits. **`cell(at, bit)`**, **`bit_of(at,
cell)`**: a bit and its cell. **`pick(random, choices)`**,
**`prefer(random, wanted, open)`**: one drawn;
**`set_bit_of_rank(bits, rank)`**: the set bit with so many under it,
the crate's. **`layer(turn, type, at)`**: the 3x3 cells of a layer
about a cell, one window squeezed. **`occupied(turn, at)`**: those
entities stand on. **`free_beside(turn, at, open)`**: one of `open`
none stands on, drawn.

## `area.rs`

**`Area`** `{set, hot}`, a row a `u16`, `AREA_SIDE` (16) a side, its
centre at `AREA_CENTRE`; **`Area::count`**; `FARTHEST_SCALE` (6), the
coarsest tiles asked of. **`layer(turn, type, centre)`**,
**`layers(turn, types, centre)`**: the area of one layer, or of several
at once, four windows each. **`occupied(turn, centre)`**: the entities
on its cells. **`of_tiles(turn, type, centre, scale)`**: the tiles of
`scale` around `centre`, set where the type holds at any cell.

## `mask.rs`

**`Mask`**, a square of cells a bit each, its side one of **`SIDES`**
(4 to 1,024): **`empty(side)`**, **`full(side)`**, **`disc(side)`**;
**`side`**, **`row(y)`**, **`get(x, y)`**, **`set(x, y, to)`**,
**`clear`**, **`count`**, **`is_empty`**; **`and`**, **`or`**,
**`and_not`** with another of its side (**`with`**); **`cells()`** the
set ones, **`pick(random)`** one drawn. **`about(centre, side)`**: the
top left cell of the square about a cell; **`cell(origin, x, y)`**: a
cell of it. `WORD` (64): bits in a word of a row; `row_words`: words in
a row. **`layer(turn, type, origin, set, hot)`**: the square of a layer
into two masks, a window a time; **`layer_under(turn, type, origin,
under, set, hot)`**: the cells `under` has alone, the windows it has
none in not read (**`read_where`**). `WINDOW` (8): cells along a
window's side, the most a turn reads at once. **`set_under(turn, type,
origin, mask)`**, **`clear_under`**: writes queued for every cell of
the mask, as rectangles (**`write_under`**, **`next_cell`**) -- how
many. `RECT` (255): the most cells along a rectangle's side.

## `walking.rs`

What an entity that walks asks, of a turn: the simulation's cells, the
terrain's walls and `pathfinding` met here. **`around_unwalled(turn,
at)`**: the 3x3 cells about `at` no wall is before, nine bits.
**`area_walls(turn, centre)`**: the area's walls, for paths.
**`step_towards(turn, at, goals, passable)`**, **`step_to(turn, at, to,
passable)`**: the cell to step to for the nearest goal, or for one
cell, round the entities in the way. **`seek(turn, at, type)`**: the
step to the nearest cell the type holds at, the area first, then tiles
by scale, `FARTHEST_SCALE` first and the finest that reach after -- a
**`SoughtStep`** `{to, scale}`. Its own: `HERE`, where an entity stands
in its area; **`of_the_area(at, cell)`**, a cell of the area as the
world's; **`unoccupied(turn, at, rows)`**, rows of the area less the
cells entities stand on.

## The root (`lib.rs`)

**`layers`**: `GRASS`, `TREE`, `TREE_STAGE` (and `OLDEST_TREE_STAGE`),
`WET`, `WALL_EAST`, `WALL_SOUTH`: the layers a world's cells have, from
the type registry (`../../type_registry/`). **`entity_types`**:
`SHEEP`, and its attributes `HUNGRY_AT`, `PREGNANT`, `LAMB`, `ROAMING`
and `BEARING` -- and `Roaming`, the layout of `ROAMING`.

The words the instructions are asked in: `CellIndex`, `CellCartesian`,
`SuperchunkIndex`, `NEIGHBOURS`, `SUPERCHUNK_SIDE_CELLS`, `LayerType`,
`Wide`, `Bits4`, `Attribute`, `AttributeBlock`, `AttributeType`,
`Layout`, `EntityType`,
`EntityId`, `Header`, `EntityRef`, `EntityEdit`, `Turn`, `TickReport`,
`Rng`, `Chance`.

**`place_counted(counted, name)`**: the place of a name in a rule's
list of what it counts, worked out as the rule is compiled -- a name
not listed does not compile.
**`RuleCounts`**: what a rule did, up to
`COUNTS_OF_A_RULE` numbers, each at a place the rule names, added with
`+=` -- one shape for every rule, so the server lists them all in one
table.
