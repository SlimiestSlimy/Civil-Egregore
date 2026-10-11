# The entity manager: reference

The entities as kept, beside the bitplane manager's cells: a bucket a
chunk, a timer wheel a superchunk, instructions queued and applied.
The simulation ticks them (`../../simulation/`); the design is in
`entity_manager.md`, and how a tick uses it in
`../../simulation/docs/simulation.md`, "Entities".

## The store

**`entity.rs`**: `EntityType`, the type
registry's (`../../type_registry/`), handed on; **`Header`** `{kind,
at, wake}`, `NEVER` -- no ID, the cell names it; **`EntityRef`** `{header, attributes}` --
its attributes' blocks -- with **`attribute(attribute)`**, what one
holds as its layout, and **`attribute_blocks(kind)`**, one's blocks.
An entity is not copied to be changed: what is written of it is
written to the store, a member at a time.

**`attributes.rs`** (`entity_manager.md`, "Attributes, a block each"):
`Attribute`, `AttributeType`, `Layout`, `Member` and `BLOCK_WORDS`,
the type registry's, handed on. **`AttributeBlock`**: two words,
sixteen bytes, aligned as such; **`holding(attribute, value)`**, the
one block of a layout that takes one; **`WholeAttribute`**: one
attribute of any layout laid out where it is made, read as its
blocks (**`WholeAttribute::holding`**);
**`kind()`**, the type in its first word; **`blocks()`**, how many
blocks the attribute it begins takes, by its type's last four bits,
one at least. On an entity's blocks, sorted by type:
**`blocks_sum(blocks)`** -- a sum of blocks, the same for the same
blocks (**`words_sum(words)`**, of the words they are); **`attribute_seen(blocks)`** -- what a compare carries of one
attribute, 64 bits: that sum, or `NO_ATTRIBUTE` of none; **`each_attribute(blocks)`** -- the attributes one after
another; **`find_attribute(blocks, kind)`** -- the walk: an attribute's blocks,
or where they would go; **`attribute_bytes(blocks, offset, bytes)`**,
what a member of an attribute holds, and
**`write_attribute_bytes(blocks, offset, bytes, value)`**, it written
in place, never over the type -- **`attribute_blocks(blocks, kind)`**,
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
the garbage count. **`get(at)`**, **`iter`**, **`occupied(place)`**,
**`in_word_tile(first)`** (the places on a word tile, a run),
**`make(header, attributes)`** -- a **`Put`**: `New`, or `Refused`, an
entity standing there -- and **`put(header, was, attributes)`**, of
the entity standing on `was`, with no attributes given those it has
kept: `InPlace`; `Moved` (**`shift_entity`**) to its cell if that is
another and free, else `Stayed`; `PassedOver`, nobody standing on
`was`. **`rewrite`**: attributes in place when as many blocks,
else a new run at the end. **`remove(at)`**, **`index_of(place)`**,
**`find(place)`**, **`entity`**, **`edit(place, kind, blocks)`**
(one attribute set in place if as long as it was, or the run made anew
with it added, removed -- no blocks given -- or of another length),
**`write_bytes(place, kind, offset, bytes, value)`** (a member of an
attribute written in place, nothing moved), **`set_wake(place,
wake)`**, **`prefetch_entity(at)`**, **`prefetch_attributes(at)`**:
asked of memory ahead, an entity's attributes a cache line and the next (`CACHE_LINE_BLOCKS`); **`sweep`** once garbage reaches the blocks
in use (and 64).

**`wheel.rs`**: `WHEEL_TICKS` (1024); **`Wake`** `{at}`;
**`Wheel`**: **`due(tick)`**, **`file(earliest, tick, wake)`** -- a slot
if within `WHEEL_TICKS` of `earliest`, else the list further off --
**`pass(tick)`**: the slot passed emptied, and every half of
`WHEEL_TICKS` the wakes now in reach filed; **`sort(tick)`**: a tick's
wakes by cell, each cell once.

**`store.rs`** (with `store/world_entities.rs`, `Entities`, and
`store/entity_reader.rs`, `EntityReader`): **`SuperchunkEntities`**: a bucket a chunk and a wheel;
**`len()`**, **`is_empty()`** -- how many it holds, whether none --
**`get(at)`**, **`iter`**, **`chunk(place)`**, **`woken(tick)`** and
**`woken_prefetching(tick, prefetch)`** -- the wheel's slot, each wake
found and still due, the entities `ENTITY_AHEAD` on asked of memory --
**`make(earliest, header, attributes)`** -- a new one, its wake filed --
**`put(earliest, header, from, attributes)`** -- of the one standing on
`from`, within a chunk or from one to another (**`move_between`**), a
`Put` -- **`in_word_tile(chunk,
first)`**, **`remove(at)`**, **`arrived(arrival)`** -- an entity crossed in,
noted, an **`Arrival`** `{left, at, attributes}`, the last the sum
of the attributes it was put with -- **`take_arrived(arrived)`**,
**`settle_leavers(arrived, settled)`** -- those of a neighbour's
arrivals that left this superchunk removed, what each ended the tick
with noted if it is not what it was put with, one removed here
meanwhile noted as turned back (**`Settled`** `{turned_back, changed,
attributes}`, **`is_empty`**, **`clear`**) -- **`settle_arrivals(settled)`**
-- those put here given what they have, those turned back removed --
**`occupied(at)`**, and in `store/named_in_a_tick.rs`, the mod
`named_in_a_tick`, an entity found by the cell the tick found it on
(`left_this_tick`): `now_on`, `named_or_taken`, **`get_named(stood)`**,
**`make_named`**, **`put_named`**, **`move_named(earliest, stood, to,
collision)`** (moved, all else of it as it is), **`wake_named(earliest,
stood, wake)`** (its wake set and filed: a write to the wheel),
**`write_attribute_bytes_named(stood, kind, offset, bytes, value)`**,
**`edit_named`**, **`remove_named`**,
**`free_for_a_new_entity(at, collision)`** (what a making is held
against),
**`names_anew(collision)`** -- and the collision plane as lent to
them, **`CollisionCells`** (`held`, `hold`, `free`), what the puts and
moves ask and keep, the cells `vacated` cleared in it when the tick is
over --
**`edit(at, kind, blocks)`**, **`pass(tick)`**, **`sort_wakes(tick)`** -- after the second phase for
the next tick, after `Entities::apply` for the tick about to run --
**`counts`**. **`Entities`**: the tick about to run, the superchunks by
superchunk index, and instructions queued outside a tick: **`now`**,
**`len`**, **`superchunk(superchunk)`**, **`get(at)`**,
**`align(superchunk_indices)`** -- added empty, dropped, how many
entities dropped -- **`queue_put(header, attributes)`**,
**`queue_remove(at)`**, **`queued`**, **`apply`** -- as the arena's
`queue` and `apply` --
**`iter`**, **`advance`**. **`EntityReader`**:
every superchunk's entities read in a tick, as the bitplanes' `Reader`:
**`get(at)`**, **`chunk(chunk)`**, **`occupied(origin, width,
height)`** -- the cells entities stand on among up to 16x16
(`OCCUPIED_SIDE`), a row a word, from the up to nine word tiles' runs of
places (**`in_word_tile`**).

**`instructions.rs`**: **`Instructions`**: the instructions queued for one
superchunk -- put, move, edit, remove -- the blocks the puts and the
edits carry in a list beside (an `Instruction`'s cell, `lands`): **`put(header, attributes)`** -- an entity made --
**`cross(header, left, attributes)`** (both by **`push`**),
**`move_entity(header, from)`**, **`set_attribute(at, attribute,
value)`**, **`set_attribute_blocks(at, attribute)`**,
**`unset_attribute(at, kind)`**, **`remove(at)`** -- each of the entity
the tick found on `at` -- **`apply_some(some, superchunks,
earliest, applied)`** -- those at `some` alone, how the instructions under a compare
refused are left out -- **`apply(superchunks, earliest,
applied)`** in order, each on its cell's superchunk (a put elsewhere
lost, one of an entity no longer where it stood passed over, a new
one on a cell taken refused, a mover to one staying),
**`count_lost`**, **`clear`**, **`len()`** and **`is_empty()`** -- how
many are queued, whether none. **`InstructionsApplied`** `{puts, moves,
edits, wakes, removes, lost, stayed, refused, crossed, passed_over, turned_back}`,
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
