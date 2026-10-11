# Instructions

What a rule is made of. An instruction is one small thing asked or done
on a superchunk's turn -- a free function over the simulation's `Turn`
-- and a rule of the cells or of an entity (`../../sca_rules/`,
`../../entity_rules/`) is a few of them put together, holding only what
is its own. Function by function: `reference.md`.

## Where it stands

The simulation (`../../simulation/`) reads and writes cells and
entities, and ticks: its turn hands out windows of a layer, a cell's
bit or number, the entities waking and standing, and queues writes and
the four entity instructions -- put, move, edit, remove. It abstracts
no further. Everything a rule would otherwise write for itself is
here, once: a query is a plain call that reads the turn and answers in
the rule's terms, a change one that queues what carries least.

The instructions are the simulation's public API: all of it a rule
sees. A rule depends on the instructions and on nothing else -- its
crate names no other in its `Cargo.toml` -- and nothing that holds a
world is named by them: not the hot bitmaps, the entities' store or the
storage. What they are asked in is theirs to give, from the crate's
root: where a cell and an entity are, what a layer and an attribute
are, a turn, a tick's report, the lot drawn; the layers a world has
before a rule adds its own are `layers`. Off a turn there is one thing
more: `entities::EntitiesBetweenTicks`, where a rule puts the entities
a world starts with, lent by whoever runs the world. No world is made
here: that is the server's (`../../server/`), a rule's tests and tools
with it. What a rule lacks is added here as an instruction, not gone
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
the world hot. A neighbour is named by its **way** -- 0 to 8, row by
row from the top left, the cell itself `HERE` (4) -- only when one is
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

A rule is written for one cell or one entity: `cells::each_sampled`
and `entities::each_woken` go over them, inlined into the rule, so the
loop costs nothing; each says, as it comes to a cell or an entity,
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
Should the grass be gone, all of it is skipped at once. All that is
under a compare lands in the compare's superchunk. A tree's death is its stage put to 0 if the tree
still stands and its stage is as seen, then the tree cleared.

Nothing is applied together: each write has its own compare, read
when the write is come to, in the order the rule queued. So a rule
that wants several things to happen or not as one holds them all
against the same thing and queues last the write that changes it --
or chains them, each held against what the one before wrote.

Two things a rule must see to. What a write is held against is in the
superchunk the write lands in, so what may land in the next -- a lamb
put beside its mother -- is not held against the ground under her.
And an entity woken must be put back whatever is refused: the rule
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

An entity's own change (`entities::commit`) is such instructions: one
for each attribute it changed, held against what it saw of that
attribute, then a move. It is never put whole over itself. So another
entity may write it in the same tick, awake or asleep
(`entities::set_attribute_of`, `unset_attribute_of`), with these
guarantees:

- what is written of an attribute is applied only if the attribute is
  still what the writer saw, so no write is lost under another;
- two writing two attributes of one entity both do; two writing one,
  the first applied does and the other is refused;
- which is first is fixed, the same on any number of threads;
- which is first does not hang on where the borders fall either: it
  is the order of the writers' cells, read as a page is;
- an entity moving in the tick it is written is written all the same,
  where it now stands, and one crossing to another superchunk crosses
  with what was written.

## The same wherever the borders fall

A world shifted is the same world. What an entity does must not hang
on which chunk or superchunk it stands in, nor on a border running
between it and what it acts on: the borders are how the world is
kept, not something in it. Three things make it so
(`../../simulation/docs/simulation.md`, "One order, wherever the
borders fall"; `../../entity_manager/docs/entity_manager.md`,
"Instructions", An entity's name in a tick):

- what is queued is applied in the order of its authors' cells over
  the whole world -- `each_woken` and `cells::each_sampled` say whom
  the rule is seeing to (`Turn::seeing_to`) -- not in the order
  superchunks are gone over in;
- an entity is named, all a tick long, by the cell the tick found it
  on, so a write finds it though it moved, and a cell stood on as the
  tick began is no other's that tick;
- an entity crossing a border takes with it what was written to it.

The test of it (`../tests/fast/shifted.rs`) puts one flock on two
worlds, on the second some cells east and south so that other borders
cross it, ticks both, and holds every cell and every walker to be the
same seen from the flock, tick after tick. All the test needs is in
the test: the shift, a rule of its own -- walkers that die, eat, lay
stone, write their neighbours, make others and step blind onto each
other's cells, over and along the borders -- and one generator
(`utilities::rng::Rng`) every chance in that rule is read from, by the
tick and the walker, so that chance is the same in both worlds and
what differs is the simulation's doing. Nothing outside the test is
made for it.

Chance is another matter, and meant to be: each superchunk draws from
a random stream of its own -- the sampling of cells, a sheep's lot, a
new entity's ID -- so what is drawn is another draw where the borders
fall otherwise. What must not see a border is what is not chance.
The same of a neighbour drawn (`around::pick`): which one a draw comes
to goes by how the cells about a cell lie in their small tiles, in
Morton order; each is as likely wherever the tiles fall. The test's
walkers draw by the ways' own order instead, in the test's file, as
they draw from its generator.

Not so yet: a compare must still be in the superchunk of what it lets
be written.

## Rules ask instructions, and nothing else

A rule depends on this crate alone, and asks its turn nothing itself:
the turn is handed to an instruction. The tick and the superchunk's
lot are instructions too (`this_tick::now`, `this_tick::random`), as
cells, counts, compares and entities are. What a rule names of the
crates under this one are the words instructions are asked in -- a
cell, a layer, an entity's header, an attribute, the turn as a thing
to hand on -- and nothing that holds a world. The repository's own
tests hold it (`the_rules_ask_instructions_alone`): a rule's source
that calls anything of its turn, or names a crate under this one,
fails there.

## What an entity's rule is given

**An entity being changed** (`EntityEdit`): its attributes read, set and
removed as if already its own, nothing copied until one is changed, and
`entities::commit` picks the instructions -- a move alone if none
was, else a write of each attribute changed before it. A rule states what the entity is to be; what that costs is
not its concern.

**What must be made** (`entities::spawn`): a new entity's cell
may be taken between the rule seeing it free and the put being applied,
and the put is then refused -- nothing is put elsewhere for it. A rule
that must not lose it asks the tick after whether it stands there
(`entities::stands_beside`), and puts it again if not.

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
aligned tiles, and is a mask in each -- composed by an instruction,
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
tiles, and is a mask in each. Composing them is the instructions'
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
and back by the two places (`way_of_bit`) -- asked for the one cell
drawn or kept, never for a set. One drawn is the set bit of a rank, in
the word's own order: so which neighbour a given draw comes to goes by
how the tiles fall about the cell. That is the lot's, as a superchunk
having a lot of its own is; nothing but the draw hangs on it. A test
that takes the draws out of what it compares draws by the ways' order
itself (`../tests/fast/shifted.rs`).

Before and after, by `Civil_Egregore server pasture 1000 333 4000 4 1`,
a sheep's wake: 233 ns reading windows turned into rows; 272 ns with
the tiles first read a lookup each and turned into nine bits a cell at
a time; 198 ns with a lookup a layer and nothing turned.

## Moved out of the simulation

The neighbourhood, the area, the tiles further off, the entity made,
put to sleep and committed, and the going over cells and entities were
the turn's own; they are instructions now, the turn left with reads and
writes. The tick's instructions (`Civil_Egregore server pasture 300 333 4000 4
1`, less the same run with no ticks) went from 53.86 million to 53.77,
the world the same to the cell.
