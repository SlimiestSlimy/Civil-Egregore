//! The table ([`REGISTRY`]): every type a world has, its constant and
//! its row written by hand, and checked as the crate is built
//! ([`first_clash`]): `docs/type_registry.md`, "Why one table".

use crate::layouts::{data_words, Attribute, Layout, SheepState};
use crate::type_ids::{Bits4, EntityType, LayerType, Wide, Width};

/// What a registered type is of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A layer: a bit a cell.
    Layer,
    /// A wide plane: a number a cell, kept cold as a layer a bit.
    WidePlane,
    /// A type of entity.
    EntityType,
    /// An attribute of an entity.
    Attribute,
}

/// One row of the registry: a type a world has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registered {
    /// Its name, in words.
    pub name: &'static str,
    /// What it is of.
    pub kind: Kind,
    /// Its number, what its ID keeps between its first byte and its
    /// last: for a wide plane, its first -- its lowest bit's layer's.
    pub number: u64,
    /// Its width, in bits: what a cell of a layer or a wide plane
    /// holds, what an attribute's data holds if its size is fixed; 0
    /// for an entity type, which holds nothing itself, and for an
    /// attribute whose size varies.
    pub width: u32,
    /// Whether it is an attribute whose size varies: each of it then
    /// says its own length, in the last four bits of its first word.
    pub varies_in_size: bool,
}

impl Registered {
    /// How many numbers it takes, from [`Registered::number`] on: a wide
    /// plane one a bit, its cold planes'; anything else one.
    pub const fn numbers(&self) -> u64 {
        match self.kind {
            Kind::WidePlane => self.width as u64,
            _ => 1,
        }
    }

    /// Its layer type, as the hot bitmaps know it -- a wide plane's
    /// with its width -- if it is a layer or a wide plane.
    pub const fn layer_type(&self) -> Option<LayerType> {
        match self.kind {
            Kind::Layer | Kind::WidePlane => Some(LayerType::wide(self.number, self.width)),
            _ => None,
        }
    }
}

/// A row of a layer of a bit a cell.
const fn layer(name: &'static str, layer_type: LayerType) -> Registered {
    Registered { name, kind: Kind::Layer, number: layer_type.number(), width: 1, varies_in_size: false }
}

/// A row of a wide plane.
const fn wide<W: Width>(name: &'static str, plane: Wide<W>) -> Registered {
    Registered { name, kind: Kind::WidePlane, number: plane.layer_type().number(), width: W::BITS, varies_in_size: false }
}

/// A row of an entity type.
const fn entity(name: &'static str, entity_type: EntityType) -> Registered {
    Registered { name, kind: Kind::EntityType, number: entity_type.number(), width: 0, varies_in_size: false }
}

/// A row of an attribute.
const fn attribute<L: Layout>(name: &'static str, attribute: Attribute<L>) -> Registered {
    Registered { name, kind: Kind::Attribute, number: attribute.attribute_type().number(), width: data_words(L::BLOCKS) as u32 * u64::BITS, varies_in_size: false }
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
    entity("sheep", SHEEP),
    attribute("sheep state", SHEEP_STATE),
    layer("wet", WET),
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

// The build fails here, naming the type, if a row of the registry
// takes a number or a name another before it has.
const _: () = {
    if let Some(row) = first_clash(REGISTRY) {
        panic!("{}", REGISTRY[row].name)
    }
};

/// The row of the type named `name`, if one is.
pub fn named(name: &str) -> Option<&'static Registered> {
    REGISTRY.iter().find(|registered| registered.name == name)
}

/// The row that `number` is one of the numbers of, if any: a wide plane's for
/// any of its cold planes'.
pub fn of_number(number: u64) -> Option<&'static Registered> {
    REGISTRY.iter().find(|registered| (registered.number..registered.number + registered.numbers()).contains(&number))
}

/// Every layer type a world's cells have, as the hot bitmaps know
/// them: each layer, and each wide plane whole. Dirt has none: it is a
/// cell with nothing on it.
pub fn layer_types() -> Vec<LayerType> {
    REGISTRY.iter().filter_map(Registered::layer_type).collect()
}
