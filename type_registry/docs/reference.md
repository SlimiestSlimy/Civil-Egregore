# The type registry, item by item

The design is in `type_registry.md`.

## `type_ids.rs`

`type_registry.md`, "The bytes of an ID". **`Supertype`**: what an ID
is of, its first byte -- `Attribute`, `Layer`, `EntityType`.
`SUPERTYPE_SHIFT`, `SIZE_BITS`, `SIZE_MOST`: where the first byte and
the last lie; **`NUMBERS`**: how many numbers the six bytes between
hold. `type_id(supertype, number, size)` makes an ID; `number_of`,
`size_of`, `is_of` read one.
**`LayerType(u64)`**: what a layer represents; **`of_a_bit(number)`** a
layer, a bit a cell; **`wide(first, bits)`** a
type of 2, 4, 8 or 16 bits a cell, **`bits`** how many, its last byte, **`number`** its first, **`plane(bit)`**
and **`planes`** the layers of a bit a cell it is kept cold as,
**`holds(plane)`** whether one is its own. **`Width`**: a plane's width
as a type, `BITS` bits a cell -- **`Bits2`**, **`Bits4`**, **`Bits8`**, **`Bits16`**.
**`Wide<W>`**: a wide plane, its width in its type: **`new(first)`**,
**`layer_type`**, **`most`** the most a cell of it holds.
**`EntityType(u64)`**: an entity's type: **`of(number)`**, **`number`**. **`AttributeType(u64)`**: an
attribute's, its last four bits (`ATTRIBUTE_SIZE_BITS`) how many
blocks one takes, less one, its number the 52 bits before
(`ATTRIBUTE_NUMBERS`): **`of_blocks(number, blocks)`**,
**`of_varying_size(number)`** (the four bits 0, each attribute of it
saying its own: **`with_blocks(blocks)`**), **`blocks`**,
**`identity`** (all but those bits: what tells types apart),
**`is_an_attributes`**, **`number`**.

## `layouts.rs`

`type_registry.md`, "Layouts". `BLOCK_WORDS` (2): the words of an
attribute block, `BLOCK_BYTES` its bytes; `ATTRIBUTE_BLOCKS_MOST` (16) and `ATTRIBUTE_WORDS_MOST`: the most an attribute is; **`data_words(blocks)`**: the words of data so many
blocks hold. **`Layout`**: how an attribute's data is laid out --
`WORDS`, `BLOCKS`, `MEMBERS`, **`write(data)`**, **`read(data)`** -- had by `u64`, one
number in the first word, and by **`SheepState`** `{hungry_at,
lamb_due_at, grown_at, roaming_until, roaming_neighbour}`, a
word each in that order; a tick of 0 none. **`Attribute<L>`**: an attribute, its
layout in its type: **`new(number)`**, **`attribute_type`**. **`Member`** `{offset, bytes}`: a field by where its bytes lie; **`Member::word(word)`**, **`Member::within(word, first, bytes)`**, **`Member::of(data)`**.

## `registry.rs`

**`Kind`**: `Layer`, `WidePlane`, `EntityType`, `Attribute`.
**`Registered`** `{name, kind, number, width, varies_in_size, default_attribute}`: a row, an attribute's
width the bits of its data, an entity type's `default_attribute` the number of the attribute every entity of it has (0: none; **`with_default(attribute)`** gives a row one); **`numbers`** how many
numbers it takes from `number` on, **`layer_type`** its layer type if it is of
the cells. `layer`, `wide`, `entity`, `attribute`: a row made from its constant. **`REGISTRY`**: the table, written by hand. The constants:
`GRASS`, `TREE`, `TREE_STAGE`, `WALL_EAST`, `WALL_SOUTH`, `COLLISION`, `WET`, the
cells'; `SHEEP`, the entity; `SHEEP_STATE`, its one attribute, three blocks; **`OLDEST_TREE_STAGE`**, the most a tree's stage holds.
**`first_clash(registry)`**: the first row to take a number or a name of
one before it -- checked on `REGISTRY` as the crate is built, as is
**`first_default_amiss(registry)`**: the first row whose default attribute is no attribute of a fixed size of the table, or that has one and is no entity type's.
**`default_attribute_of(entity_type)`**: the row of its default attribute, if it has one; **`default_of(entity_type)`**: its type and the words of its data, from `DEFAULTS`, worked out as the crate is built.
**`named(name)`**, **`of_number(number)`**: a row by its name, and by any number it
takes. **`layer_types()`**: every layer type a world's cells have, a
wide plane's whole: what the server lists a world's layers from.

## `diagnostics/`, `transient_data.rs`

Nothing gathered, nothing kept yet; `TRANSIENT_DATA` names the
folder all the same.
