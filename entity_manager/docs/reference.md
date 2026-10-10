# The entity manager: reference

The entities as kept, beside the bitplane manager's cells: a bucket a
chunk, a timer wheel a superchunk, instructions queued and applied.
The simulation ticks them (`../../simulation/`); the design is in
`entity_manager.md`, and how a tick uses it in
`../../simulation/docs/simulation.md`, "Entities".

## The store

**`entity.rs`**: `EntityId` (a `u64`); `EntityType`, the type
registry's (`../../type_registry/`), handed on; **`Header`** `{id,
kind, at, wake}`, `NEVER`; **`EntityRef`** `{header, attributes}` --
its attributes' blocks -- with **`attribute(attribute)`**, what one
holds as its layout, and **`attribute_blocks(kind)`**, one's blocks.
**`EntityEdit::of(entity, room)`**: an entity being changed --
**`header`**, **`get(attribute)`**, **`set(attribute, value)`**,
**`set_blocks(attribute)`**, **`unset(attribute)`**, **`attributes`**,
**`edited`**: its blocks copied into `room` (**`own`**) when first one
changes, not before.

**`attributes.rs`** (`entity_manager.md`, "Attributes, a block each"):
`Attribute`, `AttributeType`, `Layout` and `BLOCK_WORDS`, the type
registry's, handed on. **`AttributeBlock`**: eight words, aligned as a
cache line; **`holding(attribute, value)`**, the one block of a layout
that takes one (**`holding_one`**, the same unchecked);
**`kind()`**, the type in its first word; **`blocks()`**, how many
blocks the attribute it begins takes, by its type or its block length,
one at least. On an entity's blocks, sorted by type:
**`blocks_sum(blocks)`** -- a sum of blocks, the same for the same
blocks; **`each_attribute(blocks)`** -- the attributes one after
another; **`find_attribute(blocks, kind)`** -- the walk: an attribute's blocks,
or where they would go -- **`attribute_blocks(blocks, kind)`**,
**`attribute(blocks, attribute)`** (read as its layout),
**`set_attribute_blocks(blocks, attribute)`** and
**`set_attribute(blocks, attribute, value)`** (in place of the one of
its type, or added), **`remove_attribute(blocks, kind)`** (whether it
was there), **`push_attribute(blocks, attribute, value)`** (appended),
**`sorted(blocks)`**: whole attributes, of attributes' types, in order,
each type once.

The 3x3 about a cell as nine bits is not kept here: it is an
instruction's shape (`../../instructions/src/around.rs`).

**`bucket.rs`**: **`place(cell)`**: a cell's place in its chunk, a
`u16`. **`Bucket`**: the chunk's entities, one a cell, sorted by cell:
their places alone in a list (`places`), which is what is searched,
among one search tile's at a time (`SEARCH_TILES`, 16 of 64x64 cells,
**`search_tile`**, `tile_starts`); beside it a **`StoredEntity`** each
(its header, its attributes' first block and how many); the blocks;
the garbage count. **`get(id, at)`**, **`iter`**, **`occupied(place)`**,
**`in_word_tile(first)`** (the places on a word tile, a run),
**`put(header, was, attributes)`** -- with no attributes given, those it
has kept, and not made if not there -- a **`Put`**: `InPlace`; `Moved`
(**`shift_entity`**) to its cell if that is another and free, else
`Stayed`; `New` if it is not there, `was` is its cell and it is free,
else `Refused`; `PassedOver` if it was to have moved and is not where it
stood. **`rewrite`**: attributes in place when as many blocks,
else a new run at the end. **`remove(id, at)`**, **`index_of(place)`**,
**`find(place, id)`**, **`entity`**, **`edit(id, place, kind, blocks)`**
(one attribute set in place if as long as it was, or the run made anew
with it added, removed -- no blocks given -- or of another length),
**`prefetch_entity(at)`**, **`prefetch_attributes(at)`**:
asked of memory ahead; **`sweep`** once garbage reaches the blocks
in use (and 64).

**`wheel.rs`**: `WHEEL_TICKS` (1024); **`Wake`** `{id, at}`;
**`Wheel`**: **`due(tick)`**, **`file(earliest, tick, wake)`** -- a slot
if within `WHEEL_TICKS` of `earliest`, else the list further off --
**`pass(tick)`**: the slot passed emptied, and every half of
`WHEEL_TICKS` the wakes now in reach filed; **`sort(tick)`**: a tick's
wakes by cell, then ID.

**`store.rs`** (with `store/world_entities.rs`, `Entities`, and
`store/entity_reader.rs`, `EntityReader`): **`SuperchunkEntities`**: a bucket a chunk and a wheel;
**`len()`**, **`is_empty()`** -- how many it holds, whether none --
**`get(id, at)`**, **`iter`**, **`chunk(place)`**, **`woken(tick)`** and
**`woken_prefetching(tick, prefetch)`** -- the wheel's slot, each wake
found and still due, the entities `ENTITY_AHEAD` on asked of memory --
**`put(earliest, header, from, attributes)`** -- within a chunk or from
one to another (**`move_between`**), a `Put` -- **`in_word_tile(chunk,
first)`**, **`remove(id, at)`**, **`arrived(arrival)`** -- an entity crossed in,
noted, an **`Arrival`** `{id, left, at, attributes}`, the last the sum
of the attributes it was put with -- **`take_arrived(arrived)`**,
**`settle_leavers(arrived, turned_back)`** -- those of a neighbour's
arrivals that left this superchunk removed if they ended the tick here
with those attributes, else turned back -- **`settle_arrivals(turned_back)`**
-- those put here and turned back removed --
**`edit(id, at, kind, blocks)`**, **`pass(tick)`**, **`sort_wakes(tick)`** -- after the second phase for
the next tick, after `Entities::apply` for the tick about to run --
**`counts`**. **`Entities`**: the tick about to run, the superchunks by
superchunk index, and instructions queued outside a tick: **`now`**,
**`len`**, **`superchunk(superchunk)`**, **`get(id, at)`**,
**`align(superchunk_indices)`** -- added empty, dropped, how many
entities dropped -- **`queue_put(header, attributes)`**,
**`queue_remove(header)`**, **`queued`**, **`apply`** -- as the arena's
`queue` and `apply` --
**`iter`**, **`advance`**. **`EntityReader`**:
every superchunk's entities read in a tick, as the bitplanes' `Reader`:
**`get(id, at)`**, **`chunk(chunk)`**, **`occupied(origin, width,
height)`** -- the cells entities stand on among up to 16x16
(`OCCUPIED_SIDE`), a row a word, from the up to nine word tiles' runs of
places (**`in_word_tile`**).

**`instructions.rs`**: **`Instructions`**: the instructions queued for one
superchunk -- put, move, edit, remove -- the blocks the puts and the
edits carry in a list beside, and the cells a new entity may be put on
(`cells`): **`put(header, from, attributes)`**,
**`put_on_the_first_free(header, others, attributes)`**,
**`cross(header, left, attributes)`** (both by **`push`**),
**`move_entity(header, from)`**, **`set_attribute(id, at, attribute,
value)`**, **`set_attribute_blocks(id, at, attribute)`**,
**`unset_attribute(id, at, kind)`**, **`remove`**, **`apply_some(some, superchunks,
earliest, applied)`** -- those at `some` alone, how the instructions under a compare
refused are left out -- **`apply(superchunks, earliest,
applied)`** in order, each on its cell's superchunk (a put elsewhere
lost, one of an entity no longer where it stood passed over, a new
one on a cell taken refused, a mover to one staying),
**`count_lost`**, **`clear`**, **`len()`** and **`is_empty()`** -- how
many are queued, whether none. **`InstructionsApplied`** `{puts, moves,
edits, removes, lost, stayed, refused, crossed, beside, passed_over}`,
added with `+=`.

**`Entities`** (`store/world_entities.rs`), the world's: **`len()`**,
**`is_empty()`**, **`superchunks()`** -- its superchunks by Morton
index -- and **`superchunks_mut()`**, the same to change, which is how
a tick hands each thread its own.

**`saved.rs`**: a superchunk's state as words: **`encode_state(random,
entities)`** -- the words, and how many entities -- and
**`decode_state(words, now, entities)`**, its entities
queued -- a wake passed made `now` -- a **`SavedState`** `{random,
entities}`; **`entity_count(words)`**; all three read by
**`read(words, each)`**. **`Entities::at_tick(now)`**: what a load
puts them back into.

## `diagnostics/entities.rs`

**`EntityStats::of(entities)`**: superchunks, entities, attribute
blocks in use and as garbage, wakes filed.
