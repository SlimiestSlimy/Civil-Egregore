# The simulation, function by function

The design is in `simulation.md`.

## `sampling.rs`

**`gap(random, log_unchosen)`**: set cells passed over before the next
chosen, from `1 - unit()`, in `(0, 1]`. **`select(word, rank)`**: the
`rank`-th set bit's position. **`sample_layer(superchunk, layer,
probability, random, emit)`**: one superchunk's layer's chosen cells,
in Morton order, found by the counts: the chunks', the count tiles',
the words'. **`sample(arena, type,
probability, random, emit)`**: every superchunk's, in Morton order.

## `turn/`

`mod.rs` the turn, its cells and its outbox; `entities.rs` the entities
and their instructions. What a rule makes of them is
`../../instructions/`.

**`Outbox`**: nine `WriteQueues` and nine `Instructions`, by **`slot(dx,
dy)`**.

**`Turn`**: a superchunk's turn in the first phase:
**`superchunk`**, **`random`**, **`now`**, **`sample(type, probability,
samples)`** of its own cells. Cells read, anywhere hot, as the tick
found them: **`holds(type, cell)`**, **`value(plane, cell)`** (a wide
plane's number), **`window(type, origin, width, height)`** (up to 8x8
cells as a `Window`), **`windows(types, ...)`** (of several types at
once), **`any_in_tile(type, cell, scale)`**, **`tiles_holding(type,
cell)`** (tiles holding any cell, off the counts). **`queue(type,
write)`**: a write, into the slot of each superchunk it lands in.
Entities read: **`woken()`**, **`woken_reading(layers)`** (the cells
about each asked of memory ahead) -- its entities waking this tick, in
Morton order, borrowed from the world as the tick found it, not from
the turn, so instructions can be queued while going through them;
**`entity(id, at)`**, **`entities_in(chunk)`**, **`occupied(origin,
width, height)`** (the cells entities stand on). Entities written:
**`new_id`**; **`put(header, attributes)`**: an entity made or changed
where it stands, waking after this tick; **`update(before, after,
attributes)`**: changed, and moved to its cell if that is free --
staying if not; passed over if no longer where the tick found it -- or,
to another superchunk, crossing; **`step(entity, to, wake)`**: a move,
no attributes carried -- whole, as `update`, to another superchunk;
**`set_attribute(entity, kind, value)`**, **`unset_attribute(entity,
kind)`**: an edit of any entity in reach; **`remove(header)`**.
**`slot_of`**: the slot of a superchunk, past the neighbours panicking.

## `hot.rs`

**`Hot`** `{side, entity}`: the world's size and the hot entity --
**`about(entity)`**, **`span()`**, **`within(superchunk)`**,
**`wanted(entities)`**: the superchunks to be hot, or nothing where
they are forced in a world of no size. **`about(of)`**: the 3x3
superchunks about each, sorted, each once.

## `halos.rs`

**`Held`** `{arena, storage, entities, simulation, cold, layers,
generate}`: the world's, lent for a call. **`Halos`** `{hot, jobs,
warming, cooling, writing_back, flushing}`, **`new(hot,
dispatcher)`**, **`restore_cooling(cooling)`**. `WARM_TICKS` (256): the ticks a superchunk is warming;
`COOL_TICKS` (256): the ticks one is cooling.
**`HaloChange`** `{reached, generated, restored, cooled}`, added with
`+=`. **`Warming`** `{superchunk, due, from}`, from a
**`WarmedFrom`**: `Lingering`, or `Job(ticket)`.
**`Halos::move_to_hot_entities`**: the halos moved to the hot entities, those
reached hot `WARM_TICKS` on, those left cold `COOL_TICKS` on.
**`Halos::keep_hot(wanted)`**: `wanted` made the hot superchunks now.
**`Halos::warming`**, **`Halos::cooling`**: the superchunks warming,
and those cooling, each with its due tick. Both through
**`Halos::make_hot_within(wanted, warm_ticks, cool_ticks)`**: the
write-backs encoded landed; those cooling wanted again no longer
cooling; every hot one not wanted cooling, due no later than
`cool_ticks` on; those due made cold -- state kept, bitmaps lingering
(`BitmapArena::make_cold_superchunk`), their dirty ones sent to be
encoded; those warming not wanted dropped (`BitmapArena::let_go`, or
the job forgotten); every one warming due no later than `warm_ticks`
on; the wanted ones neither hot nor warming started; those due made hot, the
entities aligned, the kept states put back and the random streams with
them. **`Halos::start_warming(superchunk, due)`**: held if lingering
(`BitmapArena::hold`), else sent as a job.
**`Halos::finish_warming`**: a warming superchunk's bitmaps made hot -- again
as they were (`BitmapArena::make_hot_again`), or as its job made
them (`BitmapArena::make_hot_cells`), its image put in the cold pool if
generated. **`Halos::land_write_backs(wait)`**: the encoded write-backs
put into the ring, in order (`ChunkStorage::try_write_back`, then
`BitmapArena::written_back`), the tail flushed whenever it needs the
room, then the flushes landed. **`Halos::flush_tail`**: the tail
superchunk's changes taken (`ChunkStorage::take`) and sent to be
flushed, its flush before landed first. **`Halos::land_flushes(wait)`**:
the images rewritten put in the cold pool, the arena told of each with
no change left in the ring (`BitmapArena::flushed`).
**`Halos::write_back_all`**: every hot superchunk's dirty bitmaps sent
to be encoded, and every write-back landed. **`Halos::flush_all`**:
every superchunk with changes in the ring flushed, on all the threads.

## `tick.rs`

**`TickReport`** `{writes_applied, instructions_applied, rules,
computing, applying}`.

**`threads_for(superchunks)`**: every thread the machine has, no more
than the superchunks. **`Simulation::for_superchunks(superchunks)`**: on
those; **`Simulation::new(threads)`**: on a number given, to measure
against another; **`threads`**. **`random_states()`**: each
superchunk's index and random stream's state; **`restore_random(
states)`**: taken up, as a save kept them. **`tick(arena, entities,
seed, rule)`**: the entities aligned to the arena's superchunks (those
dropped counted lost); the superchunks claimed by the threads one at a
time (`CLAIMED`); the first phase runs the rule on each, a `Reader` a
thread; the second, each thread the superchunks it claims and
their entities, passes each wheel's tick, then applies every outbox's
writes and instructions; writes to superchunks not in use counted
missed (`count_missed`), entities put there lost; the crossings settled
(**`settle_crossings`**: each superchunk's arrivals taken, then each
thread its run of superchunks, each removing its leavers from its
neighbours' arrivals); the outboxes emptied; the entities' tick advanced. **`neighbours`**: the nine
offsets in a fixed order.
