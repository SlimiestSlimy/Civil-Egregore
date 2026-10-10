# Civil Egregore's style guide

How Civil Egregore's code, tests and docs are written. Code is for people to
understand first, so every rule here is one of the
[design statements](design_statements.md) applied to the code itself.
Each rule names the statement it comes from:

- **#1**: unknowns can be implied, implieds can be forgotten, forgottens
  risk harm.
- **#2**: dynamics risk unknowns.
- **#4**: slow can be complex, complex can be efficient, efficient can be
  simple, simple can be slow.
- **#5**: process and processee are two halves of one whole, and how
  both divide can be influenced.

## One shape for every crate

Every crate, the root included, has the same folders. A folder that has
nothing yet stays anyway, holding only a line that says so. That way no
reader has to wonder whether something is missing or was never needed
(#1).

| path | what it holds |
|---|---|
| `Cargo.toml` | the crate: a library, with no `[[bin]]` |
| `docs/<crate>.md` | its design: what it is, and every decision with its reason |
| `docs/reference.md` | every item, function by function, in the glossary's words; the code's comments point here |
| `src/lib.rs` | a table of the crate's modules: one line each, what it is |
| `src/diagnostics/` | code that gathers data and judges nothing: counts, censuses. A tool that prints them goes in `src/diagnostics/tool`, a function among the crate's commands |
| `src/transient_data.rs` | names the crate's `transient_data/` folder, through `utilities::transient_data`, and says what goes where in it |
| `transient_data/` | what runs leave behind: measurements, renders, saves. Never in git, never needed as an input |
| `tests/<tier>.rs` | one test program per tier -- `fine.rs`, `fast.rs`, `complete.rs` -- a module per topic in it, each a file in `tests/<tier>/` once the tier's would pass some 300 lines; only the tiers the crate has tests for |
| `tests/tests.rs` | what the tiers' tests share, where they share anything: a module of each, no test program itself |

- A crate is a library: no `[[bin]]` and no folder of binaries, under the
  crate or under its sources. Only
  the root (`Civil_Egregore`) and the renderer are programs. A tool is a
  function in `src/diagnostics/tool`, listed among the crate's
  `COMMANDS` and run by `Civil_Egregore <crate> <tool>`; the root hands the
  crate the rest of the line and knows none of its tools.
- A doc gives a performance figure only where it explains an
  optimization: what it was before and after. Everywhere else it says
  what was found and the command that measures it; the latest figures
  are in `transient_data/measurements/`.
- A rule is written for one cell or one entity; the loop over them is
  an instruction (`read::cells::each_sampled`, `read::entities::each_woken`), inlined,
  so it costs nothing.
- A command's parameters are declared once, name and default
  (`utilities::commands::Parameter`): the usage, the parsing and the
  report's line all come from that. No crate reads `std::env::args`.
- Every text file kept -- a seed, settings, a world's file, a report --
  is CSV, read and written by `utilities::csv`, a first row naming its
  columns: one format, one reader (#2). A seed is 64 bits in
  hexadecimal wherever it is written (`utilities::seed::hex`).
- **`utilities/` is the shared top folder.** Code that two crates need
  goes there, once (#2). It is never copied into both.
- A crate holds what its name says and nothing else: what belongs to
  another's name goes there, however small. A world -- the program's, a
  test's, a tool's -- is made in one place, the server's `start`,
  whatever it is to be given as its `Start`: no crate makes a world of
  its own. The server starts and coordinates, and generates nothing:
  what is on a cell is `worldgen`'s to say, a superchunk's image
  `chunk_storage`'s to make and keep, the server's to pass between
  them, the simulation and the disk. A client -- the renderer -- asks
  the server alone (`server::host`): its frames for what is hot, its
  `terrain_seen` for the ground past it, and names no crate under it
  but `coordinates` and `utilities`.
- The rules' crates hold their rules alone: no diagnostics, no
  `transient_data`, no tests. A rule is tried on a world, so its tests
  and tools are the server's (`server/tests/fast/`,
  `server/src/diagnostics/`).
- A crate depends only on the crates below it. The README lists the
  order. The rules' crates (`entity_rules`, `mc_rules`) depend on
  `instructions` alone. Nothing depends on `AI_SCRATCHPAD`, the AI's
  own probes, which is no part of the program.

## One word for one thing

The [glossary](glossary.md) is the authority on names (#1, #2).

- Every word of Civil Egregore's own means one thing everywhere: in types,
  functions, variables, comments and docs.
- Every thing has one word.
- The glossary's "not" column lists the words a thing is never called.
- A new thing goes into the glossary before its name goes into the
  code.
- A word already taken is never reused for something else, however
  close the meaning.
- Ordinary Rust and English words (`len`, `new`, "count") mean what they
  always mean and are not listed.

Some things are named by rule:

- **Morton is the default.** Wherever something is in the world, it is
  given by a Morton index:
  - `SuperchunkIndex`, `ChunkIndex` and `CellIndex`, each nested in the
    next;
  - inside its parent, a **place**.

  Anything given in `x` and `y` instead says "cartesian" in its name:
  `CellCartesian`, `from_cartesian`, `cartesian()`,
  `place_from_cartesian`. A name without "cartesian" is never `x, y`.
- **A tile is aligned to its own size.** A tile `n` cells on a side
  starts at a multiple of `n`, always. Anything else is not a tile: it
  is a window, an area or a rectangle, and is called that.
- **A type's name says what it holds, then how it is given**, as in
  `CellCartesian`: a cell, given by its cartesian coordinates. The way
  a thing is given never comes first.

## One representation for one thing

- **One type per thing** (#2). Every extra representation of the same
  thing is a conversion someone has to know about, and can get wrong.
  When two types say nearly the same thing, they are merged. That is
  how seven position types became three indices and one cartesian
  type.
- **A repeat becomes a function.** The second time the same steps
  appear, they get a name and a home: in the crate if only it uses
  them, in `utilities/` if more than one does.
- **No cache without a measurement.** A cache, a hash or a fast path
  is a second way to the same answer (#2). It stays only if the tick's
  instruction count shows it pays. The arena's and write queues'
  hashed 16-entry caches went once measured: without them a tick is
  6.5% fewer instructions.
- **Simple first** (#4). Choose the plainer code unless it measures more
  than 1% worse on the reference run. A cost of 1% or more stays only
  if it is written down beside what it buys.

## The same on every machine

- A world follows from its seed alone, to the bit, on any machine and
  any number of threads (#2). `Civil_Egregore server check` prints a
  world's hash to compare.
- Nothing a world follows from is a float worked on by the machine's
  maths library: no `ln`, `exp`, `powf`, `sin` or their like. A chance
  is `utilities::chance::Chance`, a logarithm
  `utilities::fixed_point::log2`. The workspace's `clippy.toml` forbids
  the rest in every crate that has no `clippy.toml` of its own; only a
  crate no world follows from has one.

## Comments

- **Code explains itself roughly; the docs explain the code in
  detail.** A name and a short comment say what a thing is. How it
  works and why it is so are in `docs/`: the crate's `<crate>.md` for
  the design, its `reference.md` item by item. The docs are part of a
  change: it is not done until every doc that describes what it
  touched says what the code now does (#1: a doc behind the code is an
  unknown implied).
- Every item is documented, private ones included
  (`missing_docs`, `clippy::missing_docs_in_private_items`).
- A comment says what the thing is, in the glossary's words. It says
  why only where the why is not in the design doc.
- A number in a comment or doc names the command that gave it.
- Comments are as dense as the code around them. A new file matches
  its neighbours.
- `cargo fmt` is not run on the repository. Lines are laid out by
  hand, following the code around them.

## Tests: few, and generated

- **Few tests, many cases.** A test is a generator, a loop or a seeded
  draw, that makes many cases from a few lines and checks each against
  a plain rule or a second way of computing it. One test checking a
  thousand cells beats ten tests checking one each: there is less code
  to read and more ground covered.

  Examples already in the tree:
  - the coordinates tests: every cell on a list of edges, by every
    offset;
  - `entities_never_overlap`: hundreds of entities jostling for 400
    ticks, every tick checked;
  - Tessera's sample families (`Plan`, `LineSet`).
- A test by hand is for the one case a generator cannot reach, or a
  behaviour best shown by a single picture.
- **Three tiers**, each one test program:
  - **fine**: instant, one behaviour per test;
  - **fast**: seconds, seeded runs;
  - **complete**: minutes, `#[ignore]`d, run in release.

  A test belongs to the lowest tier it can. A test that only prints
  belongs to none: it is a tool. [Testing protocol](testing_protocol.md)
  gives the commands.
- **Diagnostics, tests and tools stay apart** (#5):
  - diagnostics gather;
  - tests judge;
  - tools print.

## Measuring

- Speed is never a test. It is measured by a tool, and a number written
  down names the command that gave it.
- The reference for the tick is the instructions callgrind counts for
  `Civil_Egregore server pasture 300 333 4000 4 1`, less the same run with no
  ticks. A change keeps within 1% of it, or says why not.
