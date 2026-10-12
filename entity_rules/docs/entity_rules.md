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
(`../actions/docs/actions.md`): the 3x3
cells beside an entity as masks, the area about it, a free cell for a
newborn, the step towards a goal round whatever is in the way, and
the writes of what an entity comes to (`entities::set_attribute_of`,
`entities::step`), which queue no more than changed. The sheep's rule is 100 lines of what a sheep
decides.

A kind knows the cells it reads and writes, and no other rule. The
server is what runs the entities' rules and the cells' in one tick,
each a row of its table of rules (`server::RULES`). A rule holds no
world: the worlds it is tested and measured on are the server's to
make -- its tests here (`../tests/fast/`), the server a dependency of
the tests alone, and what they share the server's own tests'
(`../../server/tests/tests.rs`).

| file | entity |
|---|---|
| `src/sheep.rs` | sheep: they rest while fed, eat grass, walk to it when hungry, breed on lush pasture and leave thin, starve, grow old, and never stand two on a cell |

## Adding one

A new file in `src/`, named in `lib.rs`; its type and its attributes'
types rows of the type registry
(`../../type_registry/docs/type_registry.md`), which refuses a number
taken, handed to it by `actions::types`; its rule a row of
the server's table beside the others; its tests in `../tests/fast/`,
a file a kind.

## The sheep

A sheep sleeps until it next needs something, and wakes for that
alone:

- **Rests while satisfied**: fed, it sleeps where it stands until it is
  hungry again, `MEAL` after its meal -- or until its lamb is
  due, or it is grown, if that is sooner. It does not wake to wander: a
  sheep with nothing to do costs the tick nothing.
- **Eats**: hungry and on grass, it eats it, the cell back to dirt.
  Hungry and not, it walks, a step every `STEP` and up to
  `STEP_JITTER` more, drawn each wake so the flock's wakes spread over
  the ticks; it starves `STARVATION` after it grew hungry.
- **Breeds**: a grown sheep may fall pregnant on a meal taken on lush
  pasture -- `LUSH_CELLS` of the 16 by 16 cells about it grass, a
  quarter of them -- once in `MEAN_TIME_TO_CONCEIVE` of such eating on
  average (`CONCEIVE_CHANCE` a meal); `GESTATION` on,
  a lamb is born on a free cell beside it, grown `LAMBHOOD` after.
  The times are a sheep's own at a scale: its life of ten years or
  so is an hour and a quarter, so a year is seven and a half minutes
  -- a lamb grown in one, carried five months, and four lambs or so
  to the grown part of a life on lush pasture: every sheep bears, so
  that is the eight of a ewe.
  Grass left alone covers a third of the dirt and grows fastest
  covering a sixth: so a flock stops growing while the grass still
  grows back faster than it is eaten, and never strips it.
- **Roams**: a meal taken where it is not lush, the sheep
  sets off when next hungry, `ROAM` of steps one way -- 96 or so
  -- eating nothing on the way, and looks for grass where it comes to.
  Without it lambs stay beside their mothers, a flock grazes its own
  patch bare, and breeds no more though the world is green. A meal on
  lush pasture, it sets off all the same now and then: once in
  `MEAN_TIME_TO_ROAM` of such eating on average (`ROAM_CHANCE` a
  meal), so a flock spreads over land it could have stayed on. Fed, it
  sleeps as before; it is the hungry sheep that walks.
- **Dies**: of hunger, or of old age -- `LIFE` of sleep to a life
  on average: before a sleep it dies at that sleep's part of
  `LIFE`, so a long sleep is as much of a life as many short
  ones.
- **Never stands where another does**: it steps only to cells the
  collision plane does not hold, where no sheep stood as the tick
  found it; of two stepping for one cell in a tick the second is
  turned back as it is applied, and stays where it is; a lamb is born
  on a cell seen free beside its mother, who waits a step's time for
  one; and a path to grass goes round the entities in the way.
- **Walks, hungry**: onto a neighbour with grass if there is one, else
  a step along the shortest path to the nearest grass in the 16 by 16
  cells about it, and with none there to the nearest further off, as
  far as it reaches (`walking::seek`) -- one pathfinding step a
  wake, no route kept; with no grass in reach, onto any neighbour.
  Hemmed in, it stays. Never off the hot bitplanes, never through a
  wall -- up a step, down a drop of four: it hops off a height it
  could not climb back onto, and is not trapped on steep ground --
  and never onto a cell the collision plane holds -- a tree's
  (`walking::around_steppable`).

All a sheep keeps is one attribute, three blocks (`SHEEP_STATE`, laid out
as `SheepState`): when it is next hungry, when its lamb is due, when
it is grown, the tick it roams until and which of the nine cells about
it it heads for, each a member. A tick of 0 is none -- not pregnant, no
lamb, not roaming -- so what comes and goes at run time is a member
written, not a block added and taken away. They are ticks, not counts of wakes, because a sheep's
wakes are as far apart as its needs; and every time the rule adds to a
tick is said in seconds of the simulation
(`../../simulation/docs/simulation.md`, "Simulation time"), so it is
the same sheep at any tick rate.

The rule runs in a tick's first phase, as grass does, reading the
world as the tick found it. No two sheep eat one cell: a sheep eats the
cell it stands on, and a cell holds one entity. A cell a sheep eats may
decay in the same tick. So **a meal hangs on the grass**
(`../../actions/docs/actions.md`, "Compare-and-write"):
all the sheep comes to that wake -- fed, roaming set, its age, a death
of old age -- and what it counts are held against the grass under it
being there still when they are applied, and the grass is cleared
last, the meal counted with it. After them, and before the grass is
cleared, the rule queues a sleep of a step held against the grass
being gone: one of the two is applied -- refused its meal, it wakes
hungry as it was and looks again. What a sheep comes to is worked out
in a `SheepState` of the rule's own, and written if it differs from
what the tick found: the words that changed, held against what the
rule saw of the attribute, then the sheep moved or put to sleep. No
copy of the sheep is kept to be changed, and it is never put whole,
so what another entity wrote to it in the tick would stand. A meal is
one thing to happen whole and a birth another: a sheep due to bear at
a meal bears at its next wake.

**A lamb is born with its mother's knowing, or not at all** (`bear`).
A lamb is made on a cell seen free, which another lamb or a stepping
sheep may take first in the same tick. So the birth is one thing
queued under two compares: the mother as her wake left her, and,
within it, the lamb's cell free (`entities::make`). Under them the
lamb is made, the mother's `lamb_due_at` cleared and the birth
counted, until `entities::made`. The cell free, all three happen in
that tick; taken, none does -- she is pregnant and due as before, and
bears when she next wakes, a step's time on, on a cell drawn anew. No
birth is lost and none counted that was not. The lamb's cell may be in
the next superchunk: nothing in the rule says so, and nothing differs
(`../../simulation/docs/simulation.md`, "An act over a border"). She
stays where she is the tick she bears: the cell is not to be the one
she steps to. How it came to be -- what was measured, what was
thrown away -- is in `../../docs/civil_egregore.md`.

## Layout

| folder | what is in it |
|---|---|
| `src/sheep.rs` | the sheep |
| `tests/fast/` | each kind's rule alone on a plain the server makes, a file a kind |
| `docs/` | this, and the reference, function by function |
