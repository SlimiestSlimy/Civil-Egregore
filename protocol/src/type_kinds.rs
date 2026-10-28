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

/// How many bits a plane's number is: one of the widths there are, so
/// no other can be held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaneWidth {
    /// 1 bit.
    Bits1,
    /// 2 bits.
    Bits2,
    /// 4 bits.
    Bits4,
    /// 8 bits.
    Bits8,
    /// 16 bits.
    Bits16,
    /// 32 bits.
    Bits32,
}

impl PlaneWidth {
    /// Every width, narrowest first.
    pub const ALL: [Self; 6] = [Self::Bits1, Self::Bits2, Self::Bits4, Self::Bits8, Self::Bits16, Self::Bits32];

    /// Its bits.
    pub const fn bits(self) -> u32 {
        match self {
            Self::Bits1 => 1,
            Self::Bits2 => 2,
            Self::Bits4 => 4,
            Self::Bits8 => 8,
            Self::Bits16 => 16,
            Self::Bits32 => 32,
        }
    }

    /// The width of `bits` bits: one there is, the manifest's reader
    /// having refused any other.
    pub fn of_bits(bits: u32) -> Self {
        Self::ALL.into_iter().find(|width| width.bits() == bits).unwrap_or_else(|| panic!("{bits} bits: no plane's width"))
    }
}

/// How many cells a side a plane's tile is: one of the sides there are,
/// so no other can be held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileSide {
    /// A tile a cell.
    Cells1,
    /// 2 cells a side.
    Cells2,
    /// 4 cells a side.
    Cells4,
    /// 8 cells a side.
    Cells8,
    /// 16 cells a side.
    Cells16,
    /// 32 cells a side.
    Cells32,
    /// 64 cells a side.
    Cells64,
    /// 128 cells a side.
    Cells128,
    /// 256 cells a side.
    Cells256,
}

impl TileSide {
    /// Every side, smallest first.
    pub const ALL: [Self; 9] = [Self::Cells1, Self::Cells2, Self::Cells4, Self::Cells8, Self::Cells16, Self::Cells32, Self::Cells64, Self::Cells128, Self::Cells256];

    /// Cells a tile's side.
    pub const fn side_cells(self) -> u32 {
        match self {
            Self::Cells1 => 1,
            Self::Cells2 => 2,
            Self::Cells4 => 4,
            Self::Cells8 => 8,
            Self::Cells16 => 16,
            Self::Cells32 => 32,
            Self::Cells64 => 64,
            Self::Cells128 => 128,
            Self::Cells256 => 256,
        }
    }

    /// Cells a tile.
    pub const fn tile_cells(self) -> u32 {
        self.side_cells() * self.side_cells()
    }

    /// The side of `side_cells` cells: one there is, the manifest's
    /// reader having refused any other.
    pub fn of_side_cells(side_cells: u32) -> Self {
        Self::ALL.into_iter().find(|tile_side| tile_side.side_cells() == side_cells).unwrap_or_else(|| panic!("{side_cells} cells: no tile's side"))
    }
}

/// A type's kind, and that kind's fields: each type it names there of
/// the kind it must be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A mod: what holds types.
    Mod(ModFields),
    /// A plane: a number a cell, or a tile.
    Plane {
        /// Bits a number.
        width: PlaneWidth,
        /// Cells a tile's side.
        tile_side: TileSide,
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

