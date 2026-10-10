# Utilities, function by function

The design is in `utilities.md`.

## `diagnostics/table/mod.rs`

**`Table::new(headings)`**, **`left_aligned(headings)`**,
**`row(fields)`**, **`divider()`**, **`print()`**, **`rendered()`**:
columns sized to their widest field (**`column_widths`**,
**`printed_line`**, **`divider_line`**).

## `diagnostics/table/report.rs`

**`Report::new(name, command)`**, **`note`**, **`add(title, table)`**,
**`print`**, **`to_text`** / **`from_text`**, **`read(folder, name)`**,
**`publish(folder)`** -- printed, noted with the commit (**`commit`**),
and kept as `<name>.csv`; **`keep(folder)`**, the same unprinted, for a
run whose standard output is something else (**`note_commit`**,
**`write`**). **`path(folder, name)`**, **`kept(folder)`**:
the reports kept.

## `diagnostics/table/csv.rs`

A table as CSV and back: **`Table::to_csv`**, **`from_lines`**,
**`from_csv`**.

## `csv.rs`

CSV, which every text file Civil Egregore keeps is -- the seed, the settings,
the sliders, a world's file and its hot file, a measurement's report --
a first row naming the columns. **`Line`**: a row, a divider
(`DIVIDER`) or a note (`COMMENT`, `#`). **`lines(text)`**: every line;
**`rows(text)`**: the rows alone; **`rows_named(text)`**: those after
the one naming the columns; **`rows_by_column(text, columns)`**: those
rows, each the fields of the columns named, found by the first row
whatever their order -- a column or field missing is empty. **`row(fields)`**: a row written, a field
quoted where it has to be (**`field`**).

## `seed.rs`

The one seed every seeded run starts from, kept in the workspace's
`transient_data/seed.csv` (**`file()`**; `FILE` its name) with how many
runs have used it. `VARIABLE`: `CIVIL_EGREGORE_SEED`, the environment
variable that picks a seed for one run and leaves the file alone --
a number in hexadecimal, or `FRESH` (`fresh`) for one drawn.
`USES_BEFORE_THE_SEED_ROLLS` (5): the runs a seed serves before the
next run rolls another.

**`counted()`**: the run's seed, counted as a use: the variable's if
set, else the file's, a fresh one rolled in its place once it is used
up. **`uncounted()`**: the same seed, not counted -- the fine tests',
which run too often to count; with no file yet one is rolled and kept
at no uses. Either settles the seed once a process and says it in a
one-row table on standard error: the seed, which use it is, where it
came from. **`in_use()`**: the seed settled on and whether it was
fresh, if any was asked for: what a measurement's report notes.

**`hex(seed)`**: a seed as it is written everywhere, `0x` and 16
hexadecimal digits; **`of_hex(text)`**: back, with or without the `0x`.

## `commands.rs`

**`Parameter::new(name, default)`**: one thing a command takes, by its
place. **`Command { name, does, parameters, run }`**. **`dispatch(called,
commands, arguments)`**: runs the command the first argument names on
the rest, or gives the usage as why not; **`usage(called, commands)`**:
the commands as a table. **`Given`**, what a command is run with:
`name()`, `arguments()`, `given(name)` (if given), `text(name)` and
`number(name)` (as given, or the default), `route()` (the words between
the program and the command), `usage()`, `resolved()` (the line it ran
on, defaults filled in: what a report records). **`Crate { name, does,
commands }`** and **`program(called, crates, arguments)`**: a whole
program -- the crate the first word names is handed the rest; `help`,
`--help` or `-h` (`HELP`) prints **`help(called, crates)`**, every
command of every crate.

## `stale_docs.rs`

**`COMMANDS`**: the program's `docs` commands, one: `stale [from
commit]` (`FROM_COMMIT`, `HEAD` unless given), run by `run` -- the
root, the files and the lines asked of **`git(folder, arguments)`**,
what it prints or why it failed; the rows printed as a table, or a
line saying there are none. **`Lines { first, last }`**: a stretch of
a file's lines, from 1, both ends in it; `meet(other)`.
**`changed_lines(diff)`**: each hunk of a `git diff -U0` as (file,
lines now); a `+++` line is a file's only after a `---` line, and a
removed file gives none. **`pointers(root, path, text)`**: (doc from
the root, section or none) for each pointer in a source's comments,
found from **`crate_of(root, path)`** or the root; **`spans(line)`**:
what is in backticks; **`quoted_titles(after)`**: the titles quoted
after a path, to the sentence's end, their own backticks left off. **`section_lines(text, title)`**: a section's
lines, by **`heading_depth(line)`** (the `#`s of a heading), the whole
doc if not there. **`untouched_sections(root, changed_files,
diff)`**: the rows, sorted, each once. `WHOLE_DOC`: what a pointer
with no section is listed as.

## `rng.rs`

**`Rng::new(seed)`**, **`for_stream(seed, stream)`** -- a source of its
own for a stream of a seed, its state mixed -- **`state()`** (what a
save keeps), **`draw`**, **`below`**, **`between`**, **`percent_chance`**;
**`chance(chance)`**: true with a `Chance`, the draw's high 32 bits
under its parts; **`chance_among(part, whole)`**: true `part` times in
`whole`; **`unit`**: a float in `[0, 1)`, for what is not the
simulation -- forbidden to it (`clippy.toml`).

## `chance.rs`

**`PARTS`** (2^32): what a chance is out of. **`Chance`**: so many
parts, and the negated logarithm of the chance it does not happen,
worked out where it is made. **`NEVER`**, **`ALWAYS`**, **`HALF`**;
**`of_parts(parts)`**, no more than all; **`one_in(times)`**, to the
nearest part; **`plus(other)`**, of two that never both happen;
**`parts`**, **`is_never`**, **`is_always`**. **`passed_over(draw)`**:
the things passed over before the next chosen, a gap of the geometric
law from a draw's high 53 bits (`DRAW_BITS`) -- one division.
**`fraction`**: the chance as a float, for a report or a test's
expectation alone.

## `fixed_point.rs`

**`LOG2_FRACTION_BITS`** (48). **`log2(value)`**: the logarithm to base
2, times 2^48 -- the whole part from the highest bit, the fraction from
a row of **`MIDDLE_LOG2`** and **`ONE_OVER_MIDDLE`** (`ROWS`, 128,
picked by `TABLE_BITS`, 7; each row's **`middle`**) and a series to the
sixth power (**`times`**, `SERIES_ONE_BITS`, `ONE_OVER_LN_2`), kept in
`TABLE_FRACTION_BITS` (56) until rounded.
**`log2_by_squaring(value, fraction_bits)`**: the same a bit a
squaring: what the tables are made with and `log2` tested against.

## `hash.rs`

**`slot(key, slots)`**: a key's slot in a table of a power of two slots,
by Fibonacci hashing (`GOLDEN_RATIO`). **`mix(word)`**: SplitMix64's
finalizer (`MIX_1`, `MIX_2`). **`fold(hash, word)`**: a hash with one
word more folded in, the order telling; **`fold_all(hash, words)`**:
with many, after how many they are -- what a world's hash is built on
(`server::world_hash`).

## `fixed_list.rs`

**`FixedList<T, N>`**: **`new`**, **`clear`**, **`push`** (past `N`
panics), **`pop`**; derefs to a slice.

## `diagnostics/process_memory.rs`

**`process_memory()`**: **`Memory`** `{resident, peak}`, if the system
says. **`MemoryTrack`**: **`read`** (one reading), **`average`**, **`peak`**.
**`mebibytes(bytes)`**: how a report shows memory.

## `transient_data.rs`

`TRANSIENT_DATA`: the crate's `transient_data/` folder.
**`TransientData::of(crate_folder)`**: a crate's `transient_data/`, from
its `env!("CARGO_MANIFEST_DIR")`; **`under(relative)`**,
**`measurements()`**, **`publish(report)`** -- printed, and kept as
`measurements/<name>.csv`. `FOLDER`: the folder's name.

## `cache.rs`

**`prefetch(value)`**: its line of memory asked for ahead of being read;
the crate's one `unsafe` line, on a reference's address.

## `dispatcher.rs`

**`Dispatcher::new(threads)`**: `threads - 1` workers started and kept;
**`of_the_machine()`**: every thread the machine has. **`threads`**.
**`run(job)`**: part 0 here, the others on the workers not busy with a
queued job; returns once every part started is done, a part's panic
raised after. **`queue(job)`**: done once by a worker when one is free,
or at once where there is no worker. **`work`**: a worker's loop -- wait
for work, a job run before one queued, do it, say so. Dropping it stops
and joins the workers.

## `settings.rs`

The settings of this machine: `FILE` (`settings.csv`) in `FOLDER`
(`Civil Egregore`) under the system's place for what a user's programs keep.
**`folder()`**, **`file()`**. `default_settings.csv`, at the crate's root: every setting
there is and what it is unless changed, built into the program.
**`Settings`**: a name and a value a row, under `setting,value`, found
by name in any order -- **`defaults()`**,
**`read()`** (the machine's file, given a copy of the default settings
if it has none, and the default ones for what it lacks; the default
ones alone under the feature `force_default_settings`),
**`read_or_start(path)`** (the same of any file), **`read_from(path)`**
(a file's lines and no more), **`get(name)`**, **`number::<T>(name)`**,
**`set(name, value)`** (none takes the line out), **`write()`**,
**`write_to(path)`**. `WORLDS`: the setting naming the folder worlds
are kept in; **`worlds()`**: that folder, `worlds` in Civil Egregore's unless
set; **`world(named)`**, **`world_in(worlds, named)`**: where a world
is kept -- a plain name in the worlds' folder, anything more a path;
**`world_name(given)`**: a world's name as a folder may have it on
Windows and Linux alike.

## `tuning.rs`

`NAMES`: the numbers' names, each at its place, and the places
(`STEP_LIGHT` ... `SHEEP`), by **`places!`**. **`Tuned`** `{name,
line, range, group, toggle, what}` -- `toggle` on or off, not dragged -- -- `what` the tooltip -- and
**`tuned(index)`**: a number as the sliders' file (`sliders.csv`, at
the crate's root) has it. **`Group`**: `Shading`, `Land`,
`Lines`, `Finer`, `Grass`, `Trees`, `World` (`GROUPS`, in the menu's
order, the world's own last), **`name()`** and
**`setup_only()`** -- the world's group alone, read only when a world
is made. **`Tuning`**: the
numbers together, a value held by whoever sets them -- nothing of them
is held here. **`unless_set(index)`**: what a number is unless set,
from the default settings; **`defaults()`**: every one so.
**`kept()`**: what the machine's settings have. **`settled(index,
value)`**: a value set, or the default if it is no number.
**`keep(tuning)`**: the numbers written to the machine's settings.
