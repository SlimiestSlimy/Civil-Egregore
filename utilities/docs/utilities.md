# Utilities

General-purpose utilities, shared by every crate in Civil Egregore and owned by
none.

- **Diagnostics** (`src/diagnostics/`), what every crate's diagnostics
  are made with:
  - **tables and reports** (`table/`): the one table printer; a
    measurement's report -- its tables, notes, and the command and
    commit it came from -- printed and kept as CSV in a folder the
    caller names (each crate's `transient_data/measurements/`), and
    read back;
  - **the process's memory** (`process_memory.rs`), as the system
    counts it: held now and at its peak, from `/proc/self/status`
    (Linux only), and tracked over a run for the average.
- **Transient data** (`transient_data.rs`): where a crate's runs leave
  what they make, `transient_data/` beside its `Cargo.toml`, out of git.
  Each crate names its own in its `src/transient_data.rs`.
- **Commands** (`commands.rs`): a command line's first word found among
  a crate's commands and handed the rest. A crate's tools are functions
  listed with their parameters, each declared once with its default;
  the one program routes to a crate by name and knows none of its tools.
- **Settings** (`settings.rs`): this machine's settings, kept from run
  to run in one file in one folder of Civil Egregore's own, wherever the
  system keeps such, a line each, shared by whatever has settings (so
  far the renderer's sliders), found by name in any order -- what the
  file lacks is the default settings'. The default settings are such a file
  built into the program: copied to a machine that has none, never
  over one that has, and all there is under the build feature
  `force_default_settings`. The same folder holds the worlds, in `worlds`,
  unless the settings name another folder for them.
- **What is tuned by eye** (`tuning.rs`): the numbers a window's
  sliders set -- the near view's shading, read each frame, and how a
  new world is made, read once when one is -- by name and place: a
  value handed about, nothing of it held in a static. What a slider
  reaches, its group and what it does are in the sliders' file
  (`sliders.csv`, at the crate's root), written by hand; what each is
  unless set is in the default settings.
- **The seed** (`seed.rs`): the one seed every crate's tests and tools
  start from, in the workspace's `transient_data/seed.csv`, rolled every 5
  counted runs; `CIVIL_EGREGORE_SEED` picks one for a run.
- **A seeded random source** (`rng.rs`), its whole state one word: every
  random number in Civil Egregore comes from it.
- **A chance** (`chance.rs`): how likely a thing is, as a whole number
  of parts in 2^32. See "Chances" below.
- **Fixed point** (`fixed_point.rs`): a logarithm to base 2 by
  whole-number arithmetic. See "Fixed point" below.
- **Hashing** (`hash.rs`): a key's slot in a table, by
  Fibonacci hashing, a word's bits mixed, SplitMix64's way, and many
  words folded into one hash, the same on every machine.
- **A fixed-capacity list** (`fixed_list.rs`), allocated once, never
  growing: for structures sized once and reused.
- **The cache** (`cache.rs`): memory asked for ahead of its being read.

## Layout

| folder | what is in it |
|---|---|
| `src/` | the utilities above |
| `src/diagnostics/` | tables, reports, the process's memory |
| `tests/` | each, judged |
| `docs/` | this, and the reference, function by function |
| `default_settings.csv`, `sliders.csv` | written by hand: every setting and what it is unless changed; every slider, what it reaches and does |
| `transient_data/` | out of git: what its tests keep |

## Commands

A crate is a library: its diagnostics tools are functions, not
programs. It lists them as `Command`s, each with the `Parameter`s it
takes -- a name and what it is if not given -- and `dispatch` runs the
one the command line's first word names, handing it the rest. The one
program, `Civil_Egregore`, is a list of the `Crate`s with commands
handed to `program`: it knows nothing of a crate's tools, and
`Civil_Egregore help` prints every command of every crate from the
lists themselves. With `help`, no word at all, or a word that names no
crate, `program` gives every command there is -- printed if asked for,
given as the reason for failing otherwise.

A parameter is declared once: its default is what the usage table
shows, what the command reads (`Given::number`) and what its report
says it ran on (`Given::resolved`). So the three cannot disagree.

## CSV

Every text file Civil Egregore keeps is CSV -- a seed, the settings,
the sliders, a world's file, a measurement's report -- so that one
reader and one writer serve them all, and any of them opens in whatever
reads tables. A file's first row names its columns.

Fields are parted by a comma (`SEPARATOR`). A field is quoted -- inside
`"` (`QUOTE`), with any `"` in it doubled -- when it holds a comma, a
quote or a newline, or is a row's one field and empty, or could be read
as one of the lines that are not rows. Those are two: a divider
(`---`, a table's), and a line starting with `#`, which is a note to
whoever reads the file and no row. Everything else is written bare.

## Tables and reports

One table printer serves every measurement, so that a column means the
same thing and looks the same wherever it is printed. What it enforces,
not leaving it to the caller: a divider under the headings, a bar
between columns, and a heading that names the whole of what the column
holds. A column headed "bits" says neither whose bits nor per what; one
headed "encoded bits a bitmap" does, and it is not the table's business
to make that shorter. A heading with newlines in it stacks, so a long
name costs height, not width. There is a divider above the headings as
well as below: a tall heading leaves blank cells over the short
columns, and with nothing to close the top they read as empty rows of
the table.

A table is also kept as text, so a measurement is written once and read
back, never copied by hand. `src/diagnostics/table/csv.rs` writes and reads one table
as CSV. A report (`src/diagnostics/table/report.rs`) is a measurement's tables, each
titled, with notes on what they were measured on -- the command, the
seed, the commit -- printed, and kept in a folder its caller names: one
file a measurement (`<name>.csv`, `EXTENSION`), rewritten by every run,
so the latest numbers are always in a file. The file is the tables' CSV
with two kinds of line around them: `# ` and a note, before the first
table, and `## ` and a title, before each table. Read back, a table
ends where the next title or the file's end is (`finish`).

## Transient data

A crate's `transient_data/` is beside its `Cargo.toml` and out of git:
what its runs leave behind, one folder a kind. Nothing there is an
input the code needs; a fresh checkout has none of it, and the first
run that needs a part makes it. Every crate names its own with
`TransientData::of` in its `src/transient_data.rs`; its measurements go
to `measurements/`, one CSV a tool (`TransientData::publish`).

## The dispatcher

Worker threads are started once and kept, parked while there is nothing
to do. They take two kinds of work.

- A job **run** (`Dispatcher::run`) is done on every thread at once, a
  part each, the caller's thread doing the first: a tick's phase. It
  borrows what the caller holds, for no longer than `run` takes: `run`
  does not return -- not even when a part panics -- until every worker
  has finished its part. That is what lets a borrowed job be handed to
  threads that outlive it, and the crate's one `unsafe` beside the
  prefetch rests on it (`JobPointer`: the job's address, handed to the
  workers for that long and no longer).
- A job **queued** (`Dispatcher::queue`) is done by one worker,
  whenever one is free, the caller not waiting: chunk storage's slow
  work, off the tick.

One set of threads does both, so neither crowds the other out of the
machine: a worker takes a job run before one queued, and one busy with
a queued job sits a run out -- the run is then split among the others.
A job run must therefore not count on every part being run: only on
part 0, and on each other part at most once.

Inside: the dispatcher and its workers share a `State` behind one
lock, and two waits on it (`Shared`): the workers' for work, the
caller's for the parts to finish. The state is the job running, if any;
a count of the jobs run (`generation`), by which a worker knows a job
it has not yet done its part of, and does that part once; how many
workers are still running their part; whether one panicked; whether
the workers are to stop; the jobs queued, oldest first; and how many
workers are busy with a queued job, which sit a run out. `Job` is a
job run, a function of the part's number; `Queued` a job queued, done
once. A worker's loop (`work`) waits until there is something, takes a
`Work` -- its part of a job run before a job queued -- does it, and
says so; told to stop, it returns.

## The seed

No test or tool has a seed written in it. The seed is one number in one
file for the whole workspace, so that using the same one twice is a
thing seen, and using another costs no edit.

Reusing a seed is what comparing two versions of the code needs:
holding what is tested still while the code moves. But a seed held for
long becomes the only one every change was ever tried on, and what
passes may pass on that seed alone. So the file keeps, with the seed,
how many runs have used it, and after `USES_BEFORE_THE_SEED_ROLLS` (5)
the next run rolls a fresh one by itself, from the process's own
randomness (`fresh_seed`: the standard library's hasher keys). The
first asking of a run settles the seed, once a process (`SETTLED`, a
`Settled`: the seed, whether it is fresh, which use this is, where it
came from; made by `settle`, which reads the variable or the file,
rolls a used-up one, and rewrites the file), and a one-row table on
standard error says which it is, which use and from where -- so a
failure names the seed that made it.

`CIVIL_EGREGORE_SEED` in the environment picks a seed for one run and
leaves the file alone, its count too; `fresh` as its value draws one
for the run. A use may be left uncounted (`uncounted`; `Counted` says
which): for the fine tests, run far more often than anything measured.

A seed is 64 bits, and written everywhere as they are: `0x` and 16
hexadecimal digits (`hex`), read back with or without the `0x`
(`of_hex`). The file is CSV, `seed,uses` (`COLUMNS`) and a row, in the
workspace's own `transient_data/`, beside the crates and not tracked by
git: a seed and its count belong to the working copy they were used in.

## Settings

One file, in one folder of Civil Egregore's own under the place the
system gives a user's programs for what they keep (`kept_by_programs`;
where the system names none, the folder the program runs in):

| system | the folder |
|---|---|
| Linux and the like | `$XDG_DATA_HOME/Civil Egregore`, or `~/.local/share/Civil Egregore` |
| Windows | `%LOCALAPPDATA%\Civil Egregore`, or `%APPDATA%\Civil Egregore` |

The file is CSV, a row a setting: its name, its value (`COLUMNS`,
`setting,value`). The default settings (`default_settings.csv`, at the
crate's root, built in as `DEFAULTS`) are such a file, with every
setting there is: a machine with no file of its own is given a copy the
first time the settings are read, and a setting the machine's file
lacks is as the default settings have it. Whatever has settings shares
the one file: each reads the names it knows and, writing, leaves the
others' lines as they are. The settings are found by name: the lines,
in a file and in the default settings alike, may be in any order.

Built with the feature `force_default_settings` (`FORCE_DEFAULTS`), the
machine's file is neither read nor written: the settings are the
default ones. The folder also holds the worlds, in a folder of their
own (`worlds`), unless the setting `WORLDS` names another.

## What is tuned by eye

The numbers a window's sliders set, in groups: the near view's shading,
which a painter reads each frame; how a world is generated, read when
one is made, or made again while it runs; and what a world is set up
with, read only when one is made (`Group::setup_only`). The numbers are
a value (`Tuning`), held by whoever sets them and handed to whoever
reads them: nothing of them is held in this crate but what the files
say, read once (`TUNED`, the sliders' rows; `DEFAULTS`, every number
as the default settings have it).

Only the numbers' names and places are in the code. What a slider
reaches, its group and what it does are in the sliders' file
(`sliders.csv`, at the crate's root, built in as `SLIDERS`), written by
hand: a row each -- its number's name, the least and the most its knob
reaches, its group, whether it is a slider or a toggle, what it does.
What each number is unless set is in the default settings, and what it
was last set to is kept in the machine's, a line each.

## Chances

A world follows from its seed alone, the same to the bit on every
machine (`../../server/docs/server.md`, "The same on every machine").
A float cannot promise that where a logarithm is taken of it: `ln` is
the machine's maths library's, and two libraries round its last bit
their own ways. One sample chosen a cell apart on one machine is another
world a thousand ticks on. So nothing a world follows from is a float.

A `Chance` is a whole number of parts in `PARTS`, 2^32: a half is 2^31
parts, once in 100,000 is 42,950. A rule states its chances as
constants -- `Chance::one_in(100_000)`, `Chance::HALF` -- and two things
that never both happen are added with `plus`. The unit was chosen over
"one in N" kept as N because parts add, and because a draw against them
is a shift and a comparison: `Rng::chance` is true when a draw's high 32
bits are under the parts. `Rng::chance_among(part, whole)` says which
of two things it is that happened with `whole`: a number under
`whole`'s parts, under `part`'s or not -- what the grass uses to tell a
spread from a decay, with no division of one chance by another.

The smallest chance there is, one part, is about once in 4.3 billion.
`one_in` rounds to the nearest part, so once in 100,000 is off by a few
parts in a million: the precision is `PARTS`, a number chosen here, and
the same everywhere.

### The gap

Sampling (`../../simulation/docs/simulation.md`, "Sampling") does not
toss a coin a cell. It draws how many set cells to pass over before the
next chosen one, a gap of the geometric law: `floor(log(u) / log(1 -
p))`, `u` uniform in `(0, 1]`, `p` the chance. The base of the
logarithms does not matter, as it is a ratio; base 2 is what whole
numbers find quickly.

`Chance::passed_over(draw)` is that, in fixed point:

- `u` is the draw's high 53 bits, as a number from 1 to 2^53 over 2^53:
  never 0, so it has a logarithm. Its logarithm is `log2` of the number
  less 53, a negative number; negated, `53 - log2(number)`, with 48 bits
  of fraction.
- `log(1 - p)` is `log2(PARTS - parts)` less 32, negative too; negated,
  `32 - log2(PARTS - parts)`. It depends on the chance alone, so it is
  worked out once, where the chance is made -- as the program is built,
  for a rule's constants -- and kept in the `Chance` beside its parts.
- The gap is the first over the second: one division of two whole
  numbers, rounded down.

Fifty-three bits of draw, not 32, because a gap may be far longer than
`PARTS`: at one part the mean gap is 2^32 cells, and the long tail of
the law is in the bits under those.

How precise a gap is: the second number, for the smallest chance, is
some 94,000, so the gaps of a chance of one part are right to about a
part in 100,000; for once in 100,000 to a part in four billion. A test
draws 200,000 gaps for chances from a half to one part and holds each
to the float formula's, and their mean to the law's
(`tests/fine/chance.rs`).

## Fixed point

`log2(value)` gives the logarithm to base 2 of a whole number, times
2^48 (`LOG2_FRACTION_BITS`): a whole number again.

Its whole part is where the number's highest bit is. What is left is
the number over that power of two, in `[1, 2)`, the mantissa, and its
logarithm is the fraction. Two ways find it.

**By squaring** (`log2_by_squaring`). Square the mantissa: if the square
is 2 or more the next bit of the fraction is 1, and the square is
halved; else the bit is 0. A bit a squaring, the squares kept in 128
bits. It is exact to about a part in 2^56 and plain to see right, and
it is slow: a multiplication for each of 48 bits, some 900 instructions.
What it is for: `log2`'s tables are made with it as the crate is
built, and `log2` is tested against it.

**By table and series** (`log2`). The seven bits after the mantissa's
highest pick one of 128 rows; each row is a stretch of `[1, 2)` a part
in 128 wide, and the tables hold its middle's logarithm and 1 over its
middle. The mantissa times that inverse is 1 and a little, under a part
in 256 either way; the logarithm of 1 and a little is a series in the
little -- `x - x^2/2 + x^3/3 - ...` to the sixth power, the next term
under a part in 2^58 -- times 1 over the natural logarithm of 2. The row's
logarithm and that, added, are the fraction. Eight multiplications,
the series worked in a fixed point of its own (`ONE`, 1 in it), and
the result the true logarithm's to a part in 2^47. The
tables are made by the squaring, as the crate is built.

The second is the one used. Sampling's gaps made the tick reference
(`Civil_Egregore server pasture 300 333 4000 4 1`, less the run with no
ticks, counted by callgrind) 64.29 million instructions by squaring,
against 58.50 million with the float logarithm it replaced; by table
and series it is 58.74 million.

The two agree to within four of the last bit, on every number tried
and at every row's ends, and both are the float's to a part in 2^40
(`tests/fine/chance.rs`). A power of two's logarithm is exact, which
the gap relies on: the draw that is 1 has a gap of 0, never under.
The result is the same on every machine either way; which bits it has
is fixed by the code, not by a library.

## What may not be called

`clippy.toml`, at the workspace's root, forbids a float's logarithm,
exponential, power and trigonometry (`disallowed-methods`: `ln`, `exp`,
`powf`, `sin` and their like, on `f32` and `f64`), the float draw
`Rng::unit`, and `Chance::fraction`, in every crate without a
`clippy.toml` of its own. The crates that are no part of what a world
follows from have an empty one: the renderer, the menus, Tessera, the
AI's scratchpad, and this crate, where the float draw is defined and
measurements are tabled. A float's sum, product, quotient and square
root are exact by the standard and are let be. A test that wants a
float for what it expects allows it by name, with the reason.
