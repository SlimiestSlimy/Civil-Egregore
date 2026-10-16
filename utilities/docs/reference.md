# Utilities, function by function

The design is in `utilities.md`.

## `diagnostics/table/mod.rs`

**`Table::new(heading_texts)`**, **`left_aligned(left_headings)`**,
**`add_row(row_fields)`**, **`divider()`**, **`print_rendered()`**, **`rendered_text()`**:
columns sized to their widest field (**`column_widths`**,
**`printed_line`**, **`divider_line`**).

## `diagnostics/table/report.rs`

**`Report::new(report_name, rerun_command)`**, **`add_note`**, **`add_table(table_title, table)`**,
**`print_report`**, **`to_text`** / **`from_text`**, **`read_kept(folder, report_name)`**,
**`print_and_keep(folder)`** -- printed, noted with the commit (**`checkout_commit`**),
and kept as `<report_name>.csv`; **`write_to_folder(folder)`**, the same unprinted, for a
run whose standard output is something else (**`note_commit`**,
**`write`**). **`path(folder, report_name)`**, **`saved_reports(folder)`**:
the reports kept.

## `diagnostics/table/csv.rs`

A table as CSV and back: **`Table::to_csv`**, **`from_lines`**,
**`from_csv`**.

## `csv.rs`

CSV, which every text file Civil Egregore keeps is -- the seed, the settings,
the sliders, a world's file and its hot file, a measurement's report --
a first row naming the columns. **`Line`**: a row, a divider
(`DIVIDER`) or a note (`COMMENT_MARK`, `#`). **`lines(csv_text)`**: every line;
**`rows_with_header(csv_text)`**: the rows alone; **`rows_named(csv_text)`**: those after
the one naming the columns; **`rows_by_column(csv_text, wanted_columns)`**: those
rows, each the fields of the columns named, found by the first row
whatever their order -- a column or field missing is empty. **`row_text(row_fields)`**: a row written, a field
quoted where it has to be (**`field_as_csv`**).

## `seed.rs`

The one seed every seeded run starts from, kept in the workspace's
`transient_data/seed.csv` (**`seed_file()`**; `SEED_FILE_NAME` its name) with how many
runs have used it. `SEED_VARIABLE`: `CIVIL_EGREGORE_SEED`, the environment
variable that picks a seed for one run and leaves the file alone --
a number in hexadecimal, or `FRESH_SEED_VALUE` (`is_fresh`) for one drawn.
`USES_BEFORE_THE_SEED_ROLLS` (5): the runs a seed serves before the
next run rolls another.

**`counted_seed()`**: the run's seed, counted as a use: the variable's if
set, else the file's, a fresh one rolled in its place once it is used
up. **`uncounted_seed()`**: the same seed, not counted -- the fine tests',
which run too often to count; with no file yet one is rolled and kept
at no uses. Either settles the seed once a process (`settled_once`) and says it in a
one-row table on standard error: the seed, which use it is, where it
came from. **`in_use()`**: the seed settled on and whether it was
fresh, if any was asked for: what a measurement's report notes.

**`seed_as_hex(seed)`**: a seed as it is written everywhere, `0x` and 16
hexadecimal digits; **`of_hex(hex_text)`**: back, with or without the `0x`.

## `commands.rs`

**`CommandParameter::new(parameter_name, default_value)`**: one thing a command takes, by its
place. **`NamedCommand { command_name, what_it_does, command_parameters, run_function }`**. **`dispatch_command(reached_by,
crate_commands, argument_words)`**: runs the command the first argument names on
the rest, or gives the usage as why not; **`usage_table(reached_by, crate_commands)`**:
the commands as a table, a row each (**`usage_line(reached_by, listed_command)`**, its parameters by **`parameters_usage(listed_command)`**). **`GivenCommand`**, what a command is run with:
`command_name()`, `argument_words()`, `given_argument(parameter_name)` (if given), `argument_text(parameter_name)` and
`argument_number(parameter_name)` (as given, or the default), `route_words()` (the words between
the program and the command), `command_usage()`, `resolved_line()` (the line it ran
on, defaults filled in: what a report records). **`CommandCrate { crate_name, what_it_does,
crate_commands }`** and **`run_program(program_name, command_crates, argument_words)`**: a whole
program -- the crate the first word names is handed the rest; `help`,
`--help` or `-h` (`HELP_WORDS`) prints **`help_text(reached_by, command_crates)`**, every
command of every crate.

## `stale_docs.rs`

**`COMMANDS`**: the program's `docs` commands, one: `stale [from
commit]` (`FROM_COMMIT`, `HEAD` unless given), run by `list_stale_docs` -- the
root, the files and the lines asked of **`git_output(folder, git_arguments)`**,
what it prints or why it failed; the rows printed as a table, or a
line saying there are none. **`Lines { first_line, last_line }`**: a stretch of
a file's lines, from 1, both ends in it; `shares_line_with(other_stretch)`.
**`changed_lines(diff_text)`**: each hunk of a `git diff -U0` as (file,
lines now); a `+++` line is a file's only after a `---` line, and a
removed file gives none. **`doc_pointers(root_folder, path, source_text)`**: (doc from
the root, section or none) for each pointer in a source's comments,
found from **`crate_of(root_folder, path)`** or the root; **`backtick_spans(line)`**:
what is in backticks; **`quoted_titles(text_after_path)`**: the titles quoted
after a path, to the sentence's end, their own backticks left off. **`section_lines(doc_text, wanted_title)`**: a section's
lines, by **`heading_depth(line)`** (the `#`s of a heading), the whole
doc if not there. **`untouched_sections(root, changed_files,
diff)`**: the rows, sorted, each once. `WHOLE_DOC`: what a pointer
with no section is listed as.

## `rng.rs`

**`RandomSource::new(seed)`**, **`for_stream(seed, stream)`** -- a source of its
own for a stream of a seed, its state mixed -- **`whole_state()`** (what a
save keeps), **`next_draw`**, **`draw_below`**, **`draw_between`**, **`percent_chance`**;
**`chance(chance)`**: true with a `Chance`, the draw's high 32 bits
under its parts; **`chance_among(part, whole_chance)`**: true `part` times in
`whole`; **`draw_unit_float`**: a float in `[0, 1)`, for what is not the
simulation -- forbidden to it (`clippy.toml`).

## `chance.rs`

**`PARTS`** (2^32): what a chance is out of. **`Chance`**: so many
parts, and the negated logarithm of the chance it does not happen,
worked out where it is made. **`NEVER_HAPPENS`**, **`ALWAYS_HAPPENS`**, **`HALF_THE_TIME`**;
**`of_parts(parts)`**, no more than all; **`one_in(occasion_count)`**, to the
nearest part; **`plus_disjoint(other_chance)`**, of two that never both happen;
**`parts`**, **`is_never`**, **`is_always`**. **`passed_over(drawn_bits)`**:
the things passed over before the next chosen, a gap of the geometric
law from a draw's high 53 bits (`DRAW_BITS`) -- one division.
**`as_fraction`**: the chance as a float, for a report or a test's
expectation alone.

## `fixed_point.rs`

**`LOG2_FRACTION_BITS`** (48). **`log2_by_table(value)`**: the logarithm to base
2, times 2^48 -- the whole part from the highest bit, the fraction from
a row of **`MIDDLE_LOG2`** and **`ONE_OVER_MIDDLE`** (`TABLE_ROWS`, 128,
picked by `TABLE_BITS`, 7; each row's **`row_middle`**) and a series to the
sixth power (**`fixed_times`**, `SERIES_ONE_BITS`, `ONE_OVER_LN_2`), kept in
`TABLE_FRACTION_BITS` (56) until rounded.
**`log2_by_squaring(value, fraction_bits)`**: the same a bit a
squaring: what the tables are made with and `log2_by_table` tested against.

## `hash.rs`

**`slot(hashed_key, slots)`**: a key's slot in a table of a power of two slots,
by Fibonacci hashing (`GOLDEN_RATIO`). **`mix_word(word)`**: SplitMix64's
finalizer (`MIX_1`, `MIX_2`). **`fold_word(running_hash, word)`**: a hash with one
word more folded in, the order telling; **`fold_all(running_hash, words)`**:
with many, after how many they are -- what a world's hash is built on
(`server::world_hash`).

## `fixed_list.rs`

**`FixedList<T, N>`**: **`new`**, **`clear`**, **`push`** (past `N`
panics), **`pop`**; derefs to a slice.

## `diagnostics/process_memory.rs`

**`process_memory()`**: **`MemoryReading`** `{resident_bytes, peak_bytes}`, if the system
says. **`MemoryTrack`**: **`take_reading`** (one reading), **`average_resident_bytes`**, **`peak_bytes`**.
**`mebibytes_text(byte_count)`**: how a report shows memory.

## `transient_data.rs`

`TRANSIENT_DATA`: the crate's `transient_data/` folder.
**`TransientData::for_crate(crate_folder)`**: a crate's `transient_data/`, from
its `env!("CARGO_MANIFEST_DIR")`; **`path_under(relative_path)`**,
**`measurements_folder()`**, **`publish_report(report)`** -- printed, and kept as
`measurements/<name>.csv`. `FOLDER`: the folder's name.

## `cache.rs`

**`prefetch_cache_line(value)`**: its line of memory asked for ahead of being read;
the crate's one `unsafe` line, on a reference's address.

## `dispatcher.rs`

**`Dispatcher::new(thread_count)`**: `thread_count - 1` workers started and kept;
**`of_the_machine()`**: every thread the machine has. **`thread_count`**.
**`run_in_parts(job)`**: part 0 here, the others on the workers not busy with a
queued job; returns once every part started is done, a part's panic
raised after; one at a time (`one_run`). **`queue_job(job)`**: done once by a worker when one is free,
or at once where there is no worker. **`worker_loop`**: a worker's loop -- wait
for work, a job run before one queued, do it, say so. Dropping it stops
and joins the workers.

## `settings.rs`

The settings of this machine: `SETTINGS_FILE_NAME` (`settings.csv`) in `FOLDER`
(`Civil Egregore`) under the system's place for what a user's programs keep.
**`folder()`**, **`settings_file()`**. `default_settings.csv`, at the crate's root: every setting
there is and what it is unless changed, built into the program.
**`Settings`**: a name and a value a row, under `setting,value`, found
by name in any order (**`parse_text(settings_text)`**) -- **`default_settings()`**,
**`read_machine()`** (the machine's file, given a copy of the default settings
if it has none, and the default ones for what it lacks; the default
ones alone under the feature `force_default_settings`),
**`read_or_start(path)`** (the same of any file), **`read_from(path)`**
(a file's lines and no more), **`get(setting_name)`**, **`value_as::<T>(setting_name)`**,
**`set(setting_name, value)`** (none takes the line out), **`write()`**,
**`write_to(path)`**. `WORLDS`: the setting naming the folder worlds
are kept in; **`worlds()`**: that folder, `worlds` in Civil Egregore's unless
set; **`world(name_or_path)`**, **`world_in(worlds, name_or_path)`**: where a world
is kept -- a plain name in the worlds' folder, anything more a path;
**`world_name(given_name)`**: a world's name as a folder may have it on
Windows and Linux alike.

## `tuning.rs`

`NAMES`: the numbers' names, each at its place, and the places
(`STEP_LIGHT` ... `SHEEP_PER_SUPERCHUNK`), by **`places!`**. **`TunedNumber`** `{setting_name,
line, slider_range, slider_group, toggle, what_it_does}` -- `toggle` on or off, not dragged -- `what_it_does` the tooltip -- and
**`tuned_number(number_index)`**: a number as the sliders' file (`sliders.csv`, at
the crate's root) has it. **`SliderGroup`**: `NearViewShading`, `OceanAndLand`,
`Lines`, `FinerMeshes`, `StartingGrass`, `Trees`, `World` (`SLIDER_GROUPS`, in the menu's
order, the world's own last), **`menu_name()`** and
**`setup_only()`** -- the world's group alone, read only when a world
is made. **`Tuning`**: the
numbers together, a value held by whoever sets them -- nothing of them
is held here. **`unless_set(number_index)`**: what a number is unless set,
from the default settings; **`default_tuning()`**: every one so.
**`load_from_settings()`**: what the machine's settings have. **`value_or_default(number_index,
value)`**: a value set, or the default if it is no number.
**`save_to_settings(tuning)`**: the numbers written to the machine's settings.
