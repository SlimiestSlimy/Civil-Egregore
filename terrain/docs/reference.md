# Terrain, function by function

The design is in `terrain.md`.

## `lib.rs`

`STEP` (1): the most two cells beside one another may differ and be
stepped between. `WALL_EAST`, `WALL_SOUTH` (layer types 8 and 9);
`WALLS`: each with the neighbour it is towards. A diagonal has no wall
of its own: `pathfinding::Walls::new` and
`Turn::around_unwalled` bar it from the two. `OCTAVES`, `ONE`.

**`Shape`** `{weights, ocean, coast, ground, rise, rise_span}`: the
hill octaves' shares of a height; the ocean's height; how far over it
the hills are whole; the lowest ground; the most the land rises over
it, and how broadly. `Shape::DEFAULT` (168, 22, 61, 4; ocean 800; coast
64; ground 256; rise 1,024; span 14). `WATER` (layer types 24 to 31): a
cell's water, its depth over eight bitplanes. **`rise(shape, seed, x,
y)`**: how far the land has risen over the lowest ground.
**`height(seed, x, y)`**: a cell's height; **`height_shaped(shape,
seed, x, y)`**: the same in a world shaped otherwise, to try a shape
out: the land, and the hills by how far it is over the ocean. **`noise(seed, index, shift, x,
y)`**: smooth noise, one octave of a height. Private: **`point`**, an
octave's number at a point; **`between`**. **`wall(a, b)`**: whether two heights are too far apart.

**`Terrain`** `{heights, walls}`: a superchunk's height map, and for
each way each chunk's cells that keep a wall.
**`Terrain::generate(seed, superchunk)`**;
**`Terrain::from_heights(height_at)`**: from any heights, those a cell
past the edges asked for too; **`height(place)`**, **`walled(way,
place)`**: a cell's, by its place in the superchunk; **`wall_counts()`**.
