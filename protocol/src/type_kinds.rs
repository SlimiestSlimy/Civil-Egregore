//! The kinds of types, as both the manifest says them and the type
//! registry keeps them: one enum ([`Kind`]), each kind with its own
//! fields -- a mod among them, a type too -- and the hooks a type of a
//! kind may take ([`Hook`]).

use crate::mod_packages::PackagePath;
use crate::simulation_time::SimulationSeconds;
use crate::type_names::QualifiedTypeName;

/// The side a mod's code runs on. A common mod depends on common ones
/// alone; a server mod on common and server ones; a client mod on common
/// and client ones; a container mod, loading nothing, on any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Both: client calls, and types both sides know.
    Common,
    /// The server.
    Server,
    /// The client.
    Client,
}

/// A mod depended on: its name, and the version depended on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    /// The mod's name.
    pub mod_name: String,
    /// The version depended on: any text, compared whole.
    pub version: String,
}

/// A mod's fields, as the type registry keeps them: the side its code
/// runs on, the mods it depends on, and the package its library is read
/// from -- none if it is a container.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModFields {
    /// The side its code runs on.
    pub side: Side,
    /// The mods it depends on, each at the version it was built against:
    /// the only ones but its own whose types its types name, a
    /// container's reaching what it carries.
    pub depends_on: Vec<Dependency>,
    /// The package its library is read from, if it is no container mod.
    pub package: Option<PackagePath>,
}

/// A type's kind, and that kind's fields: each type it names there of
/// the kind it must be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A mod: what holds types.
    Mod(ModFields),
    /// A plane: a number a cell, or a tile.
    Plane {
        /// Bits a number: 1, 2, 4, 8, 16 or 32.
        width_bits: u32,
        /// A tile is `2^tile_scale` cells a side: 0 for a number a
        /// cell, 8 the most, 256 a side.
        tile_scale: u32,
    },
    /// A type of entity.
    Entity {
        /// The attribute every entity of it has, if any: an attribute.
        default_attribute: Option<QualifiedTypeName>,
    },
    /// An attribute of an entity, of a fixed size: what its data is
    /// follows from it.
    Attribute {
        /// Its size, in blocks of 16 bytes: 1 to 256.
        size_in_blocks: u32,
    },
    /// A proc of an entity: run on every entity of its type, one of as
    /// many procs as name the type.
    EntityProc {
        /// The type of entity it runs on: an entity.
        entity: QualifiedTypeName,
    },
    /// A proc of the stochastic cell automaton: run on the cells sampled
    /// of the planes it names.
    ScaProc {
        /// The planes it is sampled on: one or more, each a plane.
        planes: Vec<QualifiedTypeName>,
        /// How long, on average, between its samples of a tile not 0.
        seconds_per_sample: SimulationSeconds,
    },
    /// A way of generating a world.
    Worldgen,
    /// A function a client may call across the connection.
    ClientCall,
    /// A proc of the renderer.
    RenderProc {
        /// Where it is drawn: the higher over the lower.
        draw_order: u32,
    },
    /// A window of the client's.
    Gui {
        /// The window it is nested in, if any: a gui.
        parent: Option<QualifiedTypeName>,
    },
}

impl Kind {
    /// Whether a type of it has code, its set up function's to make: a
    /// proc's, a worldgen's, a gui's.
    pub fn has_code(&self) -> bool {
        matches!(self, Kind::EntityProc { .. } | Kind::ScaProc { .. } | Kind::Worldgen | Kind::RenderProc { .. } | Kind::Gui { .. })
    }
}

/// What a type is hooked into, each taken by one kind alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hook {
    /// It generates the world: a worldgen's.
    GeneratesTheWorld,
    /// The world is kept hot about it: an entity's.
    KeepsTheWorldHot,
    /// It is shown as a world starts: a gui's.
    ShownByDefault,
}

