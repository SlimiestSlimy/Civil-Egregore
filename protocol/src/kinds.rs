//! The kinds of types, as both the manifest says them and the type
//! registry keeps them: one enum ([`Kind`]), each kind with its own
//! fields, and the hooks a type of a kind may take ([`Hook`]).

use crate::type_names::TypeName;

/// A type's kind, and that kind's fields: each type it names there of
/// the kind it must be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A plane: a number a cell, or a tile.
    Plane {
        /// Bits a number: 1, 2, 4, 8, 16 or 32.
        width_bits: u32,
        /// Cells a tile's side: a power of two, 1 to 256.
        tile_size: u32,
    },
    /// A type of entity.
    Entity {
        /// The attribute every entity of it has, if any: an attribute.
        default_attribute: Option<TypeName>,
    },
    /// An attribute of an entity, of a fixed size: what its data is
    /// follows from it.
    Attribute {
        /// Its size, in blocks of 16 bytes: 1 to 256.
        size: u32,
    },
    /// A proc of an entity: run on every entity of its type, one of as
    /// many procs as name the type.
    EntityProc {
        /// The type of entity it runs on: an entity.
        entity: TypeName,
    },
    /// A proc of the stochastic cell automaton: of the cells.
    ScaProc,
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
        parent: Option<TypeName>,
    },
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

