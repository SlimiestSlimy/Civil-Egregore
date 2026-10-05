# What TileSim costs, and what was found measuring it

What measuring has shown, and the command that shows it. Figures are
written only where they explain an optimization -- what it was before
and after, on a Ryzen 5 5600 (6 cores, 12 threads) -- and nowhere else:
a figure is one machine's on one day, and the latest are where the
tools keep them (`<crate>/transient_data/measurements/`). The
reasons behind each finding are in the design it belongs to, named
beside it. How things are measured: `testing_protocol.md`.

## Where memory takes over from the processor

`tilesim server pasture <ticks> 333 1000 <superchunks>`, the superchunks
stepped from 16 to 1,024: grass and sheep, every thread. A sample and a
wake are the time of the thread doing them.

- **While the world fits the caches the processor is the limit**: a
  sample and a wake cost the same however many superchunks there are,
  and a few superchunks are too few to keep every thread busy.
- **Past that memory takes over**: the work done a second peaks, and
  what each sample and wake costs starts to climb -- every one is a
  line of memory not in the caches.
- **In a large world it is all memory**: a sample and a wake cost about
  twice what they did and level off; nothing read is in a cache any
  more, and more superchunks cost no more each.

So the tick's cost in a large world is what it waits for, not what it
computes: what helps there is asking memory ahead (below), and touching
fewer lines a sample and a wake.

## The build's target

`.cargo/config.toml` builds for `x86-64-v3`: TileSim is a game, played
on many machines, so not for the processor building it. Two things were
found on the way:

- **The popcount instruction matters.** A generic build counts bits in
  software, and the sampler (`sample_layer`) spends most of its time
  counting a count tile's words: `x86-64-v2`, which has the
  instruction, is clearly faster than generic and as fast as `native`.
- **`x86-64-v3` over `v2` changes nothing yet**: the two are within
  each other's spread on `tilesim server pasture`. It is the target for
  the bit instructions (BMI1, BMI2) and AVX2, which nothing uses so far.

The popcount, generic against `native`:

`tilesim server pasture 20000 333 1000 <superchunks> 12`:

| superchunks | build | ticks a second | a grass sample, ns |
|---|---|---|---|
| 64 | generic | 18,750 | 227 |
| 64 | native | 20,540 | 172 |
| 400 | generic | 3,144 | 426 |
| 400 | native | 3,444 | 367 |

A tenth more ticks a second at both sizes, and the same world to the
cell: the two builds end 20,000 ticks with the same flock and grass.


Tried and not kept: asking memory ahead for the dirt beside each sample
and for the word each write lands in. Nothing gained: the sample's cost
is in finding it, not in what the rule reads after.

## Smaller count tiles

What was left in a sample at scale was the walk itself: up to 64 words
-- eight lines of memory -- to find the chosen cell in its count tile.
The counts are now kept of count tiles of 16 words
(`COUNT_TILE_WORDS`), 32x32 cells:
128 bytes of counts a bucket where there were 32, and a walk of two
lines at most.

`tilesim server pasture <ticks> 333 1000 <superchunks> 12`, both built
native, the same world to the cell:

| superchunks | count tiles of | ticks a second | a grass sample, ns | a sheep's wake, ns |
|---|---|---|---|---|
| 64 | 64 words | 19,356 and 19,010 | 173 | 557 |
| 64 | 16 words | 20,263 and 19,841 | 179 | 544 |
| 144 | 64 words | 10,802 and 10,808 | | |
| 144 | 16 words | 10,407 and 10,394 | | |
| 400 | 64 words | 3,381 | 369 | 910 |
| 400 | 16 words | 3,671 | 300 | 809 |
| 1,024 | 64 words | 1,263 | 470 | 1,205 |
| 1,024 | 16 words | 1,443 | 350 | 1,068 |

Nothing either way while the world fits the caches -- 4% slower at 144
-- and 9% more ticks a second at 400 superchunks, 14% at 1,024, a
quarter off a sample: it helps where memory is the limit, which is
where a large world is. The far search's tiles of 64x64 cells are now
four count tiles each.

## What a tick is made of

Profiled in the renderer (`perf record -p`, flat out, at the flock's
peak): most of a tick is the woken sheep -- its record, its attributes,
the cells about it, putting it back -- then sampling grass; pathfinding
is very little. Woken entities are asked of memory ahead: a wake 271 ns where it was
359 (`simulation/docs/simulation.md`, "Woken entities are asked of memory
ahead").

## The cells about a woken sheep, asked for ahead

`Turn::woken_reading(layers)`: as woken entities are asked of
memory ahead, so are the cells about them, of the layers the rule says
it reads -- the sheep's grass and the four walls.

| world | before | after |
|---|---|---|
| generated, 64 superchunks, walls (`tilesim server run <dir> 30000`) | 13,236 ticks a second | 13,964 |
| mock, 400 superchunks, no walls (`tilesim server pasture 20000 333 4000 400 12`) | a wake 695 ns, 2,381 ticks a second | 645 ns, 2,375 |
| mock, 64 superchunks, no walls | a wake 439 ns, 15,131 ticks a second | 477 ns, 14,566 |

Kept for the world that is played: 5% more ticks a second with walls
to read. On the mock, which has no walls, asking for the wall layers,
four then, that are not there is pure cost at 64 superchunks. The
walls are two layers now (`worldgen/docs/worldgen.md`).

## Terrain

A generated world, whose walls every hungry sheep reads
(`tilesim server new <folder> Perf 1 64`, then `tilesim server run
<folder> <ticks>`), runs somewhat slower than the mock world of the same
size without them (`tilesim server pasture`).

## Saves

A save's size is mostly its entities and its heights: the heights are
kept raw, about a byte a cell.

## Inlining is fragile: hot lookups are marked

Whether the compiler inlines a small function into its caller in
another crate is its own call, and moves with changes nowhere near it.
Drawing a random number through `utilities::hash::mix`, not the same
four lines written out, made the entity bucket's lookups
(`Bucket::find`, `get`) stop being inlined into the sheep's rule: the
tick's instructions (`pasture 300 333 4000 4 1`, the ticks alone) went
from 67.88 million to 69.61, the world's every result unchanged. The
lookups on the hot path are marked `#[inline]` now -- `Bucket::get`,
`index_of`, `find`, `entity`, `SuperchunkEntities::get`,
`Rng::draw` -- and the tick takes 67.78 million. A change that moves
the count with no change of work is looked for there first.

## Under full load: every superchunk hot

In the renderer, every superchunk shown forced hot and grass growing
everywhere (`cargo run --release -p renderer -- <superchunks> 1000 0 0
1`), flat out: the rates are from the census, over the lines whose pace
is 0. The flock grows as the run goes, and the ticks a second fall as
it does. A world of 256 x 256 superchunks cannot be held hot at all:
it does not fit in memory.

## Superchunks claimed, not dealt out

A tick used to deal the superchunks out, a fixed contiguous run a
thread; the threads with light runs then waited for the heaviest, and
12 threads were 41% busy flat out. Now each thread claims the next
superchunk not yet claimed. Measured in the renderer, 256 superchunks
shown (324 hot with their halos' rim), 8,000 sheep on each at the
start, flat out (`cargo run --release -p renderer -- 256 8000 0`):

| ticks | sheep | dealt out | claimed |
|---|---|---|---|
| 4,000 to 32,000 | 2.1 to 3.4 million | 966 a second | 1,338 a second |
| 32,000 to 44,000 | 3.4 to 3.8 million | 527 a second | 564 a second |

Workers watching for the next job a moment before parking, instead of
parking at once, was tried on the same run and gained nothing: not kept.

## Elsewhere

| what | where |
|---|---|
| the far search for grass | `simulation/docs/simulation.md`, "What a rule is given" |
| the instructions of the apply phase | the same |
| sampling, and count tiles | `tilesim.md`, "Sampling rarely, and count tiles" |
| the flock's balance over a long run | `tilesim.md`, "Sheep leave thin pasture" |
| Tessera's sizes and times | `tessera/transient_data/measurements/`, by `tilesim tessera <tool>` |
