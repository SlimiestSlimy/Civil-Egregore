# The type registry

Every type a world has is a `u64` from one namespace: a layer of its
cells, a wide plane, a type of entity, an attribute an entity carries,
a rule of the cells, a rule of an entity.
This crate is where they are all written down, once: one table,
`REGISTRY`, a row a type -- its name, its registration, its number and its width --
and the types the IDs are of (`LayerType`, `Wide`, `EntityType`,
`AttributeType`, `ScaRuleType`, `EntityRuleType`). Function by function: `reference.md`.

## The bytes of an ID

**The first byte of a type ID is the kind of the type ID.** Every ID
has its kind first, and the kind says how the rest is laid out and
what a type of it registers (`Kind`). What a row registers is its
**registration** (`Registration`), which also tells a layer from a
wide plane -- two registrations under one kind, as both are planes of
the cells. A plane's, an entity type's
and a rule's:

| byte | holds |
|---|---|
| the first (the highest) | its **kind** (`Kind`): `0x00` an attribute, `0x01` a layer or a wide plane, `0x02` an entity type, `0x03` a rule of the cells, `0x04` a rule of an entity |
| the six between | its **number**: what its row in the registry is written with (`NUMBERS`, how many there are) |
| the last (the lowest) | its **size**: of a plane, bits a cell -- 1, 2, 4, 8 or 16 -- in its low five bits and its tile size, as a scale, in the three above ("Tile size"); 0 of an entity type and of a rule, which hold nothing |

An attribute's, under the same first byte: its **number** in the 52
bits after it (`ATTRIBUTE_NUMBERS`), and in its last four
(`ATTRIBUTE_SIZE_BITS`) its **length**, blocks less one -- 1 to 16,
256 bytes at the most.

So what an ID is of, and how big the thing is, is read off the ID with
no table, the same way for a plane as for an attribute; and a number
says nothing of either, so a row is a number and the rest follows from
its kind and its width or layout. A number is one type's whatever its
kind: the check below is on numbers.

## Why one table

The IDs were written where each was first needed: the layers in world
generation, the sheep and its attributes in the sheep's rule. Nothing
saw them all, so nothing could say two did not clash -- and a wide
plane takes more numbers than the one written: four bits a cell are four
layers when cold, the numbers from its first on. A plane put at 5 would
have landed inside the tree stage, at 4 to 7, and built, and run, and
kept a world quietly wrong.

In one table every row is beside the others, and the table is checked
as the crate is built (`first_clash`, in a `const`): two rows sharing
a number or a name, or a wide plane's cold planes reaching into another
row's, stops the build, naming the row; so does an entity type whose
default attribute is none of the table's of a fixed size
(`first_default_amiss`; "Layouts"). A type is added as its
constant and its row, both written by hand in `registry.rs`, the row
made from the constant (`layer`, `wide`, `entity`, `attribute`,
`sca_rule`, `entity_rule`) so
the two cannot say different numbers. No macro makes them: the
table is to be one a world is given as it runs -- types a mod
brings, beside these -- and is written as plainly as that will be.

## Rules

A rule is registered as a type is: a row, an ID of its own kind -- a
rule of the cells, the stochastic cell automaton's (`ScaRuleType`,
`Registration::ScaRule`), or a rule of an entity (`EntityRuleType`,
`Registration::EntityRule`).

**Every rule has one master** (`Registered::master`, its number): the
one plane a rule of the cells is of, the one type of entity an
entity's rule is of -- whatever else the rule reads or writes. The
grass's rule reads and writes other planes and its master is the
grass; the sheep's eats grass and its master is the sheep. A row
holds the rule's name and its master, and no more: what the rule
does, and what it counts, are code above this crate, which the
registry cannot hold. The server lists no rule of its own for that:
it ticks the rules registered, as many and in this table's order
(`rule_at`, `id_of_rule`), and says for each ID only its function
(`../../server/docs/server.md`, "TickCounts").

So a plane and a type of entity say which rules are theirs with
nothing written twice: the rules of a master are the rows that name
it (`rules_of`), and a rule's master is one row (`master_of`).
The build fails, naming the row, if a rule of the cells has a master
that is no layer or wide plane of the table, a rule of an entity one
that is no entity type of it, or a row that is no rule has one
(`first_master_amiss`). A rule is found by its ID (`rule_of_id`);
`RULES_REGISTERED` is how many there are.

## Why its own crate

A type's ID belongs to no one user of it. World generation puts grass
on cells, the rules read and write it, the server lists a world's
layers, the renderer is told its bits -- none owns what grass is
numbered. The registry sits under all of them, on `utilities` alone.
The types an ID is of are here with it, as a constant must be of one:
`chunk_storage` and `entity_manager`, which had them, hand them on
under their own names, as what a layer and an entity are made of.

## Width

A layer is a bit a cell: width 1. A wide plane is 2, 4, 8 or 16, and
takes as many numbers. Hot, one bucket holds each cell's whole number.
Cold, it is as many layers of a bit a cell, the bit `b` of every cell
the layer of number `first + b` (`LayerType::plane`): storage and the
codec know bitmaps only. A plane keeps how wide it is in the type
itself -- its bits a cell, in the ID's last byte ("The bytes of an
ID"), beside its tile size ("Tile size") -- so `bits` is read off the ID with no table; the
width as a type (`Width::BITS`) is the same number known as the code
is built. An attribute's width is its data's bits, 448 in a block of
its own ("Layouts"). An entity type holds nothing itself, nor does a
rule: 0.

## Tile size

A plane holds a number a cell, or a number a **tile**: an aligned
square of cells, `2^scale` a side. Every plane of the ground is of
cells, a tile size of 1x1; the weather's -- the clouds (`CLOUD`, four
bits) and the climate (`CLIMATE`, sixteen: humidity, temperature and
wind together, one read for all three) -- are of tiles 8x8
(`WEATHER_TILE_SCALE`, 3), since weather does not differ from one cell
to the next and sixty-four times fewer numbers are kept and worked on.

The tile size is in the type, beside the width: the last byte of a
plane's ID is its bits a cell in its low five bits and its tile's
scale in the three above (`LayerType::of_tiles`, `tile_scale`,
`tile_side`; `Wide::of_tiles`), and a plane's row says it too
(`Registered::tile_scale`). So nothing that keeps, sends or draws a
plane is told its tile size by hand: the arena sizes its buckets from
the type, the frame a client is sent says how the clouds are laid out
from `CLOUD`'s type, and a plane of another tile size registered
tomorrow is kept as it should be with nothing else changed.

Cells are in Morton order, so a tile is a run of cells and its place
among its chunk's tiles is any of its cells' places less the bits of a
tile's cells (`tile_shift`, two a scale): a plane of tiles is read and
written at a cell, as any plane is, and every cell of a tile reads the
tile's number.

## Layouts

An attribute is blocks of 16 bytes, its type in the first word
(`../../entity_manager/docs/entity_manager.md`, "Attributes, a block
each"). Its row says its layout beside its number: the type, in the
code, of what its data holds -- a number (`u64`); all a sheep keeps
(`SheepState`) -- as a wide plane's row says its width. The constant the
row makes is typed by its layout (`Attribute`), as a wide plane's is by
its width, so an attribute is read as its fields and one layout is
never read as another.

A layout (`Layout`) says how many words of data it holds (`WORDS`;
`BLOCKS`, the blocks that takes with its type before it, 16 at the
most: `ATTRIBUTE_BLOCKS_MOST`, `ATTRIBUTE_WORDS_MOST`, `BLOCK_BYTES`)
and how its fields lie in those words (`write`, `read`; `data_words`,
how many words so many blocks hold: all but the type): words, not a
machine's bytes, so the same on any machine.

**Its members** (`MEMBERS`, each a `Member`): every field by where
its bytes lie -- its first byte, counted from the attribute's first,
and how many it is, 1 to 8, within one word (`Member::word`, a whole
word of the data; `Member::within`, some bytes of one; `Member::of`,
what it holds in the data). This is what an instruction is given to
write a field: the layout is its definer's, and the simulation is
told bytes (`../../simulation/docs/simulation.md`, "Instructions").

**What one kind of entity keeps is one layout**, not an attribute a
field: every attribute begins with its type, eight bytes, and is
walked over to find another, so five ticks kept as five attributes
are five types and five steps, and kept as the fields of one
(`SheepState`) they are one. A field that is a tick is 0 when the
entity is not in the middle of that. An attribute of its own is for
what only some entities of a kind carry and is big enough to be worth
a block.

**An entity type may register a default attribute**
(`Registered::default_attribute`, given a row by `with_default`): the
one attribute every entity of the type has for as long as it is. It
has to be of a fixed size -- only an attribute with a layout can be
given, and the build fails, naming the type, if a row's default is no
attribute of the table, one whose size varies, or the row is no entity
type's (`first_default_amiss`). What it is for: where an entity of the
type is kept, an attribute that is always there and always as long
need not say what it is, so the eight bytes of its type at its
beginning can go. The sheep's is its state (`default_attribute_of`).
The entities' store keeps it so (`default_of`: its type and the words
of its data, from a few rows worked out as the crate is built,
`DEFAULTS`; `../../entity_manager/docs/entity_manager.md`, "The
default attribute"). An attribute's row says the bits of its data
exactly, its layout's words, for that.

**What an attribute's type registers**: its name, its registration, whether
its size varies (`Registered::varies_in_size`), and its size if it
does not. **The ID's last four bits are the length**
(`AttributeType::of_blocks`, `blocks`): of a fixed type its size, set
as it is registered. A type whose size varies is registered with them
0 (`of_varying_size`), and each attribute of it says its own length
there (`with_blocks`): so the first word of any attribute says how
long it is, with no table, and what tells one type from another is
all but those bits (`identity`). **An ID of another kind is no
attribute's** (`is_an_attributes`), nor is one whose number is 0 --
the namespace is one, everything's, so a layer's or an entity type's
ID read as an attribute's says so itself. A row is written with the
number (`number`); a number of 2^52 or more, or a layout of no blocks
or of more than 16, does not build.

## The climate

The climate plane holds sixteen bits a tile, and what they are is said
here once (`climate.rs`), for worldgen that makes them, the weather's
rule that reads them and whatever shows them: **`Climate`**, its
`humidity` the lowest four bits (0 to `MOST_HUMID`), its `wind` the
next four, its `temperature` the highest eight (0 to `HOTTEST`) --
`of_number` and `number` turn the plane's number into it and back.

A **`Wind`** is one of thirteen states in its four bits: 0 calm
(`Wind::CALM`), 1 to 4 blowing towards north, west, south, east
(`WAYS_OF_WIND`, each the step to the next tile) at strength 1, 5 to
8 at 2, 9 to 12 at 3 (`STRONGEST_WIND`). The three numbers its bits
have beside are no wind's and read as calm (`of_state`). `blowing(way,
strength)` makes one, `state` is its bits, `way`, `strength` and
`towards` what a rule asks of it.

**Rain** is a climate's to say, of its temperature and its humidity
both (`Climate::rains_above`, `Climate::rains(cloud)`): the level of
cloud above which a tile is raining is 12 in the coldest, driest air
(`RAINS_ABOVE_IN_COLD_DRY_AIR`), a level more for every 64 of
temperature (`HEAT_A_LEVEL_MORE_HELD`) -- warm air holds more water
before it falls -- and a level fewer for every 4 of humidity
(`HUMIDITY_A_LEVEL_SOONER`), 14 at the most (`RAINS_ABOVE_AT_THE_MOST`):
9 over the coldest, wettest tile. It was one number, 12, for every
tile. The rule's and whatever draws rain have it from this one place.
