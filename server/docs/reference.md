# The world, function by function

The design is in `server.md`.

## `lib.rs`

**`World`** `{info, generation, arena, storage, entities, simulation,
cold, halos}` -- **`cold`**, each cold superchunk's state as a save
keeps it; **`halos`**, the simulation's -- and
**`World::empty(info, generation, threads)`**, what generating and
loading start from, hot as `info` says: forced throughout if it has a
side and is forced, else about the halos of the kind of entity it
names, `HOT_ENTITY` if none (a save from before it was kept);
**`layer_types`**.
**`start(options)`**: a `Start` (`world_start.rs`) made into a world
-- `threads`, every one the machine has if `None`; `sheep` on every
superchunk of a world with a side, on the origin (`WORLD_MIDDLE`)
alone of one without, as sheep everywhere would keep the whole of an
endless world hot; their halos hot, or, of a world forced hot, all of
it; generated, flocked: below ("Generation"); the sheep for each
superchunk generated in the viewport kept (`WorldInfo::camera_flock`) if its
camera loads superchunks and it is not forced hot.
**`World::put_flock(superchunk, sheep)`**: a flock queued on a
superchunk, from its own stream of the seed's inverse -- the same
flock whenever it is put.
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

## `world_start.rs`

What a new world starts from, whoever gives the numbers -- the command
line or a window's sliders -- the server alone knowing what they mean.
`FLOCK` (4,000): the sheep a superchunk starts with, unless told.
`LAND_TRIES` (256). **`Size`**: `Unlimited`, or `Limited {side,
forced}` -- so many superchunks along a side, a square about the
origin, and whether every one of them is hot throughout;
**`Size::of_side(side, forced)`**: a side of 0 no limit, refused if
forced without a side, which only a world with one can be.
**`Start`** `{seed, generation, size, threads, sheep, hot_entity,
camera_loads}`;
`Start::default()`: seed 1, the default generation, no size, every
thread, `FLOCK` on the origin, hot about `HOT_ENTITY`, its camera
loading nothing.
**`Start::from_tuning(seed, tuning)`**: as a window's sliders have it
(`utilities::tuning::Tuning`), from the seed given or one drawn; its
camera loading superchunks only if it is not forced hot.
**`Start::of_world(info, generation)`**: what a world opened started
from, as far as its file says -- its sheep the camera's flock, or
`FLOCK`.
**`drawn_seed(generation)`**: a seed drawn at random, the first from
it with land about the origin within `LAND_TRIES`, else the one drawn.

## `halos.rs`

`HOT_ENTITY` (the sheep, for now). **`World::with_halos(work)`**: the
simulation's halos given what the world holds, and what generates a
superchunk never made. **`World::move_halos`**: the halos moved, then,
if the camera loads superchunks with sheep, a flock put
(`put_flock`) on each the move generated that is in the viewport.
The viewport is told to the halos themselves (`Halos::keep_viewport`),
by the host on each frame asked.
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
seed, size, hot_entity, threads)`**: what `start` builds a world from,
generated so, as far as its size lets it reach, nothing hot yet, to be
hot about the entities of the kind `hot_entity` unless forced -- and what a way
of generating is tried out on by itself. **`flocked(world,
superchunks, sheep)`**: what `start` builds a flock from, put on a
world with nothing in it yet -- and what a flock is tried out on by
itself. **`generate_image(generation, seed, superchunk, codec)`**:
terrain, the ocean where it is under the ocean's level, and on the rest
grass and trees with their stages, as `Generation::growth` says of
each cell.

## `host/mod.rs`

A world run on a thread of its own for a client -- a window -- that
asks it for the cells of its viewport. `TARGET_PACE` (256 ticks a second),
`CENSUS_EVERY` (1,000 ticks), `CATCH_UP`. **`Host`**: the host as a
client holds it, each call sent to its thread and done there between
two ticks, each saying whether the host was still there --
**`Host::start()`**: the host on a thread named `host`, no world run
yet, and where its frames come back; **`sync(ask)`** -- answered with
a `Frame`, unless no world runs -- **`pause(paused)`**, **`pace(ticks
a second, or flat out)`**, **`make_world(start)`**,
**`open_world(name)`** -- that world of the worlds' folder run in
place of the one run -- **`save_world(name)`**, and **`reset(tuning)`**
-- the world run made again from its start, generated as the tuning
has it now, a world of its own of no name; several asked between two
ticks make it once, as the last says. `Request`, private:
a call as sent. **`census_path()`**: where a run's census is kept
(`transient_data/measurements/census.csv`); **`census(seed)`**: its
file, started afresh for each world. **`Running`**: the world run, the
superchunks whose heights were sent, when it began, its census.
**`HostThread`**: the world run if any, the worlds run so far, paused,
the pace, the name, what was last said -- **`run`**: requests read between ticks, a tick, and a
wait for the next one's time; paused or with no world, it waits for a
request; **`run_world`**, **`run_in_place`**, **`reset_now`**, **`save`**, **`frame(ask)`**, **`tick`**.

## `host/frame.rs`

`CHUNK_WORDS`, `DEEP` (16). **`Viewport`** `{first, last}`: what the
renderer should render, in superchunks -- not Bevy's camera viewport,
a rectangle of the window -- **`contains(at)`**. **`Ask`** `{viewport,
detail, skip, most, near}`: what a frame is to carry, `viewport` `None`
when the client renders none of the world's cells (map mode); `skip`
and `most` count the viewport's hot superchunks. **`hot_in(world,
viewport)`**: those, row by row.
**`Near`** `{first, size, pixels_a_cell}`: the cells seen from near.
**`Cells`**: a hot superchunk's planes copied -- grass, trees, their
stages, water -- its heights the first frame it is hot in, its
sheep's cells. **`Frame`** `{world, seed, generation, side, tick,
ticks_a_second, sheep, grass, trees, sync_seconds, sync_share,
viewport, hot, detail, near, named, said, cells}`: `world` counts the
worlds the host has run, so a client knows what it drew is of another;
`hot`, every hot superchunk of the viewport, so a client knows what it
drew there of any other has gone cold. **`count(world,
layer_type)`**; **`copy(world, hot, ask, sent)`** (**`layer`**,
**`sheep`**); **`Water`**: a superchunk's water off its image, kept.
