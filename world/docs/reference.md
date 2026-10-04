# The world, function by function

The design is in `world.md`.

## `lib.rs`

`GRASS_CELLS` (400,000): the grass a superchunk generated is given;
`FLOCK` (4,000): the sheep the origin starts with, unless told.
**`World`** `{info, arena, storage, entities, simulation, cold, codec}`
-- **`cold`**, each cold superchunk's state as a save keeps it --
and **`World::empty(info)`**, what generating and loading start from;
**`layer_types`**. **`generate(seed, sheep)`**: the origin
(`WORLD_MIDDLE`) hot, a flock of `sheep` on it, and its halo made.
**`generate_image(seed, superchunk, codec)`**: a superchunk's terrain
and pasture, from the seed and its superchunk index.
**`save(folder, world)`**: every dirty bitmap written back, the ring
flushed, then each superchunk's image and state -- live if hot, kept
if cold -- written, the world's file last: a **`Saved`**
`{superchunks, entities, bytes}`. **`load(folder)`**: every
superchunk's image into the cold pool and its state kept cold, then
the halos about the states holding a halo keeper made hot -- a
`World`, or a `DiskError` naming the file and what is wrong.

## `halos.rs`

`HALO_KEEPERS` (the sheep, for now): the kinds of entity that keep a
halo. **`HaloChange`** `{generated, warmed, cooled}`, added with `+=`.
**`about(keepers)`**: the 3x3 superchunks about each, sorted, each
once. **`World::keepers`**: the hot superchunks holding a keeper.
**`World::move_halos`**: the halos moved to their keepers.
**`World::keep_hot(wanted)`**: `wanted` made the hot superchunks --
the others' states kept and their bitmaps made cold
(`BitmapArena::make_cold_superchunk`), the new ones warmed from
storage and their kept states, or generated -- the entities aligned
and the random streams with them.

## `tick.rs`

**`tick_rules(simulation, arena, entities, seed)`**: grass
(`mc_rules::grass::rule`) and then sheep (`entity_rules::sheep::rule`) on
each hot superchunk's turn, the halos left -- for mock worlds;
**`TickCounts`** `{grass, sheep}`. **`World::tick`**: the rules, then
the halos moved: a **`WorldTick`** `{rules, halos}`.

## `diagnostics/`

The mock world they tick is the entities'
(`entity_rules::diagnostics::world::MockWorld`).

**`throughput::run(ticks, thousandths, superchunks, threads)`**: grass
ticked flat out: each phase's time, samples, writes, cells missed, the
process's memory read every tick, the arena's and storage's stats --
a **`Throughput`**.

**`pasture::run(ticks, thousandths, sheep, superchunks, threads)`**:
grass and sheep ticked flat out: the flock and grass over the run, what
the sheep did, each phase's time and each rule's -- timed inside the
rule, over every thread -- the memory, the entities' stats, and a
**`Census`** of the flock and grass every `CENSUS_EVERY` (100) ticks:
a **`PastureRun`**.

**`frames::frame(arena, superchunk, pixels)`**: a superchunk as RGB
pixels, dirt `BROWN`, grass `GREEN`; `FRAME_BYTES`.
**`frames::sheep(entities, superchunk, pixels)`**: its entities drawn
over it, `WHITE` squares.

## `transient_data.rs`

**`measurements()`**, **`renders()`**, **`saves()`**, **`publish(report)`**.

## `diagnostics/tool/main.rs`

**`throughput`**: runs `throughput::run` and publishes its time, rates
and memory tables. **`pasture`**: runs `pasture::run` and publishes the
flock, time a sample and a wake, rates, what is held and the census
(**`census_table`**). **`video`**: one superchunk's frames, sheep on
them if asked, on standard output, raw RGB, for ffmpeg; the census at
every frame kept, unprinted, in `measurements/video.csv`.
