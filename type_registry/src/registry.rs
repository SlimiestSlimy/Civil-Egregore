//! The table ([`REGISTRY`]): every type a world has, its constant and
//! its row written by hand, and checked as the crate is built
//! ([`first_clash`]): `docs/type_registry.md`, "Why one table".

use crate::layouts::{Attribute, Layout, SheepState, BLOCK_WORDS};
use crate::type_ids::{AttributeType, Bits16, Bits4, EntityRuleType, EntityType, LayerType, ScaRuleType, Wide, Width};

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
pub struct Registered {
    /// Its name, in words.
    pub name: &'static str,
    /// What it is of.
    pub registration: Registration,
    /// Its number, what its ID keeps between its first byte and its
    /// last: for a wide plane, its first -- its lowest bit's layer's.
    pub number: u64,
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

impl Registered {
    /// How many numbers it takes, from [`Registered::number`] on: a wide
    /// plane one a bit, its cold planes'; anything else one.
    pub const fn numbers(&self) -> u64 {
        match self.registration {
            Registration::WidePlane => self.width as u64,
            _ => 1,
        }
    }

    /// The row, an entity type's, with `default` the attribute every
    /// entity of the type has: one with a layout, so of a fixed size.
    pub const fn with_default<L: Layout>(self, default: Attribute<L>) -> Self {
        Self { default_attribute: default.attribute_type().number(), ..self }
    }

    /// Its layer type, as the hot bitmaps know it -- a wide plane's
    /// with its width -- if it is a layer or a wide plane.
    pub const fn layer_type(&self) -> Option<LayerType> {
        match self.registration {
            Registration::Layer | Registration::WidePlane => Some(LayerType::of_tiles(self.number, self.width, self.tile_scale)),
            _ => None,
        }
    }
}

/// A row of a layer of a bit a cell.
const fn layer(name: &'static str, layer_type: LayerType) -> Registered {
    Registered { name, registration: Registration::Layer, number: layer_type.number(), width: 1, tile_scale: 0, varies_in_size: false, default_attribute: 0, master: 0 }
}

/// A row of a wide plane.
const fn wide<W: Width>(name: &'static str, plane: Wide<W>) -> Registered {
    Registered { name, registration: Registration::WidePlane, number: plane.layer_type().number(), width: W::BITS, tile_scale: plane.layer_type().tile_scale(), varies_in_size: false, default_attribute: 0, master: 0 }
}

/// A row of an entity type, with no default attribute.
const fn entity(name: &'static str, entity_type: EntityType) -> Registered {
    Registered { name, registration: Registration::EntityType, number: entity_type.number(), width: 0, tile_scale: 0, varies_in_size: false, default_attribute: 0, master: 0 }
}

/// A row of a rule of the cells, its master the layer `master`.
const fn sca_rule(name: &'static str, rule: ScaRuleType, master: LayerType) -> Registered {
    Registered { name, registration: Registration::ScaRule, number: rule.number(), width: 0, tile_scale: 0, varies_in_size: false, default_attribute: 0, master: master.number() }
}

/// A row of a rule of an entity, its master the entity type `master`.
const fn entity_rule(name: &'static str, rule: EntityRuleType, master: EntityType) -> Registered {
    Registered { name, registration: Registration::EntityRule, number: rule.number(), width: 0, tile_scale: 0, varies_in_size: false, default_attribute: 0, master: master.number() }
}

/// A row of an attribute.
const fn attribute<L: Layout>(name: &'static str, attribute: Attribute<L>) -> Registered {
    Registered { name, registration: Registration::Attribute, number: attribute.attribute_type().number(), width: L::WORDS as u32 * u64::BITS, tile_scale: 0, varies_in_size: false, default_attribute: 0, master: 0 }
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
pub const TREE_STAGE: Wide<Bits4> = Wide::new(4);
/// A wall between a cell and the cell to its east: the layer of the
/// cells that keep one.
pub const WALL_EAST: LayerType = LayerType::of_a_bit(8);
/// ...to its south.
pub const WALL_SOUTH: LayerType = LayerType::of_a_bit(9);
/// The collision plane: the cells something stands on that bars
/// stepping there -- a tree. 0 is free, 1 is taken: what a
/// movement check asks, whatever the thing is. Who puts such a
/// thing sets its cell, and clears it when the thing goes.
pub const COLLISION: LayerType = LayerType::of_a_bit(10);
/// The sheep's type.
pub const SHEEP: EntityType = EntityType::of(16);
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
pub const CLOUD: Wide<Bits4> = Wide::of_tiles(40, WEATHER_TILE_SCALE);
/// The climate: sixteen bits a tile 8 by 8 cells -- humidity,
/// temperature and wind kept together, one read for all three.
pub const CLIMATE: Wide<Bits16> = Wide::of_tiles(44, WEATHER_TILE_SCALE);

/// The grass's rule: its master the grass.
pub const RULE_OF_GRASS: ScaRuleType = ScaRuleType::of(32);
/// The trees' rule: its master the cells a tree stands on.
pub const RULE_OF_TREES: ScaRuleType = ScaRuleType::of(33);
/// The sheep's rule: its master the sheep.
pub const RULE_OF_SHEEP: EntityRuleType = EntityRuleType::of(34);
/// The weather's rule: its master the climate, whose tiles it samples.
pub const RULE_OF_WEATHER: ScaRuleType = ScaRuleType::of(35);

/// Every type a world has: the layers and wide planes of its cells,
/// the types of its entities and their attributes, a row each beside
/// its constant above. In the order of their numbers.
pub const REGISTRY: &[Registered] = &[
    layer("grass", GRASS),
    layer("tree", TREE),
    wide("tree stage", TREE_STAGE),
    layer("wall east", WALL_EAST),
    layer("wall south", WALL_SOUTH),
    layer("collision", COLLISION),
    entity("sheep", SHEEP).with_default(SHEEP_STATE),
    attribute("sheep state", SHEEP_STATE),
    layer("wet", WET),
    sca_rule("grass rule", RULE_OF_GRASS, GRASS),
    sca_rule("trees rule", RULE_OF_TREES, TREE),
    entity_rule("sheep rule", RULE_OF_SHEEP, SHEEP),
    sca_rule("weather rule", RULE_OF_WEATHER, CLIMATE.layer_type()),
    wide("cloud", CLOUD),
    wide("climate", CLIMATE),
];

/// The oldest stage a tree has: sixteen in all.
pub const OLDEST_TREE_STAGE: u32 = TREE_STAGE.most();

/// Whether `one` and `other` are the same text: `==`, where a constant
/// is worked out.
const fn same(one: &str, other: &str) -> bool {
    let (one, other) = (one.as_bytes(), other.as_bytes());
    let mut at = 0;
    while at < one.len() && at < other.len() && one[at] == other[at] {
        at += 1;
    }
    at == one.len() && at == other.len()
}

/// The first row of `registry` that clashes with one before it, if any
/// does: its numbers -- one, or a wide plane's one a bit -- overlap the
/// other's, or it has the other's name.
pub const fn first_clash(registry: &[Registered]) -> Option<usize> {
    let mut later = 1;
    while later < registry.len() {
        let mut earlier = 0;
        while earlier < later {
            let (one, other) = (&registry[earlier], &registry[later]);
            if (one.number < other.number + other.numbers() && other.number < one.number + one.numbers()) || same(one.name, other.name) {
                return Some(later);
            }
            earlier += 1;
        }
        later += 1;
    }
    None
}

/// The first row of `registry` whose default attribute is amiss, if
/// any is: it names no attribute of the registry, or one whose size
/// varies -- or the row is no entity type's, and has one.
pub const fn first_default_amiss(registry: &[Registered]) -> Option<usize> {
    let mut row = 0;
    while row < registry.len() {
        let default = registry[row].default_attribute;
        if default != 0 {
            let (mut other, mut sound) = (0, false);
            while other < registry.len() {
                let attribute = &registry[other];
                sound |= matches!(attribute.registration, Registration::Attribute) && attribute.number == default && !attribute.varies_in_size;
                other += 1;
            }
            if !sound || !matches!(registry[row].registration, Registration::EntityType) {
                return Some(row);
            }
        }
        row += 1;
    }
    None
}

/// The first row of `registry` whose master is amiss, if any is: a
/// rule of the cells whose master is no layer or wide plane of the
/// registry, a rule of an entity whose master is no entity type of
/// it -- or a row that is no rule, and has one.
pub const fn first_master_amiss(registry: &[Registered]) -> Option<usize> {
    let mut row = 0;
    while row < registry.len() {
        let (rule, mut other, mut sound) = (&registry[row], 0, false);
        while other < registry.len() {
            let master = &registry[other];
            sound |= master.number == rule.master
                && match rule.registration {
                    Registration::ScaRule => matches!(master.registration, Registration::Layer | Registration::WidePlane),
                    Registration::EntityRule => matches!(master.registration, Registration::EntityType),
                    _ => false,
                };
            other += 1;
        }
        if matches!(rule.registration, Registration::ScaRule | Registration::EntityRule) != sound || (!sound && rule.master != 0) {
            return Some(row);
        }
        row += 1;
    }
    None
}

// The build fails here, naming the type, if a row of the registry
// takes a number or a name another before it has, its default
// attribute is none of the registry's of a fixed size, or its master
// is none of the registry's of the kind its rule is of.
const _: () = {
    if let Some(row) = first_clash(REGISTRY) {
        panic!("{}", REGISTRY[row].name)
    }
    if let Some(row) = first_default_amiss(REGISTRY) {
        panic!("{}", REGISTRY[row].name)
    }
    if let Some(row) = first_master_amiss(REGISTRY) {
        panic!("{}", REGISTRY[row].name)
    }
};

/// The row of the rule whose ID is `id` -- a rule of the cells or of
/// an entity -- if it is registered: `None` of any other ID.
pub const fn rule_of_id(id: u64) -> Option<&'static Registered> {
    let (cells, entity, mut row) = (ScaRuleType(id), EntityRuleType(id), 0);
    while row < REGISTRY.len() {
        let registered = &REGISTRY[row];
        let same = match registered.registration {
            Registration::ScaRule => ScaRuleType::of(registered.number).0 == cells.0,
            Registration::EntityRule => EntityRuleType::of(registered.number).0 == entity.0,
            _ => false,
        };
        if same {
            return Some(registered);
        }
        row += 1;
    }
    None
}

/// The row of the rule at `place` among the rules registered, in the
/// table's order: none past the last. What whoever runs the rules
/// goes through them by, so the order is said here and nowhere else.
pub const fn rule_at(place: usize) -> Option<&'static Registered> {
    let (mut passed, mut row) = (0, 0);
    while row < REGISTRY.len() {
        if matches!(REGISTRY[row].registration, Registration::ScaRule | Registration::EntityRule) {
            if passed == place {
                return Some(&REGISTRY[row]);
            }
            passed += 1;
        }
        row += 1;
    }
    None
}

/// The ID of the rule `rule` is a row of: its kind's, of its number.
/// None of a row that is no rule.
pub const fn id_of_rule(rule: &Registered) -> Option<u64> {
    match rule.registration {
        Registration::ScaRule => Some(ScaRuleType::of(rule.number).0),
        Registration::EntityRule => Some(EntityRuleType::of(rule.number).0),
        _ => None,
    }
}

/// How many rules are registered, of the cells and of entities.
pub const RULES_REGISTERED: usize = {
    let (mut count, mut row) = (0, 0);
    while row < REGISTRY.len() {
        count += matches!(REGISTRY[row].registration, Registration::ScaRule | Registration::EntityRule) as usize;
        row += 1;
    }
    count
};

/// The row of the master of the rule `rule`, a row of the registry:
/// the plane or the type of entity it is of.
pub fn master_of(rule: &Registered) -> Option<&'static Registered> {
    REGISTRY.iter().find(|registered| rule.master != 0 && registered.number == rule.master && !matches!(registered.registration, Registration::ScaRule | Registration::EntityRule | Registration::Attribute))
}

/// The rules whose master is the row `master`: those of a plane, or
/// of a type of entity.
pub fn rules_of(master: &Registered) -> impl Iterator<Item = &'static Registered> {
    let number = master.number;
    REGISTRY.iter().filter(move |registered| matches!(registered.registration, Registration::ScaRule | Registration::EntityRule) && registered.master == number)
}

/// The row of the type named `name`, if one is.
pub fn named(name: &str) -> Option<&'static Registered> {
    REGISTRY.iter().find(|registered| registered.name == name)
}

/// The row that `number` is one of the numbers of, if any: a wide plane's for
/// any of its cold planes'.
pub fn of_number(number: u64) -> Option<&'static Registered> {
    REGISTRY.iter().find(|registered| (registered.number..registered.number + registered.numbers()).contains(&number))
}

/// The entity types registered with a default attribute, each with
/// that attribute's type and the words its data is, and how many
/// there are: worked out once, as the crate is built, so that asking
/// of one is a look down a few rows and no walk of the registry.
const DEFAULTS: ([(u64, AttributeType, usize); REGISTRY.len()], usize) = {
    let (mut defaults, mut count, mut row) = ([(0, AttributeType(0), 0); REGISTRY.len()], 0, 0);
    while row < REGISTRY.len() {
        let registered = &REGISTRY[row];
        let mut other = 0;
        while other < REGISTRY.len() && matches!(registered.registration, Registration::EntityType) && registered.default_attribute != 0 {
            let attribute = &REGISTRY[other];
            if matches!(attribute.registration, Registration::Attribute) && attribute.number == registered.default_attribute {
                let words = (attribute.width / u64::BITS) as usize;
                defaults[count] = (registered.number, AttributeType::of_blocks(attribute.number, (1 + words).div_ceil(BLOCK_WORDS)), words);
                count += 1;
            }
            other += 1;
        }
        row += 1;
    }
    (defaults, count)
};

/// The default attribute of `entity_type`, if it is registered with
/// one: its type, as any attribute of it would say it, and the words
/// its data is.
#[inline]
pub const fn default_of(entity_type: EntityType) -> Option<(AttributeType, usize)> {
    let (number, mut row) = (entity_type.number(), 0);
    while row < DEFAULTS.1 {
        if DEFAULTS.0[row].0 == number {
            return Some((DEFAULTS.0[row].1, DEFAULTS.0[row].2));
        }
        row += 1;
    }
    None
}

/// The row of the default attribute of `entity_type`, if it is
/// registered with one: the attribute every entity of the type has.
pub fn default_attribute_of(entity_type: EntityType) -> Option<&'static Registered> {
    let default = REGISTRY.iter().find(|registered| registered.registration == Registration::EntityType && registered.number == entity_type.number())?.default_attribute;
    REGISTRY.iter().find(|registered| default != 0 && registered.registration == Registration::Attribute && registered.number == default)
}

/// Every layer type a world's cells have, as the hot bitmaps know
/// them: each layer, and each wide plane whole. Dirt has none: it is a
/// cell with nothing on it.
pub fn layer_types() -> Vec<LayerType> {
    REGISTRY.iter().filter_map(Registered::layer_type).collect()
}
