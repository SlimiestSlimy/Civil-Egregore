# The type registry

Every type a world has is a `u64` from one namespace: a layer of its
cells, a wide plane, a type of entity, an attribute an entity carries.
This crate is where they are all written down, once: one table,
`REGISTRY`, a row a type -- its name, its kind, its ID and its width --
and the types the IDs are of (`LayerType`, `Wide`, `EntityType`,
`AttributeType`). Function by function: `reference.md`.

## Why one table

The IDs were written where each was first needed: the layers in world
generation, the sheep and its attributes in the sheep's rule. Nothing
saw them all, so nothing could say two did not clash -- and a wide
plane takes more IDs than the one written: four bits a cell are four
layers when cold, the types from its first on. A plane put at 5 would
have landed inside the tree stage, at 4 to 7, and built, and run, and
kept a world quietly wrong.

In one table every row is beside the others, and the table is checked
as the crate is built (`first_clash`, in a `const`): two rows sharing
an ID or a name, or a wide plane's cold planes reaching into another
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
takes as many IDs. An attribute's value is a word: 64. An entity type
holds nothing itself: 0.
