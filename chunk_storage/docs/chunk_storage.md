# Chunk storage

The world's chunks as stored: what loading and saving work on, and what
the bitplane manager decodes from and writes back to. The decisions
behind it are in `../../docs/civil_egregore.md`, "The world".

Coordinates -- cells, chunks, superchunks, their Morton indices -- are
the `coordinates` crate's (`../../coordinates/`).

## Layers and their codec

A layer is a type (`LayerType`, a `u64` naming what it represents) and
a bitmap, Tessera-encoded in 64-bit words. A Tessera stream ends
itself, so no length is kept: `LayerCodec::decode` reads from a
bitmap's first word whatever follows its last.

`LayerCodec` holds what encoding and decoding need, allocated once: a
Tessera, its stream, and the bitmap it decodes into. Encoding reads a
bitmap's cells from wherever they are held -- an arena's bucket, say --
and decoding writes them there. A bitmap with no cell set has no layer
at all: it encodes to no words.

## Wide planes

A wide plane (`../../type_registry/docs/type_registry.md`, "Width") is
kept two ways. Hot, a cell's number is `bits` bits together, the cell
at place `p` at bit `p * bits` of the bucket's words -- a word holds
whole cells, `bits` being a power of two. Cold, it is `bits` bitmaps, a
bit of every cell each, encoded as any layer. `wide.rs` goes between
the two: `spread` puts a bitmap into a wide bucket as one bit of every
cell, `plane` takes that bit of every cell out as a bitmap. Both pass
over the cells set alone, so a plane mostly clear costs little.

## The superchunk image

A superchunk, in the cold pool and on disk alike, is one run of words, every
part starting on a word:

1. **the chunk table**: each of its 16 chunks' offset, in Morton order;
2. **the height map** (`HeightMap`): a floor of 16 bits for each
   chunk, a mask of the tall chunks, then 1024x1024 bytes, 8 a word, in
   Morton order -- each cell's height over its chunk's floor, each
   chunk's one 64 KiB run; then, for each tall chunk (one whose heights
   span more than 255), its heights whole, 16 bits a cell, 128 KiB;
3. **the water's depths** (`ChunkMaps`): sparse by chunk, since most
   chunks have none -- a word saying which chunks have a map and which
   of those are wide, then the maps in Morton order, 16 at the worst:
   a byte a cell, 64 KiB, or 16 bits a cell, 128 KiB, where a chunk's
   water is deeper than 255. The heights are not kept so: every cell
   has one;
4. **its chunks**, in Morton order, each its layer count, its layer
   table -- a type and an offset per layer, sorted by type, the offset
   from the chunk's start -- and its encoded layers, in no order.

An image is never changed in place: `rewritten` makes a new one with
changes made. `from_words` checks words read back are an image.

In words:

| words | what they hold |
|---|---|
| 16 | the chunk table: each chunk's offset in the image, in Morton order (the height map starts after it, `HEIGHTS_START`) |
| `HEIGHT_WORDS`, and more if a chunk is tall | the height map, raw |
| 1, and a map for each chunk with water | the water's depths |
| the rest (from `CHUNKS_START` at the least) | each chunk in Morton order, its data together: its layer count, its layer table -- a type and an offset a layer (`ENTRY_WORDS`, 2), sorted by type -- then its encoded layers |

An encoded layer's offset counts from its chunk's start, so a chunk
moves whole. Encoded layers start on a word and lie in no particular
order. No length is kept: an encoded layer runs from its offset to the
next offset of its chunk, or the chunk's end; a chunk runs to the next
chunk's offset, the last to the image's end.

### The height map

A `Height` is 16 bits, but a chunk seldom spans more than 255 (`SPAN`)
from its lowest ground to its highest. So each chunk has a **floor** --
its lowest height -- and each of its cells a byte over it. A chunk that
does span more is **tall**, and has a map of its own, a whole height a
cell, kept apart.

| words | what they hold |
|---|---|
| 4 (`FLOOR_WORDS`) | the 16 chunks' floors, in Morton order, 4 a word (`HEIGHTS_IN_WORD`; read by `floor_in`) |
| 1 (`TALL_WORD`) | which chunks are tall, a bit a chunk |
| 131,072 (from `BYTES_START`) | every cell's byte over its chunk's floor, 8 a word (`BYTES_IN_WORD`) |
| 16,384 a tall chunk | the tall chunks' maps, in Morton order: every cell's height, 4 a word |

The cells are laid out in Morton order over the whole superchunk: chunk
by chunk in their Morton order, and in each chunk in the Morton order
its bitmaps use. So each chunk's heights are one run, and any aligned
square of cells is one run of heights, as it is one run of bits in a
layer.

### The water's maps

`ChunkMaps` is a number a cell kept only where there is any -- unlike
the heights, which every cell has.

| words | what they hold |
|---|---|
| 1 | which chunks have a map, a bit a chunk; and, 16 bits up (`WIDE_FROM`), which of those are wide |
| 8,192 a chunk with a map, 16,384 if it is wide | the maps, in Morton order: every cell's number, a byte each, 8 a word -- or 16 bits each, 4 a word, in a wide one |

A chunk is **wide** if any of its numbers is over 255. At worst 16
maps; with none, one word.

## The cold pool and the writeback ring

`ChunkStorage` holds the cold pool -- superchunk images by Morton index,
in memory or, past what it keeps there, on disk ("Paged to disk") -- and the writeback ring (`WritebackRing`) of changed layers, encoded,
tagged with their chunk and type; an entry of no words says the layer
is gone. Writing back appends to the ring, never touching the cold pool. The
ring is a sponge: when an entry does not fit, the superchunk at its
tail is flushed -- its image rewritten once with every entry of it, and
those entries freed -- until it fits. Entries never wrap round the
ring's end, and the ring grows only when empty and still too small.

The ring is never read to make a layer hot: the bitmap arena keeps a
written-back layer until its superchunk is flushed, and is told of
every flush. So a superchunk gone cold is not flushed then: its
changes wait in the ring, flushed when the ring needs the room, or
when the world is saved.

### The ring's words

A ring buffer of words. An entry is a header of three words
(`HEADER_WORDS`) -- its chunk's Morton index in the world, its layer
type, its length and, in the length word's lowest bit (`DEAD`), whether
it is dead -- then the encoded layer's words; an entry of no words says
the layer is gone. Entries never wrap: one that does not fit before the
end starts again at the start, a marker left where it would have gone
(`WRAP`, a first word no chunk's index can be: a chunk's takes 48
bits). Entries are written at the head and freed from the tail:
releasing a superchunk marks its entries dead, and the tail moves past
dead entries. So a superchunk's image is rewritten once for many of its
layers, not once a layer.

## Jobs

The slow work is done off the tick, on the dispatcher's threads
(`../../utilities/docs/utilities.md`, "The dispatcher"): encoding the
changed layers of superchunks gone cold, rewriting images with the
changes flushed from the ring, and generating and decoding the
superchunks warming -- each thread with a codec of its own (`CODEC`, a
thread's). A job sent (`Jobs::send`) is a `Ticket`; what it made is
taken by it (`Jobs::take`, waiting if not yet made, or
`Jobs::try_take`). What a job makes depends on nothing but the job, so
the world is the same however fast the threads are.

## Shared images

A superchunk warming is decoded on another thread, off the tick
(`../../server/`, "Halos"), from its image in the cold pool, while the
tick goes on changing the pool: other superchunks' images put in, and
flushes putting rewritten ones in place of theirs. A thread cannot
just borrow an image from a pool changed meanwhile -- a change may move
it, or free it -- so it would have to copy it: a megabyte or so, about
a millisecond. Instead the pool holds each image behind a reference
count (`Arc`): a handle any number of holders share, the image freed
when the last lets go. The thread is handed a handle
(`ChunkStorage::shared_image`), nothing copied. Since an image is never
changed in place, only replaced (`rewritten`), what the thread reads is
the image as it was when handed over, whatever the pool does next; and
an image replaced while a thread reads it lives on until that thread is
done with it. An image paged to disk is handed over as the folder it
is in, and the thread reads it ("Paged to disk").

## Paged to disk

A world grows for as long as it is walked, and every superchunk ever
made has an image, a megabyte or so: kept all in memory, the cold
pool is what a large world runs out of memory by. So the pool keeps
only so many bytes of images in memory (`ChunkStorage::page_under`,
`paging.rs`); past that, images are **paged out** -- written to a
folder and let go -- and the pool keeps only where each is
(`ChunkStorage::page_out`).

Which go: any but those the caller says are held -- hot, or turning
hot -- and those with changes waiting in the ring, which their flush
needs. They are looked for round the pool from where the last look
stopped, so each waits as long. Which are paged changes nothing the
world holds: an image on disk is read back the same, word for word.

Read back: a superchunk warming is handed to its job as the pool has
it (`Stored`) -- the image, or the folder it is in -- and the job
reads it, off the tick as it decodes it, and hands it back to be in
memory again while it is hot (`ChunkStorage::bring_in`). Anything else
that wants a paged image -- a flush, the world's hash -- reads it where
it is asked for (`shared_image`), and it stays on disk; a save copies
it file to file (`save_image`). `image` and `layer` lend from memory
only, and panic at an image on disk: they are for a hot superchunk's.
An image that does not read back is a panic naming the file: the world
cannot go on without it.

The folder is the pool's own, made under the folder it is given when
the first image goes, laid out as a save's (below), and removed with
the pool. Beside it is a lock file its process holds locked: a folder
left behind by a process that died is known by its lock being free,
and removed by the next pool to make one there.

A world loaded need not be read into memory either: past what the
pool keeps, its images are left in the save and marked as on disk
there (`insert_on_disk`), read when wanted. Saved again to that
folder, such an image is left as it is. So a save a running world was
loaded from is not to be removed under it.

Only images are paged. A cold superchunk's state -- its entities and
random number -- is the server's, small, and stays in memory; and the
pool's list of superchunks does too.

## On disk

A world is a folder (`disk.rs`), its text files CSV (`utilities::csv`).
The names as the code has them: the folder of superchunks `SUPERCHUNKS`;
the world file's columns `WORLD_COLUMNS` and its rows' names `FORMAT`
(with the number written, 2), `HOT_ENTITY`, `CAMERA_FLOCK`,
`WITHOUT_CAMERA_FLOCK`, `GENERATION` (what starts a generation number's
name); the hot file's columns `HOT_COLUMNS` and what a superchunk is in
it, `HOT`, `COOLING`, `WARMING`. The files:
`world.csv`, a row a thing, its name and what it is -- `format`, the
number of the format written; `seed`, in hexadecimal; `tick`;
`layers`; `side`, if the world has a size, and `forced`, 1, if all of
it is hot throughout, which only a world with a side can be; `hot
entity`, the kind of entity it is hot about otherwise, as a number,
if its runner gave one; `camera flock`, if its camera loads
superchunks, the sheep each superchunk generated in the viewport starts with;
`without camera flock`, the superchunks generated out of its view and
still owed theirs, spaces between them;
and a row for each number it is generated by, `generation` before its
name -- under the row `world,is`, which comes first. The other rows
are read by name, in any order, a name given twice refused and one
not known passed over; a row missing is its default -- tick 0, no
layers, no size, not forced, no hot entity or camera flock, the
generation's numbers its defaults -- but the seed, without which the
superchunks not yet made would be unlike those that are; `hot.csv`, which superchunks were hot (`HotSuperchunks`),
a row each under `superchunk,is,until`: its index in hexadecimal, then
`hot`, or `cooling` and the tick it goes cold at, or `warming` and the
tick it turns hot at; and `superchunks/`, two files a superchunk, named by its Morton
index in 11 hexadecimal digits. `.image` is its image, word for word,
checked when read (`from_words`). `.state` is words storage does not
look into: whoever ticks the world keeps there what moves on the
superchunk (`simulation`: its random numbers and entities). Words are
eight bytes, the lowest first. A file is written beside itself and
renamed, so one cut short never replaces a good one. What is saved
when, and how it is read back: `../../server/docs/server.md`.

## Layout

| folder | what is in it |
|---|---|
| `src/` | height map, layer codec, superchunk image, writeback ring, chunk storage and its paging, the world on disk |
| `src/diagnostics/` | what storage holds, gathered |
| `src/transient_data.rs` | where runs leave what they make, out of git |
| `tests/` | every part's behaviour, judged |
| `docs/` | this, and the reference, function by function |
