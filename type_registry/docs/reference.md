# The type registry, item by item

The design is in `type_registry.md`.

## `type_ids.rs`

**`LayerType(u64)`**: what a layer represents; **`wide(first, bits)`** a
type of 2, 4, 8 or 16 bits a cell, **`bits`** how many, **`plane(bit)`**
and **`planes`** the layers of a bit a cell it is kept cold as,
**`holds(plane)`** whether one is its own. `WIDTH_SHIFT`: where a type keeps how wide it is. **`Width`**: a plane's width
as a type, `BITS` bits a cell -- **`Bits2`**, **`Bits4`**, **`Bits8`**, **`Bits16`**.
**`Wide<W>`**: a wide plane, its width in its type: **`new(first)`**,
**`layer_type`**, **`most`** the most a cell of it holds.
**`EntityType(u64)`**: an entity's type. **`AttributeType(u64)`**: an
attribute's, its top byte how many blocks one takes (`BLOCKS_SHIFT`;
`SIZE_VARIES`, all ones, for a size that varies; 0 for an ID that is
no attribute's): **`of_blocks(number, blocks)`**,
**`of_varying_size(number)`**, **`blocks`**, **`is_an_attributes`**,
**`number`**, its ID under the top byte.

## `layouts.rs`

`type_registry.md`, "Layouts". `BLOCK_WORDS` (8): the words of an
attribute block; **`data_words(blocks)`**: the words of data so many
blocks hold. **`Layout`**: how an attribute's data is laid out --
`BLOCKS`, **`write(data)`**, **`read(data)`** -- had by `u64`, one
number in the first word, and by **`Roaming`** `{until, neighbour}`,
the first word and the second. **`Attribute<L>`**: an attribute, its
layout in its type: **`new(number)`**, **`attribute_type`**.

## `registry.rs`

**`Kind`**: `Layer`, `WidePlane`, `EntityType`, `Attribute`.
**`Registered`** `{name, kind, id, width}`: a row, an attribute's
width the bits of its data; **`ids`** how many
IDs it takes from `id` on, **`layer_type`** its layer type if it is of
the cells. **`registered!`**: the rows as written -- from each its
constant and its row. **`REGISTRY`**: the table. The constants:
`GRASS`, `TREE`, `TREE_STAGE`, `WALL_EAST`, `WALL_SOUTH`, `WET`, the
cells'; `SHEEP`, the entity; `HUNGRY_AT`, `PREGNANT`, `LAMB`, `ROAMING`,
its attributes; **`OLDEST_TREE_STAGE`**, the most a tree's stage holds.
**`first_clash(registry)`**: the first row to take an ID or a name of
one before it -- checked on `REGISTRY` as the crate is built.
**`named(name)`**, **`of_id(id)`**: a row by its name, and by any ID it
takes. **`layer_types()`**: every layer type a world's cells have, a
wide plane's whole: what the server lists a world's layers from.

## `diagnostics/`, `transient_data.rs`

Nothing gathered, nothing kept yet; `TRANSIENT_DATA` names the
folder all the same.
