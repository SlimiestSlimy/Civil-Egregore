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

`mod.rs` the turn, its cells and its outbox; `instruction.rs` the
instruction; `conditional.rs` the compares and the queue;
`entities.rs` what is queued for entities; `apply.rs` the queue
applied. What a rule makes of them is `../../actions/`.

**`Outbox`**: nine queues (`queues`, a `Queue` each), by **`slot(dx,
dy)`**, and for each where every author's part begins (`segments`, a
**`Segment`** `{author, instructions, compares}` each).

**`Turn`**: a superchunk's turn in the first phase:
**`superchunk`**, **`random`**, **`now`**, **`sample(type, chance,
samples)`** of its own cells. Cells read, anywhere hot, as the tick
found them: **`holds(type, cell)`**, **`value(plane, cell)`** (a wide
plane's number), **`window(type, origin, width, height)`** (up to 8x8
cells as a `Window`), **`windows(types, ...)`** (of several types at
once), **`small_tiles(types, first, wanted)`** (four aligned small
tiles, two by two, as a word in Morton order: the cells of a mask),
**`aligned_tile(type, cell, side)`** (a larger aligned tile's words, as
they lie), **`aligned_tiles_two_by_two(types, first, side)`** (four of
them, two by two),
**`any_in_tile(type, cell, scale)`**, **`tiles_holding(type,
cell)`** (tiles holding any cell, off the counts). Cells are written
by compare-and-writes alone (`conditional.rs`).
Entities read: **`woken()`**, **`woken_reading(layers)`** (the cells
about each asked of memory ahead) -- its entities waking this tick, in
Morton order, borrowed from the world as the tick found it, not from
the turn, so instructions can be queued while going through them;
**`entity_on(at)`** (the entity the tick found on a cell: its name for the tick), **`entities_in(chunk)`**.

**Queued, an instruction a write** (`turn/one_instruction.rs`;
`simulation.md`, "Instructions"): **`compare_open(compare)`**,
**`compare_close()`**: a compare queued, open until closed, one within
another; **`write(write)`**: one write, said whole as a **`Write`**
(below) -- a cell, a count (landing where the compare open last
does), an entity made, moved (within a superchunk, or over a border:
a crossing, which lands its author in both), removed, a member of an
attribute, an attribute removed, a wake;
**`write_attribute(at, attribute)`**: an attribute whole, its blocks
carried, the one write that is no `Write`.
**`compare_with_writes(compare, writes)`**: a compare, the writes
under it and its end at once, the queue found once -- every write
landing in the compare's superchunk; **`Write`**: a write as a value,
those that carry no blocks (`instruction_of`: the instruction it is
queued as and the cell it lands on). What is made of several --
an entity stepping, an attribute edited member by member --
is `../../actions/`.
**`slot_of`**: the slot of a superchunk, past the neighbours panicking;
`queue_landing_in(superchunk)`: the queue something about to be queued
goes to, and its slot -- the slot's own queue, or the queue of acts
over a border (`ACROSS`, the outbox's tenth) once the author's act is
one, which it becomes here at the first thing queued for a second
superchunk, whatever it is (`AuthorBegan`: the slot the author began
in and what its queue held before; `landed_last`, the superchunk, queue and slot of the author's last instruction, so the next for the same superchunk looks nothing up, `NO_SUPERCHUNK` until it queues; `go_across`: all the author
queued moved there, `Across` `{lands, arrived}` kept beside its
segment; `simulation.md`,
"An act over a border"); `slot_home`: the slot of the outbox's own
superchunk. **`seeing_to(at)`**: the rule is seeing to
what is on a cell -- the `author` of what it queues from here on
(`simulation.md`, "One order, wherever the borders fall").
`SLOTS` (9): an outbox's slots. A turn's fields: its `superchunk`, its
`entities`, `now`, the thread's `reader` and `entity_reader`, its
`outbox`, its `random`, the compare its instructions are queued
under (`comparing`, whether there is one) and the number its counts are `counted_from`.
`slot_between(from, to)`: the slot of one superchunk in another's
outbox, none past the neighbours. `slots()`: the nine offsets, each
with its outbox slot. `Turn::new`: a turn with nothing queued.
`begin_segment(queue, slot)`: the author's segment of a queue begun,
if the last there is another's.

`instruction.rs` (`simulation.md`, "Instructions"). **`Instruction`**:
24 bytes -- its `opcode`, sixteen bits (`small`), thirty-two (`place`:
the cell's place in its superchunk in `PLACE_BITS`, 20) and two words (`first`, `second`). **`Opcode`**,
its first two bytes, the high one its family: `CompareEnd`, `Count`;
`CompareCell`, `WriteCell`; `CompareCellFree`, `CompareEntity`,
`MakeEntity`, `MoveEntity`, `RemoveEntity`, `CrossEntity`;
`CompareAttribute`, `CompareAttributeMember`, `WriteAttributeMember`,
`WriteAttribute`, `RemoveAttribute`, `WriteWake`. One made by the function of its name
-- `compare_end`, `count`, `compare_cell`, `write_cell`,
`compare_cell_free`, `compare_entity`, `make_entity`, `move_entity`,
`remove_entity`, `cross_entity`, `compare_attribute`,
`compare_attribute_member` and `write_attribute_member` (the
attribute by its whole ID, the member by its first byte and how many
bytes it is, in `small`), `write_attribute` (an attribute whole, its
blocks carried), `remove_attribute`, `write_wake` -- over `on` (an
opcode on a cell). Read back: `at(superchunk)` its cell,
`to(superchunk)` the cell its first word is the place of,
`member()`, `is_a_compare()`, `is_a_write()`; `landing_in(slot)` and
`slot()`: the outbox slot of the superchunk it lands in, four bits of
`place`; `carried_moved_by(moved)`: the same with what it carries
beside the queue that much further on. `NO_COUNT`: what a
write counts under when it counts nothing.

`conditional.rs` (`simulation.md`, "Compare-and-write").
**`Compare`**: what a rule saw -- `Cell {layer_type, at, seen}`,
`CellFree {at}`, `Entity {at, kind}`, `Attribute {at, kind, seen}`,
`AttributeMember {at, kind, member, seen}` -- eight bytes of
what was seen at the most; `Compare::at`, the cell it is on;
`Compare::instruction`, what it is queued as;
**`Compare::attribute(at, kind, seen)`**: an attribute as its blocks
were seen, their sum.
**`Turn::compare_from_here(compare)`**: from here on what the rule
queues, for whichever superchunk, is skipped if the compare does
not hold when come to; **`Turn::compare_end()`**: no more.
`queue(lands, instruction)`: one queued -- a write under no compare
panics; `compare_open(compare)`,
`compare_close()`: a compare queued and pushed on the turn's stack of
compares open (`CompareOpen`, `NONE` an empty one), one within another up to `COMPARES_WITHIN` (4), and
popped, what is under it -- the queue's end less where it lies, and
the compares within it -- noted beside the queue (`Gated` `{instructions,
compares_within, writes}`, 8 bytes a compare).
**`Turn::count_under(first)`**: the number
the rule's first count is counted under; `counted_number(place)`: a
count's number, under `COUNTED_WHEN_APPLIED` (256) --
**`CountedWhenApplied`**, a tick's counts made as it applied.
`Queue` `{instructions, gated, carried}`: what is queued for one
superchunk, and the blocks its attributes given whole carry -- `clear`.

`apply.rs`: `Queue::apply(some, compares, superchunk, entities,
earliest, collision, applied)`: one author's instructions in the
order queued, each by its opcode, a compare that does not hold
(`holds`) jumping over what is under it -- a cell not free counted as
an entity refused -- the entities' collision plane lent as a
**`Standing`** `{superchunk, layer}` (its `write`);
`Queue::count_missed`: writes of cells missed and entities lost, of a
queue for a superchunk not hot; `Applied`: what a thread applied in the
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
back -- sorted. `WARM_TICKS`: the ticks a superchunk is warming;
`COOL_TICKS`: the ticks one is cooling -- a second of the simulation each.
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
applying, acts_over_a_border}` -- `instructions_compared` the compares come to that held,
and those that did not; `acts_over_a_border` the authors whose
instructions were applied to several superchunks at once.

**`threads_for(superchunks)`**: every thread the machine has, no more
than the superchunks. **`Simulation`** `{dispatcher, outboxes, samples, random, collision, applying_room}`:
the threads, and what a tick reuses -- an outbox a superchunk, room for
samples a thread, each superchunk's random stream -- so a tick allocates nothing once they
have grown. **`Simulation::for_superchunks(superchunks)`**: on
those; **`Simulation::new(threads)`**: on a number given, to measure
against another; **`Simulation::on(dispatcher)`**: on threads others
queue jobs on too; **`keep_entities_in(collision_plane)`**: the layer
the entities are kept in from then on; **`threads`**. `align_random(superchunks, seed)`:
each superchunk given the stream it had, or a new one from the seed and
its index. `TurnToClaim`: what a thread claims of the first phase --
a superchunk's outbox and its random stream. **`random_states()`**: each
superchunk's index and random stream's state; **`restore_random(
states)`**: taken up, as a save kept them. **`tick(arena, entities,
seed, rule)`**: the entities aligned to the arena's superchunks (those
dropped counted lost); the superchunks claimed by the threads one at a
time; the first phase runs the rule on each, a `Reader` a
thread; the second, each thread the superchunks it claims and
their entities, passes each wheel's tick, then applies what the nine
outboxes hold for it, segment by segment in the order of their authors
(`simulation.md`, "One order, wherever the borders fall"); writes to superchunks not in use counted
missed, entities made there lost (`Queue::count_missed`); every
entity named anew, the cells left cleared in the collision plane;
the outboxes emptied; the entities' tick advanced.

`tick/applying.rs` (`simulation.md`, "An act over a border"): the
second phase. **`apply_what_was_queued(dispatcher, superchunks,
entities, room, superchunk_indices, outboxes, now, collision)`**: what
each thread applied. `Room`: what it keeps from tick to tick.
`Applying`: a superchunk as it applies -- itself, its entities, its
`order`, the `next` of it, whether it has `begun`. `Phase`: what every
thread is given. `Segmented`: an author's segment queued for a
superchunk -- the author, whose outbox, which queue, which segment.
`Phase::gather(here,
order)`: what is queued for a superchunk, sorted by author;
`landed_in(source, lands)`: the hot superchunks an act lands in; `go_on(here, part, applied)`: applies down a superchunk's
order until it is done or waits at an act, then the superchunks it may
be waiting for. `tick/applying/following.rs` (`simulation.md`, "An entity over a border"):
`Following` (`Phase::following`), all that routes what is queued for
an entity to where it stands by then, made by
`Following::of_what_was_queued(outboxes)`: `named_by_others`, the
entities others named, with those authors (`nobody_named_another`:
none were); `Repointed` `{also, arrived}`, what became of a segment
naming an entity that crossed, kept in `repointed` and asked with
`repointed_of(source, from, segment)` once `any_repointed`;
`Phase::have_follow(crossed, held)`: as a crossing is applied, the later
segments naming the entity re-pointed in the orders held.

`tick/applying/who_goes_on.rs`: which thread goes on with which
superchunk. `LetGoOn` `{list, lying}`: superchunks let go on after an
act, a thread's or anyone's -- `put`, `pop`, `take` (one no thread
took up first), `empty`. On `Phase`: `next_for(part)`, a thread's next
superchunk -- its own kept, one not begun (`begin`), anyone's
(`anyones`); `take_up(superchunk, part)`; `let_lie(superchunk, of)`;
`after_waiting(waited_for, part)`: the superchunk waited for, if the
thread can take it up, else all it kept given to the thread that has
it; `give_up_those_kept(part)`; `is_something_to_do(part)`, `sleep`,
`wake(all)`. `LONGEST_REST`: the longest a thread rests between two
looks before it sleeps.

In `turn/landing.rs`: `Landing`, where instructions land as they are
applied (`SEVERAL`: whether in several superchunks, known as the code is built, so a slot's queue asks nothing of crossings; `of_slot(slot)`: a slot's superchunk and entities, if hot;
`of_named(instruction)`: where the entity it names stands now;
`of_crossing(instruction)`: the two superchunks of a crossing;
`crossed(stood, to)`: one noted) -- `Held`, a superchunk and its
entities; `LandingInOne`, a slot's queue, and `LandingInSeveral`
`{source, lands, also, crossed}`, an act over a border. In `turn/apply.rs`:
`Queue::jump_over`: over all under a compare that does not
hold; `Queue::names_alone(some, stood)`: whether a segment is of one
entity alone. `Instruction::names_an_entity`: whether an instruction
names one. `Turn::note_named(at, instruction)`: an author naming an
entity not itself, noted in `Outbox::named_by_others`.

## `simulation_time.rs`

`simulation.md`, "Simulation time". **`simulation_time`**, the module.
**`TICKS_A_SIMULATION_SECOND`** (256): the tick rate, written there
alone; `MILLISECONDS_A_SECOND`. **`SimulationTime`**: a length of
simulation time, kept as its ticks: **`seconds(seconds)`**,
**`milliseconds(milliseconds)`** to the nearest tick and one at least,
**`ticks`**, **`after(moment)`** the tick so long after another,
**`between(moment, later)`** the time from a tick to a later one,
**`part_drawn(random)`** a shorter time drawn, any tick of it as
likely, **`chance_a_tick`** the chance each tick of a thing that
takes so long on average, **`chance_each(every)`** its chance each
time something happens that happens once in `every` on average.

## `transient_data.rs`

`transient_data::TRANSIENT_DATA` names the crate's `transient_data/`
folder, where its runs would leave what they make; nothing is kept
there yet. The simulation gathers no diagnostics of its own: what the
entities hold is `../../entity_manager/`'s to gather, what the arena
holds `../../bitplane_manager/`'s.
