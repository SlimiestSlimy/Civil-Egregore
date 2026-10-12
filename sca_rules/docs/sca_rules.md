# The SCA rules

The rules the cells run by, a file each. A rule is a function of one
superchunk's turn in a tick's first phase (`simulation/`): it samples
the cells of a layer at a chance -- a stochastic cell automaton
(SCA), so no cell is visited that is not drawn -- reads the world as the tick found it, and queues
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
  -- a cell with no grass: dirt has no layer -- and not under water.
- **Decay**: a cell of grass is held against its climate with
  `DECAY_CHANCE` -- once in `MEAN_TIME_TO_DECAY` on average, a very
  small rate -- and then turns back to dirt with a chance of how
  harsh the climate of its tile is (`harshness`, of `HARSHEST`): how
  dry it is times how hot. In the driest and hottest it always does;
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
  collision plane by something else standing there.
- **Else it grows** a stage (`GROW_SHARE` of its samples); or, at the
  oldest stage, may die -- once in `MEAN_TIME_TO_DIE` of standing
  there on average, `DIE_CHANCE` a sample -- its stage put to 0 if the tree still stands, then
  the cell cleared, so the next tree there starts at 0 -- and lives on
  otherwise.
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
(`cells::each_tile_sampled`), and named by its first cell. It reads
the tile's climate -- humidity, wind, temperature
(`../../type_registry/docs/type_registry.md`, "The climate") -- and
its cloud, 0 to 15, and does one of two things.

**The wind blows.** If the tile has cloud and its wind is not calm,
then with a chance of its strength over four -- a wind of 3 three
times in four -- a level of the cloud is taken from the tile and
added to the tile the wind blows towards, if that one has room
(`cells::move_one`). Both writes are under both compares, what was
seen here and what was seen there, so a level is taken only if one is
added: the wind makes no cloud and loses none, wherever a superchunk's
border falls between the two.

**Or the cloud deepens or thins a level** (`deepens_and_thins`), with
parts of 256 added up:

- towards the humidity: a sixteenth for each level the cloud is under
  the tile's humidity, to deepen; a sixteenth for each level over it,
  to thin. More humidity, a likelier level more;
- over water -- a tile most of whose cells are under it -- a
  sixteenth of the temperature, to deepen: the hotter, the more of
  the water goes up. It was a quarter, which the wind could not carry
  off: the seas stood under cloud the wind did not move;
- over land a quarter of the temperature, to thin: the hotter, the
  more is burnt off;
- a raining cloud sheds: a quarter more, to thin. Whether it rains
  is the climate's to say, of its temperature and its humidity both
  (`Climate::rains_above`): warm air holds more cloud before it
  rains, humid air less.

So a cloud gathers about its tile's humidity, some levels over it on
warm water and under it on warm land, and nowhere without end: what
pulls it back grows with how far it is. So what forms, and when it
rains, hang on humidity and temperature together. Rain is drawn, and it thins the cloud; nothing under it is
changed by it yet. That a cloud over its humidity thins of itself,
and that rain sheds, were not asked for: without them every tile of
water came to the deepest cloud and stayed.

Every write says what the rule saw, so a tile's cloud moves a level
a tick at the most. The rule counts what it sampled, deepened,
thinned, blew and found raining.

Not yet: the climate does not change -- the wind is the one worldgen
gave -- and water is not kept count of: cloud is made and unmade, no
humidity is used up, no lake dries.
