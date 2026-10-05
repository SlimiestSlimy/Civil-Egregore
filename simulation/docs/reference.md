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

`mod.rs` the turn, its cells and its outbox; `area.rs` the cells about
a cell; `entities.rs` the entities and their instructions.

**`Outbox`**: nine `WriteQueues` and nine `Instructions`, by **`slot(dx,
dy)`**.

**`Turn`**: a superchunk's turn in the first phase:
**`superchunk`**, **`random`**, **`sample(type, probability, samples)`**, **`value(plane, cell)`** and **`queue_value(plane, cell, value)`** (a wide plane's number, read and written whole), **`each_sampled(type, probability, samples, each)`** (`each` run on every cell sampled: a rule is written for one cell, the loop is here), **`each_woken(layers, state, each)`** (the same for the entities waking)
of its own cells, **`holds(type, cell)`** anywhere, **`queue(type,
write)`** -- into the slot of each superchunk it lands in.
**`window(type, origin, width, height)`**: up to 8x8 cells from
`origin` as the tick found them, as a `Window`, through the reader.
**`area(type, centre)`**: the 16x16 cells about `centre` (`AREA_SIDE`,
`AREA_CENTRE`), four windows, as an **`Area`** `{set, hot}`, a row a
`u16`: what pathfinding is handed. **`windows(types, ...)`**,
**`areas(types, centre)`**: of several types at once.
**`occupied(origin, width, height)`**: the cells entities stand on, as
the tick found them. **`now`**, **`woken_reading(layers)`** (the cells
about each asked of memory ahead), **`woken()`**: its entities waking
this tick, in Morton order, borrowed from the world as the tick found
it, not from the turn, so instructions can be queued while going through
them. **`entity(id, at)`**, **`entities_in(chunk)`**: entities in any
hot superchunk, as the tick found them -- the entities' `holds`. **`new_id`**. **`put(header,
attributes)`**: an entity made or changed where it stands, waking after
this tick; **`update(before, after, attributes)`**: changed, and moved
to its cell if that is free -- staying if not; passed over if no longer
where the tick found it -- or, to another superchunk, crossing;
**`remove(header)`**. **`spawn(kind, at, wake, attributes)`**: a new
entity, its ID drawn and returned. **`step(entity, to, wake)`**,
**`sleep(entity, wake)`**: a move, no attributes carried -- whole, as
`update`, to another superchunk. **`set_attribute(entity, kind,
value)`**, **`unset_attribute(entity, kind)`**: an edit of any entity in
reach. **`commit(edit, to, wake)`**: an `EntityEdit`'s entity moved if no
attribute changed, else put whole. **`around(type, at)`**: the 3x3
cells about `at`, an **`Around`** `{set, hot}` of nine bits;
**`around_occupied(at)`**: those entities stand on;
**`free_beside(at, open)`**: one of `open` none stands on, drawn.
**`area_occupied(centre)`**: the entities on an area's cells.
**`area_of_tiles(type, centre, scale)`**: an `Area` of the tiles of
`scale` around `centre`, set where the type holds at any cell.
`FARTHEST_SCALE` (6) the coarsest. **`Area::count`**.
**`slot_of`**: the slot of a
superchunk, past the neighbours panicking.

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
