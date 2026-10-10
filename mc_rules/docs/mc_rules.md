# The Monte Carlo rules

The rules the cells run by, a file each. A rule is a function of one
superchunk's turn in a tick's first phase (`simulation/`): it samples
the cells of a layer at a chance -- Monte Carlo, so no cell is visited
that is not drawn -- reads the world as the tick found it, and queues
writes, which change nothing until the second phase. A rule knows no
other rule and no entity; the world's tick (`server/`) runs them
together with the entities (`entity_rules/`).

Two so far, grass and trees, each ticked wherever its cells are hot
(`../../server/docs/server.md`, "Halos"). The server lists them, with
the entities' rules, in one table (`server::RULES`).

## Layout

| folder | what is in it |
|---|---|
| `src/grass.rs` | grass over dirt |
| `src/trees.rs` | trees: spreading by how crowded they stand, growing through sixteen stages, dying |
| `tests/fast/` | each rule alone on a plain the server makes, a file a rule |
| `docs/` | this, and the reference, function by function |

## Grass

Each tick every cell of grass may spread onto a dirt neighbour, or
decay back to dirt the more grass is around it:

- **Spreading**: a cell of grass tries to spread with `SPREAD_CHANCE`,
  onto one of its eight neighbours drawn at random, if that one is dirt
  -- a cell with no grass: dirt has no layer -- and not under water.
- **Decay**: a cell of grass with `k` of its eight neighbours grass
  turns back to dirt with `k / 8` of `DECAY_CHANCE`: none with no grass
  around, the whole chance with grass all round.

One sampling pass serves both, and no sample is wasted: every cell of
grass is sampled with the two chances together (`SAMPLE_CHANCE`), and
each sample draws one neighbour and which of the two it tries --
spreading, in `SPREAD_CHANCE` of the sum, else decay. Decay so happens
when the neighbour drawn is grass: `k / 8` of the time, as asked, from
one neighbour read rather than eight. The neighbour is stepped to on
the Morton index itself, no cartesian coordinates made.

The writes are queued as the samples come, in Morton order -- grass
spreading over a border into the neighbour's queue -- and applied in
the second phase, so every sample reads the world as the tick found
it. The two never touch one cell in a tick: decay clears cells that
were grass, spreading fills cells that were dirt. Two samples may
spread onto one cell, which then changes once.

## Trees

Each tick every tree is sampled with the trees' `SAMPLE_CHANCE`, and a
tree sampled does one thing, by lot:

- **It tries to spread**, in `SPREAD_SHARE` of its samples, if it is at
  least `SEEDS_FROM` old. It counts the other trees in the 8 by 8 cells
  about it (`AROUND`), one window read: the more there are the less
  likely it spreads, never with `CROWDED` or more. If it does, a cell
  of those 64 is drawn, and a tree of stage 0 is put there if there is
  none and the cell is hot and not under water.
- **Else it grows** a stage; or, at the oldest stage, dies one time in
  `DIE_ONE_IN` -- the cell cleared, and its stage with it, so the next
  tree there starts at 0 -- and lives on otherwise.

Trees stand on dirt and grass alike and change neither.

## Trees: more than a bit a cell

A tree is a cell set in `TREE`, with a stage of sixteen kept over four
more bits (`TREE_STAGE`): a wide plane, four bits a cell, a cell's
number held together and read and written whole through
`cells::value` and `cells::set_value`. The bit that says a tree stands
there is a plane of its own, not stage 0: sampling the trees and
counting those about one are then each one read of one plane, as for
grass, where a tree found by any of four planes being set would take
four.
