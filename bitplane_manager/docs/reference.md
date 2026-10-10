# The bitplane manager, function by function

The design is in `bitplane_manager.md`.

## `lib.rs`

**`BucketKey`** `{layer_type, chunk}`: which bitmap. **`NotHot`**: a
cell asked of a bitmap not hot.

**`ChunkSet`** (`u16`, a bit a chunk), **`contains`**, **`put`**,
**`members`**.

`COUNT_TILE_WORDS` (16), `COUNT_TILES_IN_CHUNK` (64),
`COUNT_TILE_CELLS` (1,024: 32x32); `COARSEST_SCALE` (6),
`COARSEST_TILES_IN_CHUNK` (16), `COUNTS_IN_COARSEST` (4: the count
tiles in a tile of the coarsest scale, checked as the crate is built).
`WIDEST` (16): the most bits a cell a layer has; `NO_CELLS`: a bucket
with no cell set, as wide as any -- what a chunk with no bucket is
read as, so a read needs no branch on whether there is one.

## `superchunk_layer.rs`

**`ChunkFlags`**: a layer's four chunk sets, packed in 8 bytes.

**`SuperchunkLayer`**: an allocation, one layer type over one
superchunk -- its owned block, flags, counts less one, count tiles'
counts, hot count. **`count`** / **`set_count`** a bucket's set cells;
**`cells`** a bucket of a bit a cell as a bitmap's words, **`words`** / **`words_mut`** a bucket's words at any width; **`get`** a cell by
Morton index; **`value`** and **`put_value`** the number of a wide layer's cell, read and put whole; **`put_cell`** a cell set or clear if not already, the
bucket dirty and the counts moved by one: whether it changed.
`bucket_words`: the words a bucket takes, a bitmap's times the bits a
cell. `let_bucket_go(chunk)`: a chunk's bucket given up, the chunk
then having no cell set; `let_unused_go`: the same of every chunk
neither hot nor waiting in the ring. `value_of(chunk, cell, bits)`:
`value` with the bits a cell given, for where the plane's width is
known from its type.


## `superchunk.rs`

**`Superchunk`** `{index, layers, on_their_way}`: one superchunk,
owning its allocations, and its write-backs taken not yet in the ring. **`index`**, **`layer(type)`** -- a
**`LayerView`** (**`hot_count`**, **`is_hot(chunk)`**,
**`count(chunk)`**, **`cells(chunk)`**, **`tile_counts(chunk)`** -- the
set cells of each of its `COUNT_TILES_IN_CHUNK` count tiles of
`COUNT_TILE_WORDS` words) -- and **`apply(type, write, applied)`**, the
write's part in it. Private: **`layer_index`**.


**`Bucket`**: a hot bitmap to read: **`count`**, **`get(place)`**,
**`cells`**.


## `reader.rs`

**`Reader::new(superchunks)`**: **`holds(type, cell)`**,
**`window(type, origin, width, height)`** -- a **`Window`** `{set, hot}`,
up to 8x8 cells row by row from `origin`, bit `y * 8 + x` --
**`windows(types, ...)`**, the same of several types at once, where it
lies worked out once --
**`any_in_tile(type, cell, scale)`**: whether any cell is set of the
tile of `scale`, `2^scale` cells a side (to `COARSEST_SCALE`, 6), `cell`
is in; **`tiles_holding(type, cell)`**: which of its chunk's 16 tiles of
the coarsest scale (`COARSEST_TILES_IN_CHUNK`, 64x64 cells, four count
tiles each) hold any, a bit each, off the counts --
**`superchunk(superchunk)`**, remembering the last superchunk.
**`prefetch(type, cell)`**: asks memory for the word `cell` is in ahead
of its being read; nothing if its bitmap is not hot.

**`Lookup`**: lookups remembering the last superchunk; one a thread. **`superchunk`** an entry by superchunk
index; **`find`** an allocation by type and superchunk index, as a
`LayerAt` (entry, index among its allocations); **`holds`** a cell, from
its index's fields; **`windows`** up to 8x8 cells from the one to four
word tiles they overlap -- the **`bucket`** looked up once, word tiles
in it by index (`WORD_TILE_X`, `WORD_TILE_Y`, **`word_tile`**), one
across its edge looked up again (**`word_tile_at`**); **`any_in_tile`**,
**`tiles_holding`**; **`forget`** when the directory changes shape.

## `arena.rs`, `making_hot.rs`, `write_back.rs`

The arena, its one type's methods over three files: `arena.rs` the
directory and what is read off it, `making_hot.rs` bitmaps made hot and
cold and the lingering superchunks, `write_back.rs` dirty buckets
taken, put in the ring, flushed and evicted.

**`Lingering`** `{superchunk, wanted}`: a superchunk gone cold, its
allocations kept; **`done`**, whether it can be let go.

**`BitmapArena`**: **`new`**; **`len`**, **`is_empty`**, **`allocations`**;
**`is_hot`**, **`bucket`**, **`holds(type, cell)`**,
**`superchunk_count(type, superchunk)`**; **`make_hot(key, layer,
codec)`** -- a bucket waiting in the ring made hot as it is, else
decoded or emptied, counted -- and **`make_hot_cells(key, cells)`**,
the cells given, both through **`make_hot_with`**;
**`make_hot_layers(chunk, types, storage, codec)`** and
**`make_hot_superchunk(superchunk, types, storage, codec)`**, every
chunk of it; **`make_cold_superchunk(superchunk)`** -- its dirty
buckets taken and returned, its allocations lingering -- **`hold`**,
**`let_go`**, **`make_hot_again`**, **`lingers(superchunk)`**, **`lingering`** (how many);
**`run(type)`** and **`keys`**, in Morton order; **`superchunks`** /
**`superchunks_mut`**, for the simulation, and
**`superchunk_indices`**, theirs; **`take_dirty(superchunk)`** -- the
dirty buckets' cells copied out, marked clean, a write-back on its way
-- **`written_back(superchunk, keys)`** -- the write-back in the ring:
each bucket marked waiting, held until flushed -- and
**`write_back(superchunk, storage, codec)`**, both at once, encoded and
put in the ring here, flushes reported as they come;
**`flushed`**; **`evict(key)`** -- an allocation with nothing hot or
waiting released. Private: **`allocation`** (found or
made), **`hot`**, **`at`**/**`at_mut`**, **`entry_mut`** (hot or
lingering), **`lingering_at`**, **`layers`**, **`layers_of`**,
**`leave_ring`**, **`release_unused`** -- lingering superchunks done with
let go too.

## `writes.rs`

`CHUNK_SIDE_U32`: a chunk's side as a coordinate, for the shapes.

**`WriteOp`**, **`Shape`**, **`Write`** `{at, op, shape}` (packed, 12
bytes); **`Write::cell(at, op)`**; **`bounds`** and **`covers`**: a
shape's cartesian rectangle and its cells; **`superchunks`**
(public, for routing): the superchunks a write lands in.

**`WritesApplied`** `{writes, changed, missed}`, added with `+=`.

**`WriteQueues`**: a queue a layer type, sorted by type, found by a
search of the few:
**`push`**, **`len`**, **`is_empty`**, **`iter`**, **`clear`**.

**`count_missed(superchunk, write, applied)`**: a write's cells in a
superchunk with no bitmap in use, counted missed.

**`apply_in(layers, superchunk, type, write, applied)`**: the part of a
write in one superchunk applied to its layers -- a cell from its index,
a shape chunk by chunk, cells in bitmaps not hot counted missed.

**`BitmapArena::queue`**, **`queued`**, **`apply`**: writes from outside
a tick, applied in order over every superchunk they land in.

## `diagnostics/arena.rs`

**`ArenaStats::of(arena)`**: superchunks, allocations, hot bitmaps, the
buckets kept and their bytes.

## `transient_data.rs`

`TRANSIENT_DATA`: the crate's `transient_data/` folder.
**`measurements()`**, **`publish(report)`**: as in every crate.

Wide planes: **`Reader::value(plane, cell)`** and **`BitmapArena::value`** read a cell's number; **`Write::value(plane, at, value)`** puts one (`WriteOp::Put`); **`Bucket::words()`** is a bucket's words at any width. `take_dirty` and `make_hot_cells` carry a bucket as a bitmap's words times its bits a cell; `written_back` takes a wide layer's planes' keys.
