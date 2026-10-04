# Chunk storage, function by function

The design is in `chunk_storage.md`.

## `height_map.rs`

**`HeightMap`**: a superchunk's heights, 8 a word, Morton order over the
whole superchunk. **`filled`**, **`from_words`**, **`get`**/**`set`**
by chunk place and cell place, **`words`**. **`height_in(words, chunk,
cell)`**: one height from an image's words. `HEIGHT_WORDS`: a height
map's words.

## `layer_codec.rs`

**`LayerType(u64)`**: what a layer represents.

**`LayerCodec`**: Tessera and its buffers, allocated once.
**`encode(cells)`**: the bitmap's stream, as words, until the next
encoding. **`encode_layer(cells)`**: as a layer's words -- encoded, or
none where no cell is set. **`decode(words, cells)`**: the bitmap whose stream starts at
`words`, at most `MOST_WORDS` of them read.

## `superchunk_image.rs`

**`SuperchunkImage::new(heights)`**: no layers. **`from_words`**: words
checked to be an image (**`check_chunk`** each chunk: a table that fits,
types sorted one a type, offsets inside the chunk and apart), else
**`InvalidImage`**. **`words`**, **`height_words`**, **`height`**.
**`layer(chunk, type)`**: an encoded layer's words, from its first to its
chunk's end. **`layer_types(chunk)`**. **`rewritten(changes)`**: a new
image with **`LayerChange`**s made in order, a later one to a layer
replacing an earlier, no words removing the layer; built by **`build`**
from each chunk's **`exact_layers`** -- each encoded layer's words to the next
offset, offsets sorted. **`layer_table(chunk)`**: a chunk's layer table.

## `writeback_ring.rs`

**`WritebackRing::new(capacity)`**, **`capacity`**, **`is_empty`**.
**`push(chunk, type, encoded)`**: an entry, if it fits (**`room_for`**:
at the head, or at the start past a wrap marker); whether it did.
**`grow(capacity)`**: an empty ring made bigger. **`tail_superchunk`**:
the oldest live entry's superchunk. **`entries_of(superchunk)`**: its
live entries, oldest first, as **`RingEntry`**s; **`encoded(entry)`**:
an entry's words. **`release(superchunk)`**: its entries marked dead,
the tail moved past the dead at it. **`all_entries`**, **`entry_start`**:
walking entries from the tail, over wrap markers.

## `chunk_storage.rs`

**`ChunkStorage::new(ring_words)`**. **`insert(superchunk, image)`**,
**`image(superchunk)`**, **`shared_image(superchunk)`** -- a handle to
it, to read on another thread -- **`superchunks()`**, **`layer(chunk,
type)`**: the cold pool, its images shared (`Arc`).
**`try_write_back(chunk, type, encoded)`**: into the ring if it fits --
an empty ring too small grown -- whether it went in.
**`write_back(chunk, type, encoded, flushed)`**: the same, the
superchunk at its tail flushed here until it fits, each added to
`flushed` -- before the encoded layer went in. **`take(superchunk)`**:
its ring entries copied out and freed, with its image as it was -- a
**`Flush`** `{superchunk, image, changes}`, to be done on any thread
(**`Flush::rewritten`**: the image with every change made; a
superchunk not stored made flat); **`tail_superchunk`**,
**`take_tail`**, **`holds_changes(superchunk)`**.
**`flush(superchunk)`**: taken and rewritten here. **`flush_all`**,
**`nothing_to_flush`**.

## `disk.rs`

**`WorldInfo`** `{name, seed, tick, layers}`; **`DiskError`**: `Io(path,
error)` or `Invalid(path, what)`. **`write_world(folder, info)`**,
**`read_world(folder)`**; **`write_image(folder, superchunk,
image)`**, **`read_image`**; **`write_state(folder, superchunk,
words)`**, **`read_state`** -- the words, and the file's path;
**`HotSuperchunks`** `{hot, cooling, warming}`, **`write_hot(folder, hot)`**,
**`read_hot(folder)`**: the hot file;
**`saved_superchunks(folder)`**: those with an image, in Morton order.
Private: `superchunk_file`, `make_folder`, `write`, `write_words`,
`read`, `read_words`, `images_in`, `WorldInfo::to_text`, `from_text`,
`HotSuperchunks::to_text`, `from_text`.

## `mock.rs`

**`grass_on_dirt(seed, grass_cells, codec)`**: a superchunk of `DIRT`,
grass (`GRASS`) on cells drawn by xorshift64*, each chunk a dirt layer
and a grass layer if any fell on it.

## `diagnostics/storage.rs`

**`StorageStats::of(storage)`**: superchunk images stored, their bytes,
the ring's bytes.

## `transient_data.rs`

**`measurements()`**, **`publish(report)`**: as in every crate.
