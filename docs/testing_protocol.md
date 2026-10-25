# How Civil Egregore is tested

> **DEPRECATED** -- out of date, kept until the docs rework at 0.8: not a source of names or designs. Names: `newserver/GLOSSARY.md`.

Few tests, each a generator of many cases. Tessera is a project of its
own, tested in its own workspace (`tessera/docs/testing_protocol.md`).

## Tests judge, diagnostics gather

- **Tests** judge: pass or fail.
- **Diagnostics** gather data -- what an arena holds, what a tick
  took, a flock's census -- and write it down for comparisons; they
  never judge. They are the only code that makes transient data, and
  are being redone from scratch.

## Where tests are

- **A crate's own tests** (`<crate>/tests/`) check its invariants
  alone: a bitmap's cells, a layer encoded and back, a path found, the
  simulation's skip tags.
- **The root's tiers** (`tests/fine/`, `tests/fast/`, `tests/complete/`
  at the root) test what crosses crates: worlds the server makes,
  ticked with the base game's rules -- the sheep, grass, trees,
  halos, saves, the host. A rule is tried on a world, and every world
  is the server's to make (`server::start_world`), so the base game's
  tests are here.
- **What tests share** is in the tests too: `tests/shared/` at the
  root (`worlds.rs` -- a world in a scratch folder of its own,
  `one_flock_world`, `put_flock`; `walkers.rs`), and a crate's
  `tests/shared/`. Nothing in the program is there for a test: no
  hook, mode or parameter. A test that needs a rule of its own writes
  it in the test, through the simulation's API.

A tier is one test program, a topic a file in its folder
(`tests/fast/halos.rs`). A crate has the tiers it has tests for.

## Three tiers

| tier | what runs | how long | command |
|---|---|---|---|
| fine | each test pinning one behaviour: the edges written by hand, and cases drawn from the seed judged against the plainest way of working the answer out | instant | `cargo test --release --test fine` |
| fast | small worlds grown from a seed and run some ticks: what the rules and the tick come to, judged within bounds or against a second run | the whole tier under 60 seconds | `cargo test --release --test fast` |
| complete | more superchunks, more seeds, far more ticks; `#[ignore]`d | minutes at most | `cargo test --release --test complete -- --ignored` |

`cargo test --release` at the root runs every crate's fine and fast
tiers but the client's, which brings Bevy and has no tests;
`--workspace` adds the client to the build. A test belongs to the
lowest tier it can: by hand if one case shows it, a seeded run only if
it takes one, the complete tier only if it takes long. A fast test that
grows past the tier's minute shrinks its world or moves to complete.

| where | fine | fast | complete |
|---|---|---|---|
| root `tests/` | acts over a border, compare and write, the docs, entities moving and waking, an entity over a border, instructions | collision, commands, double writes, a flock over borders, grass, halos, the host, paging, sheep, the tick, trees, walking, weather, the world hash, worlds saved and loaded | worlds run long |
| `utilities` | chance, dispatcher, fixed_list, hash, rng, settings, tuning | | |
| `utilities/bitmap` | cells, shapes, Morton order, windows | | |
| `utilities/coordinates` | the indices nested, in Morton order, stepping as cartesian coordinates do | | |
| `protocol/type_registry` | a type registered once by its name, frozen types found again | | |
| `server/simulation` | entities, simulation time, skip tags | area, around, mask | |
| `server/bitplane_manager` | counts and windows, hot bitmaps, writes, writing back | | |
| `server/chunk_storage` | the cold pool paged | | |
| `server/entity_manager` | attributes of any length put, found, edited and saved | | |
| `server/pathfinding` | paths and waves, walls, the Morton search | | |
| `server/basegame/worldgen` | | heights, walls, superchunks made in any order | walls over many seeds; no seam between superchunks |

The client is judged by eye, in the window.

## One seed, rolled every few runs

No test has a seed of its own written in it. Every seeded test takes
the one seed in `transient_data/test_seed.csv` at the workspace's top,
out of git (`utilities/test_seed.rs`, which each test program takes by
`#[path]`):

- `TestSeedHandle::open().seed()` is one use; after 5 the next rolls a
  fresh seed, so nothing passes for long on one seed alone. The fine
  tier's `fine_seed()` counts no use.
- `FREEZE_SEED=0x...` runs every test on that seed and leaves the file
  alone, to run a failure again; `FREEZE_SEED=` (empty) keeps the
  file's seed from rolling. Deleting the file rolls one.
- A seed is 64 bits, written everywhere in hexadecimal.

A case that can be drawn is drawn: a test's positions, counts, sizes and
timings come from the seed, so each roll tries others; what is written
by hand is what a draw would seldom land on -- a world's corner, a
superchunk's border. A test must hold on any seed: it asserts a bound,
or an equality between two runs, never a number one seed happened to
give. Climate is known broken until its rework: a climate-dependent
test failing on some seed is expected, and the ones that fail too often
are ignored, naming the rework.

## The docs are tested too

The root's fine tier (`tests/fine/docs.rs`) reads every markdown file
and every source:

- `the_rules_ask_the_simulation_api_alone`: no source of the base
  game's rules names anything of the simulation but its API.
- While crate docs are paused, ignored: `the_docs_name_what_is_there`
  (every link, path in backticks and name in backticks of the docs is
  in the workspace), `every_item_is_in_its_crates_docs`, and
  `the_code_points_at_docs_that_are_there` (every doc a comment names
  is there, with the section it quotes).

They catch a file moved or an item renamed. They do not catch a doc
that names a thing rightly and says something false of it: that takes
reading.

## What is measured, and how

Speed is not a test. A change to a hot path is measured against the
commit before it: both built (`--no-default-features`, no client), the
same world made by each, and alternating rounds of

```
Civil_Egregore --seed 0x5eed1234 --new [bench --side 8 --sheep 40 --forced_hot []]
Civil_Egregore --run [bench --ticks 10000]
```

their medians compared. While the window or another run is on the
machine, times are skewed: the instructions counted
(`perf stat -e instructions:u`) are not.

A change is kept if the tests pass, `cargo clippy --release --workspace
--all-targets` is silent, and it measures no worse -- or its cost is
written down beside what it buys.
