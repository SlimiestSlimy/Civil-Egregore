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
is of a cell (`compare::layer_holds`, `layer_lacks`, `value`) or of an entity's
attribute (`compare::attribute`), and it is queued as an instruction
of its own (`compare::from_here`): if it does not hold when it is
come to, everything the rule queued after it -- cells written, what
entities come to, counts -- is skipped, up to the next compare or
`compare::end_from_here`. `compare::write` and `compare::count` are one write
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
(`world_entities::sleep`) under the opposite one -- `layer_holds`, then `layer_lacks` --
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

An entity's own change (`world_entities::set_attribute_of`, then
`world_entities::step`) is such instructions: for each attribute it
changed, the words that differ, held against what it saw of that
attribute, then a move and its wake. It is never put whole over itself. So another
entity may write it in the same tick, awake or asleep
(`world_entities::set_attribute_of`, `unset_attribute_of`), with these
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

