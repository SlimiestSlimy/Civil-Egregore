# Actions: reference

What a rule is made of: small pieces of behaviour -- queries of the
simulation and changes queued to it alike -- each a free function over
a superchunk's `Turn`. The design: `actions.md`. `lib.rs` hands on
**`Turn`** and **`TickReport`**, the simulation's.

A module a subject: its shape, what reads it and what queues a change
to it together.

## `cells.rs`

**`held_in_tile(turn, type, cell)`**: how many cells of the tile 8x8 the cell is in the type holds at. **`move_one(turn, plane, from, seen_from, to, seen_to, counted)`**: one taken from the number at a cell and added to another's, both under both compares -- nothing made or lost, whatever border is between them. **`change_both(turn, plane, first, second, counted)`**: what it is made of -- the numbers at two cells changed together, each `(cell, seen, becomes)`, the one only if the other. **`hot(turn, type, cell)`**,
**`holds`**, **`lacks`**: the cell read. **`value(turn, plane,
cell)`**: a wide plane's number. **`square(turn, type, cell, side)`**:
up to 8x8 cells about a cell as a `Window`, with their top left cell;
**`corner_about(centre, side)`**: that cell, of a square of any side.
**`set(turn, type, cell)`**, **`clear`**: a compare-and-write queued,
the cell seen clear, or set. **`set_value(turn, plane, cell, seen,
value)`**: a wide plane's number put where `seen` was read.
**`set_counted(turn, type, cell, counted)`**, **`clear_counted`**,
**`set_value_counted`**: the same, one added to the rule's count
`counted` if applied (`actions.md`, "Compare-and-write"). All are
**`seen_becoming(turn, type, cell, seen, value, counted)`**: a compare
of the cell as seen, the write and the compare's end, queued at once
(`compares_and_writes::compare::with_writes`).

## `compare.rs`

Handed on from `../../compares_and_writes/` (its `reference.md`):
`Compare`, `holds`, `lacks`, `value`, `attribute`, `from_here`, `end`
-- an instruction each. Here, what is made of several: **`write(turn, compare, type,
cell, seen, value, counted)`**: a cell written if the compare holds
and the cell is still as seen.
**`count(turn, compare, counted)`**: a count, if it holds.

## `this_tick.rs`

**`now(turn)`**: the tick running. **`random(turn)`**: the turn's
superchunk's random numbers. **`superchunk(turn)`**: whose turn it is.
What a rule asks of its turn that is neither a cell nor an entity:
`this_tick` is the module (`actions.md`, "Rules ask actions,
and nothing else").

## `entities.rs`

**`each_woken(turn, layers, state, each)`**: `each` run on every entity
waking. **`on(turn, at)`**: the entity that stood on a cell as the
tick found it -- the cell its name for the tick, there being no ID.
**`spawn(turn, kind, at, wake, attributes)`**: a new entity. **`make(turn, kind, at, wake, attributes)`**
and **`made(turn)`**: the same, what is queued between them happening only if the entity is made. **`sleep(turn, entity, wake)`**: a move to
where it stands. **`step(turn, entity, to, wake)`**: under a compare
of the entity being there still, a move to a cell, or none to its
own, and its wake -- the same over a border. **`remove(turn, entity)`**, under a compare
of its being there still.

`entities/attribute_edits.rs` (`attribute_edits`), handed on by
`entities`: **`set_attribute_of(turn, other, attribute, seen,
value)`**, **`unset_attribute_of(turn, other, attribute, seen)`**: an
attribute of any entity in reach written by its layout, if still as
seen -- each member that differs, its bytes and no more, or the
attribute whole if it had none; **`set_attribute_blocks(turn, entity,
seen, value)`**: the same of blocks with no layout at hand, each word
that differs (`members_differing`), the attribute whole, or taken;
**`set_attribute_member(turn, entity, kind, member, seen, value)`**:
one member, held against that member alone -- two writing two members
of one attribute both do; **`update(turn, before, after,
attributes)`**: each attribute that differs written
(`set_attributes_changed`), then the step. **`EntitiesBetweenTicks`**: the world's entities off any
turn, lent by whoever runs the world (**`from_entities`**; **`where_free(free)`**,
putting only on cells the world's collision plane does not hold;
**`cells_put()`**, the cells put on, for the runner to set there). **`now()`**,
**`put(header, attributes)`**: queued, in the world once the runner
places them.

## `around.rs`

The cells about a cell, a set of them a word of bits in Morton order:
**`Cells`** (`&`, `|`, **`without`**, **`is_empty`**, **`count`**);
**`Around`** `{set, hot}`. `WAYS` (9): the ways from a
cell to the nine about it, in Morton order of where they lead --
`NORTH_WEST`, `NORTH`, `WEST`, **`HERE`** (3), `NORTH_EAST`, `EAST`,
`SOUTH_WEST`, `SOUTH`, `SOUTH_EAST`. **`about(at)`**: an **`About`**
(`tiles_about.rs`), of which **`centre()`**, **`ring()`**,
**`nine()`**, **`towards(way)`** are sets. **`cell(at, way)`**,
**`way_of(at, cell)`**: a way and its cell. **`pick(random, about,
choices)`**, **`prefer(random, about, wanted, open)`**: the way to one
drawn; **`set_bit_of_rank(bits, rank)`**: the set bit with so many
under it, the crate's. **`read_layer(turn, type, about)`**: a layer's cells
about a cell, the small tiles they lie in read as they lie.

## `tiles_about.rs`

The crate's own, but for the name of what is about a cell. The four
small tiles, two by two, a cell and its neighbours lie in, as one word
in Morton order over their 8x8 cells (`SQUARE_SIDE`, `SQUARE_CELLS`;
**`bit_at(x, y)`** a cell's bit, **`across_and_down(bit)`** back;
`TILE`, `X_BITS`, `Y_BITS`: the parts of a cell's index).
**`About`**: the first tile's first cell, and the cell's bit;
**`about_cell(at)`** takes them. **`centre_word()`**, **`nine_word()`**,
**`ring_word()`**: the cell, it and its neighbours in the world, the
neighbours alone, as bits of the word -- from `NINE_OF`, made as the
program is compiled; **`way_word(way)`**: the one cell a way off, by
`ABOUT`, each of the nine's (`NINE`) bit for each place the cell can
have; **`ways_of(word)`**: a set of them as ways, a bit a way; `WAY_OFFSETS`: where each way leads, across and down, in Morton order; `WAY_AT`: the way to each of the nine, row by row. **`read_layer(turn, type, about,
under)`**, **`read_layers(turn, types, about, under)`**: a layer in the
tiles, the cells of `under` alone, a `SmallTiles` (`actions.md`,
"The small tiles about a cell").

## `area.rs`

**`Area`** `{set, hot}`, a row a `u16`, `AREA_SIDE` (16) a side, its
centre at `AREA_CENTRE`; **`Area::count`**; `FARTHEST_SCALE` (6), the
coarsest tiles asked of. **`read_layer(turn, type, centre)`**,
**`read_layers(turn, types, centre)`**: the area of one layer, or of several
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
**`cell(first, x, y)`**: a cell of it. **`read_layer(turn, type, at, set,
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

**`types`**: the type registry (`../../type_registry/`), whole: the
layers and planes of a world's cells (`GRASS`, `TREE_STAGE`, `CLOUD`,
`CLIMATE`...), its types of entity and their attributes (`SHEEP`,
`SHEEP_STATE`, `SheepState`), what the climate's number is made of
(`Climate`, `Wind`). Nothing is listed: a type registered is one a
rule may name.
**`simulation_time`**: `SimulationTime` and `TICKS_A_SIMULATION_SECOND`,
what a rule says its times and its mean times in
(`../../simulation/docs/simulation.md`, "Simulation time").

The words the actions are asked in: `CellIndex`, `CellCartesian`,
`SuperchunkIndex`, `NEIGHBOURS`, `SUPERCHUNK_SIDE_CELLS`, `LayerType`,
`Wide`, `Bits4`, `Attribute`, `AttributeBlock`, `AttributeType`,
`Member`, `WholeAttribute`,
`Layout`, `EntityType`,
`Header`, `EntityRef`, `Turn`, `TickReport`,
`Rng`, `Chance`.

**`place_counted(counted, name)`**: the place of a name in a rule's
list of what it counts, worked out as the rule is compiled -- a name
not listed does not compile.
**`RuleCounts`**: what a rule did, up to
`COUNTS_OF_A_RULE` numbers, each at a place the rule names, added with
`+=` -- one shape for every rule, so the server lists them all in one
table.
