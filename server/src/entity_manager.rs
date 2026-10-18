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
pub use instructions::{Instructions, InstructionsApplied};
pub use attributes::{attribute, attribute_blocks, append_kept, may_be_given, DefaultAttributes, DefaultKept, attribute_seen, blocks_sum, each_attribute, attribute_bytes, find_attribute, push_attribute, remove_attribute, set_attribute, set_attribute_blocks, is_attribute_list, words_sum, write_attribute_bytes, Attribute, AttributeBlock, AttributeType, Layout, Member, WholeAttribute, ATTRIBUTE_BLOCKS_MOST, ATTRIBUTE_WORDS_MOST, BLOCK_BYTES, BLOCK_WORDS, NO_ATTRIBUTE};
pub use entity::{EntityRef, EntityType, Header, NEVER_WAKES};
pub use store::{WorldEntities, EntityReader, SuperchunkEntities, OCCUPIED_SIDE};
pub use wheel::{Wake, WHEEL_TICKS};
