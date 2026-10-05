# World generation, function by function

The design is in `worldgen.md`.

## `lib.rs`

`STEP` (1): the most two cells beside one another may differ and be
stepped between. `WALL_EAST`, `WALL_SOUTH` (layer types 8 and 9);
`WALLS`: each with the neighbour it is towards. A diagonal has no wall
of its own: `pathfinding::Walls::new` and
`entity_rules::walking::around_unwalled` bar it from the two. `ONE` (65,536): a fraction's whole.

**`Shape`** `{ground, ocean, span, sea, highest, clumping, coast, coast_low, narrow, wide,
soft, hard, warp, finer_depth, finer_share, finer_height, finer_fall, weight, raised}`: the
lowest ground and the ocean's height; the vertices' grid and the share
of them that are ocean; the highest land, and the vertices from the
ocean it is reached over; the lines' blends and sigmoidness, least and
most; how far lines are bent; the finer meshes. `Shape::DEFAULT`.
`WATER` (layer types 24 to 31): a cell's water, its depth over eight
bitplanes. **`height(seed, x, y)`**: a cell's height;
**`height_shaped(shape, seed, x, y)`**: the same in a world shaped
otherwise. **`Terrain::generate_shaped(shape, seed, superchunk)`**: a
superchunk's heights and walls. **`noise(seed, index, shift, x,
y)`**: smooth noise, one octave of a height. Private: **`point`**, an
octave's number at a point; **`between`**. **`wall(a, b)`**: whether two heights are too far apart.

**`Terrain`** `{heights, walls}`: a superchunk's height map, and for
each way each chunk's cells that keep a wall.
**`Terrain::generate(seed, superchunk)`**;
**`Terrain::from_heights(height_at)`**: from any heights, those a cell
past the edges asked for too; **`height(place)`**, **`walled(way,
place)`**: a cell's, by its place in the superchunk; **`wall_counts()`**.

## `mesh.rs`

**`Lands`**: the land asked for cell after cell, the vertices about the
last cell and its triangle kept -- **`new(shape, seed)`**, **`land(x,
y)`**, **`height(x, y)`**, **`line(x, y)`** (how far inland the cell
is, and about how far from the broad mesh's nearest line). **`land`**:
the same for one cell alone. `SIGMOID_ONE`, `FINER_MOST`, `COAST_MOST`.
Private: **`Vertex`**, **`Triangle`**, **`Blended`**, **`Mesh`**
(`vertex`, `lot`, `triangle`, `locate`, `blended`), **`raised`**,
**`width`**, **`area`**.

## `patches.rs`

How something lies in patches when a superchunk is made. `SAMPLED`
(16,384); a fraction's whole is the crate's `ONE` (65,536). **`Patches`** `{cover, patch, detail,
scatter}`; **`number(seed, x, y)`**: a cell's number -- noise as broad
as a patch, finer noise, and the cell's own lot;
**`threshold(seed)`**: the number under which `cover` of the cells are.
