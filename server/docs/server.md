# The server

The world as a whole: made from a seed, ticked, saved and loaded. The
server is the one crate that holds the hot bitmaps, the stored superchunks, the
entities and the simulation together, and that makes the one thread
dispatcher the tick and chunk storage's jobs share; the program (`../src/main.rs`)
and the renderer (`../renderer/`) call it.

## Made from a seed

`generate(seed, sheep)`: the world starts as its origin superchunk
(`WORLD_MIDDLE`) with a flock on it, and the eight about it -- the
flock's halo -- all hot before it ticks. Every superchunk's contents come from the world's seed
and its superchunk index (`generate_image`), so a superchunk is the
same whenever and in whatever order it is made: the world has no edge
but the coordinates', and grows as the sheep wander. A superchunk is
its terrain (`../worldgen/`) -- heights, and the walls they make, four
layers -- and on it grass in patches, dirt being a cell with none and
having no layer, and trees in patches of
their own, each of a stage drawn for its cell (`Generation`, `patches`).

Each superchunk's random numbers are a stream of their own
(`Rng::for_stream`): seeded by the seed moved along by the
superchunk index, as they first were, two superchunks drew one
sequence a few draws apart -- two flocks came out with the same sheep.

## Halos

Only the superchunks about the hot entities are hot, and the halos are
the simulation's (`../../simulation/docs/simulation.md`, "Halos"): the
world lends them what it holds (`World::with_halos`) and tells them
two things. The hot entity (`HOT_ENTITY`): people, to come; for now
the sheep stand in. And the world's size, if it has one
(`generate_sized`, `tilesim server new <folder> [seed] [sheep]
[side]`): so many superchunks along a side, a square about the origin,
nothing ever made outside it; a save keeps it. What generates a
superchunk never made is the server's, handed to the halos' jobs.

## TickCounts

`World::tick` runs every rule of the cells (`../mc_rules/`) and every
kind of entity (`../entity_rules/`) on each hot superchunk's turn --
grass, then sheep -- then moves the halos.

## Saved and loaded

A save is a folder (`chunk_storage::disk`, and
`../chunk_storage/docs/chunk_storage.md`, "On disk"):

| file | what it holds |
|---|---|
| `world.csv` | CSV, a row a thing, its name and what it is: the format's number, the world's seed in hexadecimal, the tick it is at, its layer types, its side if it has a size, and how it is generated, a number a row |
| `hot.csv` | CSV: the hot superchunks, a row each, those cooling with the tick each goes cold at, and the warming ones with the tick each turns hot at |
| `superchunks/<index>.image` | a superchunk's cells and heights: its image, as the cold pool holds it |
| `superchunks/<index>.state` | its random stream's state, its entities with their attributes |

`<index>` is the superchunk index, 44 bits, 11 hexadecimal
digits: its name, and nothing else is.

A world's name is its folder's, nothing in the files: only what a
folder may be named on Windows and Linux alike
(`utilities::settings::world_name`). How it is generated
(`Generation::numbers`) is kept in its file, so one loaded goes on
generating as it was made.

**Saving** is between two ticks. Every dirty bitmap is written back --
encoded on every thread, after every write-back still
on its way -- and the ring flushed, so the cold pool's images are the
world's cells; each image is written as it is, and beside it the
superchunk's state; then the hot file. The world's file is written
last, each file beside itself and then
renamed, so a save cut short leaves the one before readable.

**Loading** reads every image into the cold pool and keeps every
state -- each read whole, so a bad one is refused now -- as a cold
superchunk's. Then, before it ticks, it makes hot the superchunks the
hot file names, decoded on every thread: every layer
type of the world made hot on every chunk of them -- a type with no
cell left on a superchunk has no stored layer, and must be hot all the
same to be written to -- their entities put back at the world's tick,
their random numbers taken up. The cooling ones cool again, and the
warming ones warm again, each to turn at the tick it was to.

**A world loaded goes on as the one saved would have**, to the cell,
the entity and the random number (`world` in `tests/fast.rs`: 1,500 ticks,
saved, then 3,000 more on both; and a world saved and loaded seven
times mid run -- at ticks 1, 700, 701, 1,900, 3,333 and 4,000, and the
first tick a superchunk is warming, each
stretch run on what the files hold alone -- against one run straight
to tick 4,000: the same cells, entities and random numbers). What makes it so:

- **Random numbers are a superchunk's own, and kept.** Each superchunk
  has a generator that goes on from tick to tick (`Simulation`), first
  seeded from the world's seed and its superchunk index; its state is in
  the state file. Were they made anew from a tick's seed, a load would
  start them over.
- **Entities are put back through the same door as any other**: queued
  and applied, so their wakes are filed as they were. Wakes left over
  from before an entity was changed are not kept: they wake nothing.
- **Nothing is half done between ticks**: a crossing is settled
  within its tick, so a save holds each entity once, on one cell.
- **A superchunk turns hot at a tick fixed when a halo reaches it**,
  not when its job is done, and cold at a tick fixed when the
  halos leave it; the hot file keeps both.
- An entity whose wake has passed when it is put back -- kept while
  its superchunk was cold -- wakes the tick it is put back.

Most of a save is heights, kept raw, about a byte a cell; the layers
and the entities are each a fraction of that (`tilesim server new`
says the bytes written).

Not yet: superchunks no longer in the world are not removed from a
save's folder.

## Layout

| folder | what is in it |
|---|---|
| `src/lib.rs` | generate, save, load |
| `src/halos.rs` | the hot entity, and the world lent to the simulation's halos |
| `src/tick.rs` | the tick of every rule and entity, then the halos moved |
| `src/patches.rs` | how grass and trees lie in patches when a superchunk is made |
| `src/diagnostics/` | grass, and grass and sheep, ticked flat out and measured; the diagnostics tools |
| `src/transient_data.rs` | where runs leave what they make, out of git |
| `tests/` | the halos follow their hot entities; a world of a size is hot within it only; a superchunk warming takes nothing until due; one cooling stays hot until due; a superchunk gone cold comes back as it was; a world loaded goes on as the one saved; the files; refusals |
| `docs/` | this, and the reference, function by function |
