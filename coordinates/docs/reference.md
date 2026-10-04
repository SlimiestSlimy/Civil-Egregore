# Coordinates, function by function

The design is in `coordinates.md`.

## `lib.rs`

**Constants**: `CHUNK_SIDE` (256 cells), `SUPERCHUNK_SIDE` (4 chunks),
`CHUNKS_IN_SUPERCHUNK` (16), `SUPERCHUNK_SIDE_CELLS` (1024),
`CELLS_IN_CHUNK` (65,536), `WORLD_SIDE_SUPERCHUNKS` (2^22: a cell's
coordinates fit a `u32`), `WORLD_MIDDLE` (the superchunk the world
starts at), `NEIGHBOURS` (a cell's eight, as offsets).

**`square_side(count)`**, **`square_from_middle(count)`**: the side of
the square `count` superchunks make, and those superchunks, row by row
from `WORLD_MIDDLE`: how a world of that many is laid out.

**`SuperchunkIndex(u64)`**: **`from_cartesian(x, y)`** and
**`cartesian`**; **`top_left`**, its first cell; **`chunks`**, its 16
in Morton order; **`offset(dx, dy)`**, stepped on the index, refused
past the world's edge.

**`ChunkIndex(u64)`**: **`of(superchunk, place)`**, **`superchunk`**,
**`place`** (0 to 15), **`top_left`**.

**`CellIndex(u64)`**: **`of(chunk, place)`**, **`superchunk`**,
**`chunk`**, **`place`** (the low 16 bits: its bit in the chunk's
words), **`place_in_superchunk`** (the low 20), **`cartesian`**, and
`From<CellCartesian>`. **`offset(dx, dy)`**: the cell so far away, if
in the world.

**`CellCartesian`** `{x, y}`: a cell's cartesian coordinates.

**`place_from_cartesian(x, y)`**, **`cartesian_from_place(place)`**: a
cell's place in its superchunk from how far across and down from the
superchunk's top left it is, and back.

**`spread`**, **`gather`**, **`interleave`**: a `u32`'s bits to every
other bit and back, in five shift-and-mask steps each
(`SPREAD_STEPS`, `GATHER_STEPS`, from `alternating_runs`).

**`step`**: one coordinate's bits of a Morton index (its lane, `X_BITS`
or `Y_BITS`) added to or taken from with the distance spread out, the
other's bits filled with ones for a carry to pass, cleared for a
borrow; a result past the start is a step off the `u64`, refused. A
step of none spreads nothing, and one of a power of two -- to a
neighbour, the next word tile or chunk -- spreads to one bit, twice as
far up, with no spreading steps.
