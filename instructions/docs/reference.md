# Instructions: reference

What a rule is made of: small pieces of behaviour -- queries of the
simulation and changes queued to it alike -- each a free function over
a superchunk's `Turn`. The design: `instructions.md`. `lib.rs` hands on
**`Turn`**, **`Simulation`** and **`TickReport`**, the simulation's.

## `cells.rs`

**`each_sampled(turn, type, probability, samples, each)`**: `each` run
on every cell sampled, with its counts. **`hot(turn, type, cell)`**,
**`holds`**, **`lacks`**: the cell read; **`set`**, **`clear`**: a write
queued. **`value(turn, plane, cell)`**, **`set_value`**: a wide plane's
number. **`square(turn, type, cell, side)`**: up to 8x8 cells about a
cell as a `Window`, with their top left cell.

## `entities.rs`

**`each_woken(turn, layers, state, each)`**: `each` run on every entity
waking. **`spawn(turn, kind, at, wake, attributes)`**: a new entity, its
ID drawn and returned. **`sleep(turn, entity, wake)`**: a move to where
it stands. **`commit(turn, edit, to, wake)`**: an `EntityEdit`'s entity
moved if no attribute changed, else put whole. **`remove(turn,
entity)`**.

## `around.rs`

The 3x3 cells about a cell as nine bits: **`CENTRE`**, **`RING`**,
**`ALL`**; **`Around`** `{set, hot}`. **`read(turn, type, at)`**: one
window, squeezed (**`squeeze`**). **`occupied(turn, at)`**: those
entities stand on. **`free_beside(turn, at, open)`**: one of `open` none
stands on, drawn. **`cell(at, bit)`**, **`bit_of(at, cell)`**: a bit and
its cell. **`pick(random, choices)`**, **`prefer(random, wanted,
open)`**: one drawn.

## `area.rs`

**`Area`** `{set, hot}`, a row a `u16`, `AREA_SIDE` (16) a side, its
centre at `AREA_CENTRE`; **`Area::count`**. **`read(turn, type,
centre)`**, **`read_each(turn, types, centre)`**: four windows.
**`occupied(turn, centre)`**: the entities on its cells.
**`of_tiles(turn, type, centre, scale)`**: the tiles of `scale` around
`centre`, set where the type holds at any cell; `FARTHEST_SCALE` (6)
the coarsest.

## `mask.rs`

**`Mask`**: a square of cells a bit each, its side one of **`SIDES`**
(4 to 1,024): **`empty(side)`**, **`full(side)`**, **`disc(side)`**;
**`side`**, **`row(y)`**, **`get(x, y)`**, **`set(x, y, to)`**,
**`clear`**, **`count`**, **`is_empty`**; **`and`**, **`or`**,
**`and_not`** with another of its side; **`cells()`** the set ones,
**`pick(random)`** one drawn. **`about(centre, side)`**: the top left
cell of the square about a cell; **`cell(origin, x, y)`**: a cell of
it. **`read(turn, type, origin, set, hot)`**: the square of a layer
into two masks, a window a time; **`read_under(turn, type, origin,
under, set, hot)`**: the cells `under` has alone, the windows it has
none in not read. **`set(turn, type, origin, mask)`**, **`clear`**:
writes queued for every cell of the mask, as rectangles (**`write`**,
**`next`**) -- how many.

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
**`SoughtStep`** `{to, scale}`.
