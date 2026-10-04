# The world

The world as a whole: made from a seed, ticked, saved and loaded. It is
the one crate that holds the hot bitmaps, the stored superchunks, the
entities and the simulation together; the program (`../src/main.rs`)
and the viewer (`../viewer/`) call it.

## Made from a seed

`generate(seed, sheep)`: the world starts as its origin superchunk
(`WORLD_MIDDLE`) with a flock on it, and the eight about it -- the
flock's halo. Every superchunk's contents come from the world's seed
and its superchunk index (`generate_image`), so a superchunk is the
same whenever and in whatever order it is made: the world has no edge
but the coordinates', and grows as the sheep wander. A superchunk is
its terrain (`../terrain/`) -- heights, and the walls they make, four
layers -- and on it, for now, pasture: dirt, about a third of it grass.

Each superchunk's random numbers are a stream of their own
(`Rng::for_stream`): seeded by the seed moved along by the
superchunk index, as they first were, two superchunks drew one
sequence a few draws apart -- two flocks came out with the same sheep.

## Halos

Only the superchunks about the entities that matter are hot. An
entity of a kind that keeps a halo (`HALO_KEEPERS`: people, to come;
for now the sheep stand in) keeps its superchunk and the eight about
it hot -- as far as anything reaches in a tick, the speed of light.
Every other superchunk is cold: its cells in its image in chunk
storage, its entities and random numbers kept as a save keeps them
(`World::cold`). After every tick the halos move to where their
keepers stand (`World::move_halos`): a superchunk a halo comes to is
warmed from storage and its kept state, or, never made, generated;
one no halo reaches goes cold -- written back, flushed into its
image, dropped from the arena. So between ticks the hot superchunks
are the halos, exactly (`tests/fast/halos.rs`), and a superchunk gone
cold comes back as it was, to the cell, the entity and the random
number.

Passive rules tick only where hot: grass grows only in its circle
about the origin (`mc_rules::grass::grows_at`), and there only while a
halo covers it. An entity kept cold whose wake passes wakes the tick
its superchunk is warmed.

Not yet: a halo is a fixed 3x3 whatever its keeper; and a keeper is
found by looking through each hot superchunk's entities for one.

## TickCounts

`World::tick` runs every rule of the cells (`../mc_rules/`) and every
kind of entity (`../entity_rules/`) on each hot superchunk's turn --
grass, then sheep -- then moves the halos.

## Saved and loaded

A save is a folder (`chunk_storage::disk`, and
`../chunk_storage/docs/chunk_storage.md`, "On disk"):

| file | what it holds |
|---|---|
| `world` | text: the world's name, its seed, the tick it is at, its layer types |
| `superchunks/<index>.image` | a superchunk's cells and heights: its image, as the cold pool holds it |
| `superchunks/<index>.state` | its random stream's state, its entities with their attributes |

`<index>` is the superchunk index, 44 bits, 11 hexadecimal
digits: its name, and nothing else is.

**Saving** is between two ticks. Every dirty bitmap is written back and
the ring flushed, so the cold pool's images are the world's cells; each
image is written as it is, and beside it the superchunk's state. The
world's file is written last, each file beside itself and then
renamed, so a save cut short leaves the one before readable.

**Loading** reads every image into the cold pool and keeps every
state as a cold superchunk's, then warms the halos about the states
holding a halo keeper: every layer type of the world made hot on every
chunk of them -- a type with no cell left on a superchunk has no stored
layer, and must be hot all the same to be written to -- their entities
put back at the world's tick, their random numbers taken up.

**A world loaded goes on as the one saved would have**, to the cell,
the entity and the random number (`tests/world.rs`: 1,500 ticks,
saved, then 3,000 more on both; and a world saved and loaded six
times mid run -- at ticks 1, 700, 701, 1,900, 3,333 and 4,000, each
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
- An entity whose wake has passed when it is put back -- kept while
  its superchunk was cold -- wakes the tick it is put back.

Measured: 16 superchunks, 64,000 sheep: 24 MiB -- 1 MiB a superchunk of
heights, raw, 250 KiB of layers, 260 KiB of entities.

Not yet: superchunks no longer in the world are not removed from a
save's folder.

## Layout

| folder | what is in it |
|---|---|
| `src/lib.rs` | generate, save, load |
| `src/halos.rs` | the hot superchunks: the halos about the entities that keep one |
| `src/tick.rs` | the tick of every rule and entity, then the halos moved |
| `src/diagnostics/` | grass, and grass and sheep, ticked flat out and measured; frames; the diagnostics tool |
| `src/transient_data.rs` | where runs leave what they make, out of git |
| `tests/` | the halos follow their keepers, and a superchunk gone cold comes back as it was; a world loaded goes on as the one saved; the files; refusals |
| `docs/` | this, and the reference, function by function |
