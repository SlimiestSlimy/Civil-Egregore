# The simulation

The rules ticked over the hot bitplanes. It reads and writes them only
through the handles the bitplane manager gives: a superchunk's layers to
read (`LayerView`), cells to read anywhere (`Reader`), and writes
applied to one superchunk (`Superchunk::apply`). The decisions behind it
are in `../../docs/civil_egregore.md`, "The speed of light", "The tick" and
"Sampling".

## Sampling

Every hot set cell of a layer chosen with one chance
(`utilities::chance::Chance`, parts in 2^32), independently, handed out
in Morton order, so the writes computed from the samples are queued in
Morton order and never sorted. No sample is wasted: the set cells are
ranked in Morton order, and the gap from one chosen rank to the next is
drawn from the geometric law -- `floor(log(u) / log(1 - p))`, `u`
uniform in `(0, 1]` -- which chooses each set cell with chance `p`.

The gap is whole-number arithmetic, `Chance::passed_over`: a
fixed-point logarithm of the draw over one of the chance, worked out
once where the chance is made; one division a sample. No float is in
it, so the same cells are chosen on every machine -- with `ln` they
were whatever each machine's maths library rounded to
(`../../utilities/docs/utilities.md`, "Chances" and "Fixed point", for
how and how precisely). A chance of always passes none over and takes
every set cell; of never, none, and nothing is drawn.

The counts find each chosen rank
without a scan: a superchunk's layer passed over whole by its count, a
chunk by its count, a count tile of 16 words by its count, a word by its
bits' count, and only the word holding a chosen cell searched -- so a
sample costs the same few counts however rare samples are.

## The tick

`Simulation::tick(arena, entities, seed, rule)`, every superchunk in two phases:

1. **Computing**: each superchunk runs the rule on itself
   (`Turn`): samples its own cells, reads any cell as the
   tick found it, queues writes into its outbox -- nine queues, by the
   superchunk they land in: itself and its eight neighbours. Farther is
   past the speed of light (1024 cells a tick, a superchunk's side), and
   panics. Nothing changes, so the threads share the arena read-only.
2. **Applying**: each superchunk applies what its own and its
   neighbours' outboxes hold for it, in a fixed order, to its own
   bitmaps only, so the threads change disjoint superchunks. Writes
   landing where no bitmap is in use are counted missed.

Each superchunk has random numbers of its own, kept from tick to tick
(`Simulation`): first seeded from the seed and its superchunk index, then
going on from where the last tick left them -- so a tick is the same
on any number of threads, and a save can keep them (`random_states`,
`restore_random`). The outboxes and room
for samples are kept between ticks.

### Compare-and-write

Every rule reads the world as the tick found it, so two may each see
a thing as it was and both queue a change that hangs on it. A
**compare-and-write** says what its rule saw and is applied only if
that still holds when the write is come to; otherwise it is
**refused**: nothing happens, and it is counted
(`WritesApplied::refused`, `TickReport::instructions_compared`).

What is compared and what is written need not be the same thing
(`Compare`). A compare is of a cell -- the number a layer holds there,
1 or 0 on a layer of a bit a cell -- or of an entity: that it still
stands where it stood, with an attribute as it was seen, or without
it. What hangs on it is a cell written (`Turn::queue_if`, which says
what was seen at the cell written as well; `Turn::queue_seen`, held
against that alone), what an
entity comes to (every instruction queued from `Turn::instructions_if`
to `Turn::instructions_as_ever`), or a count (`Turn::count_if`). So an
entity changes itself on a compare of the ground, and the ground is
changed on a compare of an entity.

There are no groups: each write is held against its own compare, when
it is come to, and nothing is applied "together". What a rule wants
to happen together it queues as writes that fail together -- all held
against the one thing, the write that changes that thing last; or a
chain, each held against what the one before wrote. A rule cannot
learn in the tick that a write of its was refused, so what must
happen all the same -- an entity woken must be put to sleep again, or
never wakes -- is queued under the compare's opposite: the cell
holding, the entity is fed; the cell lacking, it sleeps a step. One of
the two is applied, and the entity is written once.

#### No write lands on another's

Two writes of one thing in one tick, the later silently winning, is a
write-after-write hazard: what the first did is lost, and neither rule
saw the other. A tick has none:

- **A cell** is written only by a compare-and-write that says what
  the rule saw at that cell, whatever else it is held against. The
  first applied changes it; any other that saw the same is refused.
  There is no plain write in a tick, and no shape: an area is its
  cells, each a write of its own.
- **An entity is written an attribute at a time**, never whole. Each
  attribute write (`Turn::set_attribute`, `unset_attribute`,
  `set_attribute_blocks`) says what the rule saw of that attribute and
  is applied only if it is still so, or still absent: of two writing
  one attribute, the first applied does and the other is refused; two
  writing two attributes both do. This holds whoever writes -- the
  entity itself, awake, or another, in the same tick.
- **An entity changing itself** (`Turn::update`) is those writes, one
  for each attribute that differs from what the tick found, then a
  move, which carries nothing. So what another writes to it in the
  tick stands beside its own change, and each of its writes is
  applied or refused by itself: every instruction is atomic, and
  leaves the entity one that could be -- a rule that needs several
  of them to go together holds each against what the one before
  wrote, and they fail one after another.
- **An entity crossing to another superchunk** is put there whole,
  which what applies its old superchunk cannot hold against anything
  there. So the crossing is settled after: it stands only if the
  entity ended the tick where it left with the attributes it was put
  with. Written there meanwhile by another, or removed, it is
  **turned back** -- taken from where it was put, left where it stood,
  asleep until its wake, with all that was written to it
  (`InstructionsApplied::turned_back`) -- and crosses a later tick.
- **A cell an entity is put on or steps to** is checked as the
  instruction is applied: taken, a step stays and a new entity goes to
  the first free of its others, or is not put. Nothing is overwritten.
- **A removal** takes the entity with whatever was written to it that
  tick: an end, not a value lost to another.

What a rule queues twice for its own entity is its own, in the order
it queued them; the rules here do not (a sheep's two ends are under
opposite compares). Between ticks the world is written by one hand at
a time (`World::write_cells`, `put_entity`), in order.

One limit: what a write is held against is in the superchunk the
write lands in (a panic otherwise). The thread applying a superchunk
reads no other, so nothing is agreed between superchunks.

A count may hang on a write: `queue_if` names one added if the write
is applied. These come back in the tick's report
(`TickReport::counted_when_applied`), by numbers: whoever runs several
rules on a turn gives each numbers of its own (`Turn::count_under`).

The order, for each superchunk, from each neighbour's outbox in turn:
its entity instructions and its compare-and-writes together, in the
order the rule queued them,
each compare read as it is come to -- so it sees what those before it
did. It is fixed, so what is applied and what refused is the same on
any number of threads (`tests/fine/compare_and_write.rs`: drawn writes
held against their own cell or another, set against the same applied
one after another; and a cell made to decay and be eaten in one
tick).

## Entities

`../../entity_manager/src/`: what stands on the cells. An entity is a header -- a
random 64-bit ID, a type, its cell, the tick it next wakes at -- and
attributes, typed blocks of 64 bytes added and removed at run time. A superchunk
holds its entities in a bucket a chunk, sorted by cell -- Morton order
-- then ID, attributes beside, and a timer wheel of when each wakes: a
tick costs the entities waking in it. An entity is found by its cell
and ID: its cell's place searched for in a list of the places alone,
two bytes an entity, then its ID among those on the cell. A wake or
instruction naming one no longer on that cell -- moved on, or dead -- is
passed over.

**Entities never overlap**: a cell holds one. A bucket has one entity
a cell, and the superchunk an instruction lands in checks as it applies
it: a mover whose cell is taken stays where it stood, changed all the
same; a new entity is not put. A rule need not look first -- few
cells have an entity, and a step turned back costs less than looking
every step -- but can: `Turn::occupied` reads the cells
entities stand on about a cell from the buckets, a word tile being a
run of a bucket's places -- a few entities read, and only when asked:
a step onto a taken cell is turned back as it is applied, asked or
not. No bitplane of them is kept: it cost
a fifth of the ticks on 12 threads. Crossing to another
superchunk, an entity is put there as new and changed here as if
its cell there were taken; once the second phase is over, each one put
there is removed here, each superchunk removing its own leavers on
the threads (`Simulation::settle_crossings`). So its cell is
never left for one it cannot have, and between ticks every entity
stands on one cell.

They tick in the same two phases as the cells. In the first, a
superchunk's entities waking run the rule (`Turn::woken`) in
Morton order -- each tick's wakes sorted by cell, then ID, once all are
filed -- so they read and write forwards through memory,
and their instructions -- below -- are queued in the
outbox slot of the superchunk they land in, in the Morton order of the
cells the entities were found on, which is the order the buckets hold
them in: the second phase goes forwards through each bucket, as writes
do through a bitmap; in the second, each
superchunk passes its wheel's tick and applies the instructions. An
entity moving to a neighbour goes as a whole copy made in the first
phase. One put in a superchunk not hot is lost, and counted.

An entity sees the world about it at once: windows of a layer's
cells as masks (`Turn::window`), which `../../instructions/` puts
together into the area `../../pathfinding/` finds a way over. An entity
takes one pathfinding step each time it ticks, and keeps no route.

### What a rule is given

The turn, and no more: cells and entities read as the tick found them,
writes and instructions queued. What a rule makes of them -- the cells
beside an entity, the area about it, the way to what it seeks, the
instruction that carries least -- is not the simulation's: it is
`../../instructions/` (`../../instructions/docs/instructions.md`), free functions over a
turn, which the rules are written in and reach the simulation through.

**Instructions queued**, one for each thing done to an entity, each
carrying no more than it changes (`../entity_manager/`):

| instruction | queued by | what it does | carries |
|---|---|---|---|
| put | `put`, a crossing | an entity made; or put whole in the superchunk it crosses to | its attributes |
| put on the first free | `put_on_the_first_free` | a new entity made on its cell or, that taken, on the first free of some others | its attributes, and the other cells |
| move | `step`, `update` | moved to a cell, or left where it stands, to wake at a tick; its attributes as they are | nothing |
| edit | `set_attribute`, `unset_attribute`, `set_attribute_blocks`, `update` | one attribute set or removed, of any entity in reach -- the rule's own or another -- if it is still as seen | the one attribute's blocks, or none |
| remove | `remove` | removed | nothing |

A walking entity is a move a step: 32 bytes queued and none of its
attributes read or written, however many it has -- until it crosses to
another superchunk, where it goes whole. An edit is how an entity is
changed, by itself or by another: two wounding one in a tick each
write their own attribute, where two whole copies would undo each
other, and of two writing the same attribute one is refused. Every instruction that
puts an entity on a cell is checked as it is applied. Instructions
may be queued under a compare, applied only if it holds
("Compare-and-write").

### Woken entities are asked of memory ahead

At 256 ticks a second an entity moves far less than once a tick: of a
flock of hundreds of thousands a few hundred wake in one, a handful a
superchunk, each where nothing has read since it last woke. Profiled in
the renderer at the flock's peak (700,000 sheep on 64 superchunks, `perf`,
2,400 ticks a second), a third of the time was two reads waiting for
memory: the woken entity itself (`Bucket::get`, 17%) and its
attributes (`EntityEdit::get`, 15%). Pathfinding, far search and all, was
under 2%.

The wakes due in a tick are known before any is seen to
(`SuperchunkEntities::woken`), so they are asked for ahead
(`utilities::cache::prefetch`): a turn's first eight entities and four
attribute runs before the first entity is given, then, as each is
given, the entity eight on and the attributes of the one four on -- the
entity, which says where they are, having come by then.
Finding where to ask searches the places alone, two bytes an entity,
which stay in the caches.

Measured (`Civil_Egregore server pasture 60000 333 4000 64 12`, the flock
growing from 256,000): a wake 271 ns of a thread where it was 359;
11,200 to 11,800 ticks a second where it was 11,000. Distances tried:
4 and 2 without the first asked up front, 297 ns; 12 and 6, 286; 16 and
12, 276.

Still waited for: the cells about it (the 3x3 window, 10% at the peak),
and the same entity again when its instruction is applied (12%).

Their API follows the bitplanes': outside a tick, instructions are
queued (`Entities::queue_put`, `queue_remove`) and applied (`apply`), as
the arena's writes are -- queuing is the only way to change an entity;
in a tick, a turn reads entities in any hot superchunk as the tick found
them (`entity`, `entities_in`, through an `EntityReader`, as cells
through a `Reader`) and queues its instructions. The decisions behind it:
`../../docs/civil_egregore.md`, "Entities".

## Halos

Only the superchunks about the entities that matter are hot
(`src/hot.rs`, `src/halos.rs`). A **hot entity** -- one of the kind
whoever holds the world names (`Hot::About`) -- keeps its superchunk
and the eight about it hot -- as far as anything reaches in a tick, the speed of light.
Every other superchunk is cold: its cells in its image in chunk
storage, its entities and random numbers kept as a save keeps them
(`Held::cold`). After every tick the halos move to where the hot
entities stand (`Halos::move_to_hot_entities`), and nothing slow is done on the
tick -- encoding, generating and decoding are chunk storage's jobs
(`../../chunk_storage/src/jobs.rs`), queued on the one dispatcher the
world's holder makes (`utilities::dispatcher`) and gives the tick too: a
thread a core, the tick's and the jobs' alike, each with its own codec,
asleep while there is nothing to do. A thread busy with a job sits a
tick's phase out.

- **Going cold** takes `COOL_TICKS` (256) ticks: a hot superchunk no
  halo reaches is **cooling**, hot still -- ticked, read and written
  like any other -- so a hot entity stepping back and forth over an edge
  does not make the superchunks behind it flicker cold and hot. A halo
  reaching it again, it just stays hot. At the tick it is due it goes
  cold: its state kept, and its bitmaps set aside, lingering
  (`BitmapArena::make_cold_superchunk`); its changed bitmaps, copied
  out, are encoded by a job and put into the writeback ring
  in the order they went cold. The ring flushes them when it needs the
  room -- not when the superchunk goes cold -- and that too in the
  job: the changes taken out of the ring, and the image
  rewritten with them on another thread (`Job::Flush`), a superchunk's
  flushes one after another. The newest cells are always on the hot
  side: its bitmaps, hot or lingering, are held until the image holding
  their changes is in the cold pool, and only then let go.
- **Coming hot** takes `WARM_TICKS` (256) ticks: the superchunk is
  **warming**. Lingering still, it is held, to be made hot as it is,
  nothing decoded; else its image -- generated first, if it was never
  made -- is decoded by a job. It turns hot at the tick it
  is due, its entities put back and its random numbers taken up,
  waiting for the job if it is not done: so the world is the
  same however fast the threads are. Until then, to the simulation,
  it is a cold superchunk like any other: not ticked or read, writes
  to it missed, entities sent to it staying where they stood, its own
  entities and random numbers kept cold. A superchunk lingering is
  cold the same way: lingering is only the arena keeping its bitmaps.
  Cooling, warming and lingering are the halos' bookkeeping, not the
  tick's: to it one cooling is hot, one warming or lingering cold. A hot entity reaches a superchunk its halo has just reached no sooner than
  it crosses its own -- 1,024 cells, a step every 64 ticks or more --
  so long after it has turned hot. A warming is never given up: a superchunk no halo
  wants any more turns hot when it is due all the same, and is cooling
  from then -- so what a warming does is one thing, whenever a save
  falls in it. 256 ticks is short of what generating a superchunk
  takes a job (about 120 ms, against some 40 ms of ticks), so
  the tick waits for a superchunk generated: a stall taken for halos
  that follow their hot entities closely.

So between ticks the superchunks hot and not cooling, or warming for a
halo that is there still, are the halos, exactly, never both (`../../server/tests/fast/halos.rs`); a superchunk
warming takes no write and no entity until it is due; one cooling
stays hot until it is due, and for good if a halo reaches it again
before; and a superchunk gone
cold comes back as it was, to the cell, the entity and the random
number.

Passive rules tick only where hot: grass grows on every hot superchunk,
so the flock, its halos and the world may grow as far as it leads them.
An entity
kept cold whose wake passes wakes the tick its superchunk turns hot.

**The world's size** (`Hot::side()`): given one, the world is a square
of so many superchunks along a side, its origin in the middle, and
nothing outside it is ever hot, whoever wants it -- so nothing is made
there and nothing goes there, an entity sent past the edge staying
where it stood. **Forced hot** (`Hot::Forced`): no halos; every
superchunk of the world is hot and stays so, whatever its entities do.
Only a world with a size can be: one of none has no end to be hot to.

**The viewport** (`Halos::keep_viewport`): whoever holds the world
may name superchunks wanted hot besides the halos -- of what a
renderer renders, if the world's hot says so (`Hot::About.viewport`:
its camera loads superchunks, `../../server/docs/server.md`, "Halos"),
every one of them, however many -- warmed and
cooled as a halo's are, generated if never made. A move says which
superchunks it generated, made new rather than read back
(`Halos::generated`), so the holder can put entities on them.

The halos work on what the world's holder lends them for each call
(`Held`): the arena, chunk storage, the entities, the tick's random
streams, the cold states, and what generates a superchunk never made
-- the one thing of the world's making they need, so they know
nothing of how worlds are generated.

Not yet: a halo is a fixed 3x3 whatever its hot entity; and a hot
entity is found by looking through each hot superchunk's entities for
one.

## The dispatcher

Threads are never held back: a simulation ticks on every thread the
machine has, unless there are fewer superchunks than threads -- a thread
takes whole superchunks -- (`Simulation::for_superchunks`); a number is
given only to measure one against another.

The dispatcher is not the simulation's: it is `utilities::dispatcher`,
and a simulation is given one (`Simulation::on`) that others may queue
jobs on -- the world makes one for the tick and chunk storage's jobs
alike. A thread busy with a queued job sits a phase out, and the phase
is split among the rest: nothing in a tick counts on every part being
run.

The threads, started once and kept, parked between jobs: a job runs on
all of them at once, the caller's thread doing the first part, and
`run` returns only once every part has -- which is what lets a job
borrow what the caller holds (the arena, the outboxes) and the one
`unsafe` rests on. A part's panic is raised to the caller after every
part is done. The superchunks are not split among the threads
beforehand: each thread claims the next one not yet claimed (`CLAIMED`,
one at a time), in Morton order, until none is left -- so no thread
idles while another still has work. Split into a fixed run a thread, a
thread whose run was light (the halos' empty rim, a thin flock) waited
for the heaviest: 12 threads were 41% busy. What a superchunk's turn
comes to does not depend on the thread that takes it.

## Layout

| folder | what is in it |
|---|---|
| `src/hot.rs` | which superchunks are to be hot: the world's size, the hot entity |
| `src/halos.rs` | the halos moved: warming and cooling, each due at a tick, by jobs off the tick -- `halos/warming.rs` superchunks made hot and let cool, `halos/write_back.rs` write-backs and flushes landed |
| `src/turn/` | a superchunk's turn: `mod` the turn, its cells and its outbox, `entities` the entities read and the instructions queued |
| `../entity_manager/` | the entities: buckets, the timer wheel, the instructions queued -- a crate of its own |
| `src/sampling.rs` | the cells a rule is given: a layer sampled by gaps drawn against a chance |
| `src/tick.rs` | the tick: every hot superchunk's turn on the dispatcher's threads, then the writes and instructions applied |
| `src/transient_data.rs` | where runs would leave what they make; nothing yet |
| `tests/` | sampling, the tick, the entities, their instructions and the dispatcher, judged |
| `docs/` | this, and the reference, function by function |

It gathers no diagnostics and keeps no transient data of its own: what
the entities hold is the entity manager's to gather, what the arena
holds the bitplane manager's, and the tick is measured by the server's
commands (`Civil_Egregore server throughput`, `Civil_Egregore server
pasture`), on its rules.
