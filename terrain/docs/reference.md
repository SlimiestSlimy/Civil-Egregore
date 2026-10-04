# Terrain, function by function

The design is in `terrain.md`.

## `lib.rs`

`STEP` (1): the most two cells beside one another may differ and be
stepped between. `WALL_EAST`, `WALL_SOUTH` (layer types 8 and 9);
`WALLS`: each with the neighbour it is towards. A diagonal has no wall
of its own: `pathfinding::Walls::new` and
`Turn::around_unwalled` bar it from the two. `OCTAVES`, `ONE`.

**`Shape`** `{ground, ocean, span, sea, levels, edge, warp}`: the
lowest ground and the ocean's height; the polygons' grid, the share of
them that are ocean, the most a plain stands over the ocean, the cells
levels are mixed over at a border, how far borders are bent.
`Shape::DEFAULT`. `WATER` (layer types 24 to 31): a cell's water, its
depth over eight bitplanes. **`height(seed, x, y)`**: a cell's height;
**`height_shaped(shape, seed, x, y)`**: the same in a world shaped
otherwise. **`noise(seed, index, shift, x,
y)`**: smooth noise, one octave of a height. Private: **`point`**, an
octave's number at a point; **`between`**. **`wall(a, b)`**: whether two heights are too far apart.

**`Terrain`** `{heights, walls}`: a superchunk's height map, and for
each way each chunk's cells that keep a wall.
**`Terrain::generate(seed, superchunk)`**;
**`Terrain::from_heights(height_at)`**: from any heights, those a cell
past the edges asked for too; **`height(place)`**, **`walled(way,
place)`**: a cell's, by its place in the superchunk; **`wall_counts()`**.

## `polygons.rs`

**`Lands`**: the land asked for cell after cell, the sites about the
last cell kept -- **`new(shape, seed)`**, **`land(x, y)`** (the cell's
polygon's level, mixed near a border with its neighbours'),
**`height(x, y)`**, **`polygon(x, y)`** (the polygon's number, whether
it is land, about how far the cell is from its border). **`land`**,
**`polygon`**: the same for one cell alone. Private: **`Site`**,
**`Lands::moved`**, **`Lands::squared`**.
