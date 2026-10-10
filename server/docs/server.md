# The server

The world as a whole: made from a seed, ticked, saved and loaded. The
server is the one crate that holds the hot bitmaps, the stored superchunks, the
entities and the simulation together, and that makes the one thread
dispatcher the tick and chunk storage's jobs share; the program (`../src/main.rs`)
and the renderer (`../renderer/`) call it.

## Made from a seed

`start(options)`: the one way a world starts, from a `Start`
`{seed, generation, size, threads, sheep, hot_entity, camera_loads}`
(`src/world_start.rs`) -- every number a world starts from, whoever
gives it: the command line (`server new`), or a window's sliders
(`Start::from_tuning`), the seed drawn with land about the origin if
none is given (`drawn_seed`). What the numbers mean is the server's
alone; a window only hands them over. A world with a size has
sheep on every superchunk of it, and their halos -- the whole of it --
hot before it ticks; or, forced hot, all of it hot throughout whatever
they do. One with none has them on its origin superchunk
(`WORLD_MIDDLE`) alone, as sheep everywhere would keep the whole of an
endless world hot: the origin and the eight about it hot before it
ticks. If its camera loads superchunks and it is not forced hot, the
viewport's superchunks are hot too, and it keeps how many sheep a
superchunk generated in the viewport starts with ("Halos", below).
Unless told otherwise (`Start::default`), it has no size and 4,000
sheep. Every superchunk's contents come from the world's seed
and its superchunk index (`worldgen::generate_superchunk`), so a superchunk is the
same whenever and in whatever order it is made: the world has no edge
but the coordinates', and grows as the sheep wander. A superchunk is
its terrain (`../worldgen/`) -- heights, and the walls they make, two
layers -- and on it grass in patches, dirt being a cell with none and
having no layer, and trees in patches of
their own, each of a stage drawn for its cell (`worldgen::Generation`,
`worldgen::patches`).

Each superchunk's random numbers are a stream of their own
(`Rng::for_stream`): seeded by the seed moved along by the
superchunk index, as they first were, two superchunks drew one
sequence a few draws apart -- two flocks came out with the same sheep.

## What a world holds together

A world (`World`) is several parts that must agree: its hot bitmaps
(the arena), every superchunk as stored, the hot superchunks'
entities, the simulation's random numbers, each cold superchunk's
state, and the halos. Four things are always so between two ticks,
and `World::broken_invariant` says which is not, if any:

- the entities are held for exactly the hot superchunks -- those the
  arena has bitmaps of, in the same order;
- no superchunk is both hot and cold;
- no hot superchunk's image is paged to disk ("The cold pool paged");
- every superchunk made is one or the other: as many stored as there
  are hot and cold together.

So the parts are not open to be changed one by one. Outside the
server a world's parts are read (`World::arena`, `World::entities`
and the others, each lent unchangeable) and the world is changed only
whole: ticked (`World::tick`, `World::tick_only`), its halos moved,
saved, or, between two ticks, by `World::put_entity`,
`World::remove_entity` and `World::write_cells` -- each applied as it
is called, and saying what came of it, as an entity or a cell off the
hot superchunks is lost or missed, not put. A debug build checks the
four after every tick (`World::tick`); the fast tier checks them at
every tick and every load of a long run that warms and cools
superchunks (`../tests/fast/world.rs`).

Inside the server the parts are open to its own modules, which are
what keeps them agreeing: the halos' moves (`World::with_halos`), the
save and the load.

## Halos

Only the superchunks about the hot entities are hot, and the halos are
the simulation's (`../../simulation/docs/simulation.md`, "Halos"): the
world lends them what it holds (`World::with_halos`) and tells them
two things. The kind of entity it is hot about (`Start::hot_entity`,
`HOT_ENTITY` unless told): people, to come; for now the sheep stand
in, and a save keeps it. And the world's size, if it has one
(`Start::size`, `Civil_Egregore server new <folder> [seed] [sheep]
[side] [forced] [threads]`): so many superchunks along a side, a square about
the origin, nothing ever made outside it -- and whether every one of
them is forced hot throughout, which only a world with a side can be;
a save keeps both. What generates a
superchunk never made is the server's, handed to the halos' jobs.

A world without a size may have its **camera load superchunks**
(`Start::camera_loads`, the slider `camera loads`; nothing to a world
forced hot, all of which is hot anyway). It starts as any other: the
origin, its flock and its halo. Then each frame a window asks of the
host tells the world its **viewport**, whatever the window should
render (`Halos::keep_viewport`; none in map mode): every superchunk
of it kept hot besides the halos, however many -- which are hot is the
simulation's to say, as the world's hot defines it (`hot_of`) --
generated as a halo's are if never made. Each
superchunk generated in the viewport gets `sheep` of its own
(`World::put_flock`, the same flock whenever it is made); one generated
only for a halo gets none while it is out of view, or the sheep's
halos would generate more sheep, and those more, without end. It is
owed its flock (`WorldInfo::without_camera_flock`) and given it once
the viewport reaches it: the eight about the starting flock among
them, generated before the camera has seen anything. The sheep keep hot what was seen,
so the slider for sheep is how heavy the world grows as the camera
wanders: with none, what was in the viewport cools once it is out of
it. A save keeps the sheep per superchunk generated in the viewport
(`camera flock`) and the superchunks still owed theirs
(`without camera flock`); a world
run with no window has no camera, and loads nothing by it.

## TickCounts

`World::tick` runs every rule of the cells (`../sca_rules/`) and every
kind of entity (`../entity_rules/`) on each hot superchunk's turn, then
moves the halos. The rules are one table, `rules::RULES`: a name, what
the rule counts, and its function, in the order a turn runs them --
grass, trees, sheep. A rule added is a row added there; nothing else
lists them. What a tick did is an array with a rule's counts at its
place in the table. Nothing is looked up by a name as a world runs: a
rule's place is a constant made of its name as the server is compiled
(`SHEEP_RULE`, `RulePlace::named`), and so is the place of each thing
a rule counts, worked out from the one list of their names
(`sheep::EATEN`, `instructions::place_counted`) -- a name that is not
there does not compile. A tick's counts are read by them
(`counts.of(SHEEP_RULE)[EATEN]`), and a diagnostic or a test ticks
only the rules it picks (`Chosen::of(&[GRASS_RULE])`).

A rule's counts come from two places. What it counts as it runs is
what its function returns. What hangs on a write being applied -- a
spread, a decay, a meal, a birth seen at a meal -- the simulation
counts as it applies it (`../simulation/docs/simulation.md`,
"Compare-and-write"), under numbers: a turn gives each rule
its own (`Turn::count_under`, a rule's place times
`COUNTS_OF_A_RULE`), and after the tick they are added to the rule's
counts (`rules::with_counts_applied`). So a count is what happened.

## The same on every machine

A world follows from its seed alone: the same seed ticked as far is the
same world, to the bit, on any machine and any number of threads.
`world_hash` says so in a few words -- one each for the hot bitmaps,
the entities, the random streams, the halos and the cold pool, folded
by nothing a machine could do its own way: no float, no pointer, no
order but Morton's -- and
`Civil_Egregore server check <seed> <ticks>` prints them every so many
ticks:

    cargo run --release -- server check bd044074d908b399 20000 500 > linux.csv

run on two machines and the two files compared; the first row to
differ says at which tick they part, the first column of it in what.
The rows are the same on any number of threads too, and nothing
printed says what machine it ran on, so two runs' files are compared
as they are.
The world checked has no camera: a camera's superchunks are hot by
where a person looks, which no seed says.

## Two writes on one cell

Rules read the world as the tick found it and queue their writes, so
two may be queued for one cell in a tick. Every cell a rule writes is
a compare-and-write, applied only if the cell is still as the rule saw
it, and what hangs on it is held against the same thing
(`../simulation/docs/simulation.md`, "Compare-and-write"):
of two such writes the first applied happens and the other is refused,
and counts nothing. Which pairs there are, and what comes of
each -- held by `tests/fast/double_writes.rs`, which sets a tick's
counts against the cells that changed and finds them the same, to the
cell:

| the two writes | can it happen | what comes of it |
|---|---|---|
| two sheep eating one cell | no: a sheep eats the cell it stands on, and a cell holds one entity | every cell eaten is a write and a cell changed |
| grass spreading onto one cell twice | yes | set once, one spread counted; the other refused |
| grass spreading onto a cell, the cell decaying or eaten | no: spreading fills cells that were dirt, decay and sheep clear cells that were grass | |
| a cell decaying and eaten | yes, rarely | cleared once by the decay, applied first, and a decay counted; every write of the meal refused -- the sheep not fed, nothing eaten counted, asleep a step and hungry still (`../entity_rules/tests/fast/sheep.rs`, where it is made to happen) |
| two trees put on one cell | yes | one tree, one counted; the other refused |
| a tree put on a cell, the tree there dying | no: a tree is put where none stood | |
| a tree growing and dying | no: a tree is sampled once a tick; dying, its stage goes if the tree still stands and the stage is as seen, then the tree | |
| two entities stepping onto one cell | yes | the first applied takes it, the other stays where it stood and wakes as it was to |
| two lambs put on one cell, or a lamb put where another sheep steps | yes | the first applied takes the cell; the lamb put after is made on another neighbour its mother may step to, or, every one taken, refused -- and born later: its mother is pregnant until she has seen it stand (`../entity_rules/docs/entity_rules.md`, "The sheep") |
| two entities setting one attribute of a third | yes (no rule does yet) | the first applied is kept and the other refused, having seen the attribute as it no longer is: the same one on any number of threads (`../simulation/tests/fine/instructions.rs`) |
| an entity's attribute set by another, the entity changing itself | yes (no rule does yet) | both stand if they are of two attributes -- an entity is written an attribute at a time, never whole; of one attribute, the first applied and the other refused. Moving in that tick, or crossing to another superchunk, it is written all the same and takes it along (`../simulation/tests/fine/instructions.rs`) |
| a sheep fed and put to sleep hungry | no: the two are under opposite compares of its grass | one is applied: the sheep is written once |

No pair leaves a cell or an entity differing by the order threads ran
in: everything is applied in one order. And no write lands on
another's: nothing a rule queues can overwrite what another did in the
same tick (`../simulation/docs/simulation.md`, "No write lands on
another's"). A lamb's put is not held
against what its mother comes to -- it may land in the next
superchunk, and a compare is of the superchunk its write lands in --
so its birth is still seen the tick after.

## Saved and loaded

A save is a folder (`chunk_storage::disk`, and
`../chunk_storage/docs/chunk_storage.md`, "On disk"):

| file | what it holds |
|---|---|
| `world.csv` | CSV, a row a thing, its name and what it is, in any order: the format's number, the world's seed in hexadecimal, the tick it is at, its layer types, its side if it has a size, the sheep each superchunk generated in the viewport starts with if its camera loads superchunks, and how it is generated, a number a row |
| `hot.csv` | CSV: the hot superchunks, a row each, those cooling with the tick each goes cold at, and the warming ones with the tick each turns hot at |
| `superchunks/<index>.image` | a superchunk's cells and heights: its image, as the cold pool holds it |
| `superchunks/<index>.state` | its random stream's state, its entities with their attributes |

`<index>` is the superchunk index, 44 bits, 11 hexadecimal
digits: its name, and nothing else is.

A world's name is its folder's, nothing in the files: only what a
folder may be named on Windows and Linux alike
(`utilities::settings::world_name`). How it is generated
(`Generation::numbers`) is kept in its file, so one loaded goes on
generating as it was made.

**Saving** is between two ticks. Every dirty bitmap is written back --
encoded on every thread, after every write-back still
on its way -- and the ring flushed, so the cold pool's images are the
world's cells; each image is written as it is, and beside it the
superchunk's state; then the hot file. The world's file is written
last, each file beside itself and then
renamed, so a save cut short leaves the one before readable.

**Loading** reads every image into the cold pool and keeps every
state -- each read whole, so a bad one is refused now -- as a cold
superchunk's. Then, before it ticks, it makes hot the superchunks the
hot file names, decoded on every thread: every layer
type of the world made hot on every chunk of them -- a type with no
cell left on a superchunk has no stored layer, and must be hot all the
same to be written to -- their entities put back at the world's tick,
their random numbers taken up. The cooling ones cool again, and the
warming ones warm again, each to turn at the tick it was to.

**A world loaded goes on as the one saved would have**, to the cell,
the entity and the random number (`tests/fast/world.rs`: 1,500 ticks,
saved, then 3,000 more on both; and a world saved and loaded seven
times mid run -- at ticks 1, 700, 701, 1,900, 3,333 and 4,000, and the
first tick a superchunk is warming, each
stretch run on what the files hold alone -- against one run straight
to tick 4,000: the same cells, entities and random numbers). What makes it so:

- **Random numbers are a superchunk's own, and kept.** Each superchunk
  has a generator that goes on from tick to tick (`Simulation`), first
  seeded from the world's seed and its superchunk index; its state is in
  the state file. Were they made anew from a tick's seed, a load would
  start them over.
- **Entities are put back through the same door as any other**: queued
  and applied, so their wakes are filed as they were. Wakes left over
  from before an entity was changed are not kept: they wake nothing.
- **Nothing is half done between ticks**: a crossing is settled
  within its tick, so a save holds each entity once, on one cell.
- **A superchunk turns hot at a tick fixed when a halo reaches it**,
  not when its job is done, and cold at a tick fixed when the
  halos leave it; the hot file keeps both.
- An entity whose wake has passed when it is put back -- kept while
  its superchunk was cold -- wakes the tick it is put back.

Most of a save is heights, kept raw, about a byte a cell; the layers
and the entities are each a fraction of that (`Civil_Egregore server new`
says the bytes written).

Not yet: superchunks no longer in the world are not removed from a
save's folder.

## The cold pool paged

A world keeps `COLD_POOL_BYTES_KEPT` bytes of its cold superchunks'
images in memory, or as many as it is told
(`World::keep_cold_pool_within`); after each tick the rest are paged
to disk (`World::page_cold_pool_out`;
`../chunk_storage/docs/chunk_storage.md`, "Paged to disk"), in a folder
of the world's own under `transient_data/paging/`, gone with it. None
of a superchunk hot, cooling, warming or lingering is paged, so what a
tick or a frame reads of an image is in memory; a superchunk turning
hot has its image read back by the job that decodes it, off the tick.
If the disk refuses an image, the world says so once and pages no
more.

Paging changes nothing a world holds: one keeping no cold image in
memory is, at every tick, save and load, the world that keeps them
all (`tests/fast/paging.rs`). A world loaded reads and checks every
image, and leaves in the save those past what it keeps
(`load_keeping`): so the save it runs from is not to be removed under
it.

Not paged: a cold superchunk's state (its entities and random number,
`World::cold`), small beside its image, and what is hot -- a world
whose hot superchunks alone pass what it keeps holds them all the same.

## The host

A world run for a window, on a thread of its own (`src/host/`): the
window holds a `Host` and calls it -- a world made or opened in place
of the one run, the world saved, paused, paced, and the cells of its
viewport -- each call sent to the host's thread and done there between
ticks. It answers only the viewport's hot superchunks, each as a tick
left it, copied and nothing more -- turning cells into pixels is the
client's; a layer's words go as the storage holds them, in Morton
order (`Cells`), and that layout is the client's format, chosen: one
process, and no word turned round for a window -- a few between two ticks, in the time the host has to spare
or a small share of a tick's (`FRAMES_SHARE`), each few sent as a
frame of their own, the last saying it is (`Frame::more`): so a
frame's superchunks are not all of the one tick, answering costs each
tick a little however much is asked, and nothing asked for is left
out. What is rendered so costs the ticks next to nothing -- and with them which are hot, so a window
drops what it drew of one gone cold; it sends nothing unasked, so a window
that falls behind slows no tick, and it is the client that sets how
often the world is drawn. It starts with no world, and waits until
asked to make one (`Host::make_world`) or open one saved
(`Host::open_world`); it makes the one run again from its start when
its generation is retuned (`Host::reset`). It ticks at the pace asked,
or flat out, until the client is gone. As a world runs it keeps a census -- the flock and
the grass every 1,000 ticks, the seconds and the pace held -- in
`transient_data/measurements/census.csv`, written as it goes, so a run
closed at any time leaves what it came to. The ground no frame
brings a client asks of it too, on another thread ("Terrain asked of
the host").

## Terrain asked of the host

A frame brings what is hot. What a client draws past that -- the
heights about a superchunk, which its shadows and its coast are
worked out from, the whole of the ground for a map, a cell's height
written on it -- it asks of the host too (`Host::terrain`,
`src/host/terrain.rs`), and works none of it out itself: the server
alone says what the world is, and a client holds no generator. The
asks are answered on a thread of the host's own, so no tick waits for
a map, and each call waits for its answer on the thread that made it
-- a client asks what takes long from a thread that may wait. The
host's thread tells the terrain's of each world it runs before it
sends a frame of it, and an ask names its world (`Frame::world`): one
of a world run no more is answered with nothing.

What is answered is the terrain as the world is generated, a cell the
same whoever works it out. Not yet: once anything changes the ground
of a made superchunk, these answers must come from the world where it
has one, and only past it from how it is generated; the asks are
where that will be done, and no client changes for it.

## Layout

| folder | what is in it |
|---|---|
| `src/lib.rs` | start, save, load |
| `src/world_start.rs` | what a new world starts from: its size, its seed drawn with land, the sliders' numbers read |
| `src/halos.rs` | the hot entity, and the world lent to the simulation's halos |
| `src/world_hash.rs` | a world's hash, part by part: what `check` prints |
| `src/rules.rs` | the table of the rules, a tick's counts by rule, the rules chosen by name |
| `src/tick.rs` | the tick of every rule in the table, then the halos moved |
| `src/world_access.rs` | a world from outside the server: its parts read, changed only whole, and what must hold between them |
| `src/host/` | a world run on a thread of its own for a window: `mod` the host's calls and the census, `host_thread` the thread itself, `frame` what it answers, `terrain` the terrain a client asks of it, answered on a thread of its own |
| `src/diagnostics/` | grass, and grass and sheep, ticked flat out and measured; the diagnostics tools |
| `src/transient_data.rs` | where runs leave what they make, out of git |
| `tests/` | the halos follow their hot entities; a world of a size is hot within it only; a superchunk warming takes nothing until due; one cooling stays hot until due; a superchunk gone cold comes back as it was; a world loaded goes on as the one saved; a world hashes the same however it is ticked; a world paged to disk is the world kept in memory; the files; refusals |
| `docs/` | this, and the reference, function by function |
