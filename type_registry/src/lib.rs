//! Civil Egregore's type registry: the one `u64` namespace every type
//! in a world is drawn from, and the one table that gives each its
//! name, registration, ID and width ([`REGISTRY`]); a clash does not build.
//!
//! The design: `docs/type_registry.md`; item by item:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

mod climate;
pub mod diagnostics;
mod layouts;
mod registry;
pub mod transient_data;
mod type_ids;

pub use climate::{Climate, Wind, HEAT_A_LEVEL_MORE_HELD, HOTTEST, HUMIDITY_A_LEVEL_SOONER, MOST_HUMID, RAINS_ABOVE_AT_THE_MOST, RAINS_ABOVE_IN_COLD_DRY_AIR, STRONGEST_WIND, WAYS_OF_WIND};
pub use layouts::{data_words, Attribute, Layout, Member, SheepState, ATTRIBUTE_BLOCKS_MOST, ATTRIBUTE_WORDS_MOST, BLOCK_BYTES, BLOCK_WORDS};
pub use registry::*;
pub use type_ids::{AttributeType, Bits16, Bits2, Bits4, Bits8, EntityRuleType, EntityType, LayerType, Kind, ScaRuleType, Wide, Width, ATTRIBUTE_NUMBERS, NUMBERS};
