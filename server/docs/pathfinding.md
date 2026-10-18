# Pathfinding

How an entity finds its way: over an **area** of 16x16 cells about it,
kept as masks, a row a `u16` -- all this crate knows of the world. What
the cells are, which may be walked on and where the walker wants to go
are for whoever calls it: an entity's turn reads the area of a layer
about a cell (`actions::area::read_layer`), and a rule hands
here the masks it made of it.

## A step a tick

A walker takes one pathfinding step each time it ticks, and keeps no
route. A route kept goes stale -- the grass it led to is eaten, a wall
is built across it -- and has to be held somewhere and checked; a step
asked for afresh is always right about the world as the tick found it,
and costs little enough to ask every time.

## Waves

What a step costs is waves. A search's whole memory is the cells
reached so far, 16 words, 32 bytes, beside the 32 of the cells that may
be walked on: one line of cache. A wave goes out from the goals, the
other way to the walker: where walls are one way's it grows by the
cells that may step onto it ("Walls"). A wave moves every reached cell's
front one cell at once: each row or-ed with the rows above and below
it, spread a column each way, and kept where it may be walked -- a few
shifts and ors a row, no queue, no cell looked at alone.

Waves spread from every goal at once (`Wave`), so the nearest goal over
what may be walked on is found without choosing one first: the walker
steps into the first wave to come beside it (`step_towards`), and the
waves that took are the steps left to walk. Of the neighbours a wave
reaches together the walker takes the `pick`-th, round and round,
`pick` a random number, so walkers do not all lean one way. The cell
the walker stands on need not be passable, and is no goal. A step is to any of the
eight neighbours, each costing one.

## A*

`a_star` finds the shortest path between two cells, for a walker with
one place to go: the cells to look at next kept in a heap in an array,
the one on the shortest path guessed first, the guess being the steps
left with nothing in the way: the greater of the distances across and
down (`steps_apart`), a step to any of the eight neighbours costing
one. It runs backwards, from the end: each cell reached remembers the
cell it was reached from, one step nearer the end, so the start is
reached remembering its first step, and no path is walked back along;
a wall is asked of each step the way the walker would take it.
Neither end need be passable -- a walker stands on one and wants the
other. A cell is queued again each time a shorter way to it is found,
from one neighbour after another at most, which is what the queue is
sized by (`QUEUE_ROOM`: the area's cells, `CELLS`, times the eight steps). It allocates nothing either, but looks at cells one by
one: for the nearest of many goals, waves are the cheaper.

## In Morton order

`morton.rs` is the same search as the waves, over cells kept as the
bitmaps keep them: `Tiles`, the 32x32 cells of four aligned tiles of
16 two by two, 16 words (`TILES_WORDS`), each an 8x8 word tile, a
cell's bit the index of its two coordinates interleaved
(`bit_at`). Read off the bitplanes as they lie, nothing is turned
into rows first.

A step east is not a shift of a row. A cell's x is every other bit of
its index, so a step up it carries through those bits: the cells of a
word tile fall in four kinds by how far the carry goes
(`STEP_UP_KINDS`, `STEP_DOWN_KINDS`), each moved by one shift
(`KIND_SHIFTS`, twice as far for y), and the fourth kind -- on the
tile's far edge -- leaves for the word tile beside
(`WORD_TILE_BESIDE`). So `step_up` and `step_down` move a whole set a
cell each way in three masked shifts a word and a carry, and a wave
spreads by eight of them (`MortonWalls::spread_through`).

Its walls (`MortonWalls`) bar a step both ways, east and south of
cells, as the walls did before they were one way's: made from the
four one-way layers it would need the four ways of `Walls::one_way`.
It was written, and tested against the waves over rows on areas
drawn (the same goal as near, by a first step as short), and parked
on a branch of its own; it came into the main line when the branches
were made one, and nothing calls it.

## Still to come

Areas larger than 16x16 -- waves over the words of several, or a
coarser area of tiles first; steps that cost more than one (mud, a
slope); and walkers larger than a cell.

## Layout

| folder | what is in it |
|---|---|
| `src/lib.rs` | areas, waves, a walker's step, the nearest cell |
| `src/walls.rs` | the walls between cells, and cells spread a step through them |
| `src/a_star.rs` | A*: the shortest path between two cells |
| `src/morton.rs` | waves in Morton order, over tiles as they lie in their bitmaps: written and tested, used by nothing yet |
| `src/diagnostics/`, `src/transient_data.rs` | the folders every crate has; nothing gathered or kept yet |
| `tests/` | each judged against a search of every cell |
| `docs/` | this, and the reference, function by function |

## Walls

A cell that may not be walked on is a bit of `passable`. A **wall** is
not a cell: it is before the step from one cell to the one beside it,
whatever the cells are -- the terrain's cliffs (`../worldgen/`). It is
one way's: the step back has a wall of its own or none, as a drop is
hopped down and not climbed. So a wall is kept by the cell the step
leaves, a mask each of the four ways. A diagonal step has no wall of
its own: it is open only when both ways round it -- across then down,
down then across -- are. `Walls::one_way` takes the four masks and
works out once, a mask a diagonal, the diagonal steps they bar
(`Walls::bars_step`); `Walls::new` takes walls barring both ways, east
and south of cells, and makes the four of them.

A wave spreads each of the eight ways apart, each masked by its walls
-- eight shifts a row where there were three. Walls of one way give
spreading a direction: the cells a step from a set comes to (`spread_forwards`,
what is beside a walker) are not the cells a step from which comes to
it (`spread_back`, what a wave going out from the goals grows by --
the walker's steps run the other way to the wave's). A* searches from
its end back to its start, and asks `bars_step` of each step as the
walker would take it. With no walls (`Walls::default`) both are what
they were.
