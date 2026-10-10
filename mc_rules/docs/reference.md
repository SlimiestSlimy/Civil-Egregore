# The Monte Carlo rules, function by function

The design is in `mc_rules.md`.

## `grass.rs`

`SPREAD_CHANCE` (once in 100,000), `DECAY_CHANCE` (once in 200,000,
with grass all round), `SAMPLE_CHANCE` (the two together): each a
`Chance`, parts in 2^32, no float.

**`rule(turn, samples)`**: hands **`cell`**, the rule for one cell of grass, to `read::cells::each_sampled`, which goes over the cells. On one superchunk's turn, every cell of grass
sampled at the two chances together; each draws a neighbour (one of the
eight, stepped on the Morton index) and whether it spreads (in
`SPREAD_CHANCE` of the sum, `Rng::chance_among`) or decays: grass set on a
neighbour with none, or its own cleared beside a grass one. Dirt is a
cell with no grass: it has no layer.
Returns its `RuleCounts`, their places named **`SAMPLED`**,
**`SPREADS`**, **`DECAYS`**, their names listed in **`COUNTED`**.

## `trees.rs`

The trees' layers are the terrain's (`instructions::layers`): `TREE`,
the cells a tree stands on, and `TREE_STAGE`, its stage, 0 to
`OLDEST_TREE_STAGE`. `SAMPLE_CHANCE` (once in 10,000), `SPREAD_SHARE` (`Chance::HALF`), `SEEDS_FROM` (4),
`CROWDED` (9), `DIE_ONE_IN` (4), `AROUND` (8).

**`rule(turn, samples)`**: hands **`tree`**, the rule for one tree, to `read::cells::each_sampled`. Every tree sampled tries to spread or grows
a stage -- or at the oldest dies one time in four, its cell and its
stage cleared. **`spread(turn, cell, stage)`**: the other trees in the
8 by 8 cells about it counted from one window; with `n` of them it goes
on `1 - n/9` of the time, never with 9; a cell of the 64 is drawn and a
tree put there if it has none. Returns its `RuleCounts`, their
places named **`SAMPLED`**, **`SPREADS`**, **`GROWN`**, **`DIED`**, their
names listed in **`COUNTED`**.
