# The world

The world as a whole: made from a seed, ticked, saved and loaded. It is
the one crate that holds the hot bitmaps, the stored superchunks, the
entities and the simulation together; the program (`../src/main.rs`)
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

Only the superchunks about the entities that matter are hot. An
entity of a kind that keeps a halo (`HALO_KEEPERS`: people, to come;
for now the sheep stand in) keeps its superchunk and the eight about
it hot -- as far as anything reaches in a tick, the speed of light.
Every other superchunk is cold: its cells in its image in chunk
storage, its entities and random numbers kept as a save keeps them
(`World::cold`). After every tick the halos move to where their
keepers stand (`World::move_halos`), and nothing slow is done on the
tick -- encoding, generating and decoding are the background's
(`src/background.rs`): threads, one a core, each with its own codec,
asleep while there is nothing to do.

- **Going cold** takes `COOL_TICKS` (256) ticks: a hot superchunk no
  halo reaches is **cooling**, hot still -- ticked, read and written
  like any other -- so a keeper stepping back and forth over an edge
  does not make the superchunks behind it flicker cold and hot. A halo
  reaching it again, it just stays hot. At the tick it is due it goes
  cold: its state kept, and its bitmaps set aside, lingering
  (`BitmapArena::make_cold_superchunk`); its changed bitmaps, copied
  out, are encoded in the background and put into the writeback ring
  in the order they went cold. The ring flushes them when it needs the
  room -- not when the superchunk goes cold -- and that too in the
  background: the changes taken out of the ring, and the image
  rewritten with them on another thread (`Job::Flush`), a superchunk's
  flushes one after another. The newest cells are always on the hot
  side: its bitmaps, hot or lingering, are held until the image holding
  their changes is in the cold pool, and only then let go.
- **Coming hot** takes `WARM_TICKS` (256) ticks: the superchunk is
  **warming**. Lingering still, it is held, to be made hot as it is,
  nothing decoded; else its image -- generated first, if it was never
  made -- is decoded in the background. It turns hot at the tick it
  is due, its entities put back and its random numbers taken up,
  waiting for the background if it is not done: so the world is the
  same however fast the background is. Until then, to the simulation,
  it is a cold superchunk like any other: not ticked or read, writes
  to it missed, entities sent to it staying where they stood, its own
  entities and random numbers kept cold. A superchunk lingering is
  cold the same way: lingering is only the arena keeping its bitmaps.
  Cooling, warming and lingering are the world's bookkeeping, not the
  simulation's: to it one cooling is hot, one warming or lingering
  cold. A keeper reaches a superchunk its halo has just reached no sooner than
  it crosses its own -- 1,024 cells, a step every 64 ticks or more --
  so long after it has turned hot. A superchunk no halo wants any more
  stops warming. 256 ticks is short of what generating a superchunk
  takes the background (about 120 ms, against some 40 ms of ticks), so
  the tick waits for a superchunk generated: a stall taken for halos
  that follow their keepers closely.

So between ticks the superchunks hot and not cooling, or warming, are
the halos, exactly, never both (`tests/fast/halos.rs`); a superchunk
warming takes no write and no entity until it is due; one cooling
stays hot until it is due, and for good if a halo reaches it again
before; and a superchunk gone
cold comes back as it was, to the cell, the entity and the random
number.

Passive rules tick only where hot: grass grows on every hot superchunk,
so the flock, its halos and the world may grow as far as it leads them.
An entity
kept cold whose wake passes wakes the tick its superchunk turns hot.

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
| `hot` | text: the hot superchunks, a line each, those cooling with the tick each goes cold at, and the warming ones with the tick each turns hot at |
| `superchunks/<index>.image` | a superchunk's cells and heights: its image, as the cold pool holds it |
| `superchunks/<index>.state` | its random stream's state, its entities with their attributes |

`<index>` is the superchunk index, 44 bits, 11 hexadecimal
digits: its name, and nothing else is.

**Saving** is between two ticks. Every dirty bitmap is written back --
encoded on all the background's threads, after every write-back still
on its way -- and the ring flushed, so the cold pool's images are the
world's cells; each image is written as it is, and beside it the
superchunk's state; then the hot file. The world's file is written
last, each file beside itself and then
renamed, so a save cut short leaves the one before readable.

**Loading** reads every image into the cold pool and keeps every
state -- each read whole, so a bad one is refused now -- as a cold
superchunk's. Then, before it ticks, it makes hot the superchunks the
hot file names, decoded on all the background's threads: every layer
type of the world made hot on every chunk of them -- a type with no
cell left on a superchunk has no stored layer, and must be hot all the
same to be written to -- their entities put back at the world's tick,
their random numbers taken up. The cooling ones cool again, and the
warming ones warm again, each to turn at the tick it was to.

**A world loaded goes on as the one saved would have**, to the cell,
the entity and the random number (`tests/fast/world.rs`: 1,500 ticks,
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
  not when the background is done, and cold at a tick fixed when the
  halos leave it; the hot file keeps both.
- An entity whose wake has passed when it is put back -- kept while
  its superchunk was cold -- wakes the tick it is put back.

Most of a save is heights, kept raw, about a byte a cell; the layers
and the entities are each a fraction of that (`tilesim world new`
says the bytes written).

Not yet: superchunks no longer in the world are not removed from a
save's folder.

## Layout

| folder | what is in it |
|---|---|
| `src/lib.rs` | generate, save, load |
| `src/halos.rs` | the hot superchunks: the halos about the entities that keep one, cooling and warming |
| `src/background.rs` | the threads encoding, generating and decoding off the tick |
| `src/tick.rs` | the tick of every rule and entity, then the halos moved |
| `src/patches.rs` | how grass and trees lie in patches when a superchunk is made |
| `src/diagnostics/` | grass, and grass and sheep, ticked flat out and measured; the diagnostics tools |
| `src/transient_data.rs` | where runs leave what they make, out of git |
| `tests/` | the halos follow their keepers; a superchunk warming takes nothing until due; one cooling stays hot until due; a superchunk gone cold comes back as it was; a world loaded goes on as the one saved; the files; refusals |
| `docs/` | this, and the reference, function by function |
