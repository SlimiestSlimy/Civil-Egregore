//! Civil Egregore's instructions: what a rule is made of. Each is one small
//! thing on a superchunk's turn, a query of the simulation as much as
//! a change queued to it -- a step found, a cell sought, the
//! neighbours open to walk to -- written once here, light, and many: a rule of the cells or of an entity (`../mc_rules`,
//! `../entity_rules`) is a few of them put together, and holds only
//! what is its own.
//!
//! An instruction is built on what is public of the crates under it:
//! the simulation's turn -- cells read, writes and entity instructions
//! queued -- the layers the type registry names (`../type_registry`) and the paths of
//! `../pathfinding`. None of those knows another; they meet here.
//!
//! Instructions are kept by what they do to the turn:
//!
//! | folder | instructions |
//! |---|---|
//! | `read/` | those that read, and queue nothing: cells, entities, the cells about a cell, the area, masks, walking |
//! | `write/` | those that queue a change: cells, entities, masks |
//!
//! One that reads and queues at once will be `rw/`'s; there is none
//! yet. Beside them, what they are asked in, no turn in it:
//!
//! | module | shape |
//! |---|---|
//! | `around` | the 3x3 cells about a cell as nine bits |
//! | `area` | the 16x16 cells about a cell, a row a word |
//! | `mask` | a square of cells as bits, 4 to 1,024 a side |
//! | `layers` | the layers a world has before a rule adds its own |
//! | `between_ticks` | entities put on the world between two ticks |
//!
//! A rule is written in these alone, and its crate depends on no other:
//! the instructions are all of the simulation a rule sees. What holds a
//! world -- the hot bitmaps, the entities' store, the storage -- is
//! named by none of them. The simulation under them reads
//! and writes cells and entities and no more: its [`Turn`] is what
//! every instruction is asked on, `Simulation` what ticks a rule,
//! [`TickReport`] what a tick says it did.
//!
//! Function by function: `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod area;
pub mod around;
pub mod between_ticks;
pub mod entity_types;
pub mod layers;
pub mod mask;
pub mod read;
mod rule_counts;
pub mod write;

// The words the instructions are asked in: where a cell and an entity
// are, what a layer and an attribute are, a turn, a tick's report, the
// lot drawn.
pub use chunk_storage::{Bits4, LayerType, Wide};
pub use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, NEIGHBOURS, SUPERCHUNK_SIDE_CELLS};
pub use entity_manager::{Attribute, AttributeType, EntityEdit, EntityId, EntityRef, EntityType, Header};
pub use rule_counts::{RuleCounts, COUNTS_OF_A_RULE};
pub use simulation::{TickReport, Turn};
pub use utilities::chance::Chance;
pub use utilities::rng::Rng;
