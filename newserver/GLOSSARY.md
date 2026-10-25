# newserver's glossary

Every word newserver uses for a thing of its own: what it means, and the
one thing in the code it names. One word is one thing, in type names,
function names, variables, comments and docs alike. Before a new name is
made, this list is read: a thing already here is reused, word and
structure both; a new thing gets one entry, never a second name for an
old one. The "not" column holds the words a thing is never called, so
they stand out where they slip in -- most are the old server's.

A thing of one side only is named with its side, `Hot` or `Cold` -- a
hot plane and a cold plane are different things, as are a hot superchunk
and a cold superchunk; a thing both sides use has no prefix.

Ordinary English and Rust words mean what they always do and are not
listed. "(to come)" marks an entry agreed but not written yet.

## The world

| word | means | in the code | not |
|---|---|---|---|
| **world** | everything one server process runs: its folder, its frozen types, its cold storage and its hot store | `World` (to come) | map, simulation handle |
| **cell** | the smallest unit of the world: one number in each plane, at most one entity | `CellIndex` (utilities) | tile, pixel, square, position |
| **superchunk** | 1024x1024 cells: what is hot or cold, ticked, saved and loaded whole | `SuperchunkIndex` (utilities) | turn, region |
| **chunk** | 256x256 cells, 16 to a superchunk: the grain planes are kept sparse by and the cold side encodes by -- nothing more | `ChunkIndex` (utilities) | bucket, allocation |
| **speed of light** | no write lands farther than 1024 cells from its author's origin cell | | reach |

## Planes

| word | means | in the code | not |
|---|---|---|---|
| **plane** | one plane type's numbers over cells, kept sparse: a superchunk keeps a plane only if it has a cell set, and a plane keeps a chunk only if it has a cell set | | layer, bitplane, bucket, allocation |
| **hot plane** | a hot superchunk's plane: the chunks it keeps, their numbers as they lie, and their count tiles | `HotPlane` | superchunk plane |
| **tile** | an aligned square of cells, 2^n a side -- one cell up to the whole world -- its corner a multiple of its side: in Morton order, one run of cells. A concept, not a type | | |
| **plane type** | what a plane is: its ID, width and tile size, as registered | `PlaneType` (protocol) | layer type, `LayerType` |
| **width** | bits a number of a plane takes: 1, 2, 4, 8, 16 or 32 | `PlaneWidth` (protocol) | depth, bits a cell |
| **tile size** | what a plane's numbers are of: a tile 1 to 256 cells a side, 1 for a plane of cells | `PlaneTileSize` (protocol) | tile scale |
| **count tile** | a 32x32 tile of a chunk, counting its cells that read a number not 0 -- a plane of tiles' tiles counted cell by cell -- so sampling passes over the empty ones | `HotPlane::tile_counts` | |
| **plane chunk** | one plane over one chunk: what a hot superchunk writes back, and what a cold superchunk encodes, a chunk at a time | `PlaneChunk` | bucket, bucket key, layer |
| **hot plane reader** | reads planes wherever their cells lie across the hot superchunks: a cell, a window, small tiles, an aligned tile, whether a tile holds anything | `HotPlaneReader` | plane reader, cell reader, lookup |
| **not hot** | what reading a cell of a superchunk not hot comes to; a plane or chunk a hot superchunk does not keep reads as all clear, not as not hot | `NotHot` | |
| **window** | up to 8x8 cells of a plane about a cell, row by row, read from one to four words | `Window` | area, around |
| **small tiles** | four 4x4 tiles, two by two, of a plane, in Morton order as they lie: a word of bits | `SmallTiles` | |
| **aligned tile** | a square of cells, 8 to 256 a side, whose corner is a multiple of its side: one run of words as it lies in its chunk | `HotPlaneReader::aligned_tile` | |

## The cold side

| word | means | in the code | not |
|---|---|---|---|
| **cold storage** | everything of the world not hot: the cold pool and the cold writeback ring | `ColdStorage` | chunk storage |
| **bitmap** | a bit a cell over a chunk, in Morton order: a plane a bit a cell holds one a plane chunk, a wider one one a bit of its width | `CellWords` (utilities) | layer |
| **bitmap codec** | encodes a bitmap into the words a cold superchunk keeps, and decodes them | `BitmapCodec` | layer codec |
| **cold superchunk** | a superchunk's planes encoded, a plane chunk a type a chunk, one run of words, in the cold pool and on disk alike | `ColdSuperchunk` | image, snapshot |
| **cold pool** | the cold superchunk of every superchunk ever made, in memory or on disk | `ColdStorage::cold_pool` | |
| **cold superchunk kept** | where the cold pool keeps a cold superchunk, and how it is handed to another thread: in memory, or in a folder to read it from | `ColdSuperchunkKept` | image stored, image kept |
| **cold writeback ring** | plane chunks written back, encoded, on their way into their cold superchunks | `ColdWritebackRing` | writeback ring |
| **flush** | a cold superchunk rewritten with its superchunk's entries from the ring | `Flush` | |
| **paging** | cold superchunks past the bytes kept in memory written to the world folder's `paging/` and read back when wanted: only inside a world, the folder emptied when paging starts and removed when the world stops | `cold_pool_paging` | |
| **world folder** | the world on disk: world.csv, types.csv, hot.csv, a superchunk's cold superchunk and state files, and `paging/` while it runs | `world_folder_files` | save folder |
| **off-tick job** | slow work on the dispatcher's threads, done by ticket: encoding, flushing, warming | `off_tick_jobs` | |

## The hot side

| word | means | in the code | not |
|---|---|---|---|
| **hot store** | every hot and cooled superchunk, one sorted list behind one handle | `HotStore` | arena, directory, entity store |
| **hot superchunk** | all a superchunk needs to be ticked: its planes, its entities, its names, its random stream, its lifecycle | `HotSuperchunk` | |
| **warming** | a superchunk built off the tick -- from its cold superchunk, from cooled, or by the world's generation -- and adopted at its due tick, 256 ticks after it began | (to come) | loading |
| **cooling** | a hot superchunk no halo wants, counting down to going cold | (to come) | lingering |
| **cooled** | a superchunk gone cold, kept as its last tick left it until written back and freed | (to come) | |
| **halo** | the superchunks kept hot about each entity of the type that keeps the world hot | (to come) | |

## The tick

| word | means | in the code | not |
|---|---|---|---|
| **tick** | one step of the world: the read phase, then the compare and write phase | `tick/` (to come) | turn |
| **read phase** | every hot superchunk's rules read the world as the tick found it and queue compares and writes; nothing changes | `read_phase` (to come) | phase one, computing |
| **compare and write phase** | every superchunk applies what is queued for it, in the one order, writing only its own cells | `compare_and_write_phase` (to come) | phase two, applying |
| **superchunk read handle** | what a rule is given for one superchunk in the read phase | `SuperchunkReadHandle` (to come) | turn, tick state |
| **the one order** | authors in the Morton order of their origin cells, the superchunk's own rule first: the same wherever the borders fall | | |
| **author** | whom what is queued is from: an entity woken or a cell sampled, named by its origin cell -- or the superchunk's own rule | | |
| **queue** | what a superchunk's authors queued for one superchunk -- itself or a neighbour -- in a tick | (to come) | outbox slot |
| **parked** | a superchunk waiting at a fence, its thread given back to take other work | (to come) | asleep, lying |

## Compare and write

| word | means | in the code | not |
|---|---|---|---|
| **compare** | what a rule saw, queued: if it no longer holds when come to, what is under it is skipped | (to come) | check, guard |
| **write** | a change queued, applied only if every compare over it holds | (to come) | naked write |
| **compare write skip set** | a rule's scope of compares and the writes under them; dropped, it places its skip tag | `CompareWriteSkipSet` (to come) | skip tag set, `SkipTagSet` |
| **skip tag** | where a failed compare skips to | (to come) | |
| **counted atomic** | a compare write skip set's verdict across superchunks: the compares that held, and a fail bit; always tied to a skip tag | (to come) | vote |
| **fence** | the instruction that waits until its counted atomic's count is reached or its fail bit is set | (to come) | barrier |

## Entities

| word | means | in the code | not |
|---|---|---|---|
| **entity** | a thing standing on one cell, of an entity type, with attributes and a wake tick | (to come) | agent, unit |
| **attribute** | data of an entity, of an attribute type, in blocks | (to come) | component, field |
| **wake** | the tick an entity next runs at; kept in its superchunk's wheel | (to come) | |
| **origin name** | an entity's name for a tick: the cell it stood on as the tick began, kept even after it crosses into another superchunk, refreshed as the compare and write phase's last step | (to come) | id, handle |
| **departed list** | a superchunk's origin names of the entities that left it this tick, each with the superchunk it went to | (to come) | left this tick |
