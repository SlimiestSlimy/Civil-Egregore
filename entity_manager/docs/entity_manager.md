# The entity manager

What stands on the cells -- sheep, people, buildings -- kept beside the
bitplane manager's cells (`../../bitplane_manager/`) and ticked with
them by the simulation (`../../simulation/`). How a tick uses it, and
what a rule is given: `../../simulation/docs/simulation.md`,
"Entities". Function by function: `reference.md`.

## An entity

A header and attributes. The header is a type, the cell it stands on,
and the tick it next wakes at. Attributes are typed
blocks of 16 bytes, added and removed at run time ("Attributes, a block
each"). Types are the type registry's (`../../type_registry/`).

An entity has no ID. No two stand on one cell, so the cell names it:
its origin cell, the one in its header. It is found by that cell --
its cell's chunk's bucket, the cell's place there -- never a search
past its chunk. Within a tick the name is the cell the tick found it
on, wherever it moves meanwhile ("Instructions", An entity's name in a
tick); from a tick to the next nothing names it but where it stands,
and what must know one again keeps what it needs in an attribute -- a
neighbour's way, say, as a sheep its lamb's. A wake or an instruction
naming a cell nobody stands on any more is passed over.

## Attributes, a block each

An attribute is one to sixteen **blocks** of 16 bytes -- two words,
aligned as such (`AttributeBlock`, `BLOCK_WORDS`,
`ATTRIBUTE_BLOCKS_MOST`): 256 bytes at the most, and what is larger
is to be kept elsewhere, the attribute saying where. The first word
of an attribute's first block is its type, the one u64 ID every type
in a world has. The rest is its data, so that what belongs together
is one attribute -- where a sheep roams to and until when, not the
two packed into a word. Blocks were 64 bytes, a cache line each,
until an attribute came to be written a member at a time: a number
then took 64 bytes to keep, and takes 16.

**Its layout is its type's, and fixed.** A type of attribute has a
layout: what the data's words hold, member by member, each where
its bytes lie (`Member`: its first byte, counted from the
attribute's, and how many it is). An instruction writes a member by
those two numbers and knows nothing else of the layout. The type registry
says it, a row an attribute, and the layout is a type in the code, so
an attribute is read and written as its fields and never as loose
words (`../../type_registry/docs/type_registry.md`, "Layouts"). Every
attribute of one type is laid out alike.

**More than one block.** A layout that needs more than 8 bytes takes
two blocks, or more: the blocks after the first are all data, two
words each, straight after the first block's one -- so the data is
one run of words, 1, 3, 5 and so on, 31 at the most. How many blocks an attribute takes is
said by its first word -- the type's last four bits, the blocks less
one -- so nothing is looked up to step over an attribute
(`AttributeBlock::blocks`). The types are one namespace, everything's:
an ID of another supertype, or whose number is 0, is **not an attribute's**, and a block that
begins with one is no attribute (`sorted` says so of a run of blocks,
and a saved entity with one is not read).

**Variable size.** Most attributes are of a fixed size, set as the
type is registered and said in its ID's last four bits. One whose
size varies -- a short text, a short list -- is registered with those
bits 0, and each attribute of it keeps its own **attribute length**
there: how many blocks it is, less one, 1 to 16. So two attributes
of one such type have first words that differ in those bits, and a
type is told from another by all the rest
(`AttributeType::identity`): what an entity's attributes are sorted
and found by. Its data starts at the second word, as any
attribute's. None is registered yet, and no layout is written for one:
it is read and written as its blocks (`EntityRef::attribute_blocks`,
`Instructions::set_attribute_blocks`). The
walk over an entity's attributes honours an attribute length all the same,
and a test makes one to say so
(`../tests/fine.rs`).

**An entity's attributes** are a run of blocks in its bucket's list,
sorted by type, each type once. Finding one is a walk from the first
(`find_attribute`): read the type, and either it is the one, or step
over as many blocks as its first word says. An
entity has few attributes, and each step reads the one word. An
attribute length that reaches past the entity's blocks is cut to them, so a walk
never leaves them.

**Read and written as fields.** `attribute(blocks, attribute)` reads
one as its layout, `set_attribute` and `push_attribute` write one, and
`AttributeBlock::holding(attribute, value)` makes the one block of a
layout that takes one -- a layout of more does not build there, and
`WholeAttribute::holding` makes any, on the stack. A layout of one
block is read from its block where it lies; one of more is gathered
into a run of words first, on the stack too. A member alone is read
and written where it lies (`attribute_bytes`,
`write_attribute_bytes`).

**What it costs.** A sheep's one attribute is four blocks, 64 bytes
beside its 24-byte header, and not aligned to a cache line: it is
asked of memory ahead a line and the next. Blocks are copied whole when an entity is put, crosses or is
saved; an edit of one attribute in place writes its block. The tick
reference (`../../docs/testing_protocol.md`), a word an attribute ->
a block: 58.72 M instructions -> 59.33 M, the blocks copied where a
sheep that ate is put whole.

**What changed with it.** An edit instruction carries the attribute's
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

**Putting.** Two things, never one for the other. An entity is made
(`Bucket::make`) on a cell nobody stands on, with its attributes, or
refused. And the entity standing on a cell is given a header
(`Bucket::put`) -- another cell, another wake -- and, if they changed,
its attributes; with none given it keeps those it has, and nobody
standing there it is passed over, never made. Its cell the same as it
stood on: changed where it stands. Another cell: moved there if no
entity stands on it, else left
where it was and changed all the same. What came of it is a `Put`
(one put where it stood because its cell was another's that tick is
said to have stayed too, `Put::stayed`).

**Attributes.** Each entity's attributes are a run of the bucket's
list of blocks. They are rewritten in place when they stay as many
blocks. When that changes -- an attribute added or removed, or one of
a varying size made longer or shorter, which is rare -- the new run
goes at the end of the list and the old run becomes garbage, swept out
once there is as much garbage as blocks in use (`attribute_counts`:
the two).

## The timer wheel

Which entities wake at which tick, so a tick's work is the entities
waking then, not every entity.

A slot a tick for the next `WHEEL_TICKS` (1,024) ticks, the tick's
number modulo that its slot. A wake further off waits in a list beside
the slots, filed into its slot once its tick is in reach; the list is
looked over every half of `WHEEL_TICKS`. A wake names its entity by
its cell -- which finds it in its chunk's bucket -- and is only good if
whoever stands there wakes at that tick: of one that moved away or
died the cell is empty, or another's, not due then, and is passed
over. Nothing is ever taken out of the wheel but the slot just passed.

A tick's slot is sorted by cell -- Morton order -- each cell once,
before the tick runs (`Wheel::sort`): a cell filed twice for one tick,
by an entity that left it and by one that came to stand there after,
wakes whoever stands on it once. So the entities wake in Morton order: their
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
| put | an entity made on a cell nobody stands on: header and attributes | its attributes |
| move | an entity moved to another cell, or left where it is, to wake at another tick; its attributes as they are | nothing |
| edit | one attribute of an entity set, or removed: by the entity itself or by another | the one attribute's blocks, or none |
| remove | an entity removed | nothing |

**In a tick the queue is the simulation's**, not this one: its
instructions are 24 bytes each, compares and writes in one stream
(`../../simulation/docs/simulation.md`, "Instructions"), and each
that is of an entity is applied by one function of the store, the
entity named by the cell the tick found it on -- made (`make_named`),
moved with nothing else of it changed (`move_named`), its wake
written to the wheel (`wake_named`), a member of an attribute
written in place (`write_attribute_bytes_named`), an attribute given
or taken (`edit_named`), removed (`remove_named`); what a compare
asks is `get_named` and `free_for_a_new_entity`. The queue here
(`Instructions`) is what is queued between ticks -- a world set up, a
save read back -- and what follows holds for both.

Whatever puts an entity on a cell checks it as it is applied: a cell
holds one entity, ever. An instruction carries all it needs: an entity
moving to a neighbouring superchunk goes as a whole copy, made in the
first phase from the world as the tick found it, so the second never
reads another superchunk's entities while that one changes them. A put
and a crossing are queued by the one private `push`, a crossing naming
the cell it left.

Applied (`Instructions::apply`; `apply_some` for a part of the queue;
`Instruction::lands` the cell that says which superchunk), each goes to the superchunk its cell
is in, in the order queued, every wake filed no earlier than the tick
given. A put in a superchunk not there is lost; one of an entity no
longer where it stood is passed over; a new entity on a cell another
stands on is refused, and one moving to it stays where it stood,
changed all the same, and wakes there. None of these is unseen: what
was lost, refused, left where it stood and passed over is each counted
in what applying did (`InstructionsApplied`).

**An entity's name in a tick.** No two entities stand on one cell,
so within a tick the cell the tick found an entity on names it -- and
goes on naming it until the tick is over, wherever it moves meanwhile
(`store/named_in_a_tick.rs`). A superchunk keeps, for the tick alone,
where each entity that left its cell now stands, or that it was
removed (`now_on`); every instruction names its entity by the cell it
was seen on and finds it through that (`get_named`, `put_named`,
`edit_named`, `remove_named`). So instructions are one stream, each
applied as it is come to, none before another for its kind: an
attribute written to an entity that moved a moment before lands on it
where it now stands, and its removal finds it there. What follows
from the name:

- Where an entity stands is written by compare-and-write, as its
  attributes are: a move of one that already left the cell it was
  seen on is passed over, so of two moving one entity the first
  applied does.
- A cell an entity stood on as the tick began is no other's that tick
  (`named_or_taken`): one stepping to it stays, one new is refused,
  though its entity left or was removed. Its being free would hang on
  when the one leaving is taken off it, which over a border is after
  the tick.
- The tick over, every entity is named by the cell it stands on
  (`names_anew`); applying a queue between ticks ends so too.

The cell is all that is asked: there is no ID beside it. An entity
made in a tick is named in it by the cell it was made on, which no
rule saw it on: nothing is queued for it.

A put makes (`make_named`, and `make` of the store and of the bucket):
never over an entity, which is written an attribute at a time and
moved, not put whole.

**Entities in the collision plane.** Whoever applies a queue may lend
the collision plane of the superchunk it lands in (`CollisionCells`:
`held`, `hold`, `free`; `()` is none). Then no entity is put on or
moved to a cell it holds -- whatever holds it, an entity or not -- the
cell one comes to is set in it, and the cells left or removed from
(`vacated`) are cleared when the tick is over (`names_anew`), as a
leaver's is when its crossing is settled. The buckets' own check
stays: a cell holds one entity whether or not a plane is kept.

**Crossing a border.** An entity crossing into a neighbouring
superchunk is put there while it still stands on the cell it left: for
the rest of the tick it stands on both, so that, its new cell taken, it
stays where it stood -- and where it left, under the name it has this
tick, is where every write to it lands. Each superchunk notes who
arrived (`take_arrived`), and each then removes from the cells they
left those that left it (`settle_leavers`); one that ended the tick
there with other attributes than it was put with (an `Arrival` carries
their sum, `blocks_sum`) has them sent after it (`Settled`), and the
superchunk it was put in gives them to it (`settle_arrivals`). So an
entity crosses with all that was written to it, exactly as one moving
within a superchunk keeps it. One removed where it left meanwhile is
turned back: the superchunk it was put in takes it back. Once every superchunk has settled, every entity
stands on one cell.

## The store

A superchunk's entities are its buckets and its wheel
(`SuperchunkEntities`); the world's are every superchunk's, in the
bitmap arena's order, with the tick the world is at (`Entities`). In a
tick they are read across superchunks by an `EntityReader`, as cells
are by the bitplanes' reader. The API follows the bitplanes': outside a
tick, instructions are queued (`Entities::queue_put`, `queue_remove`)
and applied (`Entities::apply`), as writes to cells are; in a tick, a
superchunk's turn queues them. Queuing is the only way to change an
entity. While a queue is applied an entity is found by the cell it
stood on when the applying began, wherever it moved since
("Instructions", An entity's name in a tick); whether one stands on a
cell now is `SuperchunkEntities::occupied`.

**The cells entities stand on** (`EntityReader::occupied`): among up to
16x16 cells from a top left cell, a row a word, cell `(x, y)` at bit
`x` of row `y`. They are found from the buckets, which are sorted by
cell: the cells lie on up to nine word tiles, each a run of a bucket's
places, so what is read is the few entities there, not the cells. Where
no superchunk is read, no entity stands.

## Saved

A superchunk's state is words a save keeps
(`../../chunk_storage/docs/chunk_storage.md`, "On disk"): a first word
saying what it is (`FIRST_WORD`: `TSstate` and the format's number, 5);
whether it has random numbers, and their state; how many entities; then
each entity -- its type, cell, wake tick, how many blocks its
attributes are, and the blocks, two words each. An entity whose
blocks are not whole attributes sorted by type is not read.

## Layout

| folder | what is in it |
|---|---|
| `src/entity.rs` | an entity: its header and its attributes; and one being changed by its rule |
| `src/attributes.rs` | an attribute's blocks: the walk over an entity's, and reading and writing one as its layout |
| `src/bucket.rs` | a chunk's entities, sorted by cell, one a cell, their attributes beside them |
| `src/wheel.rs` | a superchunk's timer wheel |
| `src/store.rs`, `src/store/` | a superchunk's entities, every superchunk's, the reader across them, and an entity found by its name in a tick (`named_in_a_tick.rs`) |
| `src/instructions.rs` | put, move, edit, remove: queued for a superchunk and applied by it |
| `src/saved.rs` | a superchunk's state as a save's words |
| `src/diagnostics/` | what the entities hold: entities, attribute blocks in use and as garbage, wakes filed |
| `tests/fine.rs` | the fine tier: attributes of any length put, found, edited and saved; an entity named by the cell the tick found it on |
| `docs/` | this, and the reference, function by function |

It has no tests of its own: the entities are judged through the
simulation's (`../../simulation/tests/fine/`).
