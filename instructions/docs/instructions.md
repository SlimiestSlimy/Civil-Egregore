# Instructions

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

## What is here yet

The compares, and their end. The writes -- a cell, a member of an
attribute, an entity made, moved or removed, a wake, a count -- are
still queued by the simulation's turn in groups, a compare and its
writes at once (`Turn::queue_seen`, `step`), and are asked of it by the
actions; they are to be one function each here.
