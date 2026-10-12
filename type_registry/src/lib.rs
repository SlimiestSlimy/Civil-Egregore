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

pub use climate::{nearer, passes_a_rise, pressure_falls_by, Climate, Wind, DEGREES_A_WIND_CARRIES, HEAT_A_LEVEL_MORE_HELD, HIGHEST_PRESSURE, HOTTEST, HUMIDITY_A_LEVEL_OF_CLOUD, HUMIDITY_A_LEVEL_SOONER, MOST_HUMID, PRESSURE_AT_REST, PRESSURE_GAP_AT_REST, RAINS_ABOVE_AT_THE_MOST, RAINS_ABOVE_IN_COLD_DRY_AIR, RISE_THAT_HALVES, STRONGEST_WIND, TILES_ABOUT};
pub use layouts::{data_words, Attribute, Layout, Member, SheepState, ATTRIBUTE_BLOCKS_MOST, ATTRIBUTE_WORDS_MOST, BLOCK_BYTES, BLOCK_WORDS};
pub use registry::*;
pub use type_ids::{AttributeType, Bits16, Bits2, Bits32, Bits4, Bits8, EntityRuleType, EntityType, LayerType, Kind, ScaRuleType, Wide, Width, ATTRIBUTE_NUMBERS, NUMBERS};
