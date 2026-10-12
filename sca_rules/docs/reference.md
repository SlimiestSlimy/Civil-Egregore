# The SCA rules, function by function

The design is in `sca_rules.md`.

## `grass.rs`

`MEAN_TIME_TO_SPREAD` (390.625 seconds of the simulation) and
`MEAN_TIME_TO_DECAY` (3,125, in the harshest climate): what
`SPREAD_CHANCE` and `DECAY_CHANCE` are worked out from
(`SimulationTime::chance_a_tick`; once in 100,000 ticks and in 800,000
at 256 ticks a second); `SAMPLE_CHANCE` (the two together): each a
`Chance`, parts in 2^32, no float.

**`rule(turn, cell, counts)`**: the rule on one cell of grass sampled -- whoever ticks it has the simulation sample the grass at the two chances together (`Turn::each_sampled`). Each draws whether it spreads (in
`SPREAD_CHANCE` of the sum, `Rng::chance_among`) or is held against its
climate: grass set on a neighbour with none (one of the eight, drawn,
stepped on the Morton index), or its own cleared with a chance of
**`harshness(climate)`** of `HARSHEST` -- how dry its tile is times how
hot. Dirt is a
cell with no grass: it has no layer.
Returns its `RuleCounts`, their places named **`SAMPLED`**,
**`SPREADS`**, **`DECAYS`**, their names listed in **`COUNTED`**.

## `trees.rs`

The trees' layers are the terrain's (`actions::types`): `TREE`,
the cells a tree stands on, and `TREE_STAGE`, its stage, 0 to
`OLDEST_TREE_STAGE`. `MEAN_TIME_TO_SAMPLE` (39.062 seconds of the simulation) and the `SAMPLE_CHANCE` worked out from it, `SPREAD_SHARE` and `GROW_SHARE` (`Chance::HALF` each), `SEEDS_FROM` (4),
`CROWDED` (9 trees), `MEAN_TIME_TO_DIE` (312.5 seconds at the oldest stage) and the `DIE_CHANCE` a sample worked out from it, `AROUND` (8).

**`rule(turn, cell, counts)`**: the rule on one tree sampled. Every tree sampled tries to spread or grows
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
`EVAPORATION_DIVIDES_HEAT` (16), `BURNING_OFF_DIVIDES_HEAT` (4),
`PARTS_RAIN_SHEDS` (64); `TILE_SIDE` (8). **`deepens_and_thins(climate, cloud, over_water)`**:
the parts with which a cloud deepens a level, and thins one.
**`rule(turn, sampled, counts)`**: the rule on one tile of the climate
sampled, named by its first cell (`sca_rules.md`, "Weather"). It counts in the
`RuleCounts` it is handed, their places named **`SAMPLED`**, **`DEEPENED`**,
**`THINNED`**, **`BLOWN`**, **`RAINING`**, their names listed in
**`COUNTED`**.
