# Coordinates

Where things are in Civil Egregore's world: superchunks, chunks and cells.
Every crate that places anything uses them. The decisions behind them
are in `../../docs/Civil Egregore.md`, "The world".

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
| `tests/` | the conversions and steps, judged |
| `docs/` | this, and the reference, function by function |

It has no diagnostics or transient data: nothing in it is measured on
its own.
