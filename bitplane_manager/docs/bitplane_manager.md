# The bitplane manager

The hot bitplanes: the layers whose cells are being read and changed,
decoded raw into the bitmap arena, and the one place with cells to read
and change. The decisions behind it are in `../../docs/civil_egregore.md`,
"From the disk to the cells".

## The arena

Allocations, one a layer type over a superchunk: an array of buckets
-- a 256x256 bitmap each -- one for each of its chunks that has a cell
set, 16 at the most, in their Morton order. A chunk with no cell set
has no bucket and reads clear; the first cell set in it makes one, put
in its place among the others (`SuperchunkLayer::keep`), so buckets
after it move -- a superchunk owns its own, and changes them alone. A
bucket is found by how many chunks before it have one: a count of bits
(`start`), no search. One is let go when its chunk is neither hot nor
waiting in the ring. Each
allocation keeps four 16-bit chunk sets packed in 8 bytes (hot, dirty,
waiting in the ring, non-empty), each bucket's count of set cells (a
`u16` less one -- a stored layer has 1 to 65,536 -- beside the
non-empty bit, as a hot bucket may be empty), the set cells of each
count tile of each bucket (`COUNT_TILE_WORDS`, 16 words: 32x32 cells,
64 a bucket), and the set cells of its hot buckets together: the
weights sampling picks by, and what it passes over a bitmap by.

**The directory**: the superchunks in use, sorted by superchunk index
(kept beside each), each with its allocations sorted by layer type.
It is the one thing ever sorted, and it holds no bitmaps: the
allocations lie wherever they were made, each one run of memory in
Morton order. A lookup remembers the last superchunk found (`Lookup`,
one a thread), so the runs of lookups in one superchunk that
Morton-ordered work makes -- a rule reading grass and dirt by turns --
search only that superchunk's few types. (A hashed cache of 16
superchunks and types was measured and removed: `../../docs/style_guide.md`,
"One representation for one thing".) The arena grows an allocation at a
time, as a layer type turns hot over a superchunk it had none of, and
an allocation a bucket at a time; an allocation none of whose chunks is
hot or waiting in the ring leaves the directory, its buckets with it. Each superchunk owns its buckets, so
superchunks are changed apart.

**Windows**: up to 8x8 cells at any cell read at once
(`Reader::window`), as a `Window` -- two masks, row by row: the cells set,
and the cells in hot bitmaps. A window overlaps one to four word
tiles, 8x8 cells a bitmap word each (`bitmap::window`); only those it
reaches are read. Its bucket is looked up once, the word tiles beside
and below stepped to on the word tile's index in the chunk, and only
one across the chunk's edge looked up again. Read of several layer
types at once (`Reader::windows`), where the window lies among the word
tiles is worked out once for all of them. So the 3x3 cells around a
cell are one lookup, a word or two, and a few shifts and masks.

**The far search**: whether a layer holds at any cell of a tile of a
given scale -- `2^scale` cells a side, up to `COARSEST_SCALE` (6) --
read off the count tiles' counts where they are 0, and off the words
only where they are not (`Reader::any_in_tile`); and which of a chunk's
16 tiles of the coarsest scale hold any, off the counts alone
(`Reader::tiles_holding`).

## Making hot, writing back, evicting

A bitmap is made hot decoded from chunk storage's cold pool, or empty:
a type with no encoded layer in the chunk is a bitmap with no cell set.
A bitmap already hot is left as it is, changes and all.
A changed bucket is dirty; writing back encodes it into storage's ring
-- no words where no cell is set -- and keeps it, waiting in the ring,
until storage flushes its superchunk: evicted and made hot again before
then, it is the bucket as it was, not decoded -- the ring is never
read to make a bitmap hot. A dirty bucket must be written back
before it is evicted.

Writing back is in two halves, so the slow one -- encoding -- can be
done off the tick: the dirty buckets are taken (`take_dirty`: their
cells copied out, the buckets marked clean, one write-back more on its
way), and, encoded wherever, put into the ring by its caller and marked
waiting (`written_back`), in the order taken. `write_back` does both at
once. A bucket waiting is held until storage says its superchunk's
image holds every change of it (`flushed`), however long the flush
takes, wherever it is done.

## Lingering

A superchunk is made cold as a whole at once (`make_cold_superchunk`):
nothing encoded or flushed, its dirty buckets taken and handed back to
be encoded, and its allocations set aside as they are, **lingering** --
no longer hot, so the simulation neither reads nor ticks it, writes to
it are missed, and the directory stays the hot superchunks exactly.
It is let go, its buckets with it, once storage holds its
changes: none on its way, none waiting in the ring. Wanted hot again
before then, it is held (`hold`) and made hot as it is, nothing decoded
(`make_hot_again`); no longer wanted, let go (`let_go`). A superchunk
made hot any other way has its bitmaps' cells decoded from storage, or
given decoded (`make_hot_cells`) -- off the tick, say.

## Wide planes

A layer type is a bit a cell, or wide: 2, 4, 8 or 16 bits a cell
(`chunk_storage::Wide<W>`, the width `W` -- `Bits2` to `Bits16` -- in
the plane's type). A wide plane's bucket holds a cell's whole number
together, the cell at place `p` at bit `p` times the width: a tree's
stage is one read and one write, where four bitmaps took four of each,
in four places in memory. Its buckets are as many times a bitmap's
words as it has bits. Its counts are of the cells whose number is not 0.

The width is in the type so that it is known where the plane is read:
`value(plane, cell)` shifts and masks by constants, a number of one
width cannot be read as another, and a wide plane cannot be asked
whether a cell is "set".

Cold, a wide plane is as many layers of a bit a cell, the bit `b` of
every cell under the layer type `first + b`, each encoded by Tessera
as any bitmap is (`chunk_storage::wide`): storage, saves and the codec
know bitmaps only. The planes are put together when a bucket turns hot
and taken apart when it is written back, off the tick, passing over
the cells set alone.

## Writes

The only way cells change. A write is 16 bytes: its anchor cell's
Morton index, an operation (set, unset, flip, or put a number, for a
wide plane) and a shape (the cell, a
rectangle up to 255 a side, a disc up to radius 255); the layer type is
its queue's. Queued writes change nothing until applied, and apply in
order, the latest winning. A cell write finds its bit from the index
alone; shapes are laid out in cartesian coordinates, the cheaper for
geometry (`CHUNK_SIDE_U32`: a chunk's side as a coordinate). The
index's 8 bytes, the operation and the shape -- its sides and radius a
byte each -- are packed to 4-byte alignment and come to 16; a larger
area is several writes. The queues are a queue a layer type
(`WriteQueues`), sorted by type: a rule writes to few types, so finding
one's queue is a search of a few.

A write here asks nothing of the cell it lands on. One that is to be
applied only if the cell is still as a rule saw it is the
simulation's (`../../simulation/docs/simulation.md`, "Compare-and-write
and groups"): it reads the cell as it is now (`Superchunk::value_at`),
applies a plain write if that is what was seen, and counts the write
refused otherwise (`WritesApplied::refused`).

## What the simulation reads and changes

Sampling and the tick are the simulation's (`../../simulation/`). It
reads and changes the arena only through narrow handles:
`BitmapArena::superchunks` (to read, from any thread) and
`superchunks_mut` (to change, each apart); a superchunk's
`LayerView` -- its hot buckets, counts and cells, what sampling finds
cells by -- and `apply`, a write's part in that superchunk; and a
`Reader` -- cell and window reads remembering the last lookups, one a
thread.

## Layout

| folder | what is in it |
|---|---|
| `src/lib.rs` | the crate's modules, the chunk sets, the count tiles' sizes |
| `src/superchunk_layer.rs` | an allocation: one layer type over one superchunk, its buckets and counts |
| `src/superchunk.rs` | a superchunk and its layers as the simulation is handed them |
| `src/reader.rs` | cells read wherever they lie: the reader, its windows, the lookups |
| `src/arena.rs` | the arena: the directory and what is read off it |
| `src/making_hot.rs` | bitmaps made hot and cold, superchunks lingering |
| `src/write_back.rs` | dirty buckets written back, flushed and evicted |
| `src/writes.rs` | writes, their queues, and applying them to a superchunk |
| `src/diagnostics/` | what the arena holds, gathered |
| `src/transient_data.rs` | where runs leave what they make, out of git |
| `tests/` | the arena and writes, judged |
| `docs/` | this, and the reference, function by function |
