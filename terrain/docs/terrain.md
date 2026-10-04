# Terrain

Every cell's height, and the walls heights make.

## Heights from the seed

`height(seed, x, y)` is settled by the world's seed and the cell's
place in the world, and nothing else: no neighbour, no order of
generation. So a superchunk generated today and its neighbour
generated next year meet with no seam, and a superchunk made again is
the same.

A height is a `u8`, the sum of four octaves of value noise: each octave
has a point every 512, 128, 32 or 8 cells, a number at each point from
a hash of the seed, the octave and the point, and a cell's part is
eased between the four points about it (smoothstep, so there is no
crease at the points). The octaves weigh 150, 75, 24 and 6 of the 255:
broad hills, and rougher ground on them. All of it is whole numbers,
16-bit fractions, so a world is the same on any machine.

## Walls

Two cells beside one another, across or down, more than one apart in
height (`STEP`) cannot be stepped between: there is a wall between
them. A diagonal step has no wall of its own: it is open only when both
ways round it -- across then down, and down then across -- are. So a
diagonal between cells two apart in height, each way round a step of
one, is open; one with a cliff on either side of it is not. Where the
ground is steep the walls line up into cliffs.

A wall is between two cells, not on one, so it is kept by one of the
two: the upper, or of two on a row the left. Two layers of bits hold
them -- `WALL_EAST` and `WALL_SOUTH`: the cells with a wall that way --
ordinary layers,
stored, made hot and read as any other. So a rule reads the walls about
a cell as masks, in the same windows it reads grass by, and never a
height: heights stay cold in the superchunk's image.

A superchunk's walls at its edges are with cells of its neighbours:
found from the heights beyond the edge, which are the world's, not the
neighbour's to give.

Who reads them: the turn gives the neighbours no wall is before
(`Turn::around_unwalled`) and the walls of the area about a cell
(`area_walls`); the waves and A* of `../pathfinding/` go round them.

Measured, three seeds: 0.7% of the steps across or down walled; 40 ms
a superchunk to generate heights and walls, on one thread. Diagonal
walls of their own, when there were any, were four layers where two
do, and walled 4% of diagonals.

Not yet: heights do not change. When they do -- digging -- the walls of
the cells about the change are worked out again.

## Layout

| folder | what is in it |
|---|---|
| `src/lib.rs` | heights, walls, a superchunk's terrain |
| `tests/` | heights settled by seed and cell, walls where they should be |
| `docs/` | this, and the reference, function by function |

## The land as polygons

`src/polygons.rs`, tried in the renderer's lab and not yet what worlds
are made with (`Shape::polygons`, `Polygons::NONE` by default). The
world is cut into closed shapes that share borders and never overlap:
a polygon is the cells nearer one site than any other, the sites one to
each square of a grid `2^span` cells a side, placed by lot in the
square's middle half. Each polygon is ocean -- its ground the lowest
there is -- or land, a plain at a level of its own over the ocean's.
Broad noise moves a cell before its polygon is looked up, which bends
the borders. Within the edge's width of a border the levels of the
polygons about it are mixed, each by how little farther its site is
than the nearest: a ramp or a shore, or with a narrow edge a cliff. The
hills are added to that as to the rise.

A cell looks at the sites of the 25 squares about it, so any cell's
land follows from the seed and the cell alone; a superchunk's terrain
takes about 0.6 s so (0.13 s with the rise) -- the sites near a
superchunk are yet to be found once for all its cells.

To come: polygons within polygons, plains higher or lower than the one
about them; open lines within a polygon, ridges and valleys; ranges
where two polygons meet.

## Land, ocean and hills

A height is 16 bits, 0 to 65,535. The lowest ground is at
`Shape::ground`; on it the **land rises** (`rise`) by up to
`Shape::rise` heights: five octaves of noise, the broadest
`2^rise_span` cells between points -- many superchunks -- each next half
as broad and of a smaller share (`Shape::rise_shares`), so that the
finer shape the shores and add little slope.

The **ocean** stands at one height all over the world (`Shape::ocean`):
the land under it is the ocean's floor, the land over it islands, dozens
to hundreds of superchunks each. **Hills** stand on the islands: fourteen
octaves, every power of two from 65,536 cells to 8, so that no one
octave's grid shows; their shares are heights, and come to the highest
a hill stands (the four broadest none, as worlds are by default). They grow from nothing to
their whole height over `Shape::coast` heights of land, from a line
that is the shore on average but wanders above and below it
(`Shape::shore`, `shore_span`): hills here stand out of the ocean, and
there begin well inland, and no level band rings an island.

With a coast of 0 the hills are whole everywhere, the ocean's floor
too: simply added to the land's rise.

A height is the lowest ground, the land's rise and the hills, added:
nothing is taken away, so none is under the lowest ground. What is
under the ocean's level is squeezed so that the lowest ground lies
`Shape::depth` under it.

The ocean is over a cell under its level only where the land and a
share of the hills' height (`Shape::hollows`) are under it too
(`under_ocean`): the rest are dry hollows under the ocean's level.

**Water** is a depth a cell: how far it stands over the ground, 0 none,
eight bits over eight bitplanes (`WATER`) -- the ocean deeper than 255
is kept as 255. Nothing grows or spreads under water. Water does not
move yet. There are no lakes: no water over the ocean's level.

The height map keeps a floor a chunk and a byte a cell over it; only a
chunk whose heights span more than 255 -- a tall chunk -- keeps a map of
16 bits a cell, after the bytes (see chunk storage).

To come: plains level at heights of their own, lakes, mesas, ramps, cliffs --
the generator as layers of noise, each with a curve and a mask.
