# The entities

Every kind of entity Civil Egregore has, a file each. So far there is one:
the sheep.

## What is here, and what is not

What an entity *is* belongs to the simulation
(`entity_manager/src/`): its header and attributes, the bucket
a chunk that keeps it sorted by cell, the timer wheel a superchunk that
wakes it, the instructions it queues in a tick's first phase and the
check, as each is applied in the second, that no two stand on one cell.
The simulation knows no kind of entity.

What each kind *does* is here: a file a kind, holding

- its type and its attributes' types, numbers no other kind uses;
- its constants -- how long a meal lasts, how far apart its steps are;
- its **rule**, run on one superchunk's turn in the first phase: every
  entity of the kind waking that tick reads the world as the tick found
  it, and queues writes to the cells and changes to itself;
- what it did, counted, to be added up over the superchunks;
- how a world is given some to start with.

What every kind needs is an instruction, and not written again here
(`../instructions/docs/instructions.md`): the 3x3
cells beside an entity as masks, the area about it, a free cell for a
newborn, the step towards a goal round whatever is in the way, and
`EntityEdit`, which takes what an entity is to be and queues the instruction
that carries least. The sheep's rule is 100 lines of what a sheep
decides.

A kind knows the cells it reads and writes, and no other rule. The
server is what runs the entities' rules and the cells' in one tick,
each a row of its table of rules (`server::RULES`). A rule holds no
world: the worlds it is tested and measured on are the server's
(`../../server/tests/fast/`).

| file | entity |
|---|---|
| `src/sheep.rs` | sheep: they rest while fed, eat grass, walk to it when hungry, breed on lush pasture and leave thin, starve, grow old, and never stand two on a cell |

## Adding one

A new file in `src/`, named in `lib.rs`; its type and its attributes'
types rows of the type registry
(`../../type_registry/docs/type_registry.md`), which refuses a number
taken, handed to it by `instructions::entity_types`; its rule a row of
the server's table beside the others; its tests in the server's
(`../../server/tests/fast/`), a file a kind.

## The sheep

A sheep sleeps until it next needs something, and wakes for that
alone:

- **Rests while satisfied**: fed, it sleeps where it stands until it is
  hungry again, `MEAL_TICKS` after its meal -- or until its lamb is
  due, or it is grown, if that is sooner. It does not wake to wander: a
  sheep with nothing to do costs the tick nothing.
- **Eats**: hungry and on grass, it eats it, the cell back to dirt.
  Hungry and not, it walks, a step every `STEP_TICKS` ticks and up to
  `STEP_JITTER` more, drawn each wake so the flock's wakes spread over
  the ticks; it starves `STARVE_TICKS` after it grew hungry.
- **Breeds**: a grown sheep may fall pregnant on a meal taken on lush
  pasture -- `LUSH_CELLS` of the 16 by 16 cells about it grass, a
  quarter of them -- at one in `CONCEIVE_ONE_IN`; `GESTATION_TICKS` on,
  a lamb is born on a free cell beside it, grown `LAMB_TICKS` after.
  Grass left alone covers a third of the dirt and grows fastest
  covering a sixth: so a flock stops growing while the grass still
  grows back faster than it is eaten, and never strips it.
- **Leaves thin pasture**: a meal taken where it is not lush, the sheep
  sets off when next hungry, `ROAM_TICKS` of steps one way -- 48 or so
  -- eating nothing on the way, and looks for grass where it comes to.
  Without it lambs stay beside their mothers, a flock grazes its own
  patch bare, and breeds no more though the world is green.
- **Dies**: of hunger, or of old age -- `LIFE_TICKS` of sleep to a life
  on average: before a sleep of so many ticks it dies at so many in
  `LIFE_TICKS`, so a long sleep is as much of a life as many short
  ones.
- **Never stands where another does**: a step onto a cell an entity
  stands on is turned back as it is applied, and the sheep stays where
  it is -- it does not look first, few cells having one; a lamb is born
  on a cell seen free beside its mother, who waits a step's time for
  one; and a path to grass goes round the entities in the way.
- **Walks, hungry**: onto a neighbour with grass if there is one, else
  a step along the shortest path to the nearest grass in the 16 by 16
  cells about it, and with none there to the nearest further off, as
  far as it reaches (`read::walking::seek`) -- one pathfinding step a
  wake, no route kept; with no grass in reach, onto any neighbour.
  Hemmed in, it stays. Never off the hot bitplanes, and never through a
  wall.

When it is next hungry (`HUNGRY_AT`), when its lamb is due
(`PREGNANT`) and when it is grown (`LAMB`) are attributes, each a tick;
the way it roams another (`ROAMING`: the tick it roams until, and in
its low four bits which of the nine cells about it it heads for). All
but the first are added and removed at run time, as attributes are
meant to be. They are ticks, not counts of wakes, because a sheep's
wakes are as far apart as its needs.

The rule runs in a tick's first phase, as grass does, reading the
world as the tick found it: two sheep may eat one cell in a tick, which
then changes once. How it came to be -- what was measured, what was
thrown away -- is in `../../docs/civil_egregore.md`.

## Layout

| folder | what is in it |
|---|---|
| `src/sheep.rs` | the sheep |
| `docs/` | this, and the reference, function by function |
