//! Civil Egregore's type registry: the one `u64` namespace every type
//! in a world is drawn from, and the one table that gives each its
//! name, kind, ID and width ([`REGISTRY`]); a clash does not build.
//!
//! The design: `docs/type_registry.md`; item by item:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod diagnostics;
mod layouts;
mod registry;
pub mod transient_data;
mod type_ids;

pub use layouts::{data_words, Attribute, Layout, Member, SheepState, ATTRIBUTE_BLOCKS_MOST, ATTRIBUTE_WORDS_MOST, BLOCK_BYTES, BLOCK_WORDS};
pub use registry::*;
pub use type_ids::{AttributeType, Bits16, Bits2, Bits4, Bits8, EntityType, LayerType, Supertype, Wide, Width, NUMBERS};
