# The lab, function by function

What measures and tests Tessera rather than encodes: the corpus
generators, the diagnostics -- the adversarial search among them --
`transient_data`,
and the tools beside them (`src/diagnostics/tool/`, `src/diagnostics/tool/adversarial.rs`). `docs/testing_protocol.md` says how they
are used; this file says what each function does. The encoder is in
`docs/reference.md`.

## `corpus/`: the bitmaps everything runs on

Every corpus bitmap is settled by a seed and its generator's parameters, and
grown again whenever asked for; nothing is stored.

**`corpus_seed()`**: the run's seed, counted as a use
(`seed::seed_counted`).

**`GrownShape`**, **`GROWN_SHAPES`**, **`SPARSE_SHAPES`**: a grown shape -- a density and
a clustering -- with how many bitmaps a timed run and a test take.
**`take(count)`**, **`timed()`**, **`tested()`**: that many bitmaps,
from consecutive seeds starting at the run's.

**`grown_bitmaps(seed, cell_density, cluster_weight, count)`**, **`one_grown`**:
`count` bitmaps from consecutive seeds, built one at a time
(`GrownBitmaps`, an iterator), or one.

**`HowMany`**, **`corpus_families(how_many)`**: every family -- cities, grown,
sparse, lines -- named, each generator taking its timed count, its
tested count, or the same number each (**`to_take(timed_count, tested_count)`**).

**`generate::grow_one_bitmap(seed, cell_density, cluster_weight)`**: grows a bitmap cell by
cell until `density` of it is set. Each new cell lands, with
probability `cluster`, beside a cell already set -- drawn from an edge
list kept with repeats, so a cell with three set neighbours is three
times as likely, which fills blobs in -- else anywhere clear
(`anywhere_clear`: random guesses, then a scan once guessing stops
paying, so densities near 1 finish).

**`city::CityPlan`**, **`CITY_PLANS`**, **`one_laid_out(seed, city_plan)`**: streets
on a pitch, blocks between them, courtyards cut out of blocks, some
blocks left as parks; the grid starts at a random offset, so blocks do
not land on tile corners. **`block`**: one block, filled, its
courtyards cut at random places inside it.

**`city::CityBitmaps`** and **`lines::LineDrawings`**: a run of cities, and of
drawings, from consecutive seeds, built one at a time as it is gone
over -- what `CityPlan` and `LineSet` hand out.

**`lines::LineSet`**, **`LINE_SETS`**, **`one_drawn(seed, line_set)`**:
`line_set.lines` lines, each from a random cell, across, down or diagonal,
of a random length, its width growing a cell at a time while a
percent chance holds, up to `widest_line`.

**`every_checkerboard::one_checkerboard(square_side)`**,
**`every_checkerboard()`**: drawn, not grown: odd square sides never line
up with the power-of-two tiles. Every odd side from 3 to 31.

**`seed.rs`**: the seed every run growing a corpus uses: the one every
crate shares, kept by `utilities::seed` in the workspace's
`transient_data/seed.csv` (not Tessera's own folder) with how many
runs have used it.
- `WHERE_THE_SEED_IS_KEPT`: the file as the tables say it; `FRESH_SEED_VALUE`:
  the value of `CIVIL_EGREGORE_SEED` that draws a fresh seed for one
  run. Both are `utilities::seed`'s, handed on.
- **`seed_counted()`**: `CIVIL_EGREGORE_SEED` if set (a number pins it, `fresh`
  draws one; neither touches the file); else the file's seed, counted
  as a use -- rolled to a fresh one once used
  `USES_BEFORE_THE_SEED_ROLLS` times.
- **`seed_uncounted()`**: the same seed, not counted: for the fine
  tests, which run too often to count.
- **`seed_in_use()`**: the seed this run settled on, if any corpus bitmap was
  asked for, and whether it was fresh: what a measurement notes.
- **`settled`**, **`settle`**: settle the seed once a run
  (`OnceLock`), print a one-row table on standard error saying which
  seed, which use and from where, and rewrite the file.
- **`fresh_seed()`**: from the process's own randomness (the standard
  library's hasher keys).

**Private to the files.** `GUESSES_BEFORE_SCANNING`
(`generate.rs`): the guesses at a clear cell before scanning for one.
`LINE_ORIENTATIONS` (`lines.rs`): the four ways a line runs -- across, down,
and down either diagonal.

## `diagnostics/`: data gathered, never judged

**`examination::BitmapExamination::examine_bitmap(tessera, stream, decoded_bitmap, bitmap)`**:
encodes and decodes one bitmap: the bits written, whether the stream is
the binary count tree, and the first cell decoded wrong.
**`first_difference(first_bitmap, second_bitmap)`**: compares words first, cells only if they
differ. **`tree_of(bitmap)`**: the tree a fresh `Tessera` makes.

**`measured::MeasuredBitmaps::measure_bitmaps(tessera, bitmaps)`**: bits, cells set, the
fewest and most bits, encode time, and the bitmaps that did not round
trip, over many. **`add`**: sums two, dropping the other's lost ones.

**`tree_stats::TreeStats::measure_tree(tree)`**: what a tree holds -- plain
tiles, complex tiles and their payload values, copies naming children,
flipping divides, cell lists -- walked from the top.

**`census::census(tree)`**: node kinds by level, walked from the top,
so stale nodes are not counted. **`node_kind_name`**: a node's name as
`docs/tessera.md` uses it; a divide with an `NoNode` child is a divide
naming children.

`RAW_CELLS` (`diagnostics/mod.rs`): what a bitmap costs written out, a
bit a cell -- what every size is set against.

**`census::Census`**: what the census is kept as -- for each kind of
node, how many the tree has at each level, whole bitmap to floor.

**`bitmaps::EXTRA_IMAGE_VARIABLE`**: the environment variable, `TESSERA_DIAGNOSE`,
naming one more PBM image to look at.
**`bitmaps::looked_at()`**: every adversarial worst bitmap and saved bitmap,
and the PBM image `TESSERA_DIAGNOSE` names, if any: what `census` and
`render` look at.

**`bitmap_png::bitmap_png(bitmap)`**: a greyscale PNG, two pixels a cell, written
with no library: the image data in stored (uncompressed) deflate blocks
inside a zlib stream (`stored_zlib`, `zlib_adler32`), each PNG chunk with
its CRC (`append_png_chunk`, `chunk_crc32`).

**Private to the files.** `count_children(tree, tile)`
(`tree_stats.rs`): counts every child of a tile that is a node. In
`png.rs`: `BITMAP_SIDE_CELLS`, the bitmap's side; `BLACK_GREY_LEVEL` and `WHITE_GREY_LEVEL`, the grey of a
set cell and of a clear one; the PNG format's fixed values --
`PNG_SIGNATURE`, `BIT_DEPTH` (8 bits a pixel), `GREYSCALE_COLOUR_TYPE`, `DEFLATE_COMPRESSION` and
`ADAPTIVE_FILTERING` (the only compression and filter methods there
are), `NO_INTERLACE`, `NO_FILTER` (each row's filter type) --
`CRC_POLYNOMIAL`, CRC-32's reversed polynomial; `ZLIB_HEADER`, deflate
with a 32 KiB window, no dictionary, stored blocks; `ADLER_MODULUS`,
Adler-32's, the largest prime under 2^16; `STORED_BLOCK`, the most one
stored deflate block holds.

## `diagnostics/adversarial/`: searching for the bitmaps Tessera does worst on

**`whole_search(seed, worst, search_effort, score_bitmap)`**: two stages, each the best
of several annealed starts (`best_of`). First a 64x64 window in an
otherwise clear bitmap, from clear and from noise. Then the whole
plane, from the window's best filled into the plane, from noise, and
from the worst bitmap, if any.

**`search_at_once(seed, worst, search_effort, make_score)`**:
`SEARCHES_AT_ONCE` searches on as many threads, from consecutive seeds,
each with its own score from `make_score` (so each can hold its own
encoders).

**`SearchOutcome`**: what one search found -- its worst bitmap and score --
and which start each stage's best came from.

**`BitmapScore`**: what a bitmap scored: the gap the search maximizes, and
Tessera's bits. **`SearchEffort`**: changes tried from each start, window and
plane.

**`anneal_bitmap::anneal_bitmap(start_bitmap, area, changes_tried, random_generator, score_bitmap)`**: simulated
annealing: a random change inside `area`, kept if it raises the score,
or, with probability `exp(gain / temperature)`, if it lowers it; the
temperature falls linearly from `START_TEMPERATURE` to 0. Returns the
best bitmap seen, as a **`FoundWorst`**: the bitmap and its score. **`pick_change_kind`**: draws a kind of change in proportion to
one plus the times it has raised the score.

**`moves.rs`**: the changes, each confined to the area, each a
**`ChangeKind`** -- a function of the random source, the bitmap and the
area:
- **`flip_a_cell`**, **`flip_a_tile`**: one cell, or every cell of a
  random tile inside the area;
- **`paint_a_rectangle`**: set or clear a rectangle at any offset and
  size, so its edges cut tiles;
- **`copy_almost`**: a tile made a copy of its neighbour at a near copy
  offset, then one cell of it flipped: a copy that almost fits;
- **`xor_a_checkerboard`**: a patch of odd-period checkerboard XORed
  in.
- **`some_cell`**, **`some_tile_inside`**: a random cell of the area,
  and a random tile inside it between two levels.

**`whole_bitmap::fill_the_plane(bitmap, area)`**: the plane tiled with the
window's sixteen variants -- four rotations, mirrored or not, inverted
or not (`transformed_cartesian`) -- one a window position, so no position is a
plain copy of another.

**`worst.rs`**: bitmaps as plain PBM images (`P1`, a row of `0`/`1`
a line, `#` comment lines for notes). Worst bitmaps, in
`transient_data/worst/`, are the worst found so far for each search,
replaced only when beaten; saved bitmaps, in
`external_benchmarks/adversarial/saved/`, are copied from worst bitmaps once
settled and never replaced. **`every_worst()`**, **`every_saved()`**: every one, by
name. **`read_worst`**, **`read_from`**: a 256x256 PBM, or `None`.
**`write`**, **`save`**: replace a worst bitmap or a saved bitmap.
**`notes_from`**: a file's comment lines. **`saved_path(bitmap_name)`**: the
file a saved bitmap of that name is.

**`cell_rect(area)`**: a tile's cells as an inclusive rectangle.

**Private to the files.** `VARIANT_COUNT` (`plane.rs`): the
variants of a window that fill the plane, one a window position. In
`worst.rs`: `SAVED_RELATIVE_PATH`, where the saved bitmaps are kept, under the
crate's folder; `PBM_MAGIC`, a plain PBM's first word; `worst_folder()` and
`saved_folder()`; `every_in(folder)`, every PBM image in a folder that
reads as a bitmap, each named by its file's stem, in name order;
`write_to(path, bitmap, comment_notes)`, a bitmap as a plain PBM image, each
note a comment line, replacing any file there was.

## `transient_data.rs`

Paths under `transient_data/` (`TRANSIENT_DATA`), out of git:
**`measurements_folder`**, **`worst`**, **`renders_folder`**, **`callgrind_folder`**.
**`publish_report(report)`**: notes the run's seed on the report, prints it,
and keeps it as `measurements/<tool>.csv`, replacing the last.

## `src/diagnostics/tool/adversarial.rs`

**`run_adversarial_tool`**: the search against the raw cells: `search_at_once` scoring
each bitmap by **`score_against_raw`** -- Tessera's bits less the raw cells of the
area searched, not Tessera's bits alone, which noise maximizes for any
encoder. Keeps the worst plane if it beats the worst kept, checks the
worst bitmap round trips, and publishes what each search found.
The parameters of `save`, by name: `FROM_PARAMETER` (the worst bitmap saved),
`NAME_PARAMETER` (what it is saved as), `DESCRIPTION_PARAMETER` (a line describing it).
**`save`**: `Civil_Egregore tessera adversarial_save <worst> <name> <description>` copies a
worst bitmap to the saved bitmaps with a description and the worst bitmap's notes.

**Private to the file.** `WORST`: the name the worst bitmap is kept
under.

## `src/diagnostics/tool/`: one tool a file

**`COMMANDS`**: every tool -- its name, what it takes and what it
prints -- run by name through `utilities::commands` (`Civil_Egregore tessera
<tool>`); with no name, the list. A tool that
measures is given a `Report` and published with the run's seed.

- **`measurement::run_measurement_tool`**: a table a corpus generator, a row a parameter
  set (`generator_table`, `row`), the saved adversarial bitmaps, then
  what the trees hold, family by family (`add_structure`): binary
  count tree streams, complex tiles, payload values, plain tiles,
  copies naming children, flipping divides, cell lists.
- **`census::run_census_tool`**: the census of every bitmap looked at.
- **`per_shape::run_per_shape_tool`**: bits on every shape, plan and line set alone.
- **`noisy_area::run`**: bits on noise at several densities against the raw
  cells.
- **`sparse::run_sparse_tool`**: on sparse bitmaps, scattered and clustered,
  density by density: the tree's bits and the binary count tree's
  (`Tessera::stream_bits`), the stream's, how many streams are binary
  count trees, and log2 of the ways the set cells could be placed
  (`placements_bits`), the least any encoding averages on scattered
  cells.
- **`timing::run_timing_tool`**: encode and decode wall time over a large corpus,
  every bitmap built before any is timed; `PER_GENERATOR` its one
  parameter, the bitmaps grown from each generator.
- **`instruction_count::run_instruction_count_tool`**: runs this tool's `instruction_corpus`
  (`CORPUS_TOOL`, the tool that runs the corpus alone for callgrind)
  under callgrind twice, collecting only inside `Tessera::encode`, then
  only inside `Tessera::decode`, on one pinned seed (**`count`** reads
  callgrind's total). **`run_corpus`**: the corpus alone, each bitmap
  encoded, decoded and checked, in one `Tessera`.
- **`render::run_render_tool`**: PNGs of the bitmaps looked at.
- **`show::run_show_tool`**: the kept reports, read back without measuring; `TOOL_PARAMETER`
  its parameter, the one tool whose report is printed -- every one if
  not given.

**Private to the files.** `SEED_VARIABLE`
(`instruction_count.rs`): the variable the corpus' seed is pinned by,
for both runs; `PARTS`: the two parts counted, to encode and to
decode, each with the function callgrind is asked for.
`percent_label(cell_density)` (`sparse.rs`): a density as a percentage, no
longer than it needs to be.

## `external_benchmarks/`: Tessera against other codecs

A crate of its own, so the codecs never enter Tessera's build.

**`BitmapRows::from_bitmap(bitmap)`**, **`BitmapRows::get`**: a bitmap as the raster codecs
take it: rows of packed bits, top row first, the leftmost cell the
highest bit. Converted before anything is timed.

**`BitmapCodec`**: `name`, `encode(bitmap, rows)`, `encoded_bits`, `decode`,
`decoded_matches(rows)` -- each codec keeps its buffers between bitmaps,
so neither timing includes allocating them.
- **`TesseraCodec`**: Tessera itself, on the bitmap.
- **`G4Codec`**: CCITT Group 4 by the `fax` crate: each row's colour changes
  coded against the row above's.
- **`JbigCodec`**: JBIG by jbigkit, through `external_benchmarks/csrc/jbig_shim.c` (built and
  linked by `external_benchmarks/build.rs`): one stripe, default options.
- **`ZstdCodec`**: zstd at a level on the raw rows: a reference point, no
  bitmap codec.

**`main`**: every family through every codec (**`time_codec_on_family`**: encode,
decode, check, and total the sizes and times), a table a family
(**`add_table`**) and one for all.

**`external_benchmarks/src/diagnostics/adversarial.rs`**: the adversarial search against each codec in
turn (`OPPOSING_CODECS`), each with its own worst bitmap: **`score_against_codec`** is Tessera's
bits less the codec's (**`bits`**); worst bitmaps are replaced when beaten,
and reported with both encoders' bits and median encode times
(**`median_micros`**).
