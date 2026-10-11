//! Civil Egregore's entity manager: what stands on the cells, kept
//! beside the bitplanes -- a bucket a chunk, one entity a cell, and a
//! timer wheel a superchunk -- and changed by instructions queued.
//!
//! The design: `docs/entity_manager.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

mod attributes;
mod bucket;
pub mod diagnostics;
mod instructions;
mod entity;
pub mod saved;
mod store;
mod wheel;

pub use instructions::{Instructions, InstructionsApplied};
pub use attributes::{attribute, attribute_blocks, blocks_sum, each_attribute, find_attribute, push_attribute, remove_attribute, set_attribute, set_attribute_blocks, sorted, Attribute, AttributeBlock, AttributeType, Layout, BLOCK_WORDS};
pub use entity::{EntityEdit, EntityId, EntityRef, EntityType, Header, NEVER};
pub use store::{Arrival, CollisionCells, Entities, EntityReader, Settled, SuperchunkEntities, OCCUPIED_SIDE};
pub use wheel::{Wake, WHEEL_TICKS};
