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
up to 8x8 cells about a cell as a `Window`, with their top left cell;
**`corner_about(centre, side)`**: that cell, of a square of any side.
**`set(turn, type, cell)`**, **`clear`**: a compare-and-write queued,
the cell seen clear, or set. **`set_value(turn, plane, cell, seen,
value)`**: a wide plane's number put where `seen` was read.
**`set_counted(turn, type, cell, counted)`**, **`clear_counted`**,
**`set_value_counted`**: the same, one added to the rule's count
`counted` if applied (`instructions.md`, "Compare-and-write").

## `compare.rs`

**`Compare`**: the simulation's. **`holds(type, cell)`**, **`lacks`**,
**`value(plane, cell, seen)`**: a cell as the rule saw it;
**`attribute(entity, attribute, seen)`**: an entity standing where it
stood, the attribute as seen, or none. **`write(turn, compare, type,
cell, seen, value, counted)`**: a cell written if the compare holds
and the cell is still as seen.
**`count(turn, compare, counted)`**: a count. **`entities_from_here(turn,
compare)`**, **`entities_as_ever(turn)`**: what is queued of entities
between them under the compare.

## `this_tick.rs`

**`now(turn)`**: the tick running. **`random(turn)`**: the turn's
superchunk's random numbers. **`superchunk(turn)`**: whose turn it is.
What a rule asks of its turn that is neither a cell nor an entity:
`this_tick` is the module (`instructions.md`, "Rules ask instructions,
and nothing else").

## `entities.rs`

**`each_woken(turn, layers, state, each)`**: `each` run on every entity
waking. **`stands(turn, id, at)`**: whether an entity stood on a
cell; **`stands_beside(turn, id, centre, among)`**: which of the
neighbours it stood on. **`spawn(turn, kind, at, wake, attributes)`**: a new entity,
its ID drawn and returned. **`sleep(turn, entity, wake)`**: a move to
where it stands. **`commit(turn, edit, to, wake)`**: an `EntityEdit`'s
entity moved, each attribute changed written before, held against
what was seen of it. **`set_attribute_of(turn, other, attribute, seen,
value)`**, **`unset_attribute_of(turn, other, attribute, seen)`**: an
attribute of another entity written, if still as seen. **`remove(turn,
entity)`**. **`EntitiesBetweenTicks`**: the world's entities off any
turn, lent by whoever runs the world (**`of`**; **`where_free(free)`**,
putting only on cells the world's collision plane does not hold;
**`cells_put()`**, the cells put on, for the runner to set there). **`now()`**,
**`put(header, attributes)`**: queued, in the world once the runner
places them.

## `around.rs`

The cells about a cell, a set of them a word of bits in Morton order:
**`Cells`** (`&`, `|`, **`without`**, **`is_empty`**, **`count`**);
**`Around`** `{set, hot}`. `WAYS` (9), **`HERE`** (4): the ways from a
cell to the nine about it, row by row. **`about(at)`**: an **`About`**
(`tiles_about.rs`), of which **`centre()`**, **`ring()`**,
**`nine()`**, **`towards(way)`** are sets. **`cell(at, way)`**,
**`way_of(at, cell)`**: a way and its cell. **`pick(random, about,
choices)`**, **`prefer(random, about, wanted, open)`**: the way to one
drawn; **`set_bit_of_rank(bits, rank)`**: the set bit with so many
under it, the crate's. **`layer(turn, type, about)`**: a layer's cells
about a cell, the small tiles they lie in read as they lie.

## `tiles_about.rs`

The crate's own, but for the name of what is about a cell. The four
small tiles, two by two, a cell and its neighbours lie in, as one word
in Morton order over their 8x8 cells (`SQUARE_SIDE`, `SQUARE_CELLS`;
**`bit_at(x, y)`** a cell's bit, **`across_and_down(bit)`** back;
`TILE`, `X_BITS`, `Y_BITS`: the parts of a cell's index).
**`About`**: the first tile's first cell, and the cell's bit;
**`of(at)`** takes them. **`centre_word()`**, **`nine_word()`**,
**`ring_word()`**: the cell, it and its neighbours in the world, the
neighbours alone, as bits of the word -- from `NINE_OF`, made as the
program is compiled; **`way_word(way)`**: the one cell a way off, by
`ABOUT`, each of the nine's (`NINE`) bit for each place the cell can
have; **`way_of_bit(bit)`**: back. **`layer(turn, type, about,
under)`**, **`layers(turn, types, about, under)`**: a layer in the
tiles, the cells of `under` alone, a `SmallTiles` (`instructions.md`,
"The small tiles about a cell").

## `area.rs`

**`Area`** `{set, hot}`, a row a `u16`, `AREA_SIDE` (16) a side, its
centre at `AREA_CENTRE`; **`Area::count`**; `FARTHEST_SCALE` (6), the
coarsest tiles asked of. **`layer(turn, type, centre)`**,
**`layers(turn, types, centre)`**: the area of one layer, or of several
at once, off the four aligned tiles of 16 it lies in (`TILE_CELLS`: the
cells of one), their words turned into rows; at the world's edge, a
window a quarter (**`layers_at_the_edge`**). **`of_tiles(turn, type, centre, scale)`**: the tiles of
`scale` around `centre`, set where the type holds at any cell.

## `mask.rs`

**`Mask`**, an aligned tile's cells a bit each in Morton order, its
side one of **`SIDES`** (4 to 1,024): **`empty(side)`**,
**`full(side)`**, **`disc(side)`**; **`side`**, **`words`**, **`get(x,
y)`**, **`set(x, y, to)`**, **`clear`**, **`count`**, **`is_empty`**;
**`and`**, **`or`**, **`and_not`** with another of its side
(**`with`**); **`cells()`** the set ones (**`bits`**, their places),
**`pick(random)`** one drawn. `WORD` (64): bits in a word;
`SMALL_TILE`: the bits of the one mask less than a word.
**`tile(at, side)`**: the first cell of the aligned tile a cell is in;
**`cell(first, x, y)`**: a cell of it. **`layer(turn, type, at, set,
hot)`**: the tile of a layer into two masks, its words copied
(**`runs`**: a tile's words as they lie, a chunk a run);
**`layer_under(turn, type, at, under, set, hot)`**: the cells `under`
has alone (**`read_where`**). **`any_under(turn, type, at, mask)`**:
whether the layer holds at any cell of the mask, the words ANDed as
they lie. **`set_under(turn, type, at, mask)`**, **`clear_under`**: a
compare-and-write queued for every hot cell of the mask seen otherwise
(**`write_under`**) -- how many.

## `walking.rs`

What an entity that walks asks, of a turn: the simulation's cells, the
terrain's walls and `pathfinding` met here. **`around_unwalled(turn,
about)`**: the cells about a cell no wall is before, a set;
**`around_steppable(turn, about)`**: its neighbours the collision plane
does not hold and no wall bars, the plane asked first and no wall read
where it holds the whole ring.
**`area_walls(turn, centre)`**: the area's walls, for paths.
**`step_towards(turn, at, goals, passable)`**, **`step_to(turn, at, to,
passable)`**: the cell to step to for the nearest goal, or for one
cell, round the entities in the way. **`seek(turn, at, type)`**: the
step to the nearest cell the type holds at, the area first, then tiles
by scale, `FARTHEST_SCALE` first and the finest that reach after -- a
**`SoughtStep`** `{to, scale}`. Its own: `HERE`, where an entity stands
in its area; **`of_the_area(at, cell)`**, a cell of the area as the
world's; **`in_the_way(turn, at)`**, the cells of the area the
collision plane holds and its walls, read at once; **`less(rows,
held)`**, rows without them.

## The root (`lib.rs`)

**`layers`**: `GRASS`, `TREE`, `TREE_STAGE` (and `OLDEST_TREE_STAGE`),
`WET`, `WALL_EAST`, `WALL_SOUTH`, `COLLISION`: the layers a world's cells have, from
the type registry (`../../type_registry/`). **`entity_types`**:
`SHEEP`, and its one attribute `SHEEP_STATE` -- and `SheepState`, its layout.
**`simulation_time`**: `SimulationTime` and `TICKS_A_SIMULATION_SECOND`,
what a rule says its times and its mean times in
(`../../simulation/docs/simulation.md`, "Simulation time").

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
