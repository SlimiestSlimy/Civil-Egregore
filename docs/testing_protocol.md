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
(`mod sheep` in `tests/fast.rs`) -- a file of its own in the tier's
folder once the tier's file would pass some 300 lines
(`tests/fast/halos.rs`, named by `#[path]` in `tests/fast.rs`). A crate has the tiers it has tests
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
the renderer and its menus (`gui`), and the workspace's own fine tier,
which tests the docs (below). `cargo test --release -- --ignored` runs every complete
tier, Tessera's with them.

A test belongs to the lowest tier it can be: by hand if one case shows
it, a seeded run only if it takes one, the complete tier only if it
takes long. A test that only prints belongs to none: it is a tool.

| crate | fine | fast | complete |
|---|---|---|---|
| the workspace (`tests/fine.rs`) | the docs against the code | | |
| `bitmap` | cells, rectangles, discs, squares, Morton order, windows | | |
| `bitplane_manager` | counts and windows; hot bitmaps; writes; writing back through the ring | | |
| `chunk_storage` | layers encoded and back, heights, images, the writeback ring | | |
| `coordinates` | the indices nested, in Morton order, stepping as cartesian coordinates do | | |
| `type_registry` | the clash check, the layer types | | |
| `pathfinding` | paths and waves; walls | | |
| `utilities` | chance, commands, dispatcher, fixed_list, hash, process_memory, rng, settings, table, tuning | | |
| `simulation` | entities moving, entities waking, their instructions | sampling, the tick, writes across borders | |
| `instructions` | an entity changed, the cells beside it | area, mask, walking | |
| `worldgen` | | heights, walls, the mesh, superchunks made in any order | walls over many seeds; no seam between superchunks |
| `server` | | grass and sheep (the rules, tried on a world); halos; the host; the world hash; world: saves, loads, walls; commands | a world stopped often coming to the same; a flock on generated ground lasting |
| `tessera` | fine | fast | complete |

`entity_manager`, `mc_rules`, `entity_rules`, `gui` and `renderer` have
no test program. The entities' store is judged through the simulation's
tests, the rules through the server's (a rule is tried on a world, and
every world is the server's to make), the sliders' numbers by
`utilities`' `tuning`. The renderer is judged by eye: in the window,
and in its stills (`Civil_Egregore renderer stills`).

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
- `CIVIL_EGREGORE_SEED=<seed>` picks a seed for one run and leaves the file
  alone; `CIVIL_EGREGORE_SEED=fresh` draws one for the run.
- The first asking prints the seed, which use it is and where it came
  from, so a failure names the seed that made it.

A test must hold on any seed: what it asserts is a bound or an equality
between two runs, never a number one seed happened to give.

## The docs are tested too

The code says little about itself and sends its reader to the docs
([style guide](style_guide.md), "Comments"), so the docs have to be
right and the way to them has to hold. The workspace's own fine tier,
`tests/fine.rs` at the root (`cargo test --test fine`), reads every
markdown file and every source and judges three things.

**What the docs name is there** (`the_docs_name_what_is_there`). In
every markdown file, outside its fenced blocks:

- A link's target is a file that exists, taken from the file the link
  is in. Links out to the web are not followed.
- A path in backticks exists. A span is a path if it has a `/` in it or
  ends as a file does (`.rs`, `.md`, `.csv`, `.toml`, `.txt`, `.png`,
  `.pbm`, `.json`), and has nothing that makes it a pattern or a
  command: a space, `<`, `*`, `{`, `$`, `=`, `:`. It is looked for from
  the doc's own folder, the workspace's root, and the doc's crate, its
  sources and its tests. A crate's docs may name only that crate's
  files this way; the workspace's own docs set out every crate's shape,
  so what they name may be any crate's. A bare file name has to be a
  file of the doc's crate, or a name the code writes out in a string --
  `world.csv`, which only a run makes -- or a tool's name with what a
  run's file ends in: a tool's report is named after it. Nothing under
  `transient_data/` or `target/` is looked for: it is not in git.
- A name in backticks is a name the code has. A span is taken as code
  if it has `::`, `_`, a call's bracket or a capital in it and is a
  path of names (`server::world_hash`, `Chance::one_in(n)`,
  `COUNT_TILE_WORDS`); each name of the path has to be a word somewhere
  in the workspace's sources, manifests or settings, or a file's name.
  The standard library's names said in passing are listed in the test
  (`NAMES_NOT_OURS`).
- A section sent to is there. After a path to a markdown file, each
  title in quotes up to the sentence's end has to begin one of that
  file's headings, or one of its paragraphs that starts in bold (a
  reference's entry: `**`name`**`, found by the name alone).

This catches a file moved, an item renamed or removed, a section
retitled. It does not catch a doc that names a thing rightly and says
something false of it: that takes reading. Something that is gone on
purpose and still spoken of -- a module removed, a spelling never
used -- is written without backticks.

**Every public item is in its crate's docs**
(`every_public_item_is_in_its_crates_docs`). Each crate with a `docs/`
folder names, somewhere in its markdown, every `pub` function, type,
constant and module of its sources: a reference goes function by
function and leaves none out. A new public item fails the test until
it is written up.

**The code's pointers hold**
(`the_code_points_at_docs_that_are_there`). Every markdown path a
comment writes in backticks is a doc that exists -- taken from the
source's crate or the workspace's root, however many `../` it starts
with -- and has each section the comment quotes after it.

Whether a section still says what the code does no test can tell.
Before a commit, `cargo run --release -- docs stale` lists the
sections the changed code points at that the change left untouched
(`../utilities/docs/utilities.md`, "Stale docs"): each is read, and
updated or said in the commit to hold.

## What is measured, and how

Speed is not a test: it is measured by a tool and written down with the
command that gave it (`server`'s `Civil_Egregore server pasture` and `throughput`).
A number in the docs names its command. While the renderer or another run
is on the machine, times are skewed: the instructions counted
(`perf stat -e instructions:u`) are not.

A change is kept if the tests pass, `cargo clippy --all-targets` is
silent, and it measures no worse -- or its cost is written down beside
what it buys.
