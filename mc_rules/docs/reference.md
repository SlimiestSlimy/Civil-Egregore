# The Monte Carlo rules, function by function

The design is in `mc_rules.md`.

## `grass.rs`

`SPREAD_CHANCE` (0.001%), `DECAY_CHANCE` (0.002% with grass all round).

**`rule(turn, samples)`**: on one superchunk's turn, every cell of grass
sampled at the two chances together; each draws a neighbour (one of the
eight, stepped on the Morton index) and whether it spreads (in
`SPREAD_CHANCE` of the sum) or decays: grass set and dirt cleared on a
dirt neighbour, or the cell turned back to dirt beside a grass one.
Returns **`GrassCounts`** `{sampled, spreads, decays}`, added with `+=`.

**`tick(simulation, arena, entities, seed)`**: one tick of the rule over
every superchunk in use, on the simulation's threads.

## `trees.rs`

`TREE` (layer type 3): the cells a tree stands on. `TREE_STAGE` (4 to
7): its stage, 0 to `OLDEST` (15), over four bitplanes, the lowest bit
first. `SAMPLE_CHANCE` (0.01%), `SPREAD_SHARE` (half), `SEEDS_FROM` (4),
`CROWDED` (9), `DIE_ONE_IN` (4), `AROUND` (8).

**`rule(turn, samples)`**: every tree sampled tries to spread or grows
a stage -- or at the oldest dies one time in four, its cell and its
stage cleared. **`spread(turn, cell, stage)`**: the other trees in the
8 by 8 cells about it counted from one window; with `n` of them it goes
on `1 - n/9` of the time, never with 9; a cell of the 64 is drawn and a
tree put there if it has none. Returns **`TreeCounts`** `{sampled,
spreads, grown, died}`.
