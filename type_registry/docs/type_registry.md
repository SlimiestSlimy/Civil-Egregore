# The type registry

Every type a world has is a `u64` from one namespace: a layer of its
cells, a wide plane, a type of entity, an attribute an entity carries.
This crate is where they are all written down, once: one table,
`REGISTRY`, a row a type -- its name, its kind, its number and its width --
and the types the IDs are of (`LayerType`, `Wide`, `EntityType`,
`AttributeType`). Function by function: `reference.md`.

## The bytes of an ID

Every ID is laid out alike, whatever it is of:

| byte | holds |
|---|---|
| the first (the highest) | its **supertype** (`Supertype`): `0x00` an attribute, `0x01` a layer or a wide plane, `0x02` an entity type |
| the six between | its **number**: what its row in the registry is written with (`NUMBERS`, how many there are) |
| the last (the lowest) | its **size**: bits a cell of a plane -- 1, 2, 4, 8 or 16; blocks of an attribute -- 1 to 254, all ones for a size that varies; 0 of an entity type, which holds nothing |

So what an ID is of, and how big the thing is, is read off the ID with
no table, the same way for a plane as for an attribute; and a number
says nothing of either, so a row is a number and the rest follows from
its kind and its width or layout. A number is one type's whatever its
supertype: the check below is on numbers.

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
row's, stops the build, naming the row. A type is added as a row and
nowhere else; `registered!` makes its constant from the same line, so
no type is in use that is not listed.

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
ID") -- so `bits` is read off the ID with no table; the
width as a type (`Width::BITS`) is the same number known as the code
is built. An attribute's width is its data's bits, 448 in a block of
its own ("Layouts"). An entity type holds nothing itself: 0.

## Layouts

An attribute is blocks of 64 bytes, its type in the first word
(`../../entity_manager/docs/entity_manager.md`, "Attributes, a block
each"). Its row says its layout beside its number: the type, in the
code, of what its data holds -- a number (`u64`); all a sheep keeps
(`SheepState`) -- as a wide plane's row says its width. The constant the
row makes is typed by its layout (`Attribute`), as a wide plane's is by
its width, so an attribute is read as its fields and one layout is
never read as another.

A layout (`Layout`) says how many blocks it takes (`BLOCKS`) and how
its fields lie in the data's words (`write`, `read`; `data_words`, how
many words that is: all of its blocks' but the type): words, not
bytes, so the same on any machine.

**What one kind of entity keeps is one layout**, not an attribute a
field: a block is 64 bytes whatever it holds, so five ticks kept as
five attributes are five blocks, and kept as the fields of one
(`SheepState`) they are one. A field that is a tick is 0 when the
entity is not in the middle of that. An attribute of its own is for
what only some entities of a kind carry and is big enough to be worth
a block.

**The ID's last byte is the blocks** (`AttributeType::of_blocks`,
`blocks`): 1 to 254, all ones for a size that varies
(`of_varying_size`). **An ID of another supertype is no attribute's**
(`is_an_attributes`), nor is one whose last byte is 0 -- the namespace
is one, everything's, so a layer's or an entity type's ID read as an
attribute's says so itself. A row is written with the number between
the two bytes (`number`); a number that reaches into either, or a
layout of no blocks or of 255 or more, does not build.
