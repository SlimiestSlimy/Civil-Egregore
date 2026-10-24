//! Civil Egregore's entity manager: what stands on the cells, kept
//! beside the bitplanes -- a bucket a chunk, one entity a cell, and a
//! timer wheel a superchunk -- and changed by instructions queued.
//!
//! The design: `docs/entity_manager.md`; function by function:
//! `docs/entity_manager_reference.md`.


mod attributes;
mod bucket;
mod instructions;
mod entity;
pub mod saved;
mod store;
mod wheel;

pub use bucket::EntityWritten;
#[cfg_attr(not(test), allow(unused_imports))] // only the crate's tests ask it yet
pub use instructions::{Instructions, InstructionsApplied};
#[cfg_attr(not(test), allow(unused_imports))] // only the crate's tests ask it yet
pub use attributes::{attribute_blocks, may_be_given, DefaultAttributes, attribute_seen, each_attribute, attribute_bytes, find_attribute, push_attribute, remove_attribute, set_attribute, is_attribute_list, words_sum, AttributeType, NO_ATTRIBUTE};
pub use entity::{EntityRef, EntityType};
pub use store::{EntityBeingMade, WorldEntities, EntityReader, SuperchunkEntities};
#[cfg_attr(not(test), allow(unused_imports))] // only the crate's tests ask it yet
pub use wheel::WHEEL_TICKS;
