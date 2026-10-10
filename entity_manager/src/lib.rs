//! Civil Egregore's entity manager: what stands on the cells, kept
//! beside the bitplanes -- a bucket a chunk, one entity a cell, and a
//! timer wheel a superchunk -- and changed by instructions queued.
//!
//! The design: `docs/entity_manager.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

mod bucket;
pub mod diagnostics;
mod instructions;
mod entity;
pub mod saved;
mod store;
mod wheel;

pub use instructions::{Instructions, InstructionsApplied};
pub use entity::{attribute, remove_attribute, set_attribute, Attribute, AttributeType, EntityEdit, EntityId, EntityRef, EntityType, Header, NEVER};
pub use store::{Entities, EntityReader, SuperchunkEntities, OCCUPIED_SIDE};
pub use wheel::{Wake, WHEEL_TICKS};
