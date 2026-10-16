# The type registry, item by item

The design is in `type_registry.md`.

## `type_ids.rs`

`type_registry.md`, "The bytes of an ID". **`TypeKind`**: the kind of an ID,
its first byte -- `Attribute`, `Layer`, `EntityType`, `ScaRule`, `EntityRule`.
`KIND_SHIFT`, `SIZE_BITS`, `SIZE_MOST`: where the first byte and
the last lie; **`TYPE_ID_NUMBERS`**: how many numbers the six bytes between
hold. `type_id(type_kind, type_number, size_byte)` makes an ID; `number_of`,
`size_of`, `is_of` read one.
**`LayerType(u64)`**: what a layer represents; **`of_a_bit(type_number)`** a
layer, a bit a cell; **`of_width(first_number, width_bits)`** a
type of 2, 4, 8, 16 or 32 bits a cell, **`width_bits`** how many, its last byte, **`type_number`** its first, **`bitplane_of(bit_index)`**
and **`each_bitplane`** the layers of a bit a cell it is kept cold as,
**`holds_bitplane(asked_bitplane)`** whether one is its own. **`Width`**: a plane's width
as a type, `BITS` bits a cell -- **`Bits2`**, **`Bits4`**, **`Bits8`**, **`Bits16`**, **`Bits32`**.
**`LayerType::of_tiles(first_number, width_bits, tile_scale)`**: a plane of a
number a tile (`type_registry.md`, "Tile size"), its scale beside its
width in its last byte (`WIDTH_BITS`, the bits of that byte the width
takes); **`tile_scale()`**, **`tile_side()`**, **`tile_shift()`** --
the places a cell's place is shifted by to give its tile's.
**`WidePlane<W>`**: a wide plane, its width in its type: **`new(first_number)`**, **`of_tiles(first_number, tile_scale)`**,
**`layer_type`**, **`most_value`** the most a cell of it holds.
**`EntityType(u64)`**: an entity's type: **`from_number(type_number)`**, **`type_number`**. **`ScaRuleType(u64)`**, **`EntityRuleType(u64)`**: a rule of the cells, a rule of an entity: **`from_number(type_number)`**, **`type_number`**. **`AttributeType(u64)`**: an
attribute's, its last four bits (`ATTRIBUTE_SIZE_BITS`) how many
blocks one takes, less one, its number the 52 bits before
(`ATTRIBUTE_NUMBERS`): **`of_blocks(type_number, block_count)`**,
**`of_varying_size(type_number)`** (the four bits 0, each attribute of it
saying its own: **`with_blocks(block_count)`**), **`block_count`**,
**`type_identity`** (all but those bits: what tells types apart),
**`is_an_attributes`**, **`type_number`**.

## `layouts.rs`

`type_registry.md`, "Layouts". `BLOCK_WORDS` (2): the words of an
attribute block, `BLOCK_BYTES` its bytes; `ATTRIBUTE_BLOCKS_MOST` (16) and `ATTRIBUTE_WORDS_MOST`: the most an attribute is; **`data_words(block_count)`**: the words of data so many
blocks hold. **`Layout`**: how an attribute's data is laid out --
`WORDS`, `BLOCK_COUNT`, `MEMBERS`, **`write(attribute_data)`**, **`read_data(attribute_data)`** -- had by `u64`, one
number in the first word, and by **`SheepState`** `{hungry_at,
lamb_due_at, grown_at, roaming_until, roaming_neighbour}`, a
word each in that order; a tick of 0 none. **`Attribute<L>`**: an attribute, its
layout in its type: **`new(type_number)`**, **`attribute_type`**. **`Member`** `{byte_offset, byte_count}`: a field by where its bytes lie; **`Member::word(word)`**, **`Member::within_word(word, first_byte, byte_count)`**, **`Member::value_in(attribute_data)`**.

## `registry.rs`

**`Registration`**: what a row registers -- `Layer`, `WidePlane`, `EntityType`, `Attribute`, `ScaRule`, `EntityRule`.
**`RegistryRow`** `{name, registration, type_number, width, tile_scale, varies_in_size, default_attribute, master}`: a row, a rule's `master` the number of the one plane or type of entity it is of (0: no rule), an attribute's
width the bits of its data, an entity type's `default_attribute` the number of the attribute every entity of it has (0: none; **`with_default(attribute)`** gives a row one); **`numbers_taken`** how many
numbers it takes from `type_number` on, **`layer_type`** its layer type if it is of
the cells. `layer`, `wide_plane`, `entity`, `attribute`, `sca_rule`, `entity_rule`: a row made from its constant. **`REGISTRY`**: the table, written by hand. The constants:
`GRASS`, `TREE`, `TREE_STAGE`, `WALL_EAST`, `WALL_SOUTH`, `COLLISION`, `WALL_NORTH`, `WALL_WEST` (a wall is before a cell's step one way, kept by that cell: four layers), `WET`, the
cells'; `CLOUD` and `CLIMATE`, the weather's, a number a tile 8x8 (`WEATHER_TILE_SCALE`); `SHEEP`, the entity; `SHEEP_STATE`, its one attribute, three blocks; `RULE_OF_GRASS`, `RULE_OF_TREES`, the cells' rules, `RULE_OF_SHEEP`, the entity's, `RULE_OF_WEATHER`, the weather's, its master the climate; **`OLDEST_TREE_STAGE`**, the most a tree's stage holds.
Private: `same_text(one_text, other_text)`, `==` where a constant is worked out.
**`first_clash(registry_rows)`**: the first row to take a number or a name of
one before it -- checked on `REGISTRY` as the crate is built, as is
**`first_default_amiss(registry_rows)`**: the first row whose default attribute is no attribute of a fixed size of the table, or that has one and is no entity type's.
**`rule_at(place)`**: the row of the rule at a place among the rules registered, in the table's order; **`id_of_rule(rule)`**: a rule's row's ID.
**`first_master_amiss(registry_rows)`**: the first row whose master is amiss -- a rule whose master is no row of the kind its rule is of, or a row that is no rule and has one -- checked on `REGISTRY` too (`type_registry.md`, "Rules"). **`rule_of_id(rule_id)`**: a rule's row by its ID; **`RULES_REGISTERED`**: how many rules there are; **`master_of(rule)`**: the row of a rule's master; **`rules_of(master)`**: the rules a plane or a type of entity is the master of.
**`default_attribute_of(entity_type)`**: the row of its default attribute, if it has one; **`default_of(entity_type)`**: its type and the words of its data, from `DEFAULT_ATTRIBUTES`, worked out as the crate is built.
**`row_named(type_name)`**, **`of_number(plane_number)`**: a row by its name, and by any number it
takes. **`layer_types()`**: every layer type a world's cells have, a
wide plane's whole: what the server lists a world's layers from.

## `diagnostics/`, `transient_data.rs`

Nothing gathered, nothing kept yet; `TRANSIENT_DATA` names the
folder all the same.

## `climate.rs`

What the climate plane's number holds (`type_registry.md`, "The
climate"). `RAINS_ABOVE_IN_COLD_DRY_AIR`, `HEAT_A_LEVEL_MORE_HELD`, `HUMIDITY_A_LEVEL_SOONER`, `RAINS_ABOVE_AT_THE_MOST`, `MOST_HUMID`, `HUMIDITY_A_LEVEL_OF_CLOUD`, `HOTTEST_TEMPERATURE`, `HIGHEST_PRESSURE`, `PRESSURE_AT_REST`, `PRESSURE_GAP_AT_REST`, `STRONGEST_WIND`, `TILES_ABOUT`, `DEGREES_A_WIND_CARRIES`, `RISE_THAT_HALVES`, `FALL_THAT_FREES`. **`nearer_by_step(from_value, to_value, step)`**, **`passes_a_slope(height_rise, whole_parts)`**, **`pressure_falls_by(pressure_here, pressure_there)`**; `held_to_the_circle(blows_east, blows_south)` and the `nearest_root(squared_number)` it shortens by. **`Wind`** (`blows_east`, `blows_south`): `CALM_AIR`, **`from_vector(blows_east, blows_south)`**, **`of_state`**, **`state_byte`**, **`east_south_vector`**, **`wind_strength`**, **`step_drawn(drawn_across, drawn_down)`**, **`pushed_by(push_east, push_south)`**, **`a_step_towards(other_wind)`**. **`Climate`** `{humidity, wind, temperature, pressure}`: **`of_number(plane_number)`**, **`plane_number`**, **`humidity_level`**, **`with_humidity_carried_from(upwind_climate)`**, **`with_temperature_carried_from`**, **`with_wind_carried_from`**, **`rains_above`**, **`rains(cloud)`**. In `registry.rs`: **`ELEVATION`**, a tile's height over the ocean.
