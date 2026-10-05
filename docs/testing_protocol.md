# How Civil Egregore is tested

One protocol for every crate, part of the
[style guide](style_guide.md): few tests, each a generator of many
cases. Tessera's own (`tessera/docs/testing_protocol.md`) adds what an
encoding needs: its seed file, its adversarial searches.

## Three parts, kept apart

- **Diagnostics** (`<crate>/src/diagnostics/`) gather data -- what an
  arena holds, what a tick took, a flock's census -- and never judge or
  print it.
- **Tests** (`<crate>/tests/`) judge: pass or fail.
- **Tools** (`<crate>/src/diagnostics/tool`) print what the
  diagnostics gather, as tables, and keep it in
  `<crate>/transient_data/measurements/`, out of git. A tool is a
  function, not a program: the crate lists its tools as commands
  (`utilities::commands`), each with the parameters it takes and what
  each is if not given, and `Civil_Egregore <crate> <tool> [parameters]` runs
  one -- `cargo run --release -- server pasture 300 333 4000 4 1`.
  `cargo run --release -- help` lists every crate's commands and their
  parameters; a report says the line it ran on, defaults filled in.

## Three tiers of test

Each tier is one file in a crate's `tests/`, so one test program:
`fine.rs`, `fast.rs`, `complete.rs`, a topic a module in it
(`mod sheep` in `tests/fast.rs`). A crate has the tiers it has tests
for, and no others. What two tiers share -- a check, a world compared
whole, a seed found -- is in `tests/tests.rs`, a module of each tier
that uses it and no test program of its own (`[[test]] name = "tests"`,
`test = false`, in the crate's `Cargo.toml`); a crate whose tiers share
nothing has none.

| tier | what runs | how long | command |
|---|---|---|---|
| fine | one case a test, made by hand: a bitmap drawn, a cliff placed, two entities on two cells -- each pinning one behaviour | instant | `cargo test --test fine` |
| fast | small worlds grown from a seed and run a few thousand ticks: what the rules and the tick come to, judged within bounds or against a second run | seconds | `cargo test --test fast` |
| complete | more superchunks, more seeds, far more ticks; `#[ignore]`d, and run in release | minutes at most | `cargo test --release --test complete -- --ignored` |

Plain `cargo test`, at the root, runs fine and fast of every crate but
the renderer and its menus (`gui`). `cargo test --release -- --ignored` runs every complete
tier, Tessera's with them.

A test belongs to the lowest tier it can be: by hand if one case shows
it, a seeded run only if it takes one, the complete tier only if it
takes long. A test that only prints belongs to none: it is a tool.

| crate | fine | fast | complete |
|---|---|---|---|
| `bitmap` | bitmap, morton, window | | |
| `bitplane_manager` | bitplane_manager, writes | | |
| `chunk_storage` | chunk_storage | | |
| `coordinates` | coordinates | | |
| `instructions` | | walking | |
| `pathfinding` | pathfinding | | |
| `utilities` | commands, dispatcher, fixed_list, hash, process_memory, rng, settings, table | | |
| `gui` | tuning (asked for by name: `cargo test -p gui`) | | |
| `simulation` | entities, instructions | sampling, tick | |
| `mc_rules` | | grass, and only within its limit, for now | |
| `entity_rules` | | sheep | |
| `worldgen` | | terrain | walls over many seeds; no seam between superchunks |
| `server` | | halos: hot superchunks are the halos, a cold one comes back as it was, a world of a size is hot within it only; world: saves, loads, walls; commands | a world stopped every 5,000 ticks; a flock lasting 300,000 |
| `tessera` | fine | fast | complete |

## One seed, rolled every few runs

No test or tool has a seed of its own written in it. Every seeded run,
in whatever crate, starts from the one seed in `transient_data/seed.csv`, at
the top of the workspace and out of git (`utilities::seed`):

- A run that counts (`counted()`) is one use; after 5 the next run rolls
  a fresh seed by itself, so nothing passes for long on one seed alone.
- A test that needs several seeds takes consecutive ones from it; one
  that needs a kind of world -- land about the origin, say -- takes the
  first seed from it that gives one.
- A seed is 64 bits, written everywhere in hexadecimal:
  `0x50921cc8cf51e5ba`, in the file, a world's, and whatever prints one.
- `Civil Egregore_SEED=<seed>` picks a seed for one run and leaves the file
  alone; `Civil Egregore_SEED=fresh` draws one for the run.
- The first asking prints the seed, which use it is and where it came
  from, so a failure names the seed that made it.

A test must hold on any seed: what it asserts is a bound or an equality
between two runs, never a number one seed happened to give.

## What is measured, and how

Speed is not a test: it is measured by a tool and written down with the
command that gave it (`server`'s `Civil_Egregore server pasture` and `throughput`).
A number in the docs names its command. While the renderer or another run
is on the machine, times are skewed: the instructions counted
(`perf stat -e instructions:u`) are not.

A change is kept if the tests pass, `cargo clippy --all-targets` is
silent, and it measures no worse -- or its cost is written down beside
what it buys.
