//! The table ([`REGISTRY`]): every type a world has, a row each, written
//! once in [`registered!`] and checked as the crate is built
//! ([`first_clash`]): `docs/type_registry.md`, "Why one table".

use crate::layouts::{data_words, Attribute, Layout, Roaming};
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
    /// Its ID: for a wide plane, its first -- its lowest bit's layer.
    pub id: u64,
    /// Its width, in bits: what a cell of a layer or a wide plane
    /// holds, what an attribute's data holds; 0 for an entity type,
    /// which holds nothing itself.
    pub width: u32,
}

impl Registered {
    /// How many IDs it takes, from [`Registered::id`] on: a wide
    /// plane one a bit, its cold planes'; anything else one.
    pub const fn ids(&self) -> u64 {
        match self.kind {
            Kind::WidePlane => self.width as u64,
            _ => 1,
        }
    }

    /// Its layer type, as the hot bitmaps know it -- a wide plane's
    /// with its width -- if it is a layer or a wide plane.
    pub const fn layer_type(&self) -> Option<LayerType> {
        match self.kind {
            Kind::Layer | Kind::WidePlane => Some(LayerType::wide(self.id, self.width)),
            _ => None,
        }
    }
}

/// Makes the registry: for each row, the constant the type is named by
/// in the code -- a `LayerType`, a `Wide`, an `EntityType` or an
/// `Attribute` -- and its row in [`REGISTRY`]. A wide plane's row
/// says its width, an attribute's its layout.
macro_rules! registered {
    ($( $(#[$doc:meta])* $kind:ident $constant:ident $(<$width:ident>)? = $id:literal, $name:literal; )*) => {
        $( registered!(@constant [$(#[$doc])*] $kind $constant $id $($width)?); )*

        /// Every type a world has: the layers and wide planes of its
        /// cells, the types of its entities and their attributes. In
        /// the order written, which is the order of their IDs.
        pub const REGISTRY: &[Registered] = &[ $( registered!(@row $kind $name $id $($width)?) ),* ];
    };
    (@constant [$(#[$doc:meta])*] layer $constant:ident $id:literal) => { $(#[$doc])* pub const $constant: LayerType = LayerType($id); };
    (@constant [$(#[$doc:meta])*] wide $constant:ident $id:literal $width:ident) => { $(#[$doc])* pub const $constant: Wide<$width> = Wide::new($id); };
    (@constant [$(#[$doc:meta])*] entity $constant:ident $id:literal) => { $(#[$doc])* pub const $constant: EntityType = EntityType($id); };
    (@constant [$(#[$doc:meta])*] attribute $constant:ident $id:literal $layout:ident) => { $(#[$doc])* pub const $constant: Attribute<$layout> = Attribute::new($id); };
    (@row layer $name:literal $id:literal) => { Registered { name: $name, kind: Kind::Layer, id: $id, width: 1 } };
    (@row wide $name:literal $id:literal $width:ident) => { Registered { name: $name, kind: Kind::WidePlane, id: $id, width: <$width as Width>::BITS } };
    (@row entity $name:literal $id:literal) => { Registered { name: $name, kind: Kind::EntityType, id: $id, width: 0 } };
    (@row attribute $name:literal $id:literal $layout:ident) => { Registered { name: $name, kind: Kind::Attribute, id: $id, width: data_words(<$layout as Layout>::BLOCKS) as u32 * u64::BITS } };
}

registered! {
    /// Grass: what a superchunk is generated with in patches, where its
    /// ground is dry, and the cells' rules grow and the sheep eat. A cell
    /// without it is dirt, which has no layer.
    layer GRASS = 2, "grass";
    /// The cells a tree stands on: what a superchunk is generated with in
    /// patches of their own, and the trees' rule ages, fells and seeds.
    layer TREE = 3, "tree";
    /// A tree's stage, 0 to [`OLDEST_TREE_STAGE`]: a plane four bits a cell
    /// wide, kept cold as the four layer types from 4 on, a bit each.
    wide TREE_STAGE<Bits4> = 4, "tree stage";
    /// A wall between a cell and the cell to its east: the layer of the
    /// cells that keep one.
    layer WALL_EAST = 8, "wall east";
    /// ...to its south.
    layer WALL_SOUTH = 9, "wall south";
    /// The collision plane: the cells something stands on that bars
    /// stepping there -- a tree. 0 is free, 1 is taken: what a
    /// movement check asks, whatever the thing is. Who puts such a
    /// thing sets its cell, and clears it when the thing goes.
    layer COLLISION = 10, "collision";
    /// The sheep's type.
    entity SHEEP = 16, "sheep";
    /// The tick a sheep is next hungry at.
    attribute HUNGRY_AT<u64> = 17, "hungry at";
    /// The tick a pregnant sheep's lamb is due at.
    attribute PREGNANT<u64> = 18, "pregnant";
    /// The tick a lamb is grown at.
    attribute LAMB<u64> = 19, "lamb";
    /// A sheep leaving thin pasture: the tick it roams until and the
    /// neighbour it steps to. Set once, at the meal: a step on the way
    /// changes no attribute.
    attribute ROAMING<Roaming> = 20, "roaming";
    /// A sheep whose lamb was put beside it last tick: the lamb's ID.
    /// Pregnant still, until it has seen the lamb stand beside it.
    attribute BEARING<u64> = 21, "bearing";
    /// The cells under water, however deep: what a rule asks. How deep is
    /// kept a map a chunk with water, in the superchunk's image
    /// (`chunk_storage::SuperchunkImage::depth`).
    layer WET = 24, "wet";
}

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
/// does: its IDs -- one, or a wide plane's one a bit -- overlap the
/// other's, or it has the other's name.
pub const fn first_clash(registry: &[Registered]) -> Option<usize> {
    let mut later = 1;
    while later < registry.len() {
        let mut earlier = 0;
        while earlier < later {
            let (one, other) = (&registry[earlier], &registry[later]);
            if (one.id < other.id + other.ids() && other.id < one.id + one.ids()) || same(one.name, other.name) {
                return Some(later);
            }
            earlier += 1;
        }
        later += 1;
    }
    None
}

// The build fails here, naming the type, if a row of the registry
// takes an ID or a name another before it has.
const _: () = {
    if let Some(row) = first_clash(REGISTRY) {
        panic!("{}", REGISTRY[row].name)
    }
};

/// The row of the type named `name`, if one is.
pub fn named(name: &str) -> Option<&'static Registered> {
    REGISTRY.iter().find(|registered| registered.name == name)
}

/// The row that `id` is one of the IDs of, if any: a wide plane's for
/// any of its cold planes'.
pub fn of_id(id: u64) -> Option<&'static Registered> {
    REGISTRY.iter().find(|registered| (registered.id..registered.id + registered.ids()).contains(&id))
}

/// Every layer type a world's cells have, as the hot bitmaps know
/// them: each layer, and each wide plane whole. Dirt has none: it is a
/// cell with nothing on it.
pub fn layer_types() -> Vec<LayerType> {
    REGISTRY.iter().filter_map(Registered::layer_type).collect()
}
