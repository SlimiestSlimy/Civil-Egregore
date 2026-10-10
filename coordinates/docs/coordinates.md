# Coordinates

Where things are in Civil Egregore's world: superchunks, chunks and cells.
Every crate that places anything uses them. The decisions behind them
are in `../../docs/civil_egregore.md`, "The world".

Every place is a **Morton index** -- its coordinates' bits interleaved,
`x` in the even bits -- one type a size, each nested in the next:

| type | bits | made of |
|---|---|---|
| `SuperchunkIndex` | 44 | the superchunk's coordinates interleaved |
| `ChunkIndex` | 48 | its superchunk's index, then 4 for its **place** in the superchunk |
| `CellIndex` | 64 | its chunk's index, then 16 for its place in the chunk |

A **place** is a Morton index inside the thing it is in, a `usize`: a
chunk's among its superchunk's 16, a cell's among its chunk's 65,536
(its bit's index in the chunk's bitmap words), or a cell's among its
superchunk's 2^20 (`CellIndex::place_in_superchunk`, where its height
is). Each part is a bit field, so going from a cell to its chunk,
superchunk or place is a shift or a mask.

A neighbour is a step on the index itself (`CellIndex::offset`,
`SuperchunkIndex::offset`): one coordinate's bits are added apart --
the other's set to ones so a carry passes over them, or cleared so a
borrow does -- and a step past the world's edge is refused.

A superchunk's index is what identifies it: what the directory of hot
superchunks, the cold pool and a save's files are sorted and named by.
Neighbouring superchunks mostly get near indices, so what is near in
the world is mostly near in memory and on disk.

Every coordinate is a non-negative integer counted from the world's top
left corner: `x` grows to the right and `y` downwards, as in a bitmap.
The world is 2^22 superchunks a side (`WORLD_SIDE_SUPERCHUNKS`), as
many as leave a cell's `x` and `y` a `u32` each, and starts at the
superchunk in the middle of both (`WORLD_MIDDLE`), as far from every
edge as one can be. A world of a given number of superchunks is the
least square that holds them, laid out row by row from that middle
(`square_from_middle`).

Morton indices are what everything is stored and worked in. Cartesian
coordinates are kept for what they are cheaper at -- geometry, drawing
-- and whatever is cartesian says so: `CellCartesian`, a cell's `x` and
`y` in the world; `SuperchunkIndex::from_cartesian` and `cartesian`, a
superchunk's in superchunks; `place_from_cartesian` and
`cartesian_from_place`, a cell's from its superchunk's top left.

## Layout

| folder | what is in it |
|---|---|
| `src/lib.rs` | every coordinate type and conversion |
| `src/diagnostics/`, `src/transient_data.rs` | the folders every crate has; nothing is gathered or kept yet, nothing here being measured on its own |
| `tests/` | the conversions and steps, judged |
| `docs/` | this, and the reference, function by function |
