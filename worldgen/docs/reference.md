# World generation, function by function

The design is in `worldgen.md`.

## `lib.rs`

`STEP` (1): the most two cells beside one another may differ and be
stepped between. `WALLS`: the walls' layers, `WALL_EAST` and
`WALL_SOUTH`, each with the neighbour it is towards. The layers
themselves -- the walls, `GRASS`, `TREE`, `TREE_STAGE`, `WET` -- are
rows of the type registry (`../../type_registry/docs/type_registry.md`),
not this crate's. A diagonal has no wall
of its own: `pathfinding::Walls::new` and
`instructions::read::walking::around_unwalled` bar it from the two. `ONE` (65,536): a fraction's whole.

**`Shape`** `{ground, ocean, span, sea, highest, clumping, coast, coast_low, narrow, wide,
soft, hard, warp, finer_depth, finer_share, finer_height, finer_fall, weight, raised}`: the
lowest ground and the ocean's height; the vertices' grid and the share
of them that are ocean; the highest land, and the vertices from the
ocean it is reached over; the lines' blends and sigmoidness, least and
most; how far lines are bent; the finer meshes. `Shape::DEFAULT`.
How deep water is over a `WET` cell is the image's
(`SuperchunkImage::depth`). **`height(seed, x, y)`**: a cell's height;
**`height_shaped(shape, seed, x, y)`**: the same in a world shaped
otherwise. **`Terrain::generate_shaped(shape, seed, superchunk)`**: a
superchunk's heights and walls. **`Noise`**: smooth noise asked cell after cell, the four points
about the last cell kept -- `new(seed, index, shift)`, `at(x, y)`;
**`noise(seed, index, shift, x, y)`**: the same for one cell alone.
Private: **`point`**, an octave's number at a point; **`between`**. **`wall(a, b)`**: whether two heights are too far apart.

**`Terrain`** `{heights, walls}`: a superchunk's height map, and for
each way each chunk's cells that keep a wall.
**`Terrain::generate(seed, superchunk)`**;
**`Terrain::from_heights(height_at)`**: from any heights, those a cell
past the edges asked for too; **`height(place)`**, **`walled(way,
place)`**: a cell's, by its place in the superchunk; **`wall_counts()`**.

## `mesh.rs`

`Lands` and `land` are in `mesh/lands.rs`.

**`Lands`**: the land asked for cell after cell, the vertices about the
last cell and its triangle kept -- **`new(shape, seed)`**, **`land(x,
y)`** (the lattice's points about the cell blended, a point no share is
taken from not worked out), **`height(x, y)`**,
**`heights_of_a_square(left, top, side)`** (every cell's height, row by
row, each point of the lattice worked out once), **`line(x, y)`** (how
far inland the cell is, and about how far from the broad mesh's
nearest line). Private to `mesh/lands.rs`: `LATTICE` (2) and
`LATTICE_STEP` (4), the cells from a point of the lattice to the next,
as a power of two and as cells; `Lands::at_the_lattice(x, y)`, the
meshes' land at a point, in 16-bit fractions of a height;
`between_the_lattice(points, across, down)`, a cell's land from the
four points about it, made whole once. **`land`**:
the same for one cell alone. `SIGMOID_ONE`, `FINER_MOST`, `COAST_MOST`.
Private: **`Vertex`**, **`Triangle`**, **`Blended`**, **`Mesh`**
(`new`, `lot`, `ocean`, `vertex`, `triangle`, `locate`, `blended`),
**`raised`**, **`area`**; `towards_the_slope(lot)`: a line's lot drawn
a little nearer nothing, halfway to its square; `inverse(whole)`: what a part of a
triangle's area is multiplied by to be its share of the whole -- a
division done once for the triangle, 0 where the triangle is too broad
for that to be exact enough and the division is done a cell;
`Lands::moved(x, y)`: a cell moved by the broad noise that bends the
lines. Constants: `VERTICES_SALT`, what the vertices are drawn by,
apart from all else; `WARP_INDEX` (30) and `CLUMP_INDEX` (40), the
numbers the bending and the clumping noise are drawn by; `FINEST` (4),
the finest grid's square as a power of two, 16 cells; `SIGMOID_MOST`,
16 times `SIGMOID_ONE`.

## `patches.rs`

How something lies in patches when a superchunk is made. `SAMPLED`
(16,384); a fraction's whole is the crate's `ONE` (65,536). **`Patches`** `{cover, patch, detail,
scatter}`; **`number(seed, x, y)`**: a cell's number -- noise as broad
as a patch, finer noise, and the cell's own lot;
**`threshold(seed)`**: the number under which `cover` of the cells are.
For a superchunk's every cell (crate only): `noises(seed)`, the
patches' noise and the finer, each kept cell after cell;
`weighed(seed, broad, fine, x, y)`, the number before it is divided;
`weights()`, what it is divided by.

## `generated_superchunk.rs`

**`generate_superchunk(generation, seed, superchunk)`**: its cells
(`chunk_storage::SuperchunkCells`) -- terrain, the ocean where it is
under the ocean's level, and on the rest grass and trees with their
stages, as `Generation::growth` says of each cell. No image: that is
storage's to make.

## `generation.rs`

`TREES_SALT`: what keeps the trees' numbers apart from the grass's.
**`Generation`** `{shape, grass, trees}`; `Generation::DEFAULT`;
**`Generation::plain(grass_cover)`**: a plain -- flat dry land, no
ocean, no walls, no trees, grass scattered on `grass_cover` of `ONE` of
its cells;
**`numbers()`** and **`of_numbers(numbers)`**: its numbers by name, as
a world's file keeps them (`numbers!`). **`from_tuning(tuned)`**: as
the sliders have it (**`shape_from_tuning`**). **`growth(seed)`**: a
**`Growth`**, the thresholds found once and the noises kept;
**`Growth::at(x, y)`** (it keeps the noise about the cell, so each
thread has a `Growth` of its own): a
**`Grown`** `{grass, tree}` -- the tree a lot, its stage drawn from it
by whoever counts the stages. **`has_land_about(seed, shape, near)`**:
land three superchunks each way about `near`; **`seed_with_land(from,
shape, near)`**: the first seed from `from` that has.

## `diagnostics/`, `transient_data.rs`

The folders every crate has (`../../docs/style_guide.md`, "One shape for
every crate"). `diagnostics/mod.rs` gathers nothing yet.
`transient_data::TRANSIENT_DATA` names the crate's `transient_data/`
folder, where its runs would leave what they make.
