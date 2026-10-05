# World generation

Every cell's height, the walls heights make, and how what grows lies
in patches (`src/patches.rs`). The crate was `terrain`; it is named for
all it generates.

## Heights from the seed

`height(seed, x, y)` is settled by the world's seed and the cell's
place in the world, and nothing else: no neighbour, no order of
generation. So a superchunk generated today and its neighbour
generated next year meet with no seam, and a superchunk made again is
the same.

A height is 16 bits, 0 to 65,535: what the mesh's vertices about the
cell come to there (below). All of it is whole
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
(`entity_rules::walking::around_unwalled`) and the walls of the area about a cell
(`area_walls`); the waves and A* of `../pathfinding/` go round them.

How much of the ground is walled is the shape's doing: narrow, hard
blends make cliffs, broad soft ones none. Diagonal walls of their own,
when there were any, were four layers where two do.

Not yet: heights do not change. When they do -- digging -- the walls of
the cells about the change are worked out again.

## Layout

| folder | what is in it |
|---|---|
| `src/lib.rs` | the shape, heights, walls, a superchunk's terrain, the noise |
| `src/mesh.rs` | the land as a mesh: what a height is worked out from |
| `src/patches.rs` | how grass and trees lie when a superchunk is made |
| `tests/` | heights settled by seed and cell, walls where they should be |
| `docs/` | this, and the reference, function by function |

## The land as a mesh

`src/mesh.rs`: vertices that carry heights, joined by lines that carry
how the heights are blended.

A **vertex** is one to each square of a grid `2^span` cells a side,
placed by lot in the square's middle half; the four of neighbouring
squares make a quad, cut by lot along one diagonal or the other into
two triangles. A vertex is ocean (`Shape::sea` of them) -- at the
lowest ground -- or land, at a height of its own.

Whether a vertex is ocean is its own lot mixed with smooth noise some
vertices broad (`Shape::clumping`): land and ocean clump, with no
blocks to show through. The more it counts, the less exactly the
ocean's share is kept.

Land is low by the ocean and higher inland. A land vertex's height is
drawn between just over the ocean and the highest (`Shape::highest`):
past `Shape::coast` vertices from any ocean one, any height as likely
as another; nearer, the higher the less likely (`Shape::coast_low`, the
power the lot is raised to, the most beside the ocean) -- so most
coasts are low, each by a little of its own, and a few are cliffs.

A **line** joins two vertices. It has a **blend** -- the share of its
length, about its middle, the change from one end's height to the
other's is spread over: all of it, and the line is one slope from vertex
to vertex -- and a **sigmoidness** -- 1 an even slope across the blend, more
two levels and a step between -- each by lot between the shape's least
and most. Along a line the height is its ends' blended so. Within a
triangle each vertex's height counts by how near the cell is to it
beside the nearer of the others, shaped by its two lines' blend and
sigmoidness, each counting as the cell is nearer that line's other end:
the ground slopes from vertex to vertex, level about a vertex only as
far as its lines' blends leave it, and at a line two triangles agree.

Broad noise moves a cell before its triangle is looked up, which bends
the lines. **Finer meshes** (`finer_depth`, 10 at most), each with
vertices half as far apart as the one before and none finer than 16
cells, raise or sink the land by less each (`finer_fall`), never by less than
two heights: points spread
again within the triangles of the mesh before, small variations at a
time -- the lowest land a quarter as much as the highest, so
differences compound inland.

The broad mesh has a **weight** of one. The weight that reaches a
vertex of a finer mesh is shared out by lot: each takes a share of its
own -- a byte, from one less `Shape::weight` to one -- moves the land by
its height times that share, and hands the share on to its own
subdivisions, where it is shared out again. So under some vertices the
land is broken up in detail and under others it keeps its parent's
shape: a plain stays mostly a plain, a ridge a ridge.

Every cell's height follows from the seed and the cell alone, in whole
numbers: the same whatever order cells or superchunks are made in. A
cell on a line is in two triangles; it is always given to the first of
them in a fixed order. `Lands` keeps the vertices about the last cell
and its triangle, with what a part of the triangle's area is
multiplied by to be its share -- a division for a triangle, not three
for a cell. Land costs more than ocean, which no finer mesh touches.

To come: ridges and canyons as chains of lines; true subdivision of a
triangle into its own smaller ones; noise for the ground's detail.

**Water** is a depth a cell: how far it stands over the ground, 0 none,
eight bits over eight bitplanes (`WATER`) -- the ocean deeper than 255
is kept as 255. Nothing grows or spreads under water. Water does not
move yet.

The height map keeps a floor a chunk and a byte a cell over it; only a
chunk whose heights span more than 255 -- a tall chunk -- keeps a map of
16 bits a cell, after the bytes (see chunk storage).

### Rounded once

Heights are whole numbers. What the finer meshes add is summed in
16-bit fractions and made whole once: rounded a mesh at a time, each
left its own one-height steps along its own curved lines, and the ten
together showed as long streaks on gentle slopes.
