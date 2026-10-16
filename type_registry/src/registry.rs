//! The table ([`REGISTRY`]): every type a world has, its constant and
//! its row written by hand, and checked as the crate is built
//! ([`first_clash`]): `docs/type_registry.md`, "Why one table".

use crate::layouts::{Attribute, Layout, SheepState, BLOCK_WORDS};
use crate::type_ids::{AttributeType, Bits32, Bits4, Bits8, EntityRuleType, EntityType, LayerType, ScaRuleType, WidePlane, Width};

/// What a row registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Registration {
    /// A layer: a bit a cell.
    Layer,
    /// A wide plane: a number a cell, kept cold as a layer a bit.
    WidePlane,
    /// A type of entity.
    EntityType,
    /// An attribute of an entity.
    Attribute,
    /// A rule of the cells: its master a layer or a wide plane.
    ScaRule,
    /// A rule of an entity: its master a type of entity.
    EntityRule,
}

/// One row of the registry: a type a world has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegistryRow {
    /// Its name, in words.
    pub name: &'static str,
    /// What it is of.
    pub registration: Registration,
    /// Its number, what its ID keeps between its first byte and its
    /// last: for a wide plane, its first -- its lowest bit's layer's.
    pub type_number: u64,
    /// Its width, in bits: what a cell of a layer or a wide plane
    /// holds, what an attribute's data holds if its size is fixed; 0
    /// for an entity type, which holds nothing itself, and for an
    /// attribute whose size varies.
    pub width: u32,
    /// A plane's tile size, as a scale: its tiles are `2^tile_scale`
    /// cells a side, 0 a plane of cells. 0 for anything else.
    pub tile_scale: u32,
    /// Whether it is an attribute whose size varies: each of it then
    /// says its own length, in the last four bits of its first word.
    pub varies_in_size: bool,
    /// An entity type's default attribute, by its number: the one
    /// every entity of the type has for as long as it is, of a fixed
    /// size -- so that it need not say its type where it is kept. 0
    /// for an entity type with none, and for anything else.
    pub default_attribute: u64,
    /// A rule's master, by its number: the one plane a rule of the
    /// cells is of, the one type of entity an entity's rule is of,
    /// whatever else the rule reads or writes. 0 for anything else.
    pub master: u64,
}

impl RegistryRow {
    /// How many numbers it takes, from [`RegistryRow::type_number`] on: a wide
    /// plane one a bit, its cold planes'; anything else one.
    pub const fn numbers_taken(&self) -> u64 {
        match self.registration {
            Registration::WidePlane => self.width as u64,
            _ => 1,
        }
    }

    /// The row, an entity type's, with `given_default` the attribute every
    /// entity of the type has: one with a layout, so of a fixed size.
    pub const fn with_default<L: Layout>(self, given_default: Attribute<L>) -> Self {
        Self { default_attribute: given_default.attribute_type().type_number(), ..self }
    }

    /// Its layer type, as the hot bitmaps know it -- a wide plane's
    /// with its width -- if it is a layer or a wide plane.
    pub const fn layer_type(&self) -> Option<LayerType> {
        match self.registration {
            Registration::Layer | Registration::WidePlane => Some(LayerType::of_tiles(self.type_number, self.width, self.tile_scale)),
            _ => None,
        }
    }
}

/// A row of a layer of a bit a cell.
const fn layer(name: &'static str, layer_type: LayerType) -> RegistryRow {
    RegistryRow { name, registration: Registration::Layer, type_number: layer_type.type_number(), width: 1, tile_scale: 0, varies_in_size: false, default_attribute: 0, master: 0 }
}

/// A row of a wide plane.
const fn wide_plane<W: Width>(name: &'static str, plane_type: WidePlane<W>) -> RegistryRow {
    RegistryRow { name, registration: Registration::WidePlane, type_number: plane_type.layer_type().type_number(), width: W::BITS_A_CELL, tile_scale: plane_type.layer_type().tile_scale(), varies_in_size: false, default_attribute: 0, master: 0 }
}

/// A row of an entity type, with no default attribute.
const fn entity(name: &'static str, entity_type: EntityType) -> RegistryRow {
    RegistryRow { name, registration: Registration::EntityType, type_number: entity_type.type_number(), width: 0, tile_scale: 0, varies_in_size: false, default_attribute: 0, master: 0 }
}

/// A row of a rule of the cells, its master the layer `master`.
const fn sca_rule(name: &'static str, rule: ScaRuleType, master: LayerType) -> RegistryRow {
    RegistryRow { name, registration: Registration::ScaRule, type_number: rule.type_number(), width: 0, tile_scale: 0, varies_in_size: false, default_attribute: 0, master: master.type_number() }
}

/// A row of a rule of an entity, its master the entity type `master`.
const fn entity_rule(name: &'static str, rule: EntityRuleType, master: EntityType) -> RegistryRow {
    RegistryRow { name, registration: Registration::EntityRule, type_number: rule.type_number(), width: 0, tile_scale: 0, varies_in_size: false, default_attribute: 0, master: master.type_number() }
}

/// A row of an attribute.
const fn attribute<L: Layout>(name: &'static str, attribute: Attribute<L>) -> RegistryRow {
    RegistryRow { name, registration: Registration::Attribute, type_number: attribute.attribute_type().type_number(), width: L::WORDS as u32 * u64::BITS, tile_scale: 0, varies_in_size: false, default_attribute: 0, master: 0 }
}

/// Grass: what a superchunk is generated with in patches, where its
/// ground is dry, and the cells' rules grow and the sheep eat. A cell
/// without it is dirt, which has no layer.
pub const GRASS: LayerType = LayerType::of_a_bit(2);
/// The cells a tree stands on: what a superchunk is generated with in
/// patches of their own, and the trees' rule ages, fells and seeds.
pub const TREE: LayerType = LayerType::of_a_bit(3);
/// A tree's stage, 0 to [`OLDEST_TREE_STAGE`]: a plane four bits a cell
/// wide, kept cold as the four layer types from 4 on, a bit each.
pub const TREE_STAGE: WidePlane<Bits4> = WidePlane::new(4);
/// A wall before a cell's step to the cell to its east: the layer of
/// the cells no step leaves that way. A wall is one way's: the step
/// back, the other cell's west, has its own -- a drop is stepped down
/// and not back up.
pub const WALL_EAST: LayerType = LayerType::of_a_bit(8);
/// ...to its south.
pub const WALL_SOUTH: LayerType = LayerType::of_a_bit(9);
/// The collision plane: the cells something stands on that bars
/// stepping there -- a tree. 0 is free, 1 is taken: what a
/// movement check asks, whatever the thing is. Who puts such a
/// thing sets its cell, and clears it when the thing goes.
pub const COLLISION: LayerType = LayerType::of_a_bit(10);
/// A wall before a cell's step to the cell to its north
/// ([`WALL_EAST`])...
pub const WALL_NORTH: LayerType = LayerType::of_a_bit(11);
/// ...to its west.
pub const WALL_WEST: LayerType = LayerType::of_a_bit(12);
/// The sheep's type.
pub const SHEEP: EntityType = EntityType::from_number(16);
/// All a sheep keeps: when it is next hungry, and what it is in the
/// middle of -- carrying a lamb, bearing it, growing up, leaving thin
/// pasture.
pub const SHEEP_STATE: Attribute<SheepState> = Attribute::new(17);
/// The cells under water, however deep: what a rule asks. How deep is
/// kept a map a chunk with water, in the superchunk's image
/// (`chunk_storage::SuperchunkImage::depth`).
pub const WET: LayerType = LayerType::of_a_bit(24);

/// The tile size of the weather's planes, as a scale: tiles 8 by 8
/// cells.
pub const WEATHER_TILE_SCALE: u32 = 3;
/// The clouds: a number a tile 8 by 8 cells, 0 no cloud to 15 the
/// deepest shadow a cloud casts.
pub const CLOUD: WidePlane<Bits4> = WidePlane::of_tiles(40, WEATHER_TILE_SCALE);
/// The climate: thirty-two bits a tile 8 by 8 cells -- humidity,
/// wind, temperature and pressure, a byte each, kept together: one
/// read for all four.
pub const CLIMATE: WidePlane<Bits32> = WidePlane::of_tiles(44, WEATHER_TILE_SCALE);
/// How high the land of a tile 8 by 8 cells stands over the ocean,
/// in heights, 255 at the most: what the air's moves are held
/// against, read by the weather as any plane is.
pub const ELEVATION: WidePlane<Bits8> = WidePlane::of_tiles(80, WEATHER_TILE_SCALE);

/// The grass's rule: its master the grass.
pub const RULE_OF_GRASS: ScaRuleType = ScaRuleType::from_number(32);
/// The trees' rule: its master the cells a tree stands on.
pub const RULE_OF_TREES: ScaRuleType = ScaRuleType::from_number(33);
/// The sheep's rule: its master the sheep.
pub const RULE_OF_SHEEP: EntityRuleType = EntityRuleType::from_number(34);
/// The weather's rule: its master the climate, whose tiles it samples.
pub const RULE_OF_WEATHER: ScaRuleType = ScaRuleType::from_number(35);

/// Every type a world has: the layers and wide planes of its cells,
/// the types of its entities and their attributes, a row each beside
/// its constant above. In the order of their numbers.
pub const REGISTRY: &[RegistryRow] = &[
    layer("grass", GRASS),
    layer("tree", TREE),
    wide_plane("tree stage", TREE_STAGE),
    layer("wall east", WALL_EAST),
    layer("wall south", WALL_SOUTH),
    layer("collision", COLLISION),
    layer("wall north", WALL_NORTH),
    layer("wall west", WALL_WEST),
    entity("sheep", SHEEP).with_default(SHEEP_STATE),
    attribute("sheep state", SHEEP_STATE),
    layer("wet", WET),
    sca_rule("grass rule", RULE_OF_GRASS, GRASS),
    sca_rule("trees rule", RULE_OF_TREES, TREE),
    entity_rule("sheep rule", RULE_OF_SHEEP, SHEEP),
    sca_rule("weather rule", RULE_OF_WEATHER, CLIMATE.layer_type()),
    wide_plane("cloud", CLOUD),
    wide_plane("climate", CLIMATE),
    wide_plane("elevation", ELEVATION),
];

/// The oldest stage a tree has: sixteen in all.
pub const OLDEST_TREE_STAGE: u32 = TREE_STAGE.most_value();

/// Whether `one_text` and `other_text` are the same text: `==`, where a constant
/// is worked out.
const fn same_text(one_text: &str, other_text: &str) -> bool {
    let (one_bytes, other_bytes) = (one_text.as_bytes(), other_text.as_bytes());
    let mut byte_index = 0;
    while byte_index < one_bytes.len() && byte_index < other_bytes.len() && one_bytes[byte_index] == other_bytes[byte_index] {
        byte_index += 1;
    }
    byte_index == one_bytes.len() && byte_index == other_bytes.len()
}

/// The first row of `registry_rows` that clashes with one before it, if any
/// does: its numbers -- one, or a wide plane's one a bit -- overlap the
/// other's, or it has the other's name.
pub const fn first_clash(registry_rows: &[RegistryRow]) -> Option<usize> {
    let mut later_index = 1;
    while later_index < registry_rows.len() {
        let mut earlier_index = 0;
        while earlier_index < later_index {
            let (earlier_row, later_row) = (&registry_rows[earlier_index], &registry_rows[later_index]);
            if (earlier_row.type_number < later_row.type_number + later_row.numbers_taken() && later_row.type_number < earlier_row.type_number + earlier_row.numbers_taken()) || same_text(earlier_row.name, later_row.name) {
                return Some(later_index);
            }
            earlier_index += 1;
        }
        later_index += 1;
    }
    None
}

/// The first row of `registry_rows` whose default attribute is amiss, if
/// any is: it names no attribute of the registry, or one whose size
/// varies -- or the row is no entity type's, and has one.
pub const fn first_default_amiss(registry_rows: &[RegistryRow]) -> Option<usize> {
    let mut row_index = 0;
    while row_index < registry_rows.len() {
        let default_number = registry_rows[row_index].default_attribute;
        if default_number != 0 {
            let (mut other_index, mut default_sound) = (0, false);
            while other_index < registry_rows.len() {
                let attribute = &registry_rows[other_index];
                default_sound |= matches!(attribute.registration, Registration::Attribute) && attribute.type_number == default_number && !attribute.varies_in_size;
                other_index += 1;
            }
            if !default_sound || !matches!(registry_rows[row_index].registration, Registration::EntityType) {
                return Some(row_index);
            }
        }
        row_index += 1;
    }
    None
}

/// The first row of `registry_rows` whose master is amiss, if any is: a
/// rule of the cells whose master is no layer or wide plane of the
/// registry, a rule of an entity whose master is no entity type of
/// it -- or a row that is no rule, and has one.
pub const fn first_master_amiss(registry_rows: &[RegistryRow]) -> Option<usize> {
    let mut row_index = 0;
    while row_index < registry_rows.len() {
        let (rule, mut other_index, mut master_sound) = (&registry_rows[row_index], 0, false);
        while other_index < registry_rows.len() {
            let master = &registry_rows[other_index];
            master_sound |= master.type_number == rule.master
                && match rule.registration {
                    Registration::ScaRule => matches!(master.registration, Registration::Layer | Registration::WidePlane),
                    Registration::EntityRule => matches!(master.registration, Registration::EntityType),
                    _ => false,
                };
            other_index += 1;
        }
        if matches!(rule.registration, Registration::ScaRule | Registration::EntityRule) != master_sound || (!master_sound && rule.master != 0) {
            return Some(row_index);
        }
        row_index += 1;
    }
    None
}

// The build fails here, naming the type, if a row of the registry
// takes a number or a name another before it has, its default
// attribute is none of the registry's of a fixed size, or its master
// is none of the registry's of the kind its rule is of.
const _: () = {
    if let Some(amiss_row) = first_clash(REGISTRY) {
        panic!("{}", REGISTRY[amiss_row].name)
    }
    if let Some(amiss_row) = first_default_amiss(REGISTRY) {
        panic!("{}", REGISTRY[amiss_row].name)
    }
    if let Some(amiss_row) = first_master_amiss(REGISTRY) {
        panic!("{}", REGISTRY[amiss_row].name)
    }
};

/// The row of the rule whose ID is `rule_id` -- a rule of the cells or of
/// an entity -- if it is registered: `None` of any other ID.
pub const fn rule_of_id(rule_id: u64) -> Option<&'static RegistryRow> {
    let (cells, entity, mut row_index) = (ScaRuleType(rule_id), EntityRuleType(rule_id), 0);
    while row_index < REGISTRY.len() {
        let registered_row = &REGISTRY[row_index];
        let id_matches = match registered_row.registration {
            Registration::ScaRule => ScaRuleType::from_number(registered_row.type_number).0 == cells.0,
            Registration::EntityRule => EntityRuleType::from_number(registered_row.type_number).0 == entity.0,
            _ => false,
        };
        if id_matches {
            return Some(registered_row);
        }
        row_index += 1;
    }
    None
}

/// The row of the rule at `place` among the rules registered, in the
/// table's order: none past the last. What whoever runs the rules
/// goes through them by, so the order is said here and nowhere else.
pub const fn rule_at(place: usize) -> Option<&'static RegistryRow> {
    let (mut rules_passed, mut row_index) = (0, 0);
    while row_index < REGISTRY.len() {
        if matches!(REGISTRY[row_index].registration, Registration::ScaRule | Registration::EntityRule) {
            if rules_passed == place {
                return Some(&REGISTRY[row_index]);
            }
            rules_passed += 1;
        }
        row_index += 1;
    }
    None
}

/// The ID of the rule `rule` is a row of: its kind's, of its number.
/// None of a row that is no rule.
pub const fn id_of_rule(rule: &RegistryRow) -> Option<u64> {
    match rule.registration {
        Registration::ScaRule => Some(ScaRuleType::from_number(rule.type_number).0),
        Registration::EntityRule => Some(EntityRuleType::from_number(rule.type_number).0),
        _ => None,
    }
}

/// How many rules are registered, of the cells and of entities.
pub const RULES_REGISTERED: usize = {
    let (mut count, mut row_index) = (0, 0);
    while row_index < REGISTRY.len() {
        count += matches!(REGISTRY[row_index].registration, Registration::ScaRule | Registration::EntityRule) as usize;
        row_index += 1;
    }
    count
};

/// The row of the master of the rule `rule`, a row of the registry:
/// the plane or the type of entity it is of.
pub fn master_of(rule: &RegistryRow) -> Option<&'static RegistryRow> {
    REGISTRY.iter().find(|registered_row| rule.master != 0 && registered_row.type_number == rule.master && !matches!(registered_row.registration, Registration::ScaRule | Registration::EntityRule | Registration::Attribute))
}

/// The rules whose master is the row `master`: those of a plane, or
/// of a type of entity.
pub fn rules_of(master: &RegistryRow) -> impl Iterator<Item = &'static RegistryRow> {
    let master_number = master.type_number;
    REGISTRY.iter().filter(move |registered_row| matches!(registered_row.registration, Registration::ScaRule | Registration::EntityRule) && registered_row.master == master_number)
}

/// The row of the type named `type_name`, if one is.
pub fn row_named(type_name: &str) -> Option<&'static RegistryRow> {
    REGISTRY.iter().find(|registered_row| registered_row.name == type_name)
}

/// The row that `type_number` is one of the numbers of, if any: a wide plane's for
/// any of its cold planes'.
pub fn of_number(type_number: u64) -> Option<&'static RegistryRow> {
    REGISTRY.iter().find(|registered_row| (registered_row.type_number..registered_row.type_number + registered_row.numbers_taken()).contains(&type_number))
}

/// The entity types registered with a default attribute, each with
/// that attribute's type and the words its data is, and how many
/// there are: worked out once, as the crate is built, so that asking
/// of one is a look down a few rows and no walk of the registry.
const DEFAULT_ATTRIBUTES: ([(u64, AttributeType, usize); REGISTRY.len()], usize) = {
    let (mut default_attributes, mut count, mut row_index) = ([(0, AttributeType(0), 0); REGISTRY.len()], 0, 0);
    while row_index < REGISTRY.len() {
        let registered_row = &REGISTRY[row_index];
        let mut other_index = 0;
        while other_index < REGISTRY.len() && matches!(registered_row.registration, Registration::EntityType) && registered_row.default_attribute != 0 {
            let attribute = &REGISTRY[other_index];
            if matches!(attribute.registration, Registration::Attribute) && attribute.type_number == registered_row.default_attribute {
                let words = (attribute.width / u64::BITS) as usize;
                default_attributes[count] = (registered_row.type_number, AttributeType::of_blocks(attribute.type_number, (1 + words).div_ceil(BLOCK_WORDS)), words);
                count += 1;
            }
            other_index += 1;
        }
        row_index += 1;
    }
    (default_attributes, count)
};

/// The default attribute of `entity_type`, if it is registered with
/// one: its type, as any attribute of it would say it, and the words
/// its data is.
#[inline]
pub const fn default_of(entity_type: EntityType) -> Option<(AttributeType, usize)> {
    let (type_number, mut row_index) = (entity_type.type_number(), 0);
    while row_index < DEFAULT_ATTRIBUTES.1 {
        if DEFAULT_ATTRIBUTES.0[row_index].0 == type_number {
            return Some((DEFAULT_ATTRIBUTES.0[row_index].1, DEFAULT_ATTRIBUTES.0[row_index].2));
        }
        row_index += 1;
    }
    None
}

/// The row of the default attribute of `entity_type`, if it is
/// registered with one: the attribute every entity of the type has.
pub fn default_attribute_of(entity_type: EntityType) -> Option<&'static RegistryRow> {
    let default_number = REGISTRY.iter().find(|registered_row| registered_row.registration == Registration::EntityType && registered_row.type_number == entity_type.type_number())?.default_attribute;
    REGISTRY.iter().find(|registered_row| default_number != 0 && registered_row.registration == Registration::Attribute && registered_row.type_number == default_number)
}

/// Every layer type a world's cells have, as the hot bitmaps know
/// them: each layer, and each wide plane whole. Dirt has none: it is a
/// cell with nothing on it.
pub fn layer_types() -> Vec<LayerType> {
    REGISTRY.iter().filter_map(RegistryRow::layer_type).collect()
}
