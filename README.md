# Civil Egregore

**Start with the [design statements](docs/design_statements.md).**
Every design decision in this repository is weighed against them.

Civil Egregore is a 2D procedural simulation game. The world is cut into
256x256 chunks, each held as layers of bitmaps, and the simulation is
built to run in parallel. What exists so far: the encoding of those
layers, chunk storage, the hot bitplanes -- a bit a cell or wider --
with their batched writes and Monte Carlo sampling; the rules running
on them -- grass spreading over dirt, trees growing -- and the first
entities, sheep eating the grass; a world generated from a seed as
islands in an ocean, heights and walls and all; worlds saved and
opened by name; and a renderer to see it all, with its menus and the
sliders its generation is tuned by.

## What is here

| folder | what it is |
|---|---|
| [`docs/design_statements.md`](docs/design_statements.md) | the design statements |
| [`docs/style_guide.md`](docs/style_guide.md) | how the code, tests and docs are written: every crate's folders, one word a thing, few tests made by generators |
| [`docs/glossary.md`](docs/glossary.md) | every word of Civil Egregore's own: what it means, what it relates to, and what it is never called |
| [`docs/performance.md`](docs/performance.md) | what measuring Civil Egregore has shown, and each optimization kept or thrown away: where memory takes over from the processor, what a tick is made of |
| [`docs/testing_protocol.md`](docs/testing_protocol.md) | how Civil Egregore is tested: diagnostics, tests and tools apart, and three tiers of test -- fine, fast, complete |
| [`docs/Civil Egregore.md`](docs/Civil Egregore.md) | what Civil Egregore is, and every decision about it so far: chunks, superchunks, layers, the simulation's plan |
| [`src/`](src/) | the `Civil_Egregore` crate: the program, which is only the list of crates with commands -- `cargo run --release -- help` |
| [`server/`](server/) | the world as a whole: made from a seed, ticked -- rules and entities together -- saved and loaded as a folder whose name is the world's; which entity keeps the world hot; its commands and its diagnostics tools |
| [`mc_rules/`](mc_rules/) | the Monte Carlo rules of the cells, a file each: grass over dirt, and trees |
| [`entity_manager/`](entity_manager/) | the entities as kept, beside the bitplane manager's cells: a bucket a chunk, attributes added and removed at run time, a timer wheel a superchunk, instructions queued and applied |
| [`entity_rules/`](entity_rules/) | the entities, a file each: so far the sheep, eating the grass |
| [`coordinates/`](coordinates/) | where things are: cells, chunks and superchunks, by Morton index, and cartesian where named |
| [`chunk_storage/`](chunk_storage/) | chunks as stored, what loading and saving work on: height maps, the layer codec, superchunk images, the cold pool and the writeback ring |
| [`worldgen/`](worldgen/) | world generation: every cell's height from the world's seed, the walls between cells more than a step apart in height, and how grass and trees lie in patches |
| [`instructions/`](instructions/) | what a rule is made of, and all it reaches the simulation through: small functions over a superchunk's turn -- cells asked and set, the cells about a cell, entities made and committed, walking |
| [`pathfinding/`](pathfinding/) | how an entity finds its way: waves and A* over an area of 16x16 cells kept as masks |
| [`renderer/`](renderer/) | Civil Egregore on the screen: a Bevy window, opening on the main menu, asking the server's host, on a thread of its own, for the cells of its viewport |
| [`gui/`](gui/) | Civil Egregore's menus, over whatever window shows it, a part a module: the main menu -- a new world set up, one saved opened, leaving -- the options Escape opens over a world, and the sliders of the numbers `utilities::tuning` names |
| [`simulation/`](simulation/) | the simulation: Monte Carlo sampling, the two-phase tick and its outboxes, a superchunk's turn -- a bucket a chunk, a timer wheel a superchunk; and what is hot: the halos about the hot entities, warming and cooling by the tick, within the world's size if it has one |
| [`bitplane_manager/`](bitplane_manager/) | the hot bitplanes: layers decoded into the bitmap arena, where cells are read and written -- writes batched -- and written back; planes of one bit a cell, or 2, 4, 8 or 16 |
| [`tessera/`](tessera/) | Tessera, the lossless encoding of a 256x256 bitmap: a project of its own, with its own [README](tessera/README.md), tests, tools and docs |
| [`bitmap/`](bitmap/) | the 256x256 bitmap every layer is, laid out in Morton order |
| [`utilities/`](utilities/) | general-purpose utilities: the thread dispatcher, commands and their parameters, the one seed tests and tools run on, the table printer and measurement reports, a seeded random source, a fixed-capacity list, the process's memory; and the settings: Civil Egregore's one folder on a machine, the file of what is changed there, the defaults written by hand in `utilities/default_settings.csv`, and a world's folder from its name |

Every crate but the root has the same folders -- `docs/`, `src/`,
`src/diagnostics/`, `src/transient_data.rs` naming a `transient_data/`
kept out of git, and `tests/`, a file a tier -- as the
[style guide](docs/style_guide.md) sets out; no crate has a `bin/`.
The root is `src/main.rs` and the shared `docs/`; its `transient_data/`
holds the one seed every crate's tests and tools run on.

Builds are for the x86-64 processors since about 2013 to 2015
(`.cargo/config.toml`, `target-cpu=x86-64-v3`): Civil Egregore is a game, for
many machines. They are one cargo workspace: one lock file and one `target/`, here at
the root, whichever folder cargo is run from, on the toolchain
`rust-toolchain.toml` names. `cargo test` at the root tests every crate
but the renderer and its menus (`gui/`), which bring Bevy. Run from a
crate's folder, cargo keeps to that crate. Only the root is a program
-- `cargo run --release` with no more said opens the window, and
`--no-default-features` builds it without the renderer: every other
crate is a library, and its diagnostics tools are run through the root,
by the crate's name -- `cargo run --release -- server pasture`,
`cargo run --release -- tessera measurement`.
`cargo run --release -- help` lists every command of every crate, what
each takes and what that is if not given: the list is made from the
code, so it is never out of date. Tessera's
external benchmarks are a workspace of their own, so the codecs they
compare against never enter this build.

`cargo windows_rr` builds for Windows what `cargo build --release`
builds for the machine it runs on -- the one program, window and
commands both -- into `target/x86_64-pc-windows-gnu/release/`. Started
by a click it opens the window and lets go of the console Windows
gives it; started with a command, it prints where it was typed. Rust brings the
compiler and its own libraries for Windows (`rust-toolchain.toml`) but
not Windows' own, which the link needs: on Linux **MinGW must be
installed** -- `mingw-w64-gcc` on Arch, `gcc-mingw-w64-x86-64` on
Debian and Ubuntu. Nothing else is needed, and nothing at all to build
for Linux.

What is changed on a machine is kept in Civil Egregore's one folder there
(`~/.local/share/Civil Egregore` on Linux): its settings, and the worlds
saved, a folder each.

Tessera depends on `bitmap/` and `utilities/` beside it; `coordinates/`
on `bitmap/`; `chunk_storage/` on those and Tessera;
`bitplane_manager/` on `chunk_storage/` and `coordinates/`;
`entity_manager/` on `coordinates/` and `bitmap/`; `simulation/` on
`bitplane_manager/` and `entity_manager/`; `instructions/` on
`simulation/`, `pathfinding/` and `worldgen/`, which know nothing of
one another and meet there; the rules, `mc_rules/` and `entity_rules/`,
on `instructions/`, through which alone they reach the
simulation; `worldgen/` on `chunk_storage/` and
`coordinates/`; `server/` on the rules, the entities and `worldgen/`;
the program, `src/`, on `server/` and Tessera; `gui/` on `utilities/`
alone; the renderer on `server/` and `gui/`.
