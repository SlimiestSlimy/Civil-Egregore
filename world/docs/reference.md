# The world, function by function

The design is in `world.md`.

## `lib.rs`

`GRASS_CELLS` (400,000): the grass a superchunk generated is given;
`FLOCK` (4,000): the sheep the origin starts with, unless told.
**`World`** `{info, arena, storage, entities, simulation, cold,
background, warming, writing_back, flushing}` -- **`cold`**, each cold
superchunk's state as a save keeps it; **`writing_back`**, the
write-backs of superchunks gone cold being encoded, in order;
**`flushing`**, the superchunks whose images are being rewritten -- and
**`World::empty(info)`**, what generating and loading start from;
**`layer_types`**. **`generate(seed, sheep)`**: the origin
(`WORLD_MIDDLE`) hot, a flock of `sheep` on it, and its halo made hot.
**`generate_image(seed, superchunk, codec)`**: a superchunk's terrain
and pasture, from the seed and its superchunk index.
**`save(folder, world)`**: every dirty bitmap written back
(`World::write_back_all`), the ring flushed (`World::flush_all`), then each superchunk's
image and state -- live if hot, kept if cold -- the hot file, and the
world's file last: a **`Saved`** `{superchunks, entities, bytes}`.
**`load(folder)`**: every superchunk's image into the cold pool and its
state, read whole, kept cold; then the hot file's hot superchunks made
hot and its warming ones warming again -- a `World`, or a `DiskError` naming
the file and what is wrong.

## `halos.rs`

`HALO_KEEPERS` (the sheep, for now): the kinds of entity that keep a
halo. `WARM_TICKS` (256): the ticks a superchunk is warming.
**`HaloChange`** `{reached, generated, restored, cooled}`, added with
`+=`. **`Warming`** `{superchunk, due, from}`, from a
**`WarmedFrom`**: `Cooling`, or `Background(ticket)`.
**`about(keepers)`**: the 3x3 superchunks about each, sorted, each
once. **`World::keepers`**: the hot superchunks holding a keeper.
**`World::move_halos`**: the halos moved to their keepers, those
reached hot `WARM_TICKS` on. **`World::keep_hot(wanted)`**: `wanted`
made the hot superchunks now. **`World::warming`**: the superchunks
warming, each with its due tick. Both through
**`World::make_hot_within(wanted, ticks)`**: the write-backs encoded
landed; the hot superchunks not wanted made cold -- state kept, bitmaps
cooling (`BitmapArena::make_cold_superchunk`), their dirty ones sent to
be encoded; those warming not wanted dropped (`BitmapArena::let_go`, or
the job forgotten); every one warming due no later than `ticks` on; the
wanted ones neither hot nor warming started; those due made hot, the
entities aligned, the kept states put back and the random streams with
them. **`World::start_warming(superchunk, due)`**: held if cooling
(`BitmapArena::hold`), else sent to the background.
**`World::finish_warming`**: a warming superchunk's bitmaps made hot -- again
as they were (`BitmapArena::make_hot_again`), or as the background made
them (`BitmapArena::make_hot_cells`), its image put in the cold pool if
generated. **`World::land_write_backs(wait)`**: the encoded write-backs
put into the ring, in order (`ChunkStorage::try_write_back`, then
`BitmapArena::written_back`), the tail flushed whenever it needs the
room, then the flushes landed. **`World::flush_tail`**: the tail
superchunk's changes taken (`ChunkStorage::take`) and sent to be
flushed, its flush before landed first. **`World::land_flushes(wait)`**:
the images rewritten put in the cold pool, the arena told of each with
no change left in the ring (`BitmapArena::flushed`).
**`World::write_back_all`**: every hot superchunk's dirty bitmaps sent
to be encoded, and every write-back landed. **`World::flush_all`**:
every superchunk with changes in the ring flushed, on all the threads.

## `background.rs`

**`Job`**: `Encode(dirty)`, `Flush(flush)`, or `Warm {superchunk,
image, seed, types}`; **`Job::run(codec)`**: a **`Done`** --
`Encoded(encoded)`, each bucket's layer words; `Flushed(image)`; `Warmed {generated, cells}`, every bitmap's
cells, chunk by chunk, type by type, and the image if generated; or
`Failed(said)`, a panic caught. **`Ticket`**: a job sent.
**`Background::new`**: a thread a core, each with its own codec, taking
jobs one at a time; **`send(job)`**, a ticket; **`try_take(ticket)`**;
**`take(ticket)`**, waiting; **`forget(ticket)`**, what it makes
dropped; **`keep`**, what is made kept until taken. **`checked`**: a
job's panic carried on where it is taken. Dropped, the threads stop.

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
