# The world, function by function

The design is in `world.md`.

## `lib.rs`

`FLOCK` (4,000): the sheep a superchunk starts with, unless told.
**`World`** `{info, generation, arena, storage, entities, simulation,
cold, halos}` -- **`cold`**, each cold superchunk's state as a save
keeps it; **`halos`**, the simulation's -- and
**`World::empty(info, generation, threads)`**, what generating and
loading start from, hot as `info` says: forced throughout if it has a
side and is forced, else about the hot entity's halo; **`layer_types`**.
**`Size`**: `Unlimited`, or `Limited {side, forced}` -- so many
superchunks along a side, a square about the origin, and whether
every one of them is hot throughout, which only a world with a side
can be; **`Size::from_tuning(tuned)`**: as the sliders have it, a
side of 0 no limit.
**`start(options)`**: a **`Start`** `{seed, generation, size,
threads, sheep}` made into a world -- `threads`, every one the machine
has if `None`; `sheep` on every superchunk of a world with a side, on
the origin (`WORLD_MIDDLE`) alone of one without, as sheep everywhere
would keep the whole of an endless world hot; their halos hot, or, of
a world forced hot, all of it. `Start::default()`: seed 1, the default
generation, no size, every thread, `FLOCK` on the origin.
**`generate_image(seed, superchunk, codec)`**: a superchunk's terrain
and pasture, from the seed and its superchunk index.
**`save(folder, world)`**: every dirty bitmap written back and the
ring flushed (`World::write_back_and_flush_all`), then each superchunk's
image and state -- live if hot, kept if cold -- the hot file, and the
world's file last: a **`Saved`** `{superchunks, entities, bytes}`.
**`worlds_in(folder)`**: the worlds saved in a folder, by their
folders' names, sorted.
**`load(folder)`**: every superchunk's image into the cold pool and its
state, read whole, kept cold; then the hot file's hot superchunks made
hot, its cooling ones cooling again and its warming ones warming again -- a `World`, or a `DiskError` naming
the file and what is wrong.

## `halos.rs`

`HOT_ENTITY` (the sheep, for now). **`World::with_halos(work)`**: the
simulation's halos given what the world holds, and what generates a
superchunk never made. **`World::move_halos`**,
**`World::keep_hot(wanted)`**, **`World::warming`**,
**`World::cooling`**, **`World::start_warming(superchunk, due)`**,
**`World::write_back_and_flush_all`**: each the halos' own
(`../../simulation/docs/reference.md`, "`halos.rs`").

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


## `transient_data.rs`

**`measurements()`**, **`saves()`**, **`publish(report)`**.

## `commands.rs`

**`COMMANDS`**: what `Civil_Egregore server <command>` runs, each with its
parameters and their defaults. `Civil_Egregore server new <folder> [seed]
[sheep] [side] [forced] [threads]`: a world generated from the seed --
the origin, a flock of `sheep` on it, and its halo (**`new`**), of
`side` superchunks a side unless 0, forced hot throughout if `forced`
is not 0, on `threads` threads or every one the machine has if 0 --
and saved in the folder, which must not hold one and
whose name is the world's (**`name`**). `Civil_Egregore server run <folder> [ticks]`: it
loaded, ticked and saved again (**`run`**). `Civil_Egregore server info
<folder>`: what its world file says (**`info`**). A folder given as a plain name is one
of the worlds' folder (`utilities::settings::world`); anything more is
a path. **`printed`**: a command run on its folder, its line printed. The diagnostics tools,
`throughput` and `pasture`, are listed there too.

## `diagnostics/tool.rs`

**`threads`**: the threads
asked for, every one if 0.

**`throughput`**: runs `throughput::run` and publishes its time, rates
and memory tables. **`pasture`**: runs `pasture::run` and publishes the
flock, time a sample and a wake, rates, what is held and the census
(**`census_table`**).

## Generation

How a world is generated is `worldgen`'s (`worldgen::Generation`,
saved with the world, a number a row). **`generate_sized(generation,
seed, size, threads)`**: what `start` builds a world from, generated
so, as far as its size lets it reach, nothing hot yet -- and what a way
of generating is tried out on by itself. **`flocked(world,
superchunks, sheep)`**: what `start` builds a flock from, put on a
world with nothing in it yet -- and what a flock is tried out on by
itself. **`generate_image(generation, seed, superchunk, codec)`**:
terrain, the ocean where it is under the ocean's level, and on the rest
grass and trees with their stages, as `Generation::growth` says of
each cell.

## `host/mod.rs`

A world run on a thread of its own for a client -- a window -- that
asks it for the cells in view. `TARGET_PACE` (256 ticks a second),
`CENSUS_EVERY` (1,000 ticks), `CATCH_UP`. **`Request`**: `Sync(ask)`
-- answered with a `Frame`, unless no world runs -- `Pause(bool)`,
`Pace(ticks a second, or flat out)`, `New(start)`, `Open(name)` -- that
world of the worlds' folder run in place of the one run -- and
`Save(name)`. **`census_path()`**: where a run's census is kept
(`transient_data/measurements/census.csv`); **`census(seed)`**: its
file, started afresh for each world. **`start()`**: the host on a
thread named `host`, no world run yet; where to send requests, where
frames come back. **`Running`**: the world run, the superchunks whose
heights were sent, when it began, its census. **`Host`**: the world
run if any, the worlds run so far, paused, the pace, the name, what
was last said -- **`run`**: requests read between ticks, a tick, and a
wait for the next one's time; paused or with no world, it waits for a
request; **`run_world`**, **`save`**, **`frame(ask)`**, **`tick`**.

## `host/frame.rs`

`CHUNK_WORDS`, `DEEP` (16). **`Viewport`** `{first, last}`: the
superchunks in view. **`Ask`** `{viewport, detail, skip, most, near}`:
what a frame is to carry; **`asked()`**, those it asks for.
**`Near`** `{first, size, pixels_a_cell}`: the cells seen from near.
**`Cells`**: a superchunk's planes copied -- grass, trees, their
stages, water -- its heights the first frame it is hot in, its
sheep's cells. **`Frame`** `{world, seed, generation, side, tick,
ticks_a_second, sheep, grass, trees, sync_seconds, sync_share, detail,
near, named, said, cells}`: `world` counts the worlds the host has
run, so a client knows what it drew is of another. **`count(world,
layer_type)`**; **`copy(world, ask, sent)`** (**`layer`**,
**`sheep`**); **`Water`**: a superchunk's water off its image, kept.
