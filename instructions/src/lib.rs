//! Civil Egregore's instructions: what a rule is made of. Each is one
//! small thing asked or done on a superchunk's turn; a rule is a few
//! of them put together, and its crate depends on this one alone.
//! Those that read are in `read/`, those that queue a change in
//! `write/`, the shapes they answer in beside them.
//!
//! The design: `docs/instructions.md`; function by function:
//! `docs/reference.md`.

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
