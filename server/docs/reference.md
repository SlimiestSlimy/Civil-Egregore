# The world, function by function

The design is in `world.md`.

## `lib.rs`

`FLOCK` (4,000): the sheep the origin starts with, unless told.
**`World`** `{info, generation, arena, storage, entities, simulation,
cold, halos}` -- **`cold`**, each cold superchunk's state as a save
keeps it; **`halos`**, the simulation's -- and
**`World::empty(info)`**, what generating and loading start from;
**`layer_types`**. **`seed_with_land(from, shape)`**: the first seed
from `from` with land three superchunks each way about the origin --
what the renderer and the tests make their worlds from.
**`start(options)`**: a **`Start`** `{seed, generation, side, flock}`
made into a world -- its **`Flock`**, `None` or `On(superchunks,
sheep)`, put on and its halo hot; `Start::default()`: seed 1, the
default generation, no size, `FLOCK` on the origin (`WORLD_MIDDLE`).
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
[sheep] [side]`: a world generated from the seed -- the origin, a flock
of `sheep` on it, and its halo (**`new`**), of `side` superchunks a
side unless 0 -- and saved in the folder, which must not hold one and
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

**`Generation`** `{shape, grass, trees}`: how superchunks are generated;
`Generation::DEFAULT`; `TREES_SALT`; **`numbers()`** and
**`of_numbers(numbers)`**: its numbers by name, as a world's file
keeps them (`numbers!`). **`generate_sized(generation, seed, side)`**:
what `start` builds a world from, generated so, a size if given one,
nothing hot yet -- and what a way of generating is tried out on by
itself. **`flocked(world, superchunks, sheep)`**: what `start` builds a
flock from, put on a world with nothing in it yet -- and what a flock
is tried out on by itself.
**`generate_image(generation, seed, superchunk, codec)`**:
terrain, the ocean where it is under the ocean's level, and on the rest
grass and trees with their stages. `World::generation`
is not saved: a world loaded goes on with the default.
