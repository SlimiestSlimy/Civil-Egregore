# World generation, function by function

The design is in `worldgen.md`.

## `lib.rs`

`STEP` (1): the most a cell may be higher than the one beside it and
be stepped up to; `DROP` (4): the most it may be lower and be stepped
down to. `WALLS`: the walls' layers, `WALL_EAST`, `WALL_SOUTH`,
`WALL_WEST` and `WALL_NORTH`, each with the neighbour it is towards. The layers
themselves -- the walls, `GRASS`, `TREE`, `TREE_STAGE`, `WET` -- are
rows of the type registry (`../../type_registry/docs/type_registry.md`),
not this crate's. A diagonal has no wall
of its own: `pathfinding::Walls::one_way` and
`actions::walking::around_unwalled` bar it from the four. `FRACTION_ONE` (65,536): a fraction's whole.

**`LandShape`** `{ocean_floor, ocean, vertex_spacing_power, ocean_share, highest_land, land_clumping, coast_breadth, coast_low, narrowest_blend, widest_blend,
least_sigmoidness, most_sigmoidness, line_bending, finer_depth, finer_share, finer_height, finer_fall, weight, raised_share}`: the
lowest ground and the ocean's height; the vertices' grid and the share
of them that are ocean; the highest land, and the vertices from the
ocean it is reached over; the lines' blends and sigmoidness, least and
most; how far lines are bent; the finer meshes. `LandShape::DEFAULT`.
How deep water is over a `WET` cell is the image's
(`SuperchunkImage::water_depth`). **`height(seed, x, y)`**: a cell's height;
**`height_shaped(land_shape, seed, x, y)`**: the same in a world shaped
otherwise. **`Terrain::generate_shaped(land_shape, seed, superchunk)`**: a
superchunk's heights and walls. **`SmoothNoise`**: smooth noise asked cell after cell, the four points
about the last cell kept -- `new(seed, noise_index, spacing_shift)`, `noise_at(x, y)`;
**`noise_at_cell_cartesian(seed, noise_index, spacing_shift, x, y)`**: the same for one cell alone.
Private: **`point_number_cartesian`**, an octave's number at a point; **`value_between`**. **`wall(from_height, to_height)`**: whether no step goes from a cell so high to one beside it so high -- more than a step up or a drop down; the step back is asked the other way round.

**`Terrain`** `{heights, walls}`: a superchunk's height map, and for
each way each chunk's cells that keep a wall.
**`Terrain::generate_terrain(seed, superchunk)`**;
**`Terrain::from_heights(height_at)`**: from any heights, those a cell
past the edges asked for too; **`height(place)`**, **`keeps_wall(way,
place)`**: a cell's, by its place in the superchunk; **`wall_counts()`**.

## `mesh.rs`

`WorldLand` and `land_of_one_cell_cartesian` are in `mesh/lands.rs`.

**`WorldLand`**: the land asked for cell after cell, the vertices about the
last cell and its triangle kept -- **`new(land_shape, seed)`**, **`land_at_cartesian(x,
y)`** (the lattice's points about the cell blended, a point no share is
taken from not worked out), **`height(x, y)`**,
**`heights_of_a_square(square_left, square_top, side_cells)`** (every cell's height, row by
row, each point of the lattice worked out once), **`inland_and_line_distance(x, y)`** (how
far inland the cell is, and about how far from the broad mesh's
nearest line). Private to `mesh/lands.rs`: `LATTICE_STEP_POWER` (2) and
`LATTICE_STEP` (4), the cells from a point of the lattice to the next,
as a power of two and as cells; `WorldLand::at_the_lattice(x, y)`, the
meshes' land at a point, in 16-bit fractions of a height;
`between_the_lattice(corner_points, cells_across, cells_down)`, a cell's land from the
four points about it, made whole once. **`land_of_one_cell_cartesian`**:
the same for one cell alone. `SIGMOID_ONE`, `FINER_MOST`, `COAST_MOST`.
Private: **`Vertex`**, **`MeshTriangle`**, **`BlendedCell`**, **`Mesh`**
(`new`, `vertex_lot_cartesian`, `ocean`, `vertex`, `triangle_of_corners`, `locate_triangle_cartesian`, `blended_cartesian`),
**`raised_to_power`**, **`area`**; `towards_the_slope(line_lot)`: a line's lot drawn
a little nearer nothing, halfway to its square; `inverse_of_area(whole_area)`: what a part of a
triangle's area is multiplied by to be its share of the whole -- a
division done once for the triangle, 0 where the triangle is too broad
for that to be exact enough and the division is done a cell;
`WorldLand::moved_cartesian(x, y)`: a cell moved by the broad noise that bends the
lines. Constants: `VERTICES_SALT`, what the vertices are drawn by,
apart from all else; `WARP_INDEX` (30) and `CLUMP_INDEX` (40), the
numbers the bending and the clumping noise are drawn by; `FINEST_SPACING_POWER` (4),
the finest grid's square as a power of two, 16 cells; `SIGMOID_MOST`,
16 times `SIGMOID_ONE`.

## `patches.rs`

How something lies in patches when a superchunk is made. `CELLS_LOOKED_AT`
(16,384); a fraction's whole is the crate's `FRACTION_ONE` (65,536). **`Patches`** `{cover_share, patch_size_power, detail_share,
scatter_share}`; **`cell_number_cartesian(seed, x, y)`**: a cell's number -- noise as broad
as a patch, finer noise, and the cell's own lot;
**`cover_threshold(seed)`**: the number under which `cover_share` of the cells are.
For a superchunk's every cell (crate only): `patch_noises(seed)`, the
patches' noise and the finer, each kept cell after cell;
`weighed_cartesian(seed, broad_noise, fine_noise, x, y)`, the number before it is divided;
`weights()`, what it is divided by.

## `generated_superchunk.rs`

**`generate_superchunk(world_generation, seed, superchunk)`**: its cells
(`chunk_storage::SuperchunkCells`) -- terrain, the ocean where it is
under the ocean's level, and on the rest grass and trees with their
stages, as `WorldGeneration::world_growth` says of each cell, each tree's cell set
in the collision plane. No image: that is
storage's to make.

## `climate.rs`

`CLIMATE_SALT`: what keeps the climate's noise apart from the
ground's. `BROAD_SPACING_POWER`, `FINER_SPACING_POWER`: the cells from a point to the next of its
noises, as powers of two. `STEPS_A_DEGREE_COOLER`,
`STEPS_A_LEVEL_DRIER`, `MORE_HUMID_OVER_WATER`: what height and water
do to it. **`ClimateNoises`**: the noises a world's climate lies by;
**`new(seed)`**, **`climate_at(x, y, above_ocean, under_water)`**: a tile's
climate (`worldgen.md`, "Climate"). The module is `climate`.

## `generation.rs`

`TREES_SALT`: what keeps the trees' numbers apart from the grass's.
**`WorldGeneration`** `{land_shape, grass_patches, trees}`; `WorldGeneration::DEFAULT`;
**`WorldGeneration::plain(grass_cover)`**: a plain -- flat dry land, no
ocean, no walls, no trees, grass scattered on `grass_cover` of `FRACTION_ONE` of
its cells;
**`generation_numbers()`** and **`of_numbers(numbers)`**: its numbers by name, as
a world's file keeps them (`generation_numbers!`). **`from_tuning(slider_tuning)`**: as
the sliders have it (**`shape_from_tuning`**). **`world_growth(seed)`**: a
**`WorldGrowth`**, the thresholds found once and the noises kept;
**`WorldGrowth::grown_at(x, y)`** (it keeps the noise about the cell, so each
thread has a `WorldGrowth` of its own): a
**`GrownCell`** `{has_grass, tree}` -- the tree a lot, its stage drawn from it
by whoever counts the stages. **`has_land_about(seed, land_shape, near_superchunk)`**:
land three superchunks each way about `near`; **`seed_with_land(from,
shape, near)`**: the first seed from `from` that has.

## `diagnostics/`, `transient_data.rs`

The folders every crate has (`../../docs/style_guide.md`, "One shape for
every crate"). `diagnostics/mod.rs` gathers nothing yet.
`transient_data::TRANSIENT_DATA` names the crate's `transient_data/`
folder, where its runs would leave what they make.
