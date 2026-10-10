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
