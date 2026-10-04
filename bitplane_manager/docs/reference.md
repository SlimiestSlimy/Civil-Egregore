# The bitplane manager, function by function

The design is in `bitplane_manager.md`.

## `lib.rs`

**`BucketKey`** `{layer_type, chunk}`: which bitmap. **`NotHot`**: a
cell asked of a bitmap not hot.

**`ChunkSet`** (`u16`, a bit a chunk), **`contains`**, **`put`**,
**`members`**. **`ChunkFlags`**: the four sets, packed in 8 bytes.

**`SuperchunkLayer`**: an allocation, one layer type over one
superchunk -- its owned block, flags, counts less one, count tiles'
counts, hot count. **`count`** / **`set_count`** a bucket's set cells;
**`cells`** / **`cells_mut`** a bucket's words; **`get`** a cell by
Morton index; **`put_cell`** a cell set or clear if not already, the
bucket dirty and the counts moved by one: whether it changed.

**`Superchunk`** `{index, layers, on_their_way}`: one superchunk,
owning its allocations, and its write-backs taken not yet in the ring. **`index`**, **`layer(type)`** -- a
**`LayerView`** (**`hot_count`**, **`is_hot(chunk)`**,
**`count(chunk)`**, **`cells(chunk)`**, **`tile_counts(chunk)`** -- the
set cells of each of its `COUNT_TILES_IN_CHUNK` count tiles of
`COUNT_TILE_WORDS` words) -- and **`apply(type, write, applied)`**, the
write's part in it. Private: **`layer_index`**.

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

**`Lookup`**: lookups remembering the last superchunk; one a thread. **`superchunk`** an entry by superchunk
index; **`find`** an allocation by type and superchunk index, as a
`LayerAt` (entry, index among its allocations); **`holds`** a cell, from
its index's fields; **`windows`** up to 8x8 cells from the one to four
word tiles they overlap -- the **`bucket`** looked up once, word tiles
in it by index (`WORD_TILE_X`, `WORD_TILE_Y`, **`word_tile`**), one
across its edge looked up again (**`word_tile_at`**); **`any_in_tile`**,
**`tiles_holding`**; **`forget`** when the directory changes shape.

**`Bucket`**: a hot bitmap to read: **`count`**, **`get(place)`**,
**`cells`**.

**`Cooling`** `{superchunk, wanted}`: a superchunk gone cold, its
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
buckets taken and returned, its allocations cooling -- **`hold`**,
**`let_go`**, **`make_hot_again`**, **`cooling`** (how many);
**`run(type)`** and **`keys`**, in Morton order; **`superchunks`** /
**`superchunks_mut`**, for the simulation, and
**`superchunk_indices`**, theirs; **`take_dirty(superchunk)`** -- the
dirty buckets' cells copied out, marked clean, a write-back on its way
-- **`written_back(superchunk, encoded, storage)`** -- into the ring,
each marked waiting, flushes reported as they come -- and
**`write_back(superchunk, storage, codec)`**, both at once;
**`flushed`**; **`evict(key)`** -- an allocation with nothing hot or
waiting released to the block pool. Private: **`allocation`** (found or
made), **`hot`**, **`at`**/**`at_mut`**, **`entry_mut`** (hot or
cooling), **`cooling_at`**, **`layers`**, **`layers_of`**,
**`leave_ring`**, **`release_unused`** -- cooling superchunks done with
let go too.

## `writes.rs`

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
block pool's stats; **`bytes_in_use`**.

## `transient_data.rs`

**`measurements()`**, **`publish(report)`**: as in every crate.
