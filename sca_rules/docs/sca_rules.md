# The SCA rules

The rules the cells run by, a file each. A rule is a function of one
cell sampled, in a superchunk's turn of a tick's first phase: the
simulation samples the cells of the rule's master at the rule's
chance (`SAMPLE_CHANCE`) -- a stochastic cell automaton
(SCA), so no cell is visited that is not drawn -- and goes over them
(`../../simulation/docs/simulation.md`, "Sampling"); the rule reads the world as the tick found it, and queues
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
| `src/growing.rs` | what grass and trees share of the climate: humid enough to spread, too harsh to live |
| `src/trees.rs` | trees: spreading by how crowded they stand, growing through sixteen stages, dying |
| `src/weather.rs` | the weather: clouds deepened, thinned and blown over the climate, a tile at a time |
| `tests/fast/` | each rule alone on a plain the server makes, a file a rule |
| `docs/` | this, and the reference, function by function |

## Grass

Each tick every cell of grass may spread onto a dirt neighbour, or,
far more rarely, decay back to dirt by the climate of its tile:

- **Spreading**: a cell of grass tries to spread with `SPREAD_CHANCE`
  -- once in `MEAN_TIME_TO_SPREAD` on average, a time of the
  simulation, so as fast at any tick rate
  (`../../simulation/docs/simulation.md`, "Simulation time") --
  onto one of its eight neighbours drawn at random, if that one is dirt
  -- a cell with no grass: dirt has no layer -- and not under water,
  and as often as the air over it is humid
  (`growing::humid_enough_to_spread`): always in the most humid tile,
  never in the driest, half the time half way. So grass creeps into
  wet land and stops at the edge of dry.
- **Decay**: a cell of grass is held against its climate with
  `DECAY_CHANCE` -- once in `MEAN_TIME_TO_DECAY` on average, a very
  small rate -- and then turns back to dirt with a chance of how
  harsh the climate of its tile is (`growing::too_harsh_to_live`:
  `harshness`, of `HARSHEST`): how dry it is times how hot. In the driest and hottest it always does;
  in the most humid, or the coldest, never.

Grass no longer dies of the grass around it: it once decayed by its
share of grass neighbours, which kept a plain at two thirds grass
whatever its climate. Now what a land is covered with hangs on its
climate, and where that is mild the grass, left alone, covers it.

One sampling pass serves both, and no sample is wasted: every cell of
grass is sampled with the two chances together (`SAMPLE_CHANCE`), and
each sample draws which of the two it is -- spreading, in
`SPREAD_CHANCE` of the sum, else held against its climate. The
neighbour is stepped to on the Morton index itself, no cartesian
coordinates made; the climate is one read, of the tile the cell is
in.

The writes are queued as the samples come, in Morton order -- grass
spreading over a border into the neighbour's queue -- and applied in
the second phase, so every sample reads the world as the tick found
it. The two never touch one cell in a tick: decay clears cells that
were grass, spreading fills cells that were dirt. Each write says what
the rule saw at its cell and is applied only if the cell is still so
(`../../actions/docs/actions.md`, "Compare-and-write"), and a spread or a decay is counted as it is applied: two
samples spreading onto one cell are one cell set and one spread
counted.

## Trees

Each tick every tree is sampled with the trees' `SAMPLE_CHANCE` --
once in `MEAN_TIME_TO_SAMPLE` of the simulation on average -- and a
tree sampled does one thing, by lot:

- **It tries to spread**, in `SPREAD_SHARE` of its samples, if it is at
  least `SEEDS_FROM` old. It counts the other trees in the 8 by 8 cells
  about it (`AROUND`), one window read: the more there are the less
  likely it spreads, never with `CROWDED` or more. If it does, a cell
  of those 64 is drawn, and a tree of stage 0 is put there if there is
  none and the cell is hot, not under water, and not held in the
  collision plane by something else standing there -- and as often as
  the air over it is humid (`growing::humid_enough_to_spread`), as
  grass.
- **Else it grows** a stage (`GROW_SHARE` of its samples); or, at the
  oldest stage, may die -- once in `MEAN_TIME_TO_DIE` of standing
  there on average, `DIE_CHANCE` a sample -- its stage put to 0 if the tree still stands, then
  the cell cleared, so the next tree there starts at 0 -- and lives on
  otherwise. A tree of any stage may die of its climate too: with the
  same chance a sample times how harsh its tile's is
  (`growing::too_harsh_to_live`) -- never in the most humid or the
  coldest, as often as an old tree dies of age in the driest and
  hottest. So a forest thins where the land dries out, young trees
  with the old.
  What a tree does is counted as it is applied: a tree put where
  another was put first in the tick is refused, and not counted.

Trees stand on dirt and grass alike and change neither.

**A tree bars its cell.** Where a tree stands is not stepped on: its
cell is held in the collision plane (`COLLISION`;
`../../server/docs/server.md`, "The collision plane"). A tree is put
only if its cell is still free there as the write is applied, and
then holds it -- so of two trees put on one cell, or a tree and a
sheep stepping there in one tick, the first applied has the cell. It
is cleared as the tree dies, held against the tree being there.

## Trees: more than a bit a cell

A tree is a cell set in `TREE`, with a stage of sixteen kept over four
more bits (`TREE_STAGE`): a wide plane, four bits a cell, a cell's
number held together and read and written whole through
`cells::value` and `cells::set_value` (which says the number it read). The bit that says a tree stands
there is a plane of its own, not stage 0: sampling the trees and
counting those about one are then each one read of one plane, as for
grass, where a tree found by any of four planes being set would take
four.

## Weather

The weather's rule samples the climate plane, whose tiles are 8 by 8
cells: every tile of a hot chunk is chosen with one chance
(`MEAN_TIME_TO_SAMPLE`), whatever it holds
(`Turn::each_tile_sampled`, the simulation's), and named by its first
cell. A tile's climate is four numbers kept together -- humidity,
wind, temperature, pressure
(`../../type_registry/docs/type_registry.md`, "The climate") -- and
its cloud is a fifth, on a plane of its own, 0 to 15.

**How the five are connected** is three kinds of thing, each a small
function of its own, and the rule is those put together:

| kind | what | function |
|---|---|---|
| the air moves | pressure too far over a tile about it falls there | `pressure_falls` |
| | the wind reaches a tile about it and takes air, cloud or climate with it | `wind_blows`, `carry` |
| what a tile makes of itself | its cloud forms or clears, of its humidity, heat, pressure and what is under it; and rains | `cloud_forms`, `deepens_and_thins` |
| the land | a rise holds back every move of the air, by pressure or by wind | `passes` |

Pressure and wind are the movers; humidity, temperature and cloud are
what is moved. Pressure feeds wind -- air leaving a tile turns the
wind over it -- and wind feeds pressure -- air it pushes piles up
where winds meet.

A tile sampled does this, in order:

**1. Pressure falls** (`pressure_falls`), if any of the eight tiles
about it is more than 1 under it (`PRESSURE_GAP_AT_REST`): one of
those is drawn, each as likely as it is far under
(`pressure_falls_by`) -- the lowest the likeliest, none higher ever --
times how much the tile's own wind blows that way (`leans`: once with
no wind along the step there, `A_WIND_LEANS_PRESSURE` times more for
each unit of it), and as the land lets the air up to it (`passes`), so that up a slope
it goes less often, or not at all this time. A unit of pressure goes
there, and the air leaving turns the tile's own wind that way
(`Wind::pushed`). This is tried every sample before anything else: a
gap does not wait for a chance. The two feed one another, which is the
weather's spring: air leaving turns the wind its way, and the wind
then sends the next air the same way, past where the pressures would
have evened -- until the gap it made the other side pushes back.

**2. Else the wind blows** (`wind_blows`). Its vector draws which of
the eight tiles about it it reaches, the stronger the likelier
(`Wind::step_drawn`), and the land must let it (`passes`). There, now
and then (`CARRY_CHANCE`, `MEAN_TIME_TO_CARRY_CLIMATE`), it carries
climate (`carry`): that tile's humidity up to 4 nearer this one's,
its temperature up to 4 nearer, or its wind a step nearer this wind
(`Wind::a_step_towards`) -- winds that differ wear each other down.
Otherwise it pushes a unit of its tile's pressure onto that tile --
so winds meeting or crowding on a tile raise its pressure, which then
falls outwards -- and with it a level of its cloud, if it has one
and there is room (`cells::move_one`).

**3. Then the cloud forms or clears a level** (`cloud_forms`,
`deepens_and_thins`), unless one was just blown off, with parts of
256 added up:

- towards the humidity: a sixteenth for each level the cloud is under
  the tile's humidity level, to deepen; a sixteenth for each over it,
  to thin;
- over water -- a tile most of whose cells are under it -- a
  sixteenth of the temperature, to deepen; over land a quarter of it,
  to thin;
- of the pressure: half of how far it is under rest, to deepen -- air
  rising -- and half of how far over, to thin -- air sinking
  (`PRESSURE_DIVIDES_ITS_GAP`);
- a raining cloud sheds: a quarter more, to thin
  (`Climate::rains_above`).

**Nothing is made or lost by a move.** A unit of pressure leaves one
tile and comes to another under both tiles' compares, their climates
as the rule saw them (`give_pressure`, `cells::change_both`); a level
of cloud the same. Wherever a superchunk's border falls between the
two. So the pressure of all the tiles together never changes.

**The border of what is loaded brings the wind in.** A tile with a
tile about it that is not loaded keeps its wind
(`at_the_border_of_what_is_loaded`): nothing inside turns it or
wears it down, so what blows there blows in.

**The land** is read off the elevation plane (`ELEVATION`), a tile's
height over the ocean: the land leans the air and seldom stops it.
Down a fall of 8 heights or more all of it passes, on the level a
little less, up a rise less again, mildly -- eight parts in nine on the
level, four in five up a rise of 8, half only up one of 56
(`passes_a_slope`). A rise held the air back far harder once -- half of
it at 8 heights -- and the weather stood still against every hill.

The rule counts what it sampled, deepened, thinned, blew and found
raining, what climate it carried, and the units of pressure a wind
pushed and those that fell.

Not yet: water is not kept count of -- cloud is made and unmade, no
humidity is used up, no lake dries -- and nothing but the border
makes new wind.
