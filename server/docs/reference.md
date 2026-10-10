# The world, function by function

The design is in `server.md`.

## `lib.rs`

**`World`** `{info, generation, arena, storage, entities, simulation,
cold, halos}` -- **`cold`**, each cold superchunk's state as a save
keeps it; **`halos`**, the simulation's -- and
**`World::empty(info, generation, threads)`**, what generating and
loading start from, hot as `info` says: forced throughout if it has a
side and is forced, else about the halos of the kind of entity it
names, `HOT_ENTITY` if none (**`hot_of(info)`**).
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
(`utilities::tuning::Tuning`): how it is generated, its size -- a side
of 0 none, forced hot counting only with a side -- its sheep, and its
camera loading superchunks only if it is not forced hot; from the seed
given or one drawn; on every thread the machine has.
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
`owe_camera_flocks(superchunks)`: those generated out of view kept as
owed their flock (`WorldInfo::without_camera_flock`).
The viewport is told to the halos themselves (`Halos::keep_viewport`),
by the host on each frame asked.
**`World::keep_hot(wanted)`**, **`World::warming`**,
**`World::cooling`**, **`World::start_warming(superchunk, due)`**,
**`World::write_back_and_flush_all`**: each the halos' own
(`../../simulation/docs/reference.md`, "`halos.rs`").

## `tick.rs`

**`tick_rules(simulation, arena, entities, seed)`**: every rule of
`RULES` on each hot superchunk's turn, the halos left where they are;
**`tick_chosen(.., chosen)`**: only the rules chosen.
**`World::tick`**: the rules, then the halos moved: a **`WorldTick`**
`{rules, halos}`. **`World::tick_only(chosen, timed)`**: the rules
chosen alone, each one's time taken if `timed`, the halos not moved.

## `rules.rs`

**`RULES`**: every rule a world ticks, a **`Rule`** `{name, counted,
rule}` each -- grass, trees, sheep -- in the order a turn runs them; the
one list of them. **`place_of(name)`**: a rule's place in it.
**`TickCounts`** `{counts, times}`: each rule's `RuleCounts` and time at
its place; **`of(rule)`**, **`time_of(rule)`** and **`count(rule,
counted)`** read them by name. **`Chosen`**: some of the rules --
**`ALL`**, or **`named(names)`** -- and a superchunk's turn of them,
**`turn`**, or **`timed_turn`** with each one's time.

## `diagnostics/`

The world they tick is one the server starts as any other
(`start`): **`plain_world::plain_world(superchunks, grass_cover, sheep,
threads)`**, a plain (`worldgen::Generation::plain`) with a side,
forced hot, so a rule is measured alone; **`plain_world::superchunks`**
how many it holds hot.

**`throughput::run(ticks, thousandths, superchunks, threads)`**: grass
ticked flat out: each phase's time, samples, writes, cells missed, the
process's memory read every tick, the arena's and storage's stats --
a **`Throughput`**.

**`pasture::run(ticks, thousandths, sheep, superchunks, threads)`**:
grass and sheep ticked flat out: the flock and grass over the run, what
the sheep did, each phase's time and each rule's -- `Chosen::timed_turn`,
over every thread, the rules picked by name (`RULES_TICKED`) -- the memory, the entities' stats, and a
**`Census`** of the flock and grass every `CENSUS_EVERY` (100) ticks:
a **`PastureRun`**.


## `transient_data.rs`

`TRANSIENT_DATA`: the crate's `transient_data/` folder.
**`measurements()`**, **`saves()`**, **`publish(report)`**.

## `world_hash.rs`

**`world_hash(world)`**: the world's hash, a **`WorldHash`** `{tick,
cells, entities, random_streams, halos, cold}` -- a word a part, each
folded (`utilities::hash::fold`) from the part in Morton order: every
hot bitmap of every layer type with its chunk and count; every entity's
header and attributes; every random stream; the superchunks hot,
warming and cooling with when each is due; every cold superchunk's
state and image. The writeback ring is flushed first, as a save does.
**`WorldHash::whole(seed)`**: the parts, the tick and the seed in one
word.

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
a path. **`printed`**: a command run on its folder, its line printed;
`number`: an argument as a number, or its default. `FOLDER`, `FORCED`:
the parameters' names. The diagnostics tools,
`throughput`, `pasture` and `check`, are listed there too.

## `diagnostics/tool.rs`

**`threads`**: the threads asked for, every one if 0, no more than
the superchunks. The parameters' names: `TICKS`, `GRASS` (in
thousandths), `SUPERCHUNKS`, `THREADS`, `FLOCK` (sheep a superchunk),
`SEED` (in hexadecimal; 0 the run's own), `EVERY` (ticks between
hashes), `SHEEP`. `share(part, whole)`: a share of a time as a report
shows it.

**`throughput`**: runs `throughput::run` and publishes its time, rates
and memory tables. **`pasture`**: runs `pasture::run` and publishes the
flock, time a sample and a wake, rates, what is held and the census
(**`census_table`**). **`check`**: `Civil_Egregore server check [seed]
[ticks] [ticks between hashes] [sheep] [threads]` -- a world from the
seed (0: the run's), its camera loading nothing, ticked and its hash
printed before the first tick and every so many after, a row a hash,
part by part, with nothing of the machine in it.

## Generation

How a world is generated is `worldgen`'s (`worldgen::Generation`,
saved with the world, a number a row). **`generate_sized(generation,
seed, size, hot_entity, threads)`**: what `start` builds a world from,
generated so, as far as its size lets it reach, nothing hot yet, to be
hot about the entities of the kind `hot_entity` unless forced -- and what a way
of generating is tried out on by itself. **`flocked(world,
superchunks, sheep)`**: what `start` builds a flock from, put on a
world with nothing in it yet -- and what a flock is tried out on by
itself. The server generates nothing: a superchunk's cells are
`worldgen::generate_superchunk`'s, the image of them storage's
(`chunk_storage::SuperchunkCells::image`); the server hands the one to
the other where the halos ask for a superchunk.

## `host/mod.rs`, `host/host_thread.rs`

`Running` and `HostThread` are in `host_thread.rs`, the rest in `mod.rs`.

A world run on a thread of its own for a client -- a window -- that
asks it for the cells of its viewport. `TARGET_PACE` (256 ticks a second),
`CENSUS_EVERY` (1,000 ticks), `CATCH_UP`, `FRAMES_SHARE` (an eighth of
a tick's time, what answering may take of it when the host has none to
spare). **`Host`**: the host as a
client holds it, each call sent to its thread and done there between
two ticks, each saying whether the host was still there --
**`Host::start()`**: the host on a thread named `host`, no world run
yet, and where its frames come back; **`sync(ask)`** -- answered with
frames, the last with `more` unset, unless no world runs -- **`pause(paused)`**, **`pace(ticks
a second, or flat out)`**, **`make_world(start)`**,
**`open_world(name)`** -- that world of the worlds' folder run in
place of the one run -- **`save_world(name)`**, and **`reset(tuning)`**
-- the world run made again from its start, generated as the tuning
has it now, a world of its own of no name; several asked with no
frame asked between make it once, as the last says; a frame asked
after is of the world remade. `Request`, private:
a call as sent. **`census_path()`**: where a run's census is kept
(`transient_data/measurements/census.csv`); **`census(seed)`**: its
file, started afresh for each world. **`Running`**: the world run, the
superchunks whose heights were sent, when it began, its census.
**`Answering`**: an ask being answered -- what was asked, the
viewport's hot superchunks then, those yet to copy, those copied and
not yet sent, what copying has taken.
**`HostThread`**: the world run if any, the worlds run so far, paused,
the pace, the name, what was last said, the ask being answered, what
the last tick took -- **`run`**: requests read between ticks, a little
of the ask answered, a tick, and a wait for the next one's time; paused
or with no world, it waits for a request; **`run_world`**,
**`run_in_place`** -- an ask half answered is dropped with the world it
was of -- **`reset_now`**, **`save`**, **`begin_frame(ask)`**: the
viewport kept hot, what is to copy listed; **`answer_a_little()`**: a
superchunk copied at least, and more until **`time_for_frames()`** is
up -- what is left before the next tick is due, `FRAMES_SHARE` of the
last tick's time at least, no limit while paused -- and those sent as a
frame; seen from near, sent only once all are copied, being one
picture; **`tick`**.

## `host/terrain_seen.rs`

What a client asks of a world's terrain where no frame brings it,
answered from how the world is generated: a client names nothing under
the server. `Generation` and `Height`, the words it is asked in.
**`HeightsSeen::of(generation, seed)`**, a thread's own:
**`height(x, y)`**, **`cells_from_a_mesh_line(x, y)`**.
**`CoverSeen::of(generation, seed)`**, a copy to each thread, as it keeps the noise about the last cell: **`cover(x, y)`**, a
**`Cover`** -- `Tree`, `Grass`, `Dirt`. **`levels(generation)`**:
**`Levels`** `{ground, ocean, highest}`. **`walled(one, other)`**: a
wall between two heights. **`height_in_frame(height_words, place)`**:
a cell's height from the words a frame brings.
**`seed_with_land(from, generation, near)`**.

## `host/frame.rs`

`CHUNK_WORDS`, `WORD_BITS`, `STAGE_BITS`, `OLDEST_TREE_STAGE`,
**`cell_of_bit(bit)`** -- how a frame's bits lie, so a client asks no
one else. `DEEP` (16). **`Viewport`** `{first, last}`: what the
renderer should render, in superchunks -- not Bevy's camera viewport,
a rectangle of the window -- **`contains(at)`**. **`Ask`** `{viewport,
detail, skip, most, near}`: what a frame is to carry, `viewport` `None`
when the client renders none of the world's cells (map mode); `skip`
and `most` count the viewport's hot superchunks. **`hot_in(world,
viewport)`**: those, row by row.
**`Near`** `{first, size, pixels_a_cell}`: the cells seen from near.
**`Cells`**: a hot superchunk's planes copied -- grass, trees, their
stages where a cell is a pixel or more, its water (a `Water`, shared,
not copied again) -- its heights the first frame it is hot in, its
sheep's cells. **`Frame`** `{world, seed, generation, side, tick,
ticks_a_second, sheep, grass, trees, sync_seconds, sync_share,
viewport, hot, detail, near, named, said, more, cells}`: `world` counts the
worlds the host has run, so a client knows what it drew is of another;
`hot`, every hot superchunk of the viewport, so a client knows what it
drew there of any other has gone cold; `more`, whether frames of the
same ask are still to come. **`count(world,
layer_type)`**; **`copy(world, superchunk, ask, sent)`**: one
superchunk's `Cells`, none if it has gone cold since asked
(**`layer`**, **`sheep`**); **`Water`** `{depths, deep}`: a
superchunk's water off its image, read once and kept.
