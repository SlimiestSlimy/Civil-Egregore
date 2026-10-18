//! A type as it is registered ([`Registration`]): its name, what it is
//! -- with no ID: the registry gives it one, or the world's save does
//! -- the types it depends on by name, the library it exposes
//! ([`Library`]), and what it is hooked into ([`Hook`]): what the
//! server does with it, and its library, beyond keeping it. Names are
//! for registering: once frozen, a type is found by its ID.

use std::any::Any;
use std::sync::Arc;

/// What a type exposes to the types depending on it: a handle to its
/// API, which each is given by the type's ID and casts to what it
/// knows the API as. The registry knows nothing of what is in it. Made
/// as the world's types are registered, it holds the IDs of the types
/// it works on: those registered before it.
pub type Library = Arc<dyn Any + Send + Sync>;

/// What a type is hooked into: what the simulation does with it, and
/// with its library, beyond keeping it. One more is one more variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hook {
    /// Its library generates the world: the cells of each superchunk,
    /// and what lives on it. A world has one.
    GeneratesTheWorld,
    /// Its library is run on every hot superchunk every tick: a rule's,
    /// of its master.
    RunsEveryTick,
    /// It is a type of entity the world is kept hot about: its halos
    /// follow these. A world has one for now, unless forced hot.
    KeepsTheWorldHot,
    /// It is a window shown as a world starts, not only when asked for.
    ShownByDefault,
}

/// What a type is, as it is declared: everything its ID says but its
/// number, and the types it is of by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Declared {
    /// A layer: a bit a cell.
    Layer,
    /// A wide plane: `width_bits` a cell -- or a tile, `2^tile_scale`
    /// cells a side.
    WidePlane {
        /// Bits a cell: 2, 4, 8, 16 or 32.
        width_bits: u32,
        /// Its tile size, as a scale: 0 for a plane of cells.
        tile_scale: u32,
    },
    /// A type of entity, and the attribute every entity of it has, by
    /// its name, if any.
    Entity {
        /// Its default attribute's name.
        default_attribute: Option<String>,
    },
    /// An attribute of an entity of a fixed size.
    Attribute {
        /// The blocks an attribute of it takes.
        block_count: usize,
        /// The words its data is.
        data_words: usize,
    },
    /// A rule of the cells, and the plane it is of, by its name.
    ScaRule {
        /// Its master's name.
        master: String,
    },
    /// A rule of an entity, and the type of entity it is of, by its name.
    EntityRule {
        /// Its master's name.
        master: String,
    },
    /// A way of generating a world.
    Worldgen,
    /// A rule of the renderer, and what it draws -- a plane or a type of
    /// entity -- by its name.
    RenderRule {
        /// Its master's name.
        master: String,
    },
    /// A window of the client's, and the window it is nested in, by its
    /// name, if any: a child names its parent.
    Gui {
        /// Its parent's name.
        parent: Option<String>,
    },
}

/// A type to register: its name, what it is, the names of the types it
/// depends on -- registered before it -- its library, if it exposes
/// one, and its hooks.
#[derive(Clone)]
pub struct Registration {
    /// Its name: what it is known by, in the registry and in a save.
    pub name: String,
    /// What it is.
    pub declared: Declared,
    /// The names of the types it depends on.
    pub depends_on: Vec<String>,
    /// Its library, if it exposes one.
    pub library: Option<Library>,
    /// What it is hooked into.
    pub hooks: Vec<Hook>,
}

impl Registration {
    /// The type named `name`, `declared` so.
    pub fn of(name: &str, declared: Declared) -> Self {
        Self { name: name.to_string(), declared, depends_on: Vec::new(), library: None, hooks: Vec::new() }
    }

    /// A layer named `name`.
    pub fn layer(name: &str) -> Self {
        Self::of(name, Declared::Layer)
    }

    /// A wide plane named `name`, `width_bits` a cell.
    pub fn wide_plane(name: &str, width_bits: u32) -> Self {
        Self::wide_plane_of_tiles(name, width_bits, 0)
    }

    /// A wide plane named `name`, `width_bits` a tile `2^tile_scale`
    /// cells a side.
    pub fn wide_plane_of_tiles(name: &str, width_bits: u32, tile_scale: u32) -> Self {
        Self::of(name, Declared::WidePlane { width_bits, tile_scale })
    }

    /// A type of entity named `name`, with the attribute named
    /// `default_attribute` if any.
    pub fn entity(name: &str, default_attribute: Option<&str>) -> Self {
        Self::of(name, Declared::Entity { default_attribute: default_attribute.map(str::to_string) })
    }

    /// An attribute named `name` of a fixed size: `block_count` blocks,
    /// its data `data_words` words.
    pub fn attribute(name: &str, block_count: usize, data_words: usize) -> Self {
        Self::of(name, Declared::Attribute { block_count, data_words })
    }

    /// A rule of the cells named `name`, of the plane named `master`.
    pub fn sca_rule(name: &str, master: &str) -> Self {
        Self::of(name, Declared::ScaRule { master: master.to_string() })
    }

    /// A rule of an entity named `name`, of the type of entity named `master`.
    pub fn entity_rule(name: &str, master: &str) -> Self {
        Self::of(name, Declared::EntityRule { master: master.to_string() })
    }

    /// A world generation named `name`.
    pub fn worldgen(name: &str) -> Self {
        Self::of(name, Declared::Worldgen)
    }

    /// A rule of the renderer named `name`, drawing the plane or the type
    /// of entity named `master`.
    pub fn render_rule(name: &str, master: &str) -> Self {
        Self::of(name, Declared::RenderRule { master: master.to_string() })
    }

    /// A window named `name`, nested in the window named `parent` if any.
    pub fn gui(name: &str, parent: Option<&str>) -> Self {
        Self::of(name, Declared::Gui { parent: parent.map(str::to_string) })
    }

    /// It depending on the types named `type_names` too.
    pub fn depending_on(mut self, type_names: &[&str]) -> Self {
        self.depends_on.extend(type_names.iter().map(|type_name| type_name.to_string()));
        self
    }

    /// It exposing `library`.
    pub fn with_library(self, library: Library) -> Self {
        Self { library: Some(library), ..self }
    }

    /// It hooked into `hooks` too.
    pub fn with_hooks(mut self, hooks: &[Hook]) -> Self {
        self.hooks.extend_from_slice(hooks);
        self
    }
}
