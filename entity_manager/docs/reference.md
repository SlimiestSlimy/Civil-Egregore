# The entity manager: reference

The entities as kept, beside the bitplane manager's cells: a bucket a
chunk, a timer wheel a superchunk, instructions queued and applied.
The simulation ticks them (`../../simulation/`); the design is in
`entity_manager.md`, and how a tick uses it in
`../../simulation/docs/simulation.md`, "Entities".

## The store

**`entity.rs`**: `EntityType`, the type
registry's (`../../type_registry/`), handed on; **`Header`** `{kind,
at, wake}`, `NEVER` -- no ID, the cell names it; **`EntityRef`** `{header, attributes, kept}` (`attributes` all but its type's default; **`default_attribute()`**, **`attribute_seen(kind)`**, **`attribute_member(kind, offset, bytes)`**, **`attribute_list()`**; `entity_manager.md`, "The default attribute") --
its attributes' blocks -- with **`attribute(attribute)`**, what one
holds as its layout, and **`attribute_blocks(attribute_type)`**, one's blocks.
An entity is not copied to be changed: what is written of it is
written to the store, a member at a time.

**`attributes.rs`** (`entity_manager.md`, "Attributes, a block each"):
`Attribute`, `AttributeType`, `Layout`, `Member` and `BLOCK_WORDS`,
the type registry's, handed on. **`AttributeBlock`**: two words,
sixteen bytes, aligned as such; **`holding(attribute, value)`**, the
one block of a layout that takes one; **`WholeAttribute`**: one
attribute of any layout laid out where it is made, read as its
blocks (**`WholeAttribute::holding_value`**);
**`kind()`**, the type in its first word; **`blocks()`**, how many
blocks the attribute it begins takes, by its type's last four bits,
one at least. On an entity's blocks, sorted by type:
**`blocks_sum(summed_blocks)`** -- a sum of blocks, the same for the same
blocks (**`words_sum(words)`**, of the words they are); **`attribute_seen(maybe_attribute)`** -- what a compare carries of one
attribute, 64 bits: that sum, or `NO_ATTRIBUTE` of none; **`each_attribute(attribute_list)`** -- the attributes one after
another; **`find_attribute(attribute_list, attribute_type)`** -- the walk: an attribute's blocks,
or where they would go; **`attribute_bytes(whole_attribute, byte_offset, byte_count)`**,
what a member of an attribute holds, and
**`write_attribute_bytes(whole_attribute, byte_offset, byte_count, value)`**, it written
in place, never over the type -- **`attribute_blocks(attribute_list, attribute_type)`**,
**`attribute(attribute_list, attribute)`** (read as its layout),
**`set_attribute_blocks(attribute_list, attribute)`** and
**`set_attribute(attribute_list, attribute, value)`** (in place of the one of
its type, or added), **`remove_attribute(attribute_list, attribute_type)`** (whether it
was there), **`push_attribute(attribute_list, attribute, value)`** (appended),
**`is_attribute_list(candidate_blocks)`**: whole attributes, of attributes' types, in order,
each type once.

The 3x3 about a cell as nine bits is not kept here: it is an
instruction's shape (`../../actions/src/around.rs`).

**`bucket.rs`**: **`place(cell)`**: a cell's place in its chunk, a
`u16`. **`Bucket`**: the chunk's entities, one a cell, sorted by cell:
their places alone in a list (`places`), which is what is searched,
among one search tile's at a time (`SEARCH_TILES`, 16 of 64x64 cells,
**`search_tile`**, `tile_starts`); beside it a **`StoredEntity`** each
(its header, its attributes' first block and how many); the blocks;
the garbage count. **`get(stood_cell)`**, **`iter`**, **`occupied(place)`**,
**`in_word_tile(first_place)`** (the places on a word tile, a run),
**`make(header, attributes)`** (the attributes as a bucket keeps them, `append_kept`) -- a **`Put`**: `NewlyPut`, or `RefusedOccupied`, an
entity standing there -- and **`put(header, former_place, attributes)`**, of
the entity standing on `was`, with no attributes given those it has
kept: `InPlace`; `MovedToCell` (**`shift_entity`**) to its cell if that is
another and free, else `TurnedBack`; `PassedOver`, nobody standing on
`was`. **`rewrite_entity`**: attributes in place when as many blocks,
else a new attribute list at the end. **`remove(stood_cell)`**, **`index_of(place)`**,
**`find_place(place)`**, **`entity`**, **`edit(place, attribute_type, whole_attribute)`**
(one attribute set in place if as long as it was, or the attribute list made anew
with it added, removed -- no blocks given -- or of another length),
**`write_bytes(place, attribute_type, byte_offset, byte_count, value)`** (a member of an
attribute written in place, nothing moved), **`set_wake(place,
wake)`**, **`prefetch_entity(at)`**, **`prefetch_attributes(at)`**:
asked of memory ahead, an entity's attributes a cache line and the next (`CACHE_LINE_BLOCKS`); **`sweep_garbage`** once garbage reaches the blocks
in use (and 64).

**`wheel.rs`**: `WHEEL_TICKS` (1024); **`Wake`** `{at}`;
**`Wheel`**: **`due_wakes(tick)`**, **`file_wake(earliest_tick, tick, wake)`** -- a slot
if within `WHEEL_TICKS` of `earliest`, else the list further off --
**`pass(tick)`**: the slot passed emptied, and every half of
`WHEEL_TICKS` the wakes now in reach filed; **`sort_wakes(tick)`**: a tick's
wakes by cell, each cell once.

**`store.rs`** (with `store/world_entities.rs`, `WorldEntities`, and
`store/entity_reader.rs`, `EntityReader`): **`SuperchunkEntities`**: a bucket a chunk and a wheel;
**`len()`**, **`is_empty()`** -- how many it holds, whether none --
**`get(stood_cell)`**, **`iter`**, **`chunk(place)`**, **`woken_entities(tick)`** and
**`woken_prefetching(tick, prefetch_cell)`** -- the wheel's slot, each wake
found and still due, the entities `ENTITY_AHEAD` on asked of memory --
**`make(earliest_tick, header, attributes)`** -- a new one, its wake filed --
**`put(earliest_tick, header, from_cell, attributes)`** -- of the one standing on
`named_cell`, within a chunk or from one to another (**`move_between`**), a
`Put` -- **`in_word_tile(chunk,
first)`**, **`remove(at)`**,
**`occupied(stood_cell)`**, and in `store/named_in_a_tick.rs`, the mod
`named_in_a_tick`, an entity found by the cell the tick found it on
(`left_this_tick`): `now_on`, `named_or_taken`, **`get_named(named_cell)`**,
**`make_named`**, **`put_named`**, **`move_named(earliest, stood, to,
collision)`** (moved, all else of it as it is), **`wake_named(earliest,
stood, wake)`** (its wake set and filed: a write to the wheel),
**`write_attribute_bytes_named(named_cell, attribute_type, byte_offset, byte_count, value)`**,
**`edit_named`**, **`remove_named`**, **`cross_named(destination_entities, earliest_tick, named_cell, to_cell, collision_plane)`** (moved to a cell of another superchunk, whole), **`crossed_to(named_cell)`** (the superchunk it crossed to this tick), **`named_from_abroad(named_cell)`** (whether it crossed into this one),
**`free_for_a_new_entity(checked_cell, collision_plane)`** (what a making is held
against),
**`names_anew(collision_plane)`** -- and the collision plane as lent to
them, **`CollisionCells`** (`held`, `hold`, `free`), what the puts and
moves ask and keep, the cells `vacated_cells` cleared in it when the tick is
over --
**`edit(stood_cell, attribute_type, whole_attribute)`**, **`pass(tick)`**, **`sort_wakes(tick)`** -- after the second phase for
the next tick, after `WorldEntities::apply` for the tick about to run --
**`counts`**. **`WorldEntities`**: the tick about to run, the superchunks by
superchunk index, and instructions queued outside a tick: **`now`**,
**`len`**, **`superchunk(superchunk)`**, **`get(stood_cell)`**,
**`keep_superchunks(superchunk_indices)`** -- added empty, dropped, how many
entities dropped -- **`queue_put(header, attributes)`**,
**`queue_remove(stood_cell)`**, **`queued`**, **`apply`** -- as the arena's
`queue` and `apply` --
**`iter`**, **`advance_tick`**. **`EntityReader`**:
every superchunk's entities read in a tick, as the bitplanes' `Reader`:
**`get(stood_cell)`**, **`chunk(chunk)`**, **`occupied(origin, width,
height)`** -- the cells entities stand on among up to 16x16
(`OCCUPIED_SIDE`), a row a word, from the up to nine word tiles' runs of
places (**`in_word_tile`**).

**`instructions.rs`**: **`Instructions`**: the instructions queued for one
superchunk -- put, move, edit, remove -- the blocks the puts and the
edits carry in a list beside (an `Instruction`'s cell, `landing_cell`): **`put(header, attributes)`** -- an entity made --
(by **`push`**),
**`move_entity(header, named_cell)`**, **`set_attribute(at, attribute,
value)`**, **`set_attribute_blocks(at, attribute)`**,
**`unset_attribute(named_cell, attribute_type)`**, **`remove(stood_cell)`** -- each of the entity
the tick found on `at` -- **`apply_some(some, superchunks,
earliest, applied)`** -- those at `some` alone, how the instructions under a compare
refused are left out -- **`apply(superchunks, earliest,
applied)`** in order, each on its cell's superchunk (a put elsewhere
lost, one of an entity no longer where it stood passed over, a new
one on a cell taken refused, a mover to one staying),
**`clear`**, **`len()`** and **`is_empty()`** -- how
many are queued, whether none. **`InstructionsApplied`** `{puts, moves,
edits, wakes, removes, lost, stayed, refused, crossed, passed_over}`,
added with `+=`.

**`WorldEntities`** (`store/world_entities.rs`), the world's: **`len()`**,
**`is_empty()`**, **`superchunks()`** -- its superchunks by Morton
index -- and **`superchunks_mut()`**, the same to change, which is how
a tick hands each thread its own.

**`saved.rs`**: a superchunk's state as words: **`encode_state(random,
entities)`** -- the words, and how many entities -- and
**`decode_state(words, current_tick, world_entities)`**, its entities
queued -- a wake passed made `now` -- a **`SavedState`** `{random,
entities}`; **`entity_count(words)`**; all three read by
**`read(words, each_entity)`**. **`WorldEntities::at_tick(current_tick)`**: what a load
puts them back into.

## `diagnostics/entities.rs`

**`EntityStats::measure_entities(world_entities)`**: superchunks, entities, attribute
blocks in use and as garbage, wakes filed.
