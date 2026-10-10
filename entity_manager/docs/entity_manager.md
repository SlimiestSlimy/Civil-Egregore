# The entity manager

What stands on the cells -- sheep, people, buildings -- kept beside the
bitplane manager's cells (`../../bitplane_manager/`) and ticked with
them by the simulation (`../../simulation/`). How a tick uses it, and
what a rule is given: `../../simulation/docs/simulation.md`,
"Entities". Function by function: `reference.md`.

## An entity

A header and attributes. The header is a random 64-bit ID, a type, the
cell it stands on, and the tick it next wakes at. Attributes are typed
values, a word each, added and removed at run time. Types are the type
registry's (`../../type_registry/`).

An entity is found by its ID and its cell: its cell's chunk's bucket,
then its ID there -- never a search past its chunk. That keeps it found
however it moves, and a wake or an instruction naming one that has
moved on or died passes it over.

## Attributes, a block each

**Designed 2026-10-09 and not yet built**: until it is, an attribute is
a type and one word, as the rest of this doc tells. What is written
here is what the code is being changed to; this line goes when it is.

An attribute is one or more **blocks** of 64 bytes -- eight words, a
cache line, and aligned as one. The first word of an attribute's first
block is its type, the one u64 ID every type in a world has. The rest
is its data, 56 bytes in a block of its own: way more than the one
word it had, so that what belongs together is one attribute -- where a
sheep roams to and until when, not the two packed into a word.

**Its layout is its type's, and fixed.** A type of attribute has a
layout: what the data's words hold, field by field. The type registry
says it, a row an attribute, and the layout is a type in the code, so
an attribute is read and written as its fields and never as loose
words (`../../type_registry/docs/type_registry.md`, "Layouts"). Every
attribute of one type is laid out alike.

**More than one block.** A layout that needs more than 56 bytes takes
two blocks, or more: the blocks after the first are all data, eight
words each, straight after the first block's seven -- so the data is
one run of words, 7, 15, 23 and so on. How many blocks a type takes is
said by its ID -- the ID's top byte, the blocks less one, as a layer
type's top byte says how wide it is -- so nothing is looked up to step
over an attribute, and the IDs there are, whose top byte is 0, are of
one block.

**Variable size.** Most attributes are of a fixed size. One whose
size varies -- a short text, a short list -- has a top byte of all
ones, and then its first block's second word is its **block length**:
how many blocks it takes in all, one at least. Its data starts at the
third word. None is registered yet; the walk over an entity's
attributes honours a block length all the same, and a test makes one
to say so.

**An entity's attributes** are a run of blocks in its bucket's list,
sorted by type, each type once. Finding one is a walk from the first:
read the type, and either it is the one, or step over as many blocks
as the type -- or the block length -- says. An entity has few
attributes, and each step reads the one word.

**What it costs.** A block is four times the 16 bytes an attribute
took. A sheep has one to four: 64 to 256 bytes beside its 32-byte
header. Blocks are copied whole when an entity is put, crosses or is
saved; an edit of one attribute in place writes its block. The tick
reference is counted before and after.

**What changes with it.** An edit instruction carries the attribute's
blocks, or none to remove it, in the instructions' list beside those
the puts carry. A saved entity is its header's four words, how many
blocks it has, and the blocks. The bucket's list, its garbage and its
sweep count blocks where they counted attributes; an attribute put in
place of one as long is written where it was.

## A chunk's bucket

A superchunk holds its entities in a bucket a chunk: their headers,
sorted by cell in Morton order, and their attributes, kept together in
one list beside them.

**A cell holds at most one entity.** An entity put on a cell another
stands on is not put; one moving onto it stays where it was. The bucket
is where that rule is enforced, whatever a rule asks for.

**Finding one.** A cell's place in the chunk (16 bits) is searched for
in a list holding only the places, two bytes an entity, which stays in
the caches; the headers are not read until the place is found. The
chunk is searched in 16 tiles of 64x64 cells (`SEARCH_TILES`): a place
is first narrowed to its tile -- its top bits, those above
`PLACE_IN_SEARCH_TILE_BITS` -- whose places are a run of the sorted
list. Entities woken in Morton order are therefore found by walking
forwards through the bucket, the same way cells sampled in Morton order
are read forwards through a bitmap. An aligned 8x8 tile of cells
(`WORD_TILE_CELLS`, 64) is likewise a run of places, which is how the
cells entities stand on are read a word tile at a time.

**Stepping.** An entity stepping to another cell of the same chunk
shifts the entities between its old and its new cell by one.

**Putting** (`Bucket::put`). An entity is put with the cell it stood on
and, if they changed, its attributes; with none given it keeps those it
has, and is not made if it is not there. Its cell the same as it stood
on: changed where it stands, or added if it is new and the cell is
free. Another cell: moved there if no entity stands on it, else left
where it was and changed all the same. What came of it is a `Put`.

**Attributes.** Each entity's attributes are a run of the bucket's
attribute list. They are rewritten in place when their number stays the
same. When the number changes -- an attribute added or removed, which
is rare -- the new run goes at the end of the list and the old run
becomes garbage, swept out once there is as much garbage as attributes
in use (`attribute_counts`: the two).

## The timer wheel

Which entities wake at which tick, so a tick's work is the entities
waking then, not every entity.

A slot a tick for the next `WHEEL_TICKS` (1,024) ticks, the tick's
number modulo that its slot. A wake further off waits in a list beside
the slots, filed into its slot once its tick is in reach; the list is
looked over every half of `WHEEL_TICKS`. A wake names its entity by ID
and cell -- which finds it in its chunk's bucket -- and is only good if
the entity still wakes at that tick: one that moved away, died or was
woken for another tick is not found, or not due, and is passed over.
Nothing is ever taken out of the wheel but the slot just passed.

A tick's slot is sorted by cell -- Morton order -- then ID, before the
tick runs (`Wheel::sort`), so the entities wake in Morton order: their
buckets, the cells they read and the writes they queue all go forwards
through memory, as the cells' sampling does. The entities to wake soon
are asked of memory ahead, an entity 8 wakes before it is given
(`ENTITY_AHEAD`) and its attributes 4 before (`ATTRIBUTES_AHEAD`), the
attributes after the entity that says where they are.

## Instructions

Entities change as cells do: queued in a tick's first phase, applied in
its second by the superchunk they land in. There is an instruction for
each thing a rule does to an entity, so each carries, and costs, no
more than it changes (`Instruction`, the four):

| instruction | what it does | what it carries |
|---|---|---|
| put | an entity made, or made anew whole: header and attributes | its attributes |
| move | an entity moved to another cell, or left where it is, to wake at another tick; its attributes as they are | nothing |
| edit | one attribute of an entity set, or removed: by the entity itself or by another | the one value |
| remove | an entity removed | nothing |

Whatever puts an entity on a cell checks it as it is applied: a cell
holds one entity, ever. An instruction carries all it needs: an entity
moving to a neighbouring superchunk goes as a whole copy, made in the
first phase from the world as the tick found it, so the second never
reads another superchunk's entities while that one changes them. A put
and a crossing are queued by the one private `push`, a crossing naming
the cell it left.

Applied (`Instructions::apply`), each goes to the superchunk its cell
is in, in the order queued, every wake filed no earlier than the tick
given. A put in a superchunk not there is lost; one of an entity no
longer where it stood is passed over; a new entity on a cell another
stands on is refused, and one moving to it stays where it stood,
changed all the same, and wakes there.

**Crossing a border.** An entity crossing into a neighbouring
superchunk is put there while it still stands on the cell it left: for
the rest of the tick it stands on both, so that, its new cell taken, it
stays where it stood. Each superchunk notes who arrived
(`take_arrived`), and each then removes from the cells they left those
that left it (`settle_leavers`). Once every superchunk has settled its
leavers, every entity stands on one cell.

## The store

A superchunk's entities are its buckets and its wheel
(`SuperchunkEntities`); the world's are every superchunk's, in the
bitmap arena's order, with the tick the world is at (`Entities`). In a
tick they are read across superchunks by an `EntityReader`, as cells
are by the bitplanes' reader. The API follows the bitplanes': outside a
tick, instructions are queued (`Entities::queue_put`, `queue_remove`)
and applied (`Entities::apply`), as writes to cells are; in a tick, a
superchunk's turn queues them. Queuing is the only way to change an
entity.

**The cells entities stand on** (`EntityReader::occupied`): among up to
16x16 cells from a top left cell, a row a word, cell `(x, y)` at bit
`x` of row `y`. They are found from the buckets, which are sorted by
cell: the cells lie on up to nine word tiles, each a run of a bucket's
places, so what is read is the few entities there, not the cells. Where
no superchunk is read, no entity stands.

## Saved

A superchunk's state is words a save keeps
(`../../chunk_storage/docs/chunk_storage.md`, "On disk"): a first word
saying what it is (`FIRST_WORD`: `TSstate` and the format's number, 2);
whether it has random numbers, and their state; how many entities; then
each entity -- its ID, type, cell, wake tick, how many attributes, and
each attribute's type and value.

## Layout

| folder | what is in it |
|---|---|
| `src/entity.rs` | an entity: its header and its attributes; and one being changed by its rule |
| `src/bucket.rs` | a chunk's entities, sorted by cell, one a cell, their attributes beside them |
| `src/wheel.rs` | a superchunk's timer wheel |
| `src/store.rs`, `src/store/` | a superchunk's entities, every superchunk's, and the reader across them |
| `src/instructions.rs` | put, move, edit, remove: queued for a superchunk and applied by it |
| `src/saved.rs` | a superchunk's state as a save's words |
| `src/diagnostics/` | what the entities hold: entities, attributes in use and as garbage, wakes filed |
| `docs/` | this, and the reference, function by function |

It has no tests of its own: the entities are judged through the
simulation's (`../../simulation/tests/fine/`).
