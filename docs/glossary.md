# TileSim's glossary

Every word the code and the docs use for a thing of TileSim's own: what
it means, and what it relates to. One word is one thing, everywhere --
in type names, function names, variables, comments and docs. A word
listed here is never used for anything else, and a thing listed here is
never called anything else; the "not" column names the words a thing
is not called, so they stand out where they slip in.

Ordinary English and Rust words (`len`, `new`, `iter`, "count") mean
what they always do and are not listed.

## The world

| word | means | relates to | not |
|---|---|---|---|
| **world** | everything simulated: every superchunk, its layers and its entities, and the tick it is at | made from a **seed**; saved as a **save** | map |
| **seed** | the number a world is made from: every height, every superchunk's random numbers and every first flock follow from it | world, random stream | |
| **seed rotation** | tests and tools taking their seed from one file for the whole workspace, `transient_data/seed`, rolled to a fresh one every 5 counted runs (`utilities::seed`) | seed | |
| **cell** | the smallest unit of the world: one bit of each layer, one height, at most one entity | chunk, superchunk; found by its **cell index** | tile, pixel, square |
| **chunk** | 256x256 cells: what a layer's **bitmap** covers | superchunk, layer, bucket | |
| **superchunk** | 4x4 chunks, 1024x1024 cells: the unit the world is held, ticked, saved and loaded in, and the reach of the speed of light. One word, in types too (`Superchunk`) | chunk, turn, image | super chunk, `SuperChunk` |
| **Morton index** | a number made by interleaving `x`'s and `y`'s bits, `x` in the even ones: cells, chunks and superchunks are numbered and stored in this order. The default way to say where anything is | Morton order | `morton` alone |
| **superchunk index** | a superchunk's Morton index in the world, 44 bits: what identifies a superchunk (`SuperchunkIndex`) | chunk index | position, `morton` alone |
| **chunk index** | a chunk's Morton index in the world, 48 bits: its superchunk index, then its place (`ChunkIndex`) | superchunk index, cell index | position |
| **cell index** | a cell's Morton index in the world, 64 bits: its chunk index, then its place (`CellIndex`) | chunk index, cartesian | address |
| **place** | a Morton index inside the thing around it, as a `usize`: a chunk's in its superchunk (0 to 15), a cell's in its chunk (0 to 65,535), or a cell's in its superchunk (20 bits, `CellIndex::place_in_superchunk`) | chunk index, cell index | slot, offset, position |
| **cartesian** | `x` and `y`, counted from the world's or a superchunk's top left; only for geometry and drawing, and always named so: `CellCartesian`, `from_cartesian`, `cartesian()`, `place_from_cartesian` | Morton index | world cell, coordinates (alone) |
| **Morton order** | the order of Morton indices: work is done and written in it, so memory is read forwards | sampling, wake | |
| **height** | a cell's height, 16 bits, 0 to 65,535, from the seed alone (`worldgen::height`) | wall, height map, floor, vertex | elevation |
| **dirt** | a cell with nothing on it: no layer of its own | grass | ground |
| **water** | how deep water stands over a cell's ground, 0 none: kept in the superchunk's image (`SuperchunkImage::depth`), a map only for the chunks with water -- a byte a cell, 16 bits where it is deeper than 255 (`ChunkMaps`); and the cells under any, a bitplane the rules read (`worldgen::WET`) | height map, ocean | level |
| **ocean** | the water at one height all over the world (`Shape::ocean`): the land under it is its floor | water, island, vertex | sea |
| **island** | land about land vertices that are joined, the ocean about it | ocean, vertex | continent |
| **tree** | a cell set in the layer `TREE`, with a **stage** | stage, patches | |
| **stage** | how old a tree is, 0 to 15: a number kept over four bitplanes (`TREE_STAGE`), a bit each | tree | age, growth |
| **wide plane** | a bitplane of 2, 4, 8 or 16 bits a cell, a cell's number held together where it is hot and read or written whole (`Wide<W>`, `Turn::value`); cold, a layer a bit | width, value, level | multi-bit plane |
| **width** | the bits a cell of a plane, in the plane's type: `Bits2`, `Bits4`, `Bits8`, `Bits16` | wide plane | |
| **value** | the number a cell of a wide plane holds | wide plane | |
| **patches** | how grass and trees lie when a superchunk is generated: noise under a threshold, not cells scattered (`worldgen::patches`) | generation | pasture |
| **wall** | a bar between two cells across or down more than one apart in height; kept by the upper or left cell, in the layers `WALL_EAST` and `WALL_SOUTH`. A diagonal step has no wall of its own: it is open only when both ways round it are | step, terrain | cliff (the renderer's drawing of walls) |
| **terrain** | a superchunk's heights and walls | height, wall | |

## Layers and bitmaps

| word | means | relates to | not |
|---|---|---|---|
| **layer type** | what a layer holds -- dirt, grass, a wall -- as a `u64` from the one namespace every type in TileSim is drawn from (`LayerType`) | entity type, attribute type | |
| **layer** | one layer type over one chunk: a bitmap of which cells hold it | bitmap, bitplane | plane |
| **bitplane** | one layer type over the whole world: every chunk's layer of that type | layer | |
| **bitmap** | 256x256 bits in Morton order, a word a 8x8 tile (`bitmap::Bitmap`, `CellWords`) | layer, tile | |
| **word** | 64 bits of a bitmap: an 8x8 tile of cells in Morton order | tile, window | |
| **word tile** | the 8x8 tile of cells one word holds; a word tile's index in its chunk is its word's index in the bitmap | word, window | |
| **tile** | an aligned square of cells whose side is a power of two. In a bitmap, an 8x8 tile is one word, a **word tile**; Tessera's tiles are its tree's squares, by **level**; the sampler counts the cells of 32x32 tiles; the far search looks over tiles by **scale** | window, word, level, scale | block, square |
| **scale** | a tile's side as a power of two, 0 one cell: the far search's tiles | tile | level (Tessera's, the other way round) |
| **window** | up to 8x8 cells at any cell, read out of the up to four words they overlap into one `u64`, row by row: bit `8y + x`. What rules read cells by (`Reader::window`) | around, area, tile | tile |
| **around** | the 3x3 cells about a cell, nine bits, row by row: bit `3y + x`, the cell itself bit 4 (`Around`) | window, area | neighbourhood |
| **area** | the 16x16 cells about a cell, a `u16` row each (`Area`, `Rows`): what an entity sees at once, and what paths are found over | around, window, pathfinding | |
| **mask** | bits standing for cells, any shape: a window, an around, an area's rows | | |
| **hot** | a layer decoded in the bitmap arena, read and written; a superchunk is hot when its layers are -- when a halo covers it and it is done warming, or while it is cooling. The simulation ticks the hot superchunks only | cold, arena, halo | held, loaded |
| **cold** | a layer kept encoded in chunk storage, not readable cell by cell; a superchunk no halo covers, its layers so and its entities kept as its cold state | hot, image, cold state | |
| **halo** | the 3x3 superchunks about an entity that keeps one -- its own and the eight beside it -- kept hot (`world::halos`) | halo keeper, hot | halo chunk |
| **halo keeper** | an entity of a kind that keeps a halo (`HALO_KEEPERS`): people, to come; the sheep, for now | halo | important entity |
| **origin** | the superchunk the world starts from, in its middle (`WORLD_MIDDLE`): a world is generated as it and its halo | superchunk | centre |
| **cold state** | a cold superchunk's entities and random numbers, kept as a save keeps them (`World::cold`) | cold, save | |
| **lingering** | a superchunk gone cold, done cooling -- to the simulation, cold like any other -- whose bitmaps the arena keeps as they were, until chunk storage holds its changes -- made hot again as it is if a halo reaches it before then (`BitmapArena::make_cold_superchunk`) | cold, warming, write back | |
| **cooling** | a hot superchunk no halo reaches any more: hot still to the simulation, it goes cold at the tick it is due, `COOL_TICKS` on -- or stays hot if a halo reaches it again before; so a keeper stepping to and fro over an edge makes nothing flicker | hot, halo, warming, lingering | unloading |
| **warming** | a superchunk a halo reached, not hot yet: made ready by a job, it turns hot at the tick it is due, `WARM_TICKS` on, whether a halo still reaches it or not -- a warming is never given up; until then, to the simulation, it is cold like any other | hot, halo, background, cooling, lingering | loading, pending |
| **job** | a piece of work for the dispatcher's threads. One **run** is done on all at once, a part each: a tick's phase. One **queued** is done by whichever is free, off the tick: chunk storage's (`chunk_storage::jobs`) -- encoding the write-backs of superchunks gone cold, rewriting images with the changes flushed, generating and decoding superchunks warming | dispatcher, ticket, part | background, task |
| **hot file** | a save's file naming its hot superchunks, and its cooling and warming ones with their due ticks (`HotSuperchunks`): made hot before a loaded world ticks | save, hot, cooling, warming | |
| **shared image** | an image in the cold pool held behind a reference count, so a thread reads it as it was while the pool changes (`ChunkStorage::shared_image`) | image, jond | |
| **dirty** | a hot layer changed since it was decoded: it must be written back before it is evicted | write back | |

## Storage

| word | means | relates to | not |
|---|---|---|---|
| **chunk storage** | the cold side: the cold pool and the writeback ring (`ChunkStorage`) | image, ring | |
| **cold pool** | chunk storage's superchunk images, one per superchunk stored. Never "pool" alone | image, block pool | |
| **image** | one superchunk as stored, one run of words, in memory as on disk: its height map and every chunk's encoded layers (`SuperchunkImage`) | chunk storage, save | |
| **height map** | a superchunk's heights: a floor a chunk, a byte a cell over it, and a tall chunk's 16 bits a cell (`HeightMap`) | height, floor, tall chunk | |
| **map** | the renderer's picture of the world from farther than its cells are drawn from: each pixel the cell in its middle as generated, nothing of the simulation read (`renderer/src/map.rs`) | detail, lab | overview, minimap |
| **mesh** | the land's shape: vertices joined by lines into triangles (`worldgen::mesh`) | vertex, line | polygons, Voronoi |
| **vertex** | a point of the mesh, one to each square of a grid: ocean, or land at a height of its own | mesh, line | site, node |
| **line** (the mesh's) | what joins two vertices and says how their heights are blended | blend, sigmoidness | edge, border |
| **blend** | the share of a line's length, about its middle, the change from one end's height to the other's is spread over | line | ramp width, edge |
| **sigmoidness** | the shape of the change across a blend: 1 an even slope, more a step between two levels | line, blend | hardness, sharpness |
| **weight** (a vertex's) | how much a vertex holds the land about it against the meshes finer than its own | vertex, finer mesh | roughness |
| **floor** | a chunk's lowest height, 16 bits: what its cells' bytes are counted from | height map | chunk height |
| **tall chunk** | a chunk whose heights span more than 255: kept whole, 16 bits a cell, after the bytes | height map, floor | |
| **layer codec** | encodes a layer for an image, and decodes it back, by Tessera (`LayerCodec`) | Tessera | |
| **encoded layer** | a layer's bitmap as Tessera encodes it: what images and the writeback ring hold | layer codec, image | bitmap (a bitmap is decoded) |
| **layer table** | a chunk's list in an image of its layers' types and where their encoded layers start | image | bitmap table |
| **writeback ring** | where hot layers written back wait until their superchunk's image is rewritten (`WritebackRing`) | write back, flush | |
| **write back** | encode a dirty layer into the writeback ring | dirty, flush | |
| **flush** | rewrite a superchunk's image with what waits for it in the ring | write back, image | |
| **evict** | drop a hot layer from the arena; a dirty one must be written back first | hot | |
| **bitmap arena** | the hot side: every hot layer, in allocations a superchunk and a layer type each (`BitmapArena`) | allocation, bucket, directory | |
| **allocation** | one layer type over one superchunk in the arena: one block of the block pool, a bucket a chunk | block, bucket | |
| **bucket** | one chunk's hot layer: its words in its allocation, and its counts | layer, allocation | |
| **directory** | the arena's list of hot superchunks, sorted by superchunk index, each with its allocations by layer type | lookup | |
| **lookup** | finding a layer in the directory; a reader remembers the last superchunk it found | reader, directory | |
| **block** | an equal-size piece of memory from the allocator's block pool, owned by whoever holds it (`allocator::Block`) | block pool, allocation | (for anything that is not memory) |
| **block pool** | the allocator's blocks, made, handed out, taken back (`BlockPool`). Never "pool" alone | block | |
| **save** | a world on disk: a folder of a world file and every superchunk's image and state, named by superchunk index | image, world | directory (the arena's list) |
| **folder** | a folder of the file system: a save, or where measurements are kept | save | directory |

## The tick

| word | means | relates to | not |
|---|---|---|---|
| **tick** | one step of the simulation: every hot superchunk's turn, then every write and instruction applied | phase, turn | step (an entity's move), frame |
| **phase** | half a tick: the **first** computes -- rules read the world as the tick found it and queue writes and instructions; the **second** applies them | tick | |
| **turn** | one superchunk's part of the first phase: what a rule is given (`Turn`) | rule, superchunk | |
| **rule** | what decides what changes: a Monte Carlo rule of the cells (`mc_rules`), or an entity's rule (`entity_rules`) | turn, sampling | |
| **speed of light** | 1,024 cells a tick: nothing reaches past the superchunks next to its own; an entity is at most 256x256 cells | outbox, reach | |
| **reach** | how far an entity's rule reads and acts: up to the speed of light | speed of light | |
| **outbox** | a superchunk's queues of writes and instructions, a slot each for itself and its eight neighbours | write, instruction, slot | |
| **slot** | one entry of a fixed array kept for a purpose: an outbox's, a neighbour each; the wheel's, a tick each; a hash table's, by a hash (`utilities::hash::slot`) | outbox, wheel, lookup | place |
| **write** | a change to cells, queued in the first phase and applied in the second: an operation on a shape, at a cell (`Write`) | outbox, apply | command |
| **apply** | carry out a write or an instruction, in the second phase | write, instruction | carry out, execute |
| **missed** | a write landing where no layer is hot: counted, and lost | write, lost | |
| **dispatcher** | the threads, started once and kept (`utilities::dispatcher::Dispatcher`): each runs a part of a job run, or a job queued; one set of them a world, the tick's and chunk storage's alike | part, job | thread pool, background |
| **part** | one thread's share of a job run: in a tick's phase, the superchunks it claims | dispatcher, job | |
| **random stream** | a superchunk's own random numbers, from the seed and its superchunk index, kept tick to tick and by a save | seed | |

## Sampling (Monte Carlo)

| word | means | relates to | not |
|---|---|---|---|
| **sampling** | choosing each set cell of a layer with one probability, independently, handed out in Morton order, by skipping over cells by their counts | sample, count | |
| **sample** | a cell chosen by sampling. Only that | sampling | (memory readings, test bitmaps) |
| **count** | how many cells a bucket, or a count tile of it, holds | sampling | |
| **count tile** | a 32x32 tile of a bucket whose set cells are counted, for sampling to skip over | count, tile | block |

## Entities

| word | means | relates to | not |
|---|---|---|---|
| **entity** | a thing on the cells -- a sheep -- on one cell, never two on one: its header and its attributes | header, attribute, bucket | record, object, agent |
| **header** | an entity's fixed part: its ID, type, cell and wake tick (`Header`) | entity | |
| **entity ID** | an entity's 64-bit number, drawn from its superchunk's random stream (`EntityId`) | entity | |
| **entity type** | what an entity is -- a sheep -- from the one type namespace (`EntityType`) | layer type | kind (only as the field holding it) |
| **attribute** | a typed value an entity has: added and removed as it goes (`Attribute`, `AttributeType`) | entity | property, component |
| **entity store** | where entities are kept: a bucket a chunk, a wheel a superchunk (`simulation::entity_store`, `Entities`) | bucket, wheel | |
| **entity bucket** | one chunk's entities, sorted by cell then ID, their attributes beside them | entity store | (the arena's bucket is a layer's) |
| **wake** | an entity's turn to act: at its wake tick, filed in its superchunk's wheel | wheel, sleep | |
| **sleep** | an entity waiting, doing nothing and costing nothing, until it wakes | wake | |
| **wheel** | a superchunk's timer wheel: a slot a tick for the next 1,024, the wakes due in each (`Wheel`); it **passes** a tick once the tick is run | wake | turn |
| **instruction** | one small thing a rule asks or does on a turn. The simulation's own are the changes to an entity, queued in the first phase and applied in the second: put, move, edit, remove (`Instruction`, `Instructions`); the `instructions` crate holds those built on them and on the crates beside the simulation -- a step found, a cell sought (`instructions::walking`) -- which rules are put together from | outbox, apply, rule | command, change, action |
| **put** | the instruction placing an entity whole, header and attributes | instruction | spawn (making one new, by a put) |
| **move** | the instruction stepping an entity to a cell, or setting its next wake, attributes as they are | step, instruction | |
| **edit** | the instruction setting or removing one attribute; and an entity's own attributes being changed in its rule (`EntityEdit`) | attribute | |
| **step** | an entity moving to one of its eight neighbours | move, wall | |
| **crossing** | an entity moving to another superchunk: put there, and removed here once the tick's instructions are applied, if it was (`Simulation::settle_crossings`, each superchunk its own leavers) | step | |
| **lost** | an entity put where no superchunk is hot | missed | |
| **occupied** | a cell an entity stands on | entity | taken |
| **flock** | sheep made together on a superchunk | sheep | |
| **seek** | find the step towards the nearest cell a layer holds, in the area, else over tiles by scale, as far as reach | area, scale | |

## Pathfinding

| word | means | relates to | not |
|---|---|---|---|
| **path** | the shortest way between two cells of an area: its first step and its length (`Path`) | area, step | route |
| **passable** | the cells of an area that may be walked on | area | |
| **wave** | the cells a step, two, ... from a start, spread a step at a time (`Wave`) | path | |
| **A\*** | the search for one cell's path (`a_star`) | path | |
| **bar** | what a wall does to a step (`Walls::bars_step`) | wall | block |

## Tessera

| word | means | relates to | not |
|---|---|---|---|
| **Tessera** | the lossless encoding of a bitmap: a project of its own (`tessera/`) | layer codec | |
| **stream** | the bits Tessera writes (`BitStream`) | | |
| **floor** | Tessera's 4x4 tiles, the finest its tree holds a node at; a **floor tile** is one of them | tile, last pass | block |
| **floor plan** | the floor tiles the tree leaves to the last pass: each a copy covers, with its source, and each residual one (`FloorPlan`) | floor, last pass | block plan |
| **tree** | Tessera's quadtree of the bitmap: tiles, complex tiles and copies (`Tree`) | node, tile | |
| **node** | one tile's entry in the tree (`Node`) | tree | |
| **corpus** | the bitmaps Tessera is tested and measured on, from its generators | generator | sample |
| **worst** | the bitmap an adversarial search has found Tessera does worst on, so far | adversarial | record |

## Measuring

| word | means | relates to | not |
|---|---|---|---|
| **diagnostics** | code that gathers data and judges nothing (`src/diagnostics/`) | report, test | |
| **report** | a measurement's tables and notes, printed and kept as CSV (`Report`) | table | |
| **table** | rows under headings (`Table`); a **row** is one line of it, in CSV too | report | record |
| **divider** | a line across a table between its rows, for a total or a group (`Table::divider`) | table | rule (the simulation's) |
| **transient data** | a crate's `transient_data/`, out of git: what its runs leave behind (`TransientData`) | report | |
| **census** | a count of a population: the flock and grass over a run, or Tessera's nodes by kind | | |
| **reading** | the process's memory read once (`MemoryTrack::read`) | | sample |
| **tier** | a test's size: fine, fast or complete (`docs/testing_protocol.md`) | | |
