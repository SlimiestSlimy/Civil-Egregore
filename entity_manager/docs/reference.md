# The entity manager: reference

The entities as kept, beside the bitplane manager's cells: a bucket a
chunk, a timer wheel a superchunk, instructions queued and applied.
The simulation ticks them (`../../simulation/`); the design is in
`../../simulation/docs/simulation.md`, "Entities".

## The store

**`entity.rs`**: `EntityId`, `EntityType`, `AttributeType` (`u64`s);
**`Attribute`** `{kind, value}`; **`Header`** `{id, kind, at, wake}`,
`NEVER`; **`EntityRef`** `{header, attributes}` with
**`attribute(kind)`**; free functions on a list sorted by type:
**`attribute`**, **`set_attribute`** (added if absent),
**`remove_attribute`**; **`sorted`**. **`EntityEdit::of(entity, room)`**: an
entity being changed -- **`header`**, **`get(kind)`**, **`set(kind,
value)`**, **`unset(kind)`**, **`attributes`**, **`edited`**: its
attributes copied into `room` when first one changes, not before.

**`around.rs`**: `CENTRE`, `ALL`, `RING`; **`Around`** `{set, hot}`;
**`squeeze(rows)`**: a 3x3 window's rows as nine bits;
**`cell(at, bit)`**, **`bit_of(at, cell)`**; **`pick(random,
choices)`**: one of the set bits, none and nothing drawn if there is
none; **`prefer(random, wanted, open)`**: of `wanted` if any is open.

**`bucket.rs`**: **`place(cell)`**: a cell's place in its chunk, a
`u16`. **`Bucket`**: the chunk's entities, one a cell, sorted by cell:
their places alone in a list (`places`), which is what is searched,
among one search tile's at a time (`SEARCH_TILES`, 16 of 64x64 cells,
**`search_tile`**, `tile_starts`); beside it a **`StoredEntity`** each
(its header, its attributes' first index and count); the attributes;
the garbage count. **`get(id, at)`**, **`iter`**, **`occupied(place)`**,
**`in_word_tile(first)`** (the places on a word tile, a run),
**`put(header, was, attributes)`** -- with no attributes given, those it
has kept, and not made if not there -- a **`Put`**: `InPlace`; `Moved`
(**`shift_entity`**) to its cell if that is another and free, else
`Stayed`; `New` if it is not there, `was` is its cell and it is free,
else `Refused`; `PassedOver` if it was to have moved and is not where it
stood. **`rewrite`**: attributes in place when the count is the same,
else a new run at the end. **`remove(id, at)`**, **`index_of(place)`**,
**`find(place, id)`**, **`entity`**, **`edit(id, place, kind, value)`**
(one attribute set in place, or the run made anew with it added or
removed), **`prefetch_entity(at)`**, **`prefetch_attributes(at)`**:
asked of memory ahead; **`sweep`** once garbage reaches the attributes
in use (and 64).

**`wheel.rs`**: `WHEEL_TICKS` (1024); **`Wake`** `{id, at}`;
**`Wheel`**: **`due(tick)`**, **`file(earliest, tick, wake)`** -- a slot
if within `WHEEL_TICKS` of `earliest`, else the list further off --
**`pass(tick)`**: the slot passed emptied, and every half of
`WHEEL_TICKS` the wakes now in reach filed; **`sort(tick)`**: a tick's
wakes by cell, then ID.

**`store.rs`** (with `store/world_entities.rs`, `Entities`, and
`store/entity_reader.rs`, `EntityReader`): **`SuperchunkEntities`**: a bucket a chunk and a wheel;
**`get(id, at)`**, **`iter`**, **`chunk(place)`**, **`woken(tick)`** and
**`woken_prefetching(tick, prefetch)`** -- the wheel's slot, each wake
found and still due, the entities `ENTITY_AHEAD` on asked of memory --
**`put(earliest, header, from, attributes)`** -- within a chunk or from
one to another (**`move_between`**), a `Put` -- **`in_word_tile(chunk,
first)`**, **`remove(id, at)`**, **`arrived(id, left)`** -- an entity crossed in,
noted -- **`take_arrived(arrived)`**, **`settle_leavers(arrived)`** --
those of a neighbour's arrivals that left this superchunk removed --
**`pass(tick)`**, **`sort_wakes(tick)`** -- after the second phase for
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
superchunk -- put, move, edit, remove -- the puts' attributes in a list
beside: **`put(header, from, attributes)`**, **`cross(header, left,
attributes)`**, **`move_entity(header, from)`**,
**`edit(id, at, kind, value)`**, **`remove`**, **`apply(superchunks, earliest,
applied)`** in order, each on its cell's superchunk (a put elsewhere
lost, one of an entity no longer where it stood passed over, a new
one on a cell taken refused, a mover to one staying),
**`count_lost`**, **`clear`**. **`InstructionsApplied`** `{puts, moves,
edits, removes, lost, stayed, refused, crossed}`, added with `+=`.

**`saved.rs`**: a superchunk's state as words: **`encode_state(random,
entities)`** -- the words, and how many entities -- and
**`decode_state(words, now, entities)`**, its entities
queued -- a wake passed made `now` -- a **`SavedState`** `{random,
entities}`; **`entity_count(words)`**; all three read by
**`read(words, each)`**. **`Entities::at_tick(now)`**: what a load
puts them back into.

## `diagnostics/entities.rs`

**`EntityStats::of(entities)`**: superchunks, entities, attributes in
use and as garbage, wakes filed.
