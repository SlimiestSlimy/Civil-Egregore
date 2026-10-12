# Actions

What a rule is made of. An action is one small thing asked or done
on a superchunk's turn -- a free function over the simulation's `Turn`
-- and a rule of the cells or of an entity (`../../sca_rules/`,
`../../entity_rules/`) is a few of them put together, holding only what
is its own. Function by function: `reference.md`.

## Where it stands

The simulation (`../../simulation/`) reads and writes cells and
entities, and ticks: its turn hands out windows of a layer, a cell's
bit or number, the entities waking and standing, and queues
instructions. What it queues, one function an instruction -- a
compare, a write, the end of a compare -- is `../../compares_and_writes/`
(`../../compares_and_writes/docs/compares_and_writes.md`). Everything else a rule
would otherwise write for itself is here, once, and is an action:

- **every read**: a query is a plain call that reads the turn and
  answers in the rule's terms -- a cell, the nine about it, an area, a
  mask, the entity on a cell, the tick, the lot;
- **every change made of several instructions**: a cell set if it was
  clear is a compare and a write; an entity stepping, a lamb born, an
  attribute edited member by member are more.

The actions are all of the simulation a rule sees. A rule depends on
the actions and on nothing else -- its crate names no other in its
`Cargo.toml` -- and nothing that holds a world is named by them: not
the hot bitmaps, the entities' store or the storage. What they are
asked in is theirs to give, from the crate's root: where a cell and an
entity are, what a layer and an attribute are, a turn, a tick's
report, the lot drawn; every type a world has is `types`, the type registry whole, nothing
listed by hand; the compares a rule opens and ends itself are handed
on from the instructions (`compare`). Off a turn there is one thing
more: `entities::EntitiesBetweenTicks`, where a rule puts the entities
a world starts with, lent by whoever runs the world. No world is made
here: that is the server's (`../../server/`), a rule's tests and tools
with it. What a rule lacks is added here as an action, not gone
round.

**A module a subject.** What a rule asks of a subject, what it does to
it and the shape the answers come in are one module: six of them, and
the crate's root for the words they are asked in. They were kept by
what they do to the world -- a `read` folder, a `write` folder, the
shapes beside them -- which made three files of a mask and two of
everything else, and a rule's every call two modules deep; the
function's name says as well whether it reads or queues
(`cells::holds`, `cells::set`).

| module | what it answers, and does |
|---|---|
| `cells` | does a layer hold at a cell, is the cell hot; a wide plane's number; the square about a cell; each cell sampled -- a cell set, cleared, a wide plane's number put |
| `entities` | each entity waking -- one made, put to sleep, committed as changed, removed; those a world starts with |
| `around` | the 3x3 cells about a cell as nine bits: a layer's, those entities stand on, a free one, one drawn |
| `area` | the 16x16 cells about a cell: a layer's, several at once, those entities stand on, and the tiles further off |
| `mask` | a square of cells as bits, 4 to 1,024 cells a side: a layer read into it, whole or under a mask, and set or cleared under one |
| `walking` | the steps walls leave open, the step towards a goal, the nearest of a layer in reach |

What more than one of them needs is written once: the corner of the
square about a cell (`cells::corner_about`), the set bit of a rank
(`set_bit_of_rank`, what drawing one of nine bits and one of a mask's
cells both come to), and in `walking` where an entity stands in its
area, a cell of the area as a cell of the world, and the area less the
cells entities stand on.

## The shapes

**The cells about a cell** (`around`): it and its eight neighbours, a
set of them a word of bits (`Cells`) in Morton order -- as they lie in
the small tiles of their bitmaps ("The small tiles about a cell"),
nothing turned into rows. What is about a cell is asked once
(`around::about`), and every set made with it: the cell
(`About::centre`), its neighbours (`ring`), both (`nine`), the one a
way off (`towards`). A layer's is read at once (`around::layer`); sets
are narrowed with `&`, joined with `|`, taken from with `without`, and
asked `is_empty`: those with grass, those nothing stands on, those in
the world hot. A neighbour is named by its **way** -- 0 to 8, in
Morton order of where they lead (`NORTH_WEST`, `NORTH`, `WEST`, the
cell itself `HERE` (3), `NORTH_EAST`, `EAST`, `SOUTH_WEST`, `SOUTH`,
`SOUTH_EAST`) -- only when one is
drawn or kept: a way is the same wherever the cell is, where a set's
bits go by how the tiles fall about it.

**An area** (`area`): the 16x16 cells about a cell, a row a `u16`, the
cell itself at `AREA_CENTRE` each way -- what `../../pathfinding/`
finds a way over, its side the same (asserted where the two meet).
It is read off the four aligned tiles of 16 it lies in, two by two,
taken as they lie -- a lookup a layer -- and only then turned into
rows, the word tiles it reaches alone: rows are what a wave spreads
over cheaply, a row shifted and its neighbours ORed in.

**A mask** (`mask`): an aligned tile from 4 to 1,024 cells a side, below.

## The going over

A rule is written for one cell or one entity. The cells sampled are
gone over by the simulation (`Turn::each_sampled`), which is handed the
rule for one of them; `entities::each_woken` goes over the entities
waking, inlined into the rule, so the loop costs nothing; each says, as it comes to a cell or an entity,
that the rule is seeing to it ("The same wherever the borders fall"). `each_woken` hands each entity waking, in Morton
order by cell, with a state of the rule's -- what it counts, and
whatever it keeps from one entity to the next -- and asks memory, a few
wakes ahead, for the cells of the layers the rule says it reads
(`../../simulation/docs/simulation.md`, "Woken entities are asked of
memory ahead"). An entity that is to wake again must be put back with a
later wake tick: one not put back never wakes.

## A rule's counts

What a rule did on a turn is a `RuleCounts`: a few numbers, the same
shape for every rule, so that all of them go in one table and their
counts in one array. A rule names its counts -- a constant each, the
count's place -- and lists the names in the same order.

Some counts cannot be made as the rule runs: whether a write happens
is decided where it is applied. Those are counted as applied (below),
and whoever runs the rules adds them to the same counts.

## Compare-and-write

A rule reads the world as the tick found it, and another may change
the same thing in the same tick. So every cell a rule writes is a
**compare-and-write**: the instruction says what the rule saw at the
cell -- `cells::set`, that it was clear; `cells::clear`, that it was
set; `cells::set_value`, the number it held -- and is applied only if
the cell still holds that. Otherwise it is refused, and nothing
happens: of two rules clearing one cell, one does. Each has a form
that counts (`set_counted`, `clear_counted`, `set_value_counted`): one
added to the count named, if the write is applied.

What is compared need not be what is written (`compare`). A compare
is of a cell (`compare::holds`, `lacks`, `value`) or of an entity's
attribute (`compare::attribute`), and it is queued as an instruction
of its own (`compare::from_here`): if it does not hold when it is
come to, everything the rule queued after it -- cells written, what
entities come to, counts -- is skipped, up to the next compare or
`compare::end`. `compare::write` and `compare::count` are one write
and one count under a compare. A sheep's meal is one compare, the
grass under it being there still, and under it what the sheep comes
to and what it counts; then the grass cleared, held against itself.
Should the grass be gone, all of it is skipped at once. What is
under a compare lands wherever it lands, the compare's superchunk or the next:
a border changes nothing of it. A tree's death is its stage put to 0 if the tree
still stands and its stage is as seen, then the tree cleared.

Nothing is applied together: each write has its own compare, read
when the write is come to, in the order the rule queued. So a rule
that wants several things to happen or not as one holds them all
against the same thing and queues last the write that changes it --
or chains them, each held against what the one before wrote.

One thing a rule must see to. An entity woken must be put back whatever is refused: the rule
queues what it comes to under the compare, then what stands otherwise
(`entities::sleep`) under the opposite one -- `holds`, then `lacks` --
so one of the two is applied and the entity is written once.

No write lands on another's: every cell written says what was seen
there, an area's cells one by one (`mask::set_under`), and nothing a
rule can queue overwrites what another did in the same tick. How it is
applied, and every case:
`../../simulation/docs/simulation.md`, "Compare-and-write".

### Every instruction atomic

An instruction that writes is one compare and one write, applied
whole or not at all, and leaves the cell or the entity as one that
could be. There is nothing larger: no instruction writes two things
that must agree. An entity with several things to change in a tick
changes them by several instructions, and where they must go together
the rule makes them fail together -- all held against the one thing,
or each against what the one before wrote, so a refusal carries down
the chain. That is the care a rule takes: to ask, of each instruction
it queues, what the world is if this one is applied and the next
refused.

### One entity writing another

An entity's own change (`entities::set_attribute_of`, then
`entities::step`) is such instructions: for each attribute it
changed, the words that differ, held against what it saw of that
attribute, then a move and its wake. It is never put whole over itself. So another
entity may write it in the same tick, awake or asleep
(`entities::set_attribute_of`, `unset_attribute_of`), with these
guarantees:

- what is written of an attribute is applied only if the attribute is
  still what the writer saw, so no write is lost under another;
- two writing two attributes of one entity both do; two writing one,
  the first applied does and the other is refused;
- which is first is fixed, the same on any number of threads;
- which is first does not hang on where the borders fall either: it
  is the Morton order of the writers' cells;
- an entity moving in the tick it is written is written all the same,
  where it now stands, and one crossing to another superchunk crosses
  with what was written.

## The same wherever the borders fall

What an entity does must not hang on which chunk or superchunk it
stands in, nor on a border running between it and what it acts on,
nor on how many threads the tick is run on: the borders are how the
world is kept, not something in it. Three things make it so
(`../../simulation/docs/simulation.md`, "One order, wherever the
borders fall"; `../../entity_manager/docs/entity_manager.md`,
"Instructions", An entity's name in a tick):

- what is queued is applied in the Morton order of its authors' cells
  over the whole world -- `each_woken` and `Turn::each_sampled` say whom
  the rule is seeing to (`Turn::seeing_to`) -- not in the order
  superchunks are gone over in;
- an entity is named, all a tick long, by the cell the tick found it
  on, so a write finds it though it moved, and a cell stood on as the
  tick began is no other's that tick;
- an entity crossing a border takes with it what was written to it.

The test of it (`../tests/fast/flock_over_borders.rs`) puts one flock
on two worlds, over the corner four superchunks meet at and a way
east and south of it drawn from the seed, ticks one on one thread and
the other on all, and holds every cell and every walker to be the
same, tick after tick. (A world shifted whole is not held to be the
same world: Morton order is not kept by a shift.) All the test needs
is in the test: where the flock is put, a rule of its own -- walkers that die, eat, lay
stone and count it under a compare of the cell beside them, write
their neighbours, make others and count that under the compare of the
cell being free, and step blind onto each other's cells, over and
along the borders -- and one generator
(`utilities::rng::Rng`) every chance in that rule is read from, by the
tick and the walker. Nothing outside the test is
made for it.

Chance is another matter, and meant to be: each superchunk draws from
a random stream of its own -- the sampling of cells, a sheep's lot
-- so what is drawn is another draw where the borders
fall otherwise. What must not see a border is what is not chance.
A neighbour drawn (`around::pick`) is not chance of that kind: which
one a draw comes to goes by the ways' own order, so it is the same
neighbour wherever the tiles and the borders fall about the cell. The
test's walkers draw theirs with it, from the test's generator.

## Rules ask actions, and nothing else

A rule depends on this crate alone, and asks its turn nothing itself:
the turn is handed to an action. The tick and the superchunk's
lot are actions too (`this_tick::now`, `this_tick::random`), as
cells, counts, compares and entities are. What a rule names of the
crates under this one are the words actions are asked in -- a
cell, a layer, an entity's header, an attribute, the turn as a thing
to hand on -- and nothing that holds a world. The repository's own
tests hold it (`the_rules_ask_actions_alone`): a rule's source
that calls anything of its turn, or names a crate under this one,
fails there.

## What an entity's rule is given

**An entity being changed**: a rule reads what it keeps
(`EntityRef::attribute`), works out what it is to keep in a value of
its own, and writes that if it differs (`entities::set_attribute_of`)
-- the words that changed, held against what was seen of the
attribute -- then has it step or stay (`entities::step`). Nothing of
the entity is copied to be changed. A rule states what the entity is
to be; what that costs is not its concern.

**What must be made** (`entities::spawn`): a new entity's cell
may be taken between the rule seeing it free and the put being applied,
and the put is then refused -- nothing is put elsewhere for it. A rule
that must not lose it makes it with `entities::make` and queues, before
`entities::made`, what is to happen only if it was made -- its maker
written so, a count. Refused, none of that happens, and the maker is
as it was to try again. The cell may be in the next superchunk and the
maker in this one: the rule does not know, and need not
(`../../simulation/docs/simulation.md`, "An act over a border").

**The cells beside it** (`around`): sets of the cells about a cell,
read at once (`around::layer`), narrowed with `&`; the way to one
drawn with `pick` or `prefer`, a way a cell with `around::cell`. A
draw falls on any of those chosen among as likely as on another; which
one a given draw comes to goes by how the cells lie in their tiles --
as the lot itself goes by the superchunk. What must have its cell, as
a newborn, is put on one of those open to a step: the collision plane
holds what stands there.

**The area about it, and the way**: `area::layer` gives 16x16 cells of a
layer as masks, `Area::count` how many are set. The way over them is `walking`'s, where the
simulation's cells, the terrain's walls and `../../pathfinding/` meet.
`step_towards(turn, at, goals, passable)` gives the cell to step to
for the nearest goal by the shortest way -- no cell the collision
plane holds walked on or to, the plane and the walls read at once, none if no goal can be come to -- `step_to(turn, at, to, passable)` for one cell --
waves and A* of `../../pathfinding/`, round the entities in the way,
one step a wake.

**Walls**: the terrain's (`../../worldgen/`), two layers -- east and
south -- read as any other; a diagonal is barred unless both ways round
it are open. `around_unwalled(turn, about)` is the cells about a cell no wall is before,
a set to narrow a step's choices by -- the walls' tiles asked a cell at
a time as they lie, and not at all where they hold nothing; `area_walls(turn, centre)` the
walls of the area, which `step_towards` and `step_to` go round by
themselves. Where the wall layers are not hot, nothing bars.
`around_steppable(turn, about)` is the neighbours the collision plane
does not hold and no wall bars: the plane is asked first -- the ring
ANDed with its tiles -- and a cell hemmed in reads no wall.

**What stands in the way**: the collision plane (`COLLISION`), a bit a
cell -- 1 where something stands that bars stepping there, a tree or
an entity (`../../server/docs/server.md`, "The collision plane"). An
entity's own bit is kept by the instructions that put, move and remove
it; a rule writes none for it. A walker
asks one thing whatever the thing is: `around_steppable(turn, at)` is
the neighbours no wall is before and the plane does not hold;
`step_towards` and `step_to` leave the cells it holds out of what may
be walked on and of what may be gone to, as they do the cells entities
stand on. The far
search sees no walls: the step it gives is not taken if one bars it.

**Further off** (`walking::seek(turn, at, type)`): nothing found in the area, the same
search is made over tiles of a scale, 16 by 16 of them
(`area::of_tiles`), a tile a goal if the type holds at any of its
cells. The coarsest scale first: tiles 64 cells a side, 1,024 cells
across -- an entity's reach, and no further -- each four of the arena's
count tiles, so it is read off their counts a chunk at a time with no
cell looked at (`Turn::tiles_holding`), and says at once whether
there is any in reach and how far off. Then the finest scale whose
tiles reach so far -- 2 cells a side, 4, 8, 16, 32 -- and coarser until
one sees it, each tile a run of bits in Morton order
(`Turn::any_in_tile`). The step is towards the nearest tile holding
any, over the tiles hot, entities not looked at: it is turned back if
one is in the way. It says how far it had to look
(`SoughtStep::scale`). No route is kept here either:
each step asks again, and the nearer it comes the finer it sees.

Measured (`Civil_Egregore server pasture`, 16 superchunks, 64,000 sheep, one
thread): on pasture a third grass nothing changes, no sheep looking
further than its area; with no grass at all, every sheep seeking every
step until it starves, 14,000 ticks take 9.1 s where they took 10.2
without -- 4,700 instructions a search that finds nothing.

Measured on the sheep, the first kind written on them
(`Civil_Egregore server pasture 20000 333 4000 16 1`): the rule went from 363
lines to 266, its neighbourhood, path and attribute handling gone; of a
million wakes 283,000 are put whole where all were; the run's
instructions the same within 0.2% -- a wake is bound by memory, not by
what is carried.

## Masks

An aligned tile's cells as bits (`mask::Mask`), its side a power of
two from 4 to 1,024 -- a small tile to a superchunk, an entity's
reach -- in Morton order: the cell `(x, y)` from the tile's top left at
the bit whose index is the two interleaved, which is how the tile lies
in its bitmap, one run of bits. A mask of 8 is a word, of 16 four, of
256 a chunk's bitmap. Sets of cells are masks put together with `&`,
`|` and `!`, and a shape is a mask like any other: `Mask::disc`, the
one made so far. A mask stands for the tile of its side a cell is in
(`mask::tile`); a rule keeps its masks as room and reads into them:
the largest is 128 KiB.

- **Read**: `mask::layer` fills two masks from a layer, the cells it
  holds at and the cells hot -- the tile's words copied as they lie, a
  chunk a run for the tiles of several. `mask::layer_under` keeps the
  cells another mask has.
- **Asked**: `mask::any_under` says whether a layer holds at any cell
  of a mask: the tile's words ANDed with the mask's where they lie,
  nothing copied, and the result asked whether it is anything.
- **Written**: `mask::set_under` and `mask::clear_under` queue the
  mask's cells that the rule sees otherwise, each a compare-and-write
  of its own: a cell another changes first is left as that one made
  it. A set bit is its cell: its index added to the tile's first
  cell's.

Not yet: a shape of many cells put down anywhere touches several
aligned tiles, and is a mask in each -- composed by an action,
however many they are, as the ring about a cell is into its one to
four ("The small tiles about a cell"); it waits for entities of more
than a cell. Nor masks of the common shapes made once and shipped with
the program, nor masks kept from one read to the next. The area (16x16
about a cell, what paths are found over) is still rows, read by
windows.

## The small tiles about a cell

A bitmap's cells lie in Morton order, so a tile whose side is a power
of two and whose corner is a multiple of it is one run of bits: read
as it lies, and asked things of with an AND. A mask is such a tile's
cells in the same order; a shape put down anywhere touches several
tiles, and is a mask in each. Composing them is the actions'
work, never a rule's -- a rule asks what is about a cell, or whether
it may step, and is not shown the tiles (`tiles_about` is the crate's
own).

For one cell: it and its eight neighbours lie in one, two or four
small tiles -- 4x4 cells, 16 bits. `tiles_about::of(at)` takes the four
small tiles, two by two, they can lie in: its own, the one to the left
if it is on its tile's left column, else the one to the right, and so
up and down -- a step on the cell's index, no coordinate worked out.
The four are one word, in Morton order over their 8x8 cells, and for
each place the cell can have in them the ring about it is a word made
as the program is compiled (`About::ring_word`, with the cell
`nine_word`). A layer is read under that mask (`tiles_about::layer`):
where the four lie in one chunk, its bitmap is looked up once and each
tile is a shift of one of its words, the lot ANDed with the mask.
Whether a sheep can move at all is the ring ANDed with the collision
plane's tiles, and one compare.

The word stays a word all the way to the rule (`around::Cells`): no
set of cells is turned into rows. A neighbour is told from another by
its way, and a way is a bit of the word by a table (`About::way_word`)
-- asked for the one cell kept, never for a set. The ways are numbered
in Morton order of where they lead (`WAY_OFFSETS`; `WAY_AT`, the way
to each of the nine row by row). One drawn is not the set bit of a
rank in the word's order, which would go by how the tiles fall about
the cell: the set is turned into ways, a bit a way (`About::ways_of`,
nine shifts by the same table), and the one drawn is the way of that
rank. So a draw comes to the same neighbour wherever the cell lies
in its tiles -- no exception at a tile's, a chunk's or a superchunk's
edge.

Before and after, by `Civil_Egregore server pasture 1000 333 4000 4 1`,
a sheep's wake: 233 ns reading windows turned into rows; 272 ns with
the tiles first read a lookup each and turned into nine bits a cell at
a time; 198 ns with a lookup a layer and nothing turned.

## Moved out of the simulation

The neighbourhood, the area, the tiles further off, the entity made,
put to sleep and committed, and the going over cells and entities were
the turn's own; they are actions now, the turn left with reads and
writes. The tick's instructions (`Civil_Egregore server pasture 300 333 4000 4
1`, less the same run with no ticks) went from 53.86 million to 53.77,
the world the same to the cell.

Then what is made of several instructions: a cell set if it was
clear, an entity made with its attributes, a step, a crossing, an
attribute edited member by member, an entity changed whole. The turn
built each itself; it now queues single instructions
(`../../compares_and_writes/`), and these are built here from them. A tick of
`pasture 1000 333 4000 4 1`, less the same run with no ticks, went
from 131.8 million of the processor's instructions to 140.1 with each
queued by a call of its own, and to 134.4 with a compare and its
writes queued in one call (`compares_and_writes::compare::with_writes`); the
world the same to the cell.
