# Compares and writes

What a turn queues for a tick's second phase, one function an
instruction: a compare, a write, the end of a compare. Nothing here is
made of several, and nothing here reads the world. Function by
function: `reference.md`.

## Where it stands

The simulation (`../../simulation/`) keeps what a turn queued as one
stream of small instructions and applies it
(`../../simulation/docs/simulation.md`, "Instructions"). This crate is
that set, as functions over the simulation's `Turn`: each queues one.
Above it is `../../actions/` (`../../actions/docs/actions.md`): all a
turn is asked -- the reads -- and every change made of more than one
instruction: a cell set if it was clear, an entity stepping, a lamb
born. A rule depends on the actions alone and never names this crate;
the actions hand on what of it a rule uses.

The two are kept apart so that what the second phase has to apply
stays few and plain, and so that it is plain which is which: an
instruction is one thing queued, of a fixed size, that the simulation
applies; an action is a function that reads or that queues several.

## Compares

A compare says what a rule saw -- a cell holding or lacking a layer,
the number a plane holds, an entity's attribute (`compare::holds`,
`lacks`, `value`, `attribute`; `Compare`) -- and is queued as an
instruction of its own (`compare::from_here`). Come to in the second
phase, if what it says no longer holds, everything the same author
queued after it is skipped, up to the end of the compare
(`compare::end`) or the next compare given. Every write lies under
one: none is held against nothing.

A compare within another is opened and closed in turn
(`compare::open`, `compare::close`): skipped with the outer, skipping
no more than its own. The compares there are: of a cell (`holds`,
`lacks`, `value`), of a cell being free for an entity (`cell_free`),
of an entity being there still (`entity`), of an attribute whole
(`attribute`, `attribute_blocks`) or of one member of it
(`attribute_member`).

## Writes

Each a function, each one instruction, applied only if every compare
open as it was queued holds when it is come to:

- a cell (`write::cell`), with a count added as it is applied if one
  is named; a count alone (`write::count`);
- an entity's attribute -- one member (`write::attribute_member`),
  the attribute whole (`write::attribute_whole`, its blocks carried
  beside the instruction) or taken (`write::attribute_removed`) --
  and its wake (`write::wake`);
- an entity whole (`entity`): made (`entity::make`), moved, within its
  superchunk or over a border (`entity::move_to`), removed
  (`entity::remove`).

## A compare and its writes at once

Most of what is queued is one compare and a write or two, all in one
superchunk. `compare::with_writes` queues the compare, the writes and
the end in one call, the writes said as values (`Write`): the same
instructions as opening, writing and closing one at a time, the queue
found once. Before it, with every instruction queued by a call of its
own, a tick of the sheep's pasture took 6% more of the processor's
instructions than with the simulation grouping them itself; with it,
2% more.
