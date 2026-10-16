# The world, function by function

The design is in `server.md`.

## `lib.rs`

**`World`** `{info, generation, arena, storage, entities, simulation,
cold, halos}` -- **`cold`**, each cold superchunk's state as a save
keeps it; **`halos`**, the simulation's; every one the server's own,
read from outside by `world_access.rs`'s methods -- and
**`World::empty_world(world_info, world_generation, thread_count)`**, what generating and
loading start from, hot as `world_info` says: forced throughout if it has a
side and is forced, else about the halos of the kind of entity it
names, `HOT_ENTITY` if none (**`hot_of(world_info)`**).
**`start_world(options)`**: a `WorldStart` (`world_start.rs`) made into a world
-- `thread_count`, every one the machine has if `None`; `sheep_per_flock` on every
superchunk of a world with a side, on the origin (`WORLD_MIDDLE`)
alone of one without, as sheep everywhere would keep the whole of an
endless world hot; their halos hot, or, of a world forced hot, all of
it; generated, flocked: below ("Generation"); the sheep for each
superchunk generated in the viewport kept (`WorldInfo::camera_flock`) if its
camera loads superchunks and it is not forced hot.
**`World::put_flock(superchunk, sheep_per_flock)`**: a flock queued on a
superchunk, from its own stream of the seed's inverse -- the same
flock whenever it is put -- each sheep on a cell the collision plane
does not hold, and set there.
**`save(folder, world)`**: every dirty bitmap written back and the
ring flushed (`World::write_back_and_flush_all`), then each superchunk's
image and state -- live if hot, kept if cold -- the hot file, and the
world's file last: a **`SaveWritten`** `{superchunks, entities, bytes_written}`.
**`worlds_in(folder)`**: the worlds saved in a folder, by their
folders' names, sorted.
**`load_world(folder)`**: every superchunk's image into the cold pool and its
state, read whole, kept cold; then the hot file's hot superchunks made
hot, its cooling ones cooling again and its warming ones warming again -- a `World`, or a `DiskError` naming
the file and what is wrong. **`load_keeping(folder, bytes_kept)`**: the
same, keeping so many bytes of cold images in memory, the rest left in
the save; `COLD_POOL_BYTES_KEPT` (2 GiB), what a world keeps unless
told.

## `world_start.rs`

What a new world starts from, whoever gives the numbers -- the command
line or a window's sliders -- the server alone knowing what they mean.
`FLOCK` (4,000): the sheep a superchunk starts with, unless told.
`LAND_TRIES` (256). **`WorldSize`**: `NoEnd`, or `Limited {side,
forced}` -- so many superchunks along a side, a square about the
origin, and whether every one of them is hot throughout;
**`WorldSize::of_side(side_superchunks, forced_hot)`**: a side of 0 no limit, refused if
forced without a side, which only a world with one can be.
**`WorldStart`** `{seed, generation, size, threads, sheep, hot_entity,
camera_loads}`;
`WorldStart::default()`: seed 1, the default generation, no size, every
thread, `FLOCK` on the origin, hot about `HOT_ENTITY`, its camera
loading nothing.
**`WorldStart::from_tuning(seed, tuning)`**: as a window's sliders have it
(`utilities::tuning::Tuning`): how it is generated, its size -- a side
of 0 none, forced hot counting only with a side -- its sheep, and its
camera loading superchunks only if it is not forced hot; from the seed
given or one drawn; on every thread the machine has.
**`WorldStart::of_world(world_info, world_generation)`**: what a world opened started
from, as far as its file says -- its sheep the camera's flock, or
`FLOCK`.
**`seed_with_land(first_seed_tried, world_generation)`**: the first seed from `from`
with land about the origin.
**`drawn_seed(world_generation)`**: a seed drawn at random, the first from
it with land about the origin within `LAND_TRIES`, else the one drawn.

## `halos.rs`

`HOT_ENTITY` (the sheep, for now). **`World::with_halos(halo_work)`**: the
simulation's halos given what the world holds, and what generates a
superchunk never made. **`World::move_halos`**: the halos moved, then,
if the camera loads superchunks with sheep, a flock put
(`put_flock`) on each the move generated that is in the viewport.
`owe_camera_flocks(superchunks)`: those generated out of view kept as
owed their flock (`WorldInfo::without_camera_flock`).
The viewport is told to the halos (`World::keep_viewport`), by the
host on each frame asked.
**`World::keep_hot(wanted_hot)`**, **`World::warming`**,
**`World::cooling`**, **`World::start_warming(superchunk, due_tick)`**,
**`World::flush_what_went_cold`** (every write-back on its way put in
the ring and the ring flushed, no hot bitmap encoded),
**`World::write_back_and_flush_all`**: each the halos' own
(`../../simulation/docs/reference.md`, "`halos.rs`").

## `tick.rs`

`tick_chosen(world_simulation, bitmap_arena, entity_store, seed, chosen_rules)`: the rules
chosen of `RULES` on each hot superchunk's turn, the halos left where
they are.
**`World::tick`**: the rules, then the halos moved: a **`WorldTick`**
`{rules, halos}`; in a debug build, `World::broken_invariant` checked
after it. **`World::tick_only(chosen_rules, each_rule_timed)`**: the rules
chosen alone, each one's time taken if `timed`, the halos not moved.

## `world_access.rs`

A world from outside the server (`server.md`, "What a world holds
together"). Read: **`World::world_info`**, **`World::world_generation`**,
**`World::bitmap_arena`**, **`World::chunk_storage`**, **`World::entity_store`**,
**`World::world_simulation`**, **`World::cold`**, **`World::halos`**, each
part lent unchangeable. Changed between two ticks, each applied as it
is called: **`World::put_entity(header, attributes)`** and
**`World::remove_entity(header)`**, an `InstructionsApplied` each, the
collision plane kept with them (a put on a cell held for another
refused);
**`World::write_cells(layer_type, writes)`**, a `WritesApplied`.
**`World::keep_viewport(viewport)`**: the viewport told to the halos.
**`World::keep_cold_pool_within(bytes_kept)`**: how many bytes of cold
images it keeps in memory; **`World::page_cold_pool_out`**: the rest
paged to disk, after each tick (`server.md`, "The cold pool paged").
**`World::broken_invariant`**: what is wrong between the parts, in
words, or `None`.

## `rules.rs`

**`RULES`**: every rule a world ticks, a **`Rule`** `{registered, counted,
rule}` each: the type registry's rules, as many and in its order, made
as the server is compiled (`registered_at(place)`). `registry_row` is
its row in the registry, **`Rule::master`** its master's. `RuleFunction`: what a rule is called as, on one superchunk's turn. `on_each_sampled(turn, rule_id, chance, rule)`: a rule of the cells -- the function on one sample -- handed to the simulation to run on each sample of its master, the registry's: a layer's set cells, a wide plane's every tile.
`written_for(rule_id)`: what a rule counts and its function, by its ID --
all the server says of a rule, and the build fails if the registry
has one it says nothing of. **`RulePlace`**: a rule's place in it, made of its
ID as the server is compiled (**`RulePlace::from_registry_id(rule_id)`**, an ID
not there not compiling), **`RulePlace::rule`** the row there;
**`GRASS_RULE`**, **`TREES_RULE`**, **`SHEEP_RULE`**, **`WEATHER_RULE`**: each rule's.
**`TickCounts`** `{counts, rule_times}`: each rule's `RuleCounts` and time at
its place; **`counts_of(rule)`** and **`time_of(rule)`** read them by place.
**`ChosenRules`**: some of the rules --
**`EVERY_RULE`**, or **`from_rules(rules)`** -- and a superchunk's turn of them,
**`turn`**, or **`timed_turn`** with each one's time.

## `diagnostics/`

The world they tick is one the server starts as any other
(`start`): **`plain_world::plain_world(superchunks, grass_cover, sheep,
threads)`**, a plain (`worldgen::WorldGeneration::plain`) with a side,
forced hot, so a rule is measured alone -- **`plain_world::plain_start`** how it is started, **`plain_world::pasture_start(superchunks, grass_thousandths, sheep_per_flock)`** how the pasture's is, for the same world shown in a window; **`plain_world::superchunks`**
how many it holds hot.

**`throughput::run_throughput(ticks, grass_thousandths, superchunks, thread_count)`**: grass
ticked flat out: each phase's time, samples, writes, cells missed, the
process's memory read every tick, the arena's and storage's stats --
a **`ThroughputRun`**.

**`pasture::run_pasture(ticks, grass_thousandths, sheep_per_flock, superchunks, thread_count)`**:
grass and sheep ticked flat out: the flock and grass over the run, what
the sheep did, the acts over a border applied, each phase's time and each rule's -- `ChosenRules::timed_turn`,
over every thread, the rules picked (`RULES_TICKED`) -- the memory, the entities' stats, and a
**`Census`** of the flock and grass every `CENSUS_EVERY` (100) ticks:
a **`PastureRun`**.


## `transient_data.rs`

`TRANSIENT_DATA`: the crate's `transient_data/` folder.
**`measurements()`**, **`saves()`**, **`paging_folder()`** -- where running
worlds page their cold pools -- **`publish(report)`**.

## `world_hash.rs`

**`world_hash(world)`**: the world's hash, a **`WorldHash`** `{tick,
cells, entities, random_streams, halos, cold}` -- a word a part, each
folded (`utilities::hash::fold_word`) from the part in Morton order: every
hot bitmap of every layer type with its chunk and count; every entity's
header and attributes; every random stream; the superchunks hot,
warming and cooling with when each is due; every cold superchunk's
state and image. What went cold is flushed first
(`World::flush_what_went_cold`); no hot superchunk is written back for it.
**`WorldHash::whole_hash(seed)`**: the parts, the tick and the seed in one
word.

## `commands.rs`

**`COMMANDS`**: what `Civil_Egregore server <command>` runs, each with its
parameters and their defaults. `Civil_Egregore server new <folder> [seed]
[sheep] [side] [forced] [threads]`: a world generated from the seed --
the origin, a flock of `sheep` on it, and its halo (**`new`**), of
`side` superchunks a side unless 0, forced hot throughout if `forced_hot`
is not 0, on `threads` threads or every one the machine has if 0 --
and saved in the folder, which must not hold one and
whose name is the world's (**`world_name_of`**). `Civil_Egregore server run <folder> [ticks]`: it
loaded, ticked and saved again (**`run_saved_world`**). `Civil_Egregore server info
<folder>`: what its world file says (**`describe_saved_world`**). A folder given as a plain name is one
of the worlds' folder (`utilities::settings::world`); anything more is
a path. **`print_command_on_folder`**: a command run on its folder, its line printed;
`number_argument`: an argument as a number, or its default. `FOLDER`, `FORCED_HOT_PARAMETER`:
the parameters' names. The diagnostics commands,
`throughput`, `pasture` and `check` (`print_throughput`, `print_pasture`, `print_world_hashes`), are listed there too.

## `diagnostics/tool.rs`

**`threads_asked`**: the threads asked for, every one if 0, no more than
the superchunks. The parameters' names: `TICKS`, `GRASS_THOUSANDTHS` (in
thousandths), `SUPERCHUNKS`, `THREAD_COUNT`, `FLOCK` (sheep a superchunk),
`SEED` (in hexadecimal; 0 the run's own), `TICKS_BETWEEN_HASHES` (ticks between
hashes), `ORIGIN_SHEEP`. `share_percent(part, whole_time)`: a share of a time as a report
shows it.

**`print_throughput`**: runs `throughput::run_throughput` and publishes its time, rates
and memory tables. **`print_pasture`**: runs `pasture::run_pasture` and publishes the
flock, time a sample and a wake, rates, what is held and the census
(**`census_table`**). **`print_world_hashes`**: `Civil_Egregore server check [seed]
[ticks] [ticks between hashes] [sheep] [threads]` -- a world from the
seed (0: the run's), its camera loading nothing, ticked and its hash
printed before the first tick and every so many after, a row a hash,
part by part, with nothing of the machine in it.

## Generation

How a world is generated is `worldgen`'s (`worldgen::WorldGeneration`,
saved with the world, a number a row). **`generate_sized(generation,
seed, size, hot_entity, threads)`**: what `start_world` builds a world from,
generated so, as far as its size lets it reach, nothing hot yet, to be
hot about the entities of the kind `hot_entity` unless forced -- and what a way
of generating is tried out on by itself. **`flocked_world(world,
superchunks, sheep_per_flock)`**: what `start_world` builds a flock from, put on a
world with nothing in it yet -- and what a flock is tried out on by
itself. The server generates nothing: a superchunk's cells are
`worldgen::generate_superchunk`'s, the image of them storage's
(`chunk_storage::SuperchunkCells::image`); the server hands the one to
the other where the halos ask for a superchunk.

## `host/mod.rs`, `host/host_thread.rs`

`RunningWorld` and `HostThread` are in `host_thread.rs`, the rest in `mod.rs`.

A world run on a thread of its own for a client -- a window -- that
asks it for the cells of its viewport. `TARGET_PACE` (the tick rate, `TICKS_A_SIMULATION_SECOND`: a second of the simulation in a second),
`CENSUS_EVERY` (1,000 ticks), `CATCH_UP`, `FRAMES_SHARE` (an eighth of
a tick's time, what answering may take of it when the host has none to
spare). **`Host`**: the host as a
client holds it, each call sent to its thread and done there between
two ticks, each saying whether the host was still there --
**`Host::start_on_thread()`**: the host on a thread named `host`, no world run
yet, and where its frames come back; **`Host::terrain()`**: where its
terrain is asked of (`host/terrain.rs`); `HostThread::telling_terrain(terrain)`:
the thread's state, the terrain's thread told of each world run; **`ask_frame(frame_ask)`** -- answered with
frames, the last with `more_to_come` unset, unless no world runs -- **`set_paused(ticking_paused)`**, **`set_pace(ticks
a second, or flat out)`**, **`make_world(start)`**,
**`open_world(world_name)`** -- that world of the worlds' folder run in
place of the one run -- **`save_world(world_name)`**, and **`reset(tuning)`**
-- the world run made again from its start, generated as the tuning
has it now, a world of its own of no name; several asked with no
frame asked between make it once, as the last says; a frame asked
after is of the world remade. `HostRequest`, private:
a call as sent. **`census_path()`**: where a run's census is kept
(`transient_data/measurements/census.csv`); **`census(seed)`**: its
file, started afresh for each world. **`RunningWorld`**: the world run, the
superchunks whose heights were sent, when it began, its census;
**`from_start(world_start, world)`**, a world just made, its census begun.
**`AskBeingAnswered`**: an ask being answered -- what was asked, the
viewport's hot superchunks then, those yet to copy, those copied and
not yet sent, what copying has taken.
**`HostThread`**: the world run if any, the worlds run so far, paused,
the pace, the name, what was last said, the ask being answered, what
the last tick took -- **`serve_requests`**: requests read between ticks, a little
of the ask answered, a tick, and a wait for the next one's time; paused
or with no world, it waits for a request; **`run_world`**,
**`run_in_place`** -- an ask half answered is dropped with the world it
was of -- **`reset_now`**, **`save`**, **`begin_frame(frame_ask)`**: the
viewport kept hot, what is to copy listed; **`answer_a_little()`**: a
superchunk copied at least, and more until **`time_for_frames()`** is
up -- what is left before the next tick is due, `FRAMES_SHARE` of the
last tick's time at least, no limit while paused -- and those sent as a
frame; seen from near, sent only once all are copied, being one
picture; **`tick`**.

## `host/shown.rs`, `host/wire.rs`

A ticker's world shown in a window (`server.md`, "A world shown").
`RENDER_VARIABLE`: the environment variable's name; `SHOWN_WORLDS_PORT`; `WINDOW_OPENS_WITHIN`.
**`worlds_shown_asked()`**: whether the environment asks. `TickerEnd`: the ticker's
end -- the window's socket, its requests read on a thread, pause,
pace, which world was last shown and what of it was sent
(`begin_showing`, `between_ticks(world)`); `TICKER_END`, the one of a process;
`game_program()`, the game's program to start as the window; `connect_window()`,
the window reached or started. **`between_ticks(world)`**: what the
world's tick ends with -- `SHOWING_ASKED`, the flag read once -- `serve_window(world)` where a window was asked for.
**`Host::of_shown_worlds()`**: the window's end, a host whose world is
whatever ticker's comes: `WindowTold` what the window last said, kept for
the next ticker (`send_to_ticker`); `tickers`, every ticker that connects, one
after another, its frames sent on (`next_ticker`, the next waited for). **`no_ticker_is_left()`** (`NO_TICKER_LEFT`): the last ticker gone and none come within **`WINDOW_WAITS_FOR_THE_NEXT_TICKER`** -- the window closes on it.

`wire.rs`: `MOST_WORDS`; `send_message(stream_to, words)`, `receive_message(stream_from)`, a message
of words; `MessageWritten` (`write_number`, `write_pair`, `words`, `write_text`, `write_maybe`) and
`MessageRead` (`over_words`; `read_number`, `read_pair`, `words`, `read_text`, `read_maybe`); `viewport_written`/`viewport_read`,
`near_written`/`near_read`; `request_written(host_request)`,
`request_read(words)`; `frame_written(viewport_frame)`, `frame_read(words,
waters)`.

## `host/terrain.rs`

What a client asks of a world's terrain where no frame brings it
(`server.md`, "Terrain asked of the host"), answered on a thread named
`terrain`. `Height`, the word heights come in. **`DrawnHeightRange`** `{ground,
ocean, highest}`, the heights a world is drawn between
(`DrawnHeightRange::from_generation(world_generation)`: a world with no relief, a plain, drawn between the default world's, as low land); **`CellCover`** -- `Ocean`, `Tree`, `Grass`,
`Dirt` -- what a cell is generated with.
**`HeightsAsk`** `{world, first, size, skipped_corners_cartesian}`: the heights of a
rectangle of cells, those of the superchunks `skipped_corners_cartesian` passed by and
answered as 0. **`MapAsk`** `{world, first, step, size, mesh_lines_asked}`: a
map, a pixel the cell in its middle; **`MapAnswer`** `{ask, levels,
cells}`, a **`MapCell`** `{height, above, cover, on_a_mesh_line}` a
pixel, none past the world's edges.
**`TerrainAsker`**: where a client asks, a handle to clone --
**`heights(heights_ask)`**, **`map(map_ask)`**, each waiting for its answer on the
caller's thread, none if the host is gone or runs another world.
`TerrainRequest`: what the thread is sent -- `World`, by the host's
own thread, of each world run; `Heights`; `Map`. `start_terrain_thread()`: the
thread; `answer_terrain_requests(request_receiver)`: its loop. `shared_out(row_items, width, row_work)`: rows
given out among the machine's threads. `heights(seed, generation,
ask)` and `map(seed, generation, ask)`: the answers worked out.

## `host/frame.rs`

**`frame_of(world, worlds, frame_ask, hot, cells, last_answer_measured, world_name and last_outcome, more_to_come)`**: a frame of a world, the one making of it -- the host's thread's and a shown world's.

`CHUNK_WORDS`, `WORD_BITS`, `STAGE_BITS`, `OLDEST_TREE_STAGE`,
`CLOUD_BITS`, `CLOUD_TILE_SHIFT`, `DEEPEST_CLOUD`; `CLIMATE_BITS`, `CLIMATE_TILE_SHIFT` and **`climate_over(climates, tile)`**, a tile's climate off `Cells::climates` -- a superchunk's climate plane's words, sent with its cells for a client to write on the tiles (`Climate`, the registry's, handed on); **`raining_tiles_of(clouds, climates)`**, the tiles whose cloud is above what their climate holds, a bit a tile (`Cells::raining_tiles`), read by **`rains_over(raining_tiles, tile)`** -- the cloud
plane's width, tile size and most, off its type in the registry --
**`cloud_over(clouds, tile)`**, a tile's cloud in `Cells::clouds`
(the plane's buckets as the arena holds them, empty where a
superchunk has no cloud),
**`cell_of_bit(bit_index)`** -- how a frame's bits lie, so a client asks no
one else. `DEEP_WATER_DEPTH` (16). **`Viewport`** `{first_cell_cartesian, last}`: what the
renderer should render, in superchunks -- not Bevy's camera viewport,
a rectangle of the window -- **`contains(superchunk_cartesian)`**. **`FrameAsk`** `{viewport,
detail, skip, most, near}`: what a frame is to carry, `viewport` `None`
when the client renders none of the world's cells (map mode); `superchunks_skipped`
and `most_superchunks` count the viewport's hot superchunks. **`hot_in(world,
viewport)`**: those, row by row.
**`NearRectangle`** `{first_cell_cartesian, size_in_cells, pixels_a_cell}`: the cells seen from near.
**`Cells`**: a hot superchunk's planes copied -- grass, trees, their
stages where a cell is a pixel or more, its water (a `Water`, shared,
not copied again) -- its heights the first frame it is hot in, its
sheep's cells. **`height_in_frame(height_words, place)`**: a cell's
height from those words. **`ViewportFrame`** `{world, seed, generation, levels, side, tick,
ticks_a_second, sheep, grass, trees, sync_seconds, sync_share,
viewport, hot, detail, near, named, said, more, cells}`: `world` counts the
worlds the host has run, so a client knows what it drew is of another;
`hot`, every hot superchunk of the viewport, so a client knows what it
drew there of any other has gone cold; `more_to_come`, whether frames of the
same ask are still to come. **`count(world,
layer_type)`**; **`copy_superchunk(world, superchunk, frame_ask, heights_sent)`**: one
superchunk's `Cells`, none if it has gone cold since asked
(**`layer`**, **`sheep_cells_of`**); **`Water`** `{depth_low_bits, deep_cells}`: a
superchunk's water off its image, read once and kept
(**`from_image(image)`**).
