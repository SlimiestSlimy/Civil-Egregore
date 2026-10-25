//! A type as it is registered ([`Registration`]): its name, what it is
//! -- with no ID: the registry gives it one, or the world's save does
//! -- and what it is hooked into ([`Hook`]): what the engine does with
//! it beyond keeping it. Data alone, as a mod's manifest says it: no
//! code. Names are for registering: once frozen, a type is found by its
//! ID.

use super::type_ids::{PlaneTileSize, PlaneWidth};

/// What a type is hooked into: what the engine does with it beyond
/// keeping it. One more is one more variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hook {
    /// Its code generates the world: the cells of each superchunk, and
    /// what lives on it. A world has one.
    GeneratesTheWorld,
    /// It is a type of entity the world is kept hot about: its halos
    /// follow these. A world has one for now, unless forced hot.
    KeepsTheWorldHot,
    /// It is a window shown as a world starts, not only when asked for.
    ShownByDefault,
}

/// Which side a bundle's code runs on: its library is opened there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleSide {
    /// Server and client both: what both use, client calls among it.
    Common,
    /// The server alone.
    Server,
    /// The client alone.
    Client,
}

/// What a type is, as it is declared: everything its ID says but its
/// number, and the types it names, by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Declared {
    /// A plane: a number of `width` a tile of `tile_size` -- a cell, for
    /// a plane of cells.
    Plane {
        /// Bits a number.
        width: PlaneWidth,
        /// What a number is of.
        tile_size: PlaneTileSize,
    },
    /// A type of entity: the attribute every entity of it has, and the
    /// entity rule run as one wakes, each by its name, if any.
    Entity {
        /// Its default attribute's name.
        default_attribute: Option<String>,
        /// Its rule's name.
        rule: Option<String>,
    },
    /// An attribute of an entity of a fixed size.
    Attribute {
        /// The blocks an attribute of it takes.
        block_count: usize,
        /// The words its data is.
        data_words: usize,
    },
    /// A rule of the cells, run on the cells sampled.
    ScaRule,
    /// A rule of entities, run on each entity of a type naming it as it
    /// wakes.
    EntityRule,
    /// A way of generating a world.
    Worldgen,
    /// A rule of the renderer: what it draws, its code says.
    RenderRule {
        /// Where it is drawn among the render rules: the higher over the lower.
        draw_order: u32,
    },
    /// A window of the client's, and the window it is nested in, by its
    /// name, if any: a child names its parent.
    Gui {
        /// Its parent's name.
        parent: Option<String>,
    },
    /// A function a client may call across the connection: its code is
    /// the function.
    ClientCall,
    /// A bundle: the types registered while it is open are in it
    /// (`super::TypeRegistry::open_bundle`). It names the bundles it
    /// depends on: theirs are the only types but its own those in it
    /// may name.
    Bundle {
        /// The side its code runs on: none for a mod's own bundle, which
        /// holds bundles alone.
        side: Option<BundleSide>,
        /// The names of the bundles it depends on, registered before it.
        depends_on: Vec<String>,
    },
}

/// A type to register: its name, what it is, and its hooks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registration {
    /// Its name: what it is known by, in the registry and in a save.
    pub name: String,
    /// What it is.
    pub declared: Declared,
    /// What it is hooked into.
    pub hooks: Vec<Hook>,
}

impl Registration {
    /// The type named `name`, `declared` so.
    pub fn of(name: &str, declared: Declared) -> Self {
        Self { name: name.to_string(), declared, hooks: Vec::new() }
    }

    /// A plane named `name`, a number of `width` a tile of `tile_size`.
    pub fn plane(name: &str, width: PlaneWidth, tile_size: PlaneTileSize) -> Self {
        Self::of(name, Declared::Plane { width, tile_size })
    }

    /// A type of entity named `name`, with the attribute named
    /// `default_attribute` and the entity rule named `rule`, if any.
    pub fn entity(name: &str, default_attribute: Option<&str>, rule: Option<&str>) -> Self {
        Self::of(name, Declared::Entity { default_attribute: default_attribute.map(str::to_string), rule: rule.map(str::to_string) })
    }

    /// An attribute named `name` of a fixed size: `block_count` blocks,
    /// its data `data_words` words.
    pub fn attribute(name: &str, block_count: usize, data_words: usize) -> Self {
        Self::of(name, Declared::Attribute { block_count, data_words })
    }

    /// A rule of the cells named `name`.
    pub fn sca_rule(name: &str) -> Self {
        Self::of(name, Declared::ScaRule)
    }

    /// A rule of entities named `name`.
    pub fn entity_rule(name: &str) -> Self {
        Self::of(name, Declared::EntityRule)
    }

    /// A world generation named `name`.
    pub fn worldgen(name: &str) -> Self {
        Self::of(name, Declared::Worldgen)
    }

    /// A rule of the renderer named `name`, drawn at the draw order
    /// `draw_order`.
    pub fn render_rule(name: &str, draw_order: u32) -> Self {
        Self::of(name, Declared::RenderRule { draw_order })
    }

    /// A window named `name`, nested in the window named `parent` if any.
    pub fn gui(name: &str, parent: Option<&str>) -> Self {
        Self::of(name, Declared::Gui { parent: parent.map(str::to_string) })
    }

    /// A function named `name` a client may call across the connection.
    pub fn client_call(name: &str) -> Self {
        Self::of(name, Declared::ClientCall)
    }

    /// A bundle named `name`, its code on `side` -- none for a mod's own
    /// -- depending on the bundles named `depends_on`: opened by
    /// `super::TypeRegistry::open_bundle`.
    pub fn bundle(name: &str, side: Option<BundleSide>, depends_on: &[&str]) -> Self {
        Self::of(name, Declared::Bundle { side, depends_on: depends_on.iter().map(|bundle_name| bundle_name.to_string()).collect() })
    }

    /// It hooked into `hooks` too.
    pub fn with_hooks(mut self, hooks: &[Hook]) -> Self {
        self.hooks.extend_from_slice(hooks);
        self
    }
}
