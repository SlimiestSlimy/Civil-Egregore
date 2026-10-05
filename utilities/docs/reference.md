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
the one naming the columns. **`row(fields)`**: a row written, a field
quoted where it has to be (**`field`**).

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

## `rng.rs`

**`Rng::new(seed)`**, **`for_stream(seed, stream)`** -- a source of its
own for a stream of a seed, its state mixed -- **`state()`** (what a
save keeps), **`draw`**, **`below`**, **`between`**, **`percent_chance`**,
**`unit`**.

## `hash.rs`

**`slot(key, slots)`**: a key's slot in a table of a power of two slots,
by Fibonacci hashing (`GOLDEN_RATIO`). **`mix(word)`**: SplitMix64's
finalizer (`MIX_1`, `MIX_2`).

## `fixed_list.rs`

**`FixedList<T, N>`**: **`new`**, **`clear`**, **`push`** (past `N`
panics), **`pop`**; derefs to a slice.

## `diagnostics/process_memory.rs`

**`process_memory()`**: **`Memory`** `{resident, peak}`, if the system
says. **`MemoryTrack`**: **`read`** (one reading), **`average`**, **`peak`**.
**`mebibytes(bytes)`**: how a report shows memory.

## `transient_data.rs`

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
**`Settings`**: a name and a value a row, under `setting,value` -- **`defaults()`**,
**`read()`** (the machine's file, given a copy of the default settings
if it has none, and the default ones for what it lacks; the default
ones alone under the feature `default_settings`),
**`read_or_start(path)`** (the same of any file), **`read_from(path)`**
(a file's lines and no more), **`get(name)`**, **`number::<T>(name)`**,
**`set(name, value)`** (none takes the line out), **`write()`**,
**`write_to(path)`**. `WORLDS`: the setting naming the folder worlds
are kept in; **`worlds()`**: that folder, `worlds` in Civil Egregore's unless
set; **`world(named)`**, **`world_in(worlds, named)`**: where a world
is kept -- a plain name in the worlds' folder, anything more a path;
**`world_name(given)`**: a world's name as a folder may have it on
Windows and Linux alike.
