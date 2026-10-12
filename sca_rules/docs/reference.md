# The SCA rules, function by function

The design is in `sca_rules.md`.

## `grass.rs`

`MEAN_TIME_TO_SPREAD` (390.625 seconds of the simulation) and
`MEAN_TIME_TO_DECAY` (781.25, with grass all round): what
`SPREAD_CHANCE` and `DECAY_CHANCE` are worked out from
(`SimulationTime::chance_a_tick`; once in 100,000 ticks and in 200,000
at 256 ticks a second); `SAMPLE_CHANCE` (the two together): each a
`Chance`, parts in 2^32, no float.

**`rule(turn, samples)`**: hands **`cell`**, the rule for one cell of grass, to `cells::each_sampled`, which goes over the cells. On one superchunk's turn, every cell of grass
sampled at the two chances together; each draws a neighbour (one of the
eight, stepped on the Morton index) and whether it spreads (in
`SPREAD_CHANCE` of the sum, `Rng::chance_among`) or decays: grass set on a
neighbour with none, or its own cleared beside a grass one. Dirt is a
cell with no grass: it has no layer.
Returns its `RuleCounts`, their places named **`SAMPLED`**,
**`SPREADS`**, **`DECAYS`**, their names listed in **`COUNTED`**.

## `trees.rs`

The trees' layers are the terrain's (`actions::types`): `TREE`,
the cells a tree stands on, and `TREE_STAGE`, its stage, 0 to
`OLDEST_TREE_STAGE`. `MEAN_TIME_TO_SAMPLE` (39.062 seconds of the simulation) and the `SAMPLE_CHANCE` worked out from it, `SPREAD_SHARE` and `GROW_SHARE` (`Chance::HALF` each), `SEEDS_FROM` (4),
`CROWDED` (9 trees), `MEAN_TIME_TO_DIE` (312.5 seconds at the oldest stage) and the `DIE_CHANCE` a sample worked out from it, `AROUND` (8).

**`rule(turn, samples)`**: hands **`tree`**, the rule for one tree, to `cells::each_sampled`. Every tree sampled tries to spread or grows
a stage -- or at the oldest dies one time in four, its cell and its
stage cleared, and its cell in the collision plane (`COLLISION`). **`spread(turn, cell, stage)`**: the other trees in the
8 by 8 cells about it counted from one window; with `n` of them it goes
on `1 - n/9` of the time, never with 9; a cell of the 64 is drawn and a
tree put there if it has none and the collision plane does not hold
it, its cell set there too. Returns its `RuleCounts`, their
places named **`SAMPLED`**, **`SPREADS`**, **`GROWN`**, **`DIED`**, their
names listed in **`COUNTED`**.

## `weather.rs`

The weather's planes are the registry's (`actions::types`): `CLIMATE`
and `CLOUD`. `MEAN_TIME_TO_SAMPLE` (2 seconds of the simulation) and
the `SAMPLE_CHANCE` worked out from it; `WHOLE` (256), what a sample's
chances are parts of; `PARTS_A_LEVEL_FROM_HUMIDITY` (16),
`EVAPORATION_DIVIDES_HEAT` (4), `BURNING_OFF_DIVIDES_HEAT` (4),
`PARTS_RAIN_SHEDS` (64); `TILE_SIDE` (8). **`rains(cloud)`**: above
`RAINS_ABOVE`. **`deepens_and_thins(climate, cloud, over_water)`**:
the parts with which a cloud deepens a level, and thins one.
**`rule(turn, samples)`**: hands **`tile`**, the rule for one tile, to
`cells::each_tile_sampled` (`sca_rules.md`, "Weather"). Returns its
`RuleCounts`, their places named **`SAMPLED`**, **`DEEPENED`**,
**`THINNED`**, **`BLOWN`**, **`RAINING`**, their names listed in
**`COUNTED`**.
