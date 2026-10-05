# TileSim

**Start with the [design statements](docs/design_statements.md).**
Every design decision in this repository is weighed against them.

TileSim is a 2D procedural simulation game. The world is cut into
256x256 chunks, each held as layers of bitmaps, and the simulation is
built to run in parallel. What exists so far: the encoding of those
layers, chunk storage, the hot bitplanes -- a bit a cell or wider --
with their batched writes and Monte Carlo sampling; the rules running
on them -- grass spreading over dirt, trees growing -- and the first
entities, sheep eating the grass; a world generated from a seed as
islands in an ocean, heights and walls and all; and a renderer to see
it and to tune its generation in.

## What is here

| folder | what it is |
|---|---|
| [`docs/design_statements.md`](docs/design_statements.md) | the design statements |
| [`docs/style_guide.md`](docs/style_guide.md) | how the code, tests and docs are written: every crate's folders, one word a thing, few tests made by generators |
| [`docs/glossary.md`](docs/glossary.md) | every word of TileSim's own: what it means, what it relates to, and what it is never called |
| [`docs/performance.md`](docs/performance.md) | what measuring TileSim has shown, and each optimization kept or thrown away: where memory takes over from the processor, what a tick is made of |
| [`docs/testing_protocol.md`](docs/testing_protocol.md) | how TileSim is tested: diagnostics, tests and tools apart, and three tiers of test -- fine, fast, complete |
| [`docs/tilesim.md`](docs/tilesim.md) | what TileSim is, and every decision about it so far: chunks, superchunks, layers, the simulation's plan |
| [`src/`](src/) | the `tilesim` crate: the program, which is only the list of crates with commands -- `cargo run --release -- help` |
| [`world/`](world/) | the world as a whole: made from a seed, ticked -- rules and entities together, hot only in the halos about the entities that matter -- saved and loaded; its commands and its diagnostics tools |
| [`mc_rules/`](mc_rules/) | the Monte Carlo rules of the cells, a file each: grass over dirt, and trees |
| [`entity_rules/`](entity_rules/) | the entities, a file each: so far the sheep, eating the grass |
| [`coordinates/`](coordinates/) | where things are: cells, chunks and superchunks, by Morton index, and cartesian where named |
| [`chunk_storage/`](chunk_storage/) | chunks as stored, what loading and saving work on: height maps, the layer codec, superchunk images, the cold pool and the writeback ring |
| [`worldgen/`](worldgen/) | world generation: every cell's height from the world's seed, the walls between cells more than a step apart in height, and how grass and trees lie in patches |
| [`pathfinding/`](pathfinding/) | how an entity finds its way: waves and A* over an area of 16x16 cells kept as masks |
| [`renderer/`](renderer/) | TileSim on the screen: a Bevy window asking the simulation, on a thread of its own, for the cells in view; the lab, where how the world is generated is tuned by eye |
| [`simulation/`](simulation/) | the simulation: Monte Carlo sampling, the two-phase tick and its outboxes, the thread dispatcher, and the entities -- a bucket a chunk, a timer wheel a superchunk |
| [`bitplane_manager/`](bitplane_manager/) | the hot bitplanes: layers decoded into the bitmap arena, where cells are read and written -- writes batched -- and written back; planes of one bit a cell, or 2, 4, 8 or 16 |
| [`allocator/`](allocator/) | the allocator: equal-size blocks that never move, owned by their holder, taken back and handed out again |
| [`tessera/`](tessera/) | Tessera, the lossless encoding of a 256x256 bitmap: a project of its own, with its own [README](tessera/README.md), tests, tools and docs |
| [`bitmap/`](bitmap/) | the 256x256 bitmap every layer is, laid out in Morton order |
| [`utilities/`](utilities/) | general-purpose utilities: commands and their parameters, the one seed tests and tools run on, the table printer and measurement reports, a seeded random source, a fixed-capacity list, the process's memory |

Every crate but the root has the same folders -- `docs/`, `src/`,
`src/diagnostics/`, `src/transient_data.rs` naming a `transient_data/`
kept out of git, and `tests/` -- as the
[style guide](docs/style_guide.md) sets out; no crate has a `bin/`.
The root is `src/main.rs` and the shared `docs/`; its `transient_data/`
holds the one seed every crate's tests and tools run on.

Builds are for the x86-64 processors since about 2013 to 2015
(`.cargo/config.toml`, `target-cpu=x86-64-v3`): TileSim is a game, for
many machines. They are one cargo workspace: one lock file and one `target/`, here at
the root, whichever folder cargo is run from, on the toolchain
`rust-toolchain.toml` names. `cargo test` at the root tests every crate
but the renderer, which brings Bevy and is asked for by name:
`cargo run --release -p renderer`. Run from a crate's folder, cargo keeps
to that crate. Only the root and the renderer are programs: every other
crate is a library, and its diagnostics tools are run through the root,
by the crate's name -- `cargo run --release -- world pasture`,
`cargo run --release -- tessera measurement`.
`cargo run --release -- help` lists every command of every crate, what
each takes and what that is if not given: the list is made from the
code, so it is never out of date. Tessera's
external benchmarks are a workspace of their own, so the codecs they
compare against never enter this build.

Tessera depends on `bitmap/` and `utilities/` beside it; `coordinates/`
on `bitmap/`; `chunk_storage/` on those and Tessera;
`bitplane_manager/` on `chunk_storage/`, `coordinates/` and
`allocator/`; `simulation/` on `bitplane_manager/`; `entity_rules/` on
`simulation/` and `pathfinding/`; `worldgen/` on `chunk_storage/` and
`coordinates/`; `world/` on the rules, the entities and `worldgen/`;
the program, `src/`, on `world/` and Tessera; the renderer on `world/`.
