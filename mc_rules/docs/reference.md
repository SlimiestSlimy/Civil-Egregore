# The Monte Carlo rules, function by function

The design is in `mc_rules.md`.

## `grass.rs`

`SPREAD_CHANCE` (0.001%), `DECAY_CHANCE` (0.002% with grass all round).
`GROWING_CENTRE` (the origin's middle cell), `GROWING_RADIUS` (1,536
cells): the region grass is limited to, for the present test only.
**`grows_at(cell)`**: whether a cell is in it; **`grows_in(superchunk)`**:
whether it reaches the superchunk, by its nearest cell.

**`rule(turn, samples)`**: on one superchunk's turn the limit
reaches, every cell of grass within it
sampled at the two chances together; each draws a neighbour (one of the
eight, stepped on the Morton index) and whether it spreads (in
`SPREAD_CHANCE` of the sum) or decays: grass set and dirt cleared on a
dirt neighbour, or the cell turned back to dirt beside a grass one.
Returns **`GrassCounts`** `{sampled, spreads, decays}`, added with `+=`.

**`tick(simulation, arena, entities, seed)`**: one tick of the rule over
every superchunk in use, on the simulation's threads.
