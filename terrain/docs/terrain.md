# Terrain

Every cell's height, and the walls heights make.

## Heights from the seed

`height(seed, x, y)` is settled by the world's seed and the cell's
place in the world, and nothing else: no neighbour, no order of
generation. So a superchunk generated today and its neighbour
generated next year meet with no seam, and a superchunk made again is
the same.

A height is 16 bits, 0 to 65,535: the level of the cell's polygon,
mixed near a border with its neighbours' (below). All of it is whole
numbers, 16-bit fractions and whole square roots, so a world is the
same on any machine.

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

`src/polygons.rs`. The world is cut into closed shapes that share
borders and never overlap: a polygon is the cells nearer one site than
any other, the sites one to each square of a grid `2^span` cells a
side, placed by lot in the square's middle half. Each polygon is ocean
-- its ground the lowest there is (`Shape::ground`) -- or land, a plain
at a level of its own over the ocean's (`Shape::ocean`, `levels`); the
share that are ocean is `Shape::sea`. Broad noise moves a cell before
its polygon is looked up, which bends the borders (`warp`). Within the
edge's width of a border (`edge`) the levels of the polygons about it
are mixed, each by how little farther its site is than the nearest: a
ramp or a shore, or with a narrow edge a cliff, walled.

A cell looks at the sites of the 25 squares about it, so any cell's
height follows from the seed and the cell alone. `Lands` keeps those
sites from one cell to the next -- drawn once for a square, not once
for a cell -- and takes no root of a site too far to count: a
superchunk's terrain takes about 0.1 s (0.6 s without).

To come: polygons within polygons, plains higher or lower than the one
about them; open lines within a polygon, ridges and valleys; ranges
where two polygons meet; noise for the ground's detail.

The generator before this one -- a land's rise in octaves of noise,
hills on it, a coast -- is gone; `noise` is kept for the borders and
for what lies in patches.

**Water** is a depth a cell: how far it stands over the ground, 0 none,
eight bits over eight bitplanes (`WATER`) -- the ocean deeper than 255
is kept as 255. Nothing grows or spreads under water. Water does not
move yet.

The height map keeps a floor a chunk and a byte a cell over it; only a
chunk whose heights span more than 255 -- a tall chunk -- keeps a map of
16 bits a cell, after the bytes (see chunk storage).

