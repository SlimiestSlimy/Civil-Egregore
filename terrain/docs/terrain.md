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

## Plains, hills and water

The ground everywhere is a **base**: one octave 2,048 cells across,
worth 40 of a height, so flat that a step comes every few dozen cells
and a wall never. **Hills** -- the four octaves -- stand on it only
where a mask, as broad, is past a threshold, and rise from nothing at
the threshold to their whole height a quarter of the mask further on:
foothills, not a cliff about every plain. With the threshold at a half,
about half the world is plains.

**Water** is a depth a cell: how far it stands over the ground, 0 none,
eight bits over eight bitplanes (`WATER`). A world is generated with
still water at one level: every cell lower than it is a lake, as deep
as it is lower (`world::Generation::water_level`). Nothing grows or
spreads under water. Water does not move yet.

## Heights of 16 bits

A height is 16 bits, 0 to 65,535. The lowest ground is at
`Shape::ground`; on it the land rises (`rise`) by up to `Shape::rise`
heights, as noise `2^rise_span` cells between points -- many
superchunks -- with a quarter as much again a quarter as broad: too
gentle for a wall, a step every ten cells or so at the steepest. The
base, the plains and the hills stand on that, 255 heights at most.

The height map keeps a floor a chunk and a byte a cell over it; only a
chunk whose heights span more than 255 -- a tall chunk -- keeps a map of
16 bits a cell, after the bytes (see chunk storage).

To come: plains level at heights of their own, mesas, ramps, cliffs --
the generator as layers of noise, each with a curve and a mask.
