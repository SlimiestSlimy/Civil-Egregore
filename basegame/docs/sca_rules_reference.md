# The SCA rules, function by function

The design is in `sca_rules.md`.

## `grass.rs`

`MEAN_TIME_TO_SPREAD` (2,600 seconds of the simulation) and
`MEAN_TIME_TO_DECAY` (20,800, in the harshest climate): what
`SPREAD_CHANCE` and `DECAY_CHANCE` are worked out from
(`SimulationTime::chance_a_tick`; once in 100,000 ticks and in 800,000
at 256 ticks a second); `SAMPLE_CHANCE` (the two together): each a
`Chance`, parts in 2^32, no float.

**`rule(turn, cell, counts)`**: the rule on one cell of grass sampled -- whoever ticks it has the simulation sample the grass at the two chances together (`Turn::each_sampled`). Each draws whether it spreads (in
`SPREAD_CHANCE` of the sum, `RandomSource::chance_among`) or is held against its
climate: grass set on a neighbour with none (one of the eight, drawn,
stepped on the Morton index) if the air there is humid enough this
time, or its own cleared if its climate is too harsh this time
(`growing.rs`). Dirt is a
cell with no grass: it has no layer.
Returns its `RuleCounts`, their places named **`SAMPLED_COUNT`**,
**`SPREADS_COUNT`**, **`DECAYS_COUNT`**, their names listed in **`COUNT_NAMES`**.

## `growing.rs`

What grass and trees share of the climate. **`climate_harshness(climate)`** of
**`HARSHEST_CLIMATE`**: how dry a tile is times how hot.
**`humid_enough_to_spread(turn, onto_cell)`**: drawn, as likely as the tile
of `onto_cell` is humid; always where no climate is hot.
**`too_harsh_to_live(turn, cell)`**: drawn, as likely as the tile of
`cell` is harsh; never where no climate is hot.

## `trees.rs`

The trees' layers are the terrain's (`actions::types`): `TREE`,
the cells a tree stands on, and `TREE_STAGE`, its stage, 0 to
`OLDEST_TREE_STAGE`. `MEAN_TIME_TO_SAMPLE` (260 seconds of the simulation) and the `SAMPLE_CHANCE` worked out from it, `SPREAD_SHARE` and `GROW_SHARE` (`Chance::HALF_THE_TIME` each), `SEEDS_FROM` (4),
`CROWDED_AT` (9 trees), `MEAN_TIME_TO_DIE` (2,080 seconds at the oldest stage) and the `DIE_CHANCE` a sample worked out from it, `AROUND` (8).

**`rule(turn, cell, counts)`**: the rule on one tree sampled. Every tree sampled tries to spread or grows
a stage -- or dies (**`tree_dies(turn, cell, stage)`**: its cell and its
stage cleared, and its cell in the collision plane, `COLLISION`): of
age at the oldest stage one time in four, and at any stage of its
climate, one time in four times how harsh its tile's is
(`growing::too_harsh_to_live`). **`tree_tries_to_spread(turn, cell, stage)`**: the other trees in the
8 by 8 cells about it counted from one window; with `n` of them it goes
on `1 - n/9` of the time, never with 9; a cell of the 64 is drawn and a
tree put there if it has none, the collision plane does not hold
it and the air there is humid enough this time
(`growing::humid_enough_to_spread`), its cell set there too. Returns its `RuleCounts`, their
places named **`SAMPLED_COUNT`**, **`SPREADS_COUNT`**, **`GROWN_COUNT`**, **`DIED_COUNT`**, their
names listed in **`COUNT_NAMES`**.

## `weather.rs`

The weather's planes are the registry's (`actions::types`): `CLIMATE`,
`CLOUD` and `ELEVATION` (`sca_rules.md`, "Weather"). `MEAN_TIME_TO_SAMPLE` (13 seconds of the simulation) and
the `SAMPLE_CHANCE` worked out from it; `WHOLE_PARTS` (256), what a sample's
chances are parts of; `PARTS_A_LEVEL_FROM_HUMIDITY` (16),
`EVAPORATION_DIVIDES_HEAT` (16), `BURNING_OFF_DIVIDES_HEAT` (4),
`PARTS_RAIN_SHEDS` (64), `PRESSURE_DIVIDES_ITS_GAP` (2); `TILE_SIDE` (8);
`A_WIND_LEANS_PRESSURE` (4); `MEAN_TIME_TO_CARRY_CLIMATE` (400 seconds), `CARRY_CHANCE`,
`CARRIED_OF_A_CLIMATE` (3). The small functions: `tile_beside(turn, tile,
(tiles_across, tiles_down))`, a tile about one and its climate;
`at_the_border_of_what_is_loaded(turn, tile)`; `air_passes(turn, from_tile, to_tile,
whole_parts)`, what the land lets through; `give_pressure(turn, from, to,
counted)`, a unit moved under both compares; `wind_leans_pressure(wind, step_tiles)`, how much likelier pressure falls the way the wind blows; `pressure_falls(turn,
sampled_tile, climate_here)`; `carry_climate(turn, climate_here, towards_tile, climate_there)`;
`wind_blows(turn, sampled_tile, climate_here, cloud)`;
**`deepens_and_thins(climate, cloud, over_water)`**, the parts with
which a cloud deepens a level, and thins one; `cloud_forms(turn,
sampled, here, cloud)`. **`rule(turn, sampled, counts)`**: the rule on
one tile of the climate sampled, named by its first cell. It counts in
the `RuleCounts` it is handed, their places named **`SAMPLED_COUNT`**,
**`DEEPENED_COUNT`**, **`THINNED_COUNT`**, **`BLOWN_COUNT`**, **`RAINING_COUNT`**,
**`CARRIED_COUNT`**, **`PUSHED_COUNT`**, **`EVENED_COUNT`**, their names listed in
**`COUNT_NAMES`**.
