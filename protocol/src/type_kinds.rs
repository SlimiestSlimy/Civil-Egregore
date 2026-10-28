//! The kinds of types, as both the manifest says them and the type
//! registry keeps them: one enum ([`Kind`]), each kind with its own
//! fields -- a mod among them, a type too. Only the kinds built are
//! here; each other comes as it is built.

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
    Mod {
        /// The side its code runs on.
        side: Side,
        /// The mods it depends on, each at the version it was built
        /// against: the only ones but its own whose types its types name,
        /// a container's reaching what it carries.
        depends_on: Vec<Dependency>,
        /// The package its library is read from: none if it is a
        /// container.
        package: Option<PackagePath>,
    },
    /// A plane: a number a cell, or a tile.
    Plane {
        /// Bits a number.
        width: PlaneWidth,
        /// Cells a tile's side.
        tile_side: TileSide,
    },
    /// A proc of the stochastic cell automaton: run on the cells sampled
    /// of the planes it names.
    ScaProc {
        /// The planes it is sampled on: one or more, each a plane.
        planes: Vec<QualifiedTypeName>,
        /// How long, on average, between its samples of a tile not 0.
        mean_simulation_seconds_per_sample: SimulationSeconds,
    },
}

impl Kind {
    /// Whether a type of it has code, its set up's to make: a sca
    /// proc's; a mod and a plane are data alone.
    pub fn has_code(&self) -> bool {
        matches!(self, Kind::ScaProc { .. })
    }
}
