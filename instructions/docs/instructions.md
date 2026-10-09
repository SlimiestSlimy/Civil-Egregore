# Instructions

What a rule is made of. An instruction is one small thing asked or done
on a superchunk's turn -- a free function over the simulation's `Turn`
-- and a rule of the cells or of an entity (`../../mc_rules/`,
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

A rule depends on the instructions and on nothing else: its crate
names no other in its `Cargo.toml`. `Turn`, `Simulation` and
`TickReport` are handed on from here, and so is what the crates under
the instructions have that a rule names (`handed_on`): where a cell and
an entity are, what a layer and an attribute are, the world a rule is
ticked on, the lot it draws. What a rule lacks is added here, not gone
round.

Instructions are kept by what they do to the world: those that only
read it in `read/`, those that only queue a change in `write/`; one
that reads and queues a write in the same call would go in `rw/`, and
none does yet. The shapes they answer in -- nine bits, an area, a mask
-- are modules beside them.

| module | what it answers |
|---|---|
| `read::cells` | does a layer hold at a cell, is the cell hot; a wide plane's number; the square about a cell; each cell sampled |
| `read::entities` | each entity waking |
| `read::around` | the 3x3 cells about a cell as nine bits: a layer's, those entities stand on, a free one |
| `read::area` | the 16x16 cells about a cell: a layer's, several at once, those entities stand on, and the tiles further off |
| `read::mask` | a square of a layer as bits, 4 to 1,024 cells a side, whole or under a mask |
| `read::walking` | the steps walls leave open, the step towards a goal, the nearest of a layer in reach |
| `write::cells` | a cell set, cleared; a wide plane's number put |
| `write::entities` | an entity made, put to sleep, committed as changed, removed |
| `write::mask` | a layer set or cleared under a mask |
| `around`, `area`, `mask` | the shapes: nine bits and how one is drawn, an area's masks, a mask and its sets |

## The going over

A rule is written for one cell or one entity: `read::cells::each_sampled`
and `read::entities::each_woken` go over them, inlined into the rule, so the
loop costs nothing.

## What an entity's rule is given

**An entity being changed** (`EntityEdit`): its attributes read, set and
removed as if already its own, nothing copied until one is changed, and
`write::entities::commit` picks the instruction -- a move if none was,
else a put. A rule states what the entity is to be; what that costs is
not its concern.

**The cells beside it** (`around`): the 3x3 about a cell as nine bits,
read in one window (`read::around::layer`); sets of neighbours are
masks narrowed with `&`, one drawn with `pick` or `prefer`.
`read::around::occupied` gives those entities stand on, `free_beside` one that
none does -- for what must have its cell, as a newborn; a step need not
ask.

**The area about it, and the way**: `read::area::layer` gives 16x16 cells of a
layer as masks, `Area::count` how many are set, `read::area::occupied` the
entities on them. The way over them is `walking`'s, where the
simulation's cells, the terrain's walls and `../../pathfinding/` meet.
`step_towards(turn, at, goals, passable)` gives the cell to step to
for the nearest goal, `step_to(turn, at, to, passable)` for one cell --
waves and A* of `../../pathfinding/`, round the entities in the way,
one step a wake.

**Walls**: the terrain's (`../../worldgen/`), two layers -- east and
south -- read as any other; a diagonal is barred unless both ways round
it are open. `around_unwalled(turn, at)` is the neighbours of a cell no wall is before,
nine bits to narrow a step's choices by; `area_walls(turn, centre)` the
walls of the area, which `step_towards` and `step_to` go round by
themselves. Where the wall layers are not hot, nothing bars. The far
search sees no walls: the step it gives is not taken if one bars it.

**Further off** (`read::walking::seek(turn, at, type)`): nothing found in the area, the same
search is made over tiles of a scale, 16 by 16 of them
(`read::area::of_tiles`), a tile a goal if the type holds at any of its
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

A square of cells as bits (`mask::Mask`), its side a power of two from
4 to 1,024 -- an entity's reach: the general form of what `around` and
`area` are at 3 and 16. Sets of cells are masks put together with `&`,
`|` and `!`, and a shape is a mask like any other: `Mask::disc`, the
one made so far. A rule keeps its masks as room and reads into them;
none is made a read.

- **Read**: `read::mask::layer` fills two masks from a layer, the cells it
  holds at and the cells hot, a window of 8x8 at a time. `read_under`
  reads only where another mask has cells, the windows it has none in
  passed over.
- **Written**: `write::mask::set` and `write::mask::clear` queue the mask's cells as
  rectangles -- each row's runs of cells, a run the same in the rows
  under it one rectangle with them, up to 255 cells a side -- so a
  whole square is a few writes and a disc under two a row.

Not yet: masks of the common shapes made once and shipped with the
program, masks kept from one read to the next, and the same over the
cells entities stand on. A large square is read a window at a time,
which a read of whole chunks will better when a rule asks for one.

## Moved out of the simulation

The neighbourhood, the area, the tiles further off, the entity made,
put to sleep and committed, and the going over cells and entities were
the turn's own; they are instructions now, the turn left with reads and
writes. The tick's instructions (`Civil_Egregore server pasture 300 333 4000 4
1`, less the same run with no ticks) went from 53.86 million to 53.77,
the world the same to the cell.
