# The type registry, item by item

The design is in `type_registry.md`.

## `type_ids.rs`

**`LayerType(u64)`**: what a layer represents; **`wide(first, bits)`** a
type of 2, 4, 8 or 16 bits a cell, **`bits`** how many, **`plane(bit)`**
and **`planes`** the layers of a bit a cell it is kept cold as,
**`holds(plane)`** whether one is its own. **`Width`**: a plane's width
as a type -- **`Bits2`**, **`Bits4`**, **`Bits8`**, **`Bits16`**.
**`Wide<W>`**: a wide plane, its width in its type: **`new(first)`**,
**`layer_type`**, **`most`** the most a cell of it holds.
**`EntityType(u64)`**, **`AttributeType(u64)`**: an entity's type and an
attribute's.

## `registry.rs`

**`Kind`**: `Layer`, `WidePlane`, `EntityType`, `Attribute`.
**`Registered`** `{name, kind, id, width}`: a row; **`ids`** how many
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
