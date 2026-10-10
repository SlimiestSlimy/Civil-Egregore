# The simulation, function by function

The design is in `simulation.md`.

## `sampling.rs`

**`select(word, rank)`**: the `rank`-th set bit's position.
**`sample_layer(superchunk, layer, chance, random, emit)`**: one
superchunk's layer's chosen cells, in Morton order -- the set cells
passed over before each drawn by `Chance::passed_over` from one draw,
none for a chance of always -- found by the counts: the chunks', the
count tiles', the words'. **`sample(arena, type, chance, random,
emit)`**: every superchunk's, in Morton order.

## `turn/`

`mod.rs` the turn, its cells and its outbox; `entities.rs` the entities
and their instructions; `conditional.rs` the compare-and-writes . What a rule makes of them is
`../../instructions/`.

**`Outbox`**: nine `Instructions` and nine
`Conditional`, by **`slot(dx, dy)`**.

**`Turn`**: a superchunk's turn in the first phase:
**`superchunk`**, **`random`**, **`now`**, **`sample(type, chance,
samples)`** of its own cells. Cells read, anywhere hot, as the tick
found them: **`holds(type, cell)`**, **`value(plane, cell)`** (a wide
plane's number), **`window(type, origin, width, height)`** (up to 8x8
cells as a `Window`), **`windows(types, ...)`** (of several types at
once), **`any_in_tile(type, cell, scale)`**, **`tiles_holding(type,
cell)`** (tiles holding any cell, off the counts). Cells are written
by compare-and-writes alone (`conditional.rs`).
Entities read: **`woken()`**, **`woken_reading(layers)`** (the cells
about each asked of memory ahead) -- its entities waking this tick, in
Morton order, borrowed from the world as the tick found it, not from
the turn, so instructions can be queued while going through them;
**`entity(id, at)`**, **`entities_in(chunk)`**, **`occupied(origin,
width, height)`** (the cells entities stand on, up to `OCCUPIED_SIDE`
each way, a row a word: cell `(x, y)` from `origin` at bit `x` of row
`y`). Entities written:
**`new_id`**; **`put_on_the_first_free(header, others,
attributes)`**: a new entity on its cell, or the first free of some
others; **`put(header, attributes)`**: an entity made or changed
where it stands, waking after this tick; **`update(before, after,
attributes)`**: changed, and moved to its cell if that is free --
staying if not; passed over if no longer where the tick found it -- or,
to another superchunk, crossing; **`step(entity, to, wake)`**: a move,
no attributes carried -- whole, as `update`, to another superchunk;
**`set_attribute(entity, attribute, seen, value)`**,
**`unset_attribute(entity, attribute, seen)`**: an edit of another
entity in reach, applied if the attribute is still as seen; `false`,
and nothing queued, if the entity wakes this tick;
**`remove(header)`**.
**`slot_of`**: the slot of a superchunk, past the neighbours panicking.
`SLOTS` (9): an outbox's slots. A turn's fields: its `superchunk`, its
`entities`, `now`, the thread's `reader` and `entity_reader`, its
`outbox`, its `random`, the compare its instructions are queued
under (`comparing`) and the number its counts are `counted_from`.
`slot_between(from, to)`: the slot of one superchunk in another's
outbox, none past the neighbours.

`conditional.rs` (`simulation.md`, "Compare-and-write").
**`Compare`**: what a write is held against -- `Cell {layer_type, at,
seen}` or `Attribute {id, at, kind, seen}`; `Compare::at`, the cell it
is on; `Compare::holds(superchunk, entities)`, whether it holds now.
**`Turn::queue_if(compare, type, at, seen, value, counted)`**: a cell
written if the compare holds and the cell is still as seen, with the
count added then; **`Turn::queue_seen(type, at, seen, value,
counted)`**: the same, held against the cell written alone.
`queue_instruction_if(compare, lands, queue)`: one instruction under
a compare of its own, as an edit of another entity is. **`Turn::count_if(compare, place)`**: a
count. **`Turn::instructions_if(compare)`**,
**`Turn::instructions_as_ever()`**: the entity instructions queued
between them under the compare; `close_instructions_compared`: those
queued so far made a step. **`Turn::count_under(first)`**: the number
the rule's first count is counted under; `counted_number(place)`: a
count's number, under `COUNTED_WHEN_APPLIED` (256) --
**`CountedWhenApplied`**, a tick's counts made as it applied
(`NO_COUNT`: none). `Does`: what a step does -- `Write` (with what was `seen` at the cell),
`Instructions {first, last}`, `Count`; `Step` `{compare, does,
before}`; `Conditional` `{steps}`, what is queued for one superchunk
-- `clear`, `count_missed`, and `apply(superchunk, entities,
instructions, earliest, applied)`: instructions and steps in the order
queued. `queue_step(compare, lands, does)`: a step queued, its compare
in the superchunk it lands in. `Applied`: what a thread applied in the
second phase.

## `hot.rs`

**`Hot`**: `About {entity, side, viewport}` -- every entity of the kind
keeps its halo hot, in a world of `side` superchunks a side if it has
one, and if `viewport` every superchunk of the viewport too, however
many -- or `Forced {side}`: every superchunk of a world of that side hot,
whatever its entities do; only a world with a side can be forced.
**`about(entity)`**: a world of no size, about `entity`;
**`viewport()`**: whether the viewport's superchunks are hot too;
**`side()`**, **`span()`**, **`within(superchunk)`**, **`all()`**:
every superchunk of a world with a side, none of one without;
**`wanted(entities)`**: the superchunks to be hot. **`about(of)`**: the 3x3
superchunks about each, sorted, each once.

## `halos.rs`

The halos' own methods lie in two files under it: `halos/warming.rs`
(`make_hot_within`, `start_warming`, `finish_warming`) and
`halos/write_back.rs` (`land_write_backs`, `flush_tail`,
`land_flushes`, `flush_all`, `write_back_all`, `page_cold_pool_out`,
`page_out_unheld`).

**`Viewport`** `{first, last}`: what a renderer renders, in
superchunks; **`contains(at)`**. **`Held`** `{arena, storage, entities, simulation, cold, layers,
generate}`: the world's, lent for a call. **`Halos`** `{hot, jobs,
warming, cooling, writing_back, flushing, told, viewport, generated}`
-- `told` the viewport last given, `viewport` its superchunks --, **`new(hot,
dispatcher)`**, **`restore_cooling(cooling)`**.
**`Halos::keep_viewport(viewport)`**: the viewport's superchunks --
what a renderer renders -- kept sorted, wanted hot besides the halos
from the next move on, in place of the last viewport's; nothing unless
the world's hot has its viewport's superchunks hot;
**`Halos::viewport`**: them. **`Halos::generated`**: the superchunks
the last move or `keep_hot` generated -- made new by a job, not read
back -- sorted. `WARM_TICKS` (256): the ticks a superchunk is warming;
`COOL_TICKS` (256): the ticks one is cooling.
**`HaloChange`** `{reached, generated, restored, cooled}`, added with
`+=`. **`Warming`** `{superchunk, due, from}`, from a
**`WarmedFrom`**: `Lingering`, or `Job(ticket)`.
**`Halos::move_to_hot_entities`**: the halos, and the superchunks in
view, moved to the hot entities, those
reached hot `WARM_TICKS` on, those left cold `COOL_TICKS` on.
**`Halos::keep_hot(wanted)`**: `wanted` made the hot superchunks now,
with no ticks to warm or cool in: every other one made cold, its state
kept; every one of them not hot made hot -- lingering, from storage and
its kept state, or generated -- the jobs waited for at once. One that
was warming and is not wanted turns hot, as every warming does, and is
made cold by a second pass. What generating and loading start from;
between two ticks, anything else needing superchunks hot a while may
ask too.
**`Halos::warming`**, **`Halos::cooling`**: the superchunks warming,
and those cooling, each with its due tick. Both through
**`Halos::make_hot_within(wanted, warm_ticks, cool_ticks)`**: the
write-backs encoded landed; those cooling wanted again no longer
cooling; every hot one not wanted cooling, due no later than
`cool_ticks` on; those due made cold -- state kept, bitmaps lingering
(`BitmapArena::make_cold_superchunk`), their dirty ones sent to be
encoded; every one warming due no later than `warm_ticks` on, wanted
or not -- a warming is never given up; the wanted ones neither hot nor
warming started; those due made hot, one of them no longer wanted
cooling from then; the entities aligned, the kept states put back and
the random streams with them. Wanted superchunks outside the world's
size are passed over first. **`Halos::start_warming(superchunk, due)`**: held if lingering
(`BitmapArena::hold`), else sent as a job.
**`Halos::finish_warming`**: a warming superchunk's bitmaps made hot -- again
as they were (`BitmapArena::make_hot_again`), or as its job made
them (`BitmapArena::make_hot_cells`), its image put in the cold pool if
generated, or if read back from disk (`ChunkStorage::bring_in`). **`Halos::land_write_backs(wait)`**: the encoded write-backs
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
**`Halos::page_cold_pool_out`**: while the cold pool has more images in
memory than it keeps, those of superchunks neither hot, warming nor
lingering paged out (`Halos::page_out_unheld`,
`ChunkStorage::page_out`) -- the ring flushed first if that is not
enough, a cold superchunk's changes there holding its image -- how
many, or the disk's refusal.

## `tick.rs`

**`TickReport`** `{writes_applied, instructions_applied,
instructions_compared, counted_when_applied, rules, computing,
applying}` -- `instructions_compared` the runs of instructions under a
compare applied, and those refused.

**`threads_for(superchunks)`**: every thread the machine has, no more
than the superchunks. **`Simulation`** `{dispatcher, outboxes, samples, random, arrived}`:
the threads, and what a tick reuses -- an outbox a superchunk, room for
samples a thread, each superchunk's random stream, and the entities
crossed into each in a tick -- so a tick allocates nothing once they
have grown. **`Simulation::for_superchunks(superchunks)`**: on
those; **`Simulation::new(threads)`**: on a number given, to measure
against another; **`Simulation::on(dispatcher)`**: on threads others
queue jobs on too; **`threads`**. `align_random(superchunks, seed)`:
each superchunk given the stream it had, or a new one from the seed and
its index. `PartOfTurns`: what a thread claims of the first phase --
where its superchunks start, their outboxes, their random streams. **`random_states()`**: each
superchunk's index and random stream's state; **`restore_random(
states)`**: taken up, as a save kept them. **`tick(arena, entities,
seed, rule)`**: the entities aligned to the arena's superchunks (those
dropped counted lost); the superchunks claimed by the threads one at a
time (`CLAIMED`); the first phase runs the rule on each, a `Reader` a
thread; the second, each thread the superchunks it claims and
their entities, passes each wheel's tick, then applies every outbox's
instructions and compare-and-writes; writes to superchunks not in use counted
missed (`Conditional::count_missed`), entities put there lost; the crossings settled
(**`settle_crossings`**: each superchunk's arrivals taken, then each
thread its run of superchunks, each removing its leavers from its
neighbours' arrivals); the outboxes emptied; the entities' tick advanced. **`neighbours`**: the nine
offsets in a fixed order.

## `transient_data.rs`

`transient_data::TRANSIENT_DATA` names the crate's `transient_data/`
folder, where its runs would leave what they make; nothing is kept
there yet. The simulation gathers no diagnostics of its own: what the
entities hold is `../../entity_manager/`'s to gather, what the arena
holds `../../bitplane_manager/`'s.
