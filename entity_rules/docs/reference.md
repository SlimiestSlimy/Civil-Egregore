# The entities, function by function

The design is in `entity_rules.md`.

## `sheep.rs`

`SHEEP`, and its one attribute `SHEEP_STATE`, laid out as `SheepState`:
when it is next hungry, when its lamb is due, when it is grown, the
tick it roams until and which way;
its times, each a `SimulationTime` said in seconds of the simulation,
none a number of ticks: `STEP` (0.25 s) and `STEP_JITTER`
(a sixteenth) between a walking sheep's steps, `MEAL`
(15 s), `STARVATION` (90 s), `ROAM` (27 s),
`LIFE` (4,500 s: an hour and a quarter), `GESTATION` (190 s),
`LAMBHOOD` (450 s); `LUSH_CELLS` (64, of the area's 256); and
`MEAN_TIME_TO_CONCEIVE` (800 s of eating on lush pasture) with the
`CONCEIVE_CHANCE` a meal worked out from it and `MEAL`; `MEAN_TIME_TO_ROAM`
(54 s of eating on lush pasture) with its `ROAM_CHANCE` a meal, likewise.

**`rule(turn)`**: hands **`wake`**, the rule for one sheep, to `entities::each_woken`, with a **`Flock`** (the counts). Every sheep waking on the superchunk's turn sees to
what it woke for and sleeps as long as it can. Hungry (past
its `hungry_at`) and on grass, it eats it, and is hungry again `MEAL`
on; hungry `STARVATION` with no meal, it dies. Its lamb due, it is
born on a cell open to a step beside it (`around::pick`; **`bear`**:
the lamb made, the mother no longer pregnant and the birth counted
under one compare of her state and one of the cell being free,
`entities::make` to `entities::made`) -- that cell taken first, none
of it happens, and she bears at her next wake; it falls pregnant on a meal on lush pasture
(`LUSH_CELLS` of the area about it grass, `Area::count`; with
`CONCEIVE_CHANCE`) if grown; a meal on pasture not lush -- or, with `ROAM_CHANCE`, on lush -- it roams
-- when next hungry it walks one way for `ROAM`, eating nothing,
before it looks for grass; a lamb past its tick is grown. Satisfied,
it stays where it stands and wakes at the first of those ticks to come;
hungry, it walks -- onto a grass neighbour, else a step to the nearest
grass no entity stands on in the area about it, round the entities in
the way, and with none there to the nearest further off, as far as it
reaches (`walking::seek`, one step a wake), else
onto any neighbour it may step to (`around::pick`) -- none the
collision plane holds, so none a tree or another sheep stood on as the
tick found it; two stepping for one cell in a tick, the second is
turned back -- and wakes a
step's time on (**`next_step`**). What it came to is queued by
`entities::set_attribute_of`, if its state changed, and `entities::step`. Before a
sleep it dies of old age at the sleep's part of `LIFE`. Returns
its `RuleCounts`, their places named **`WOKEN`**, **`EATEN`**,
**`BIRTHS`**, **`DEATHS`**, **`SOUGHT`**, **`PATHS`**, **`FAR`** -- paths
looked for, found, and found beyond the area -- and their names listed
in **`COUNTED`**.


**`flock(entities, superchunk, count, random)`**: grown sheep, each
some way from its next meal, queued each on a cell of its own drawn at
random -- at most half the superchunk's cells' worth of them, more
panicking -- waking over the next `STEP`.
