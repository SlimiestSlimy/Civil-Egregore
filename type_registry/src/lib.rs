//! Civil Egregore's type registry: the one `u64` namespace every type
//! in a world is drawn from -- a layer, a wide plane, an entity type,
//! an attribute -- and the one table that gives each its name, kind, ID
//! and width ([`REGISTRY`]). Two of them sharing an ID, or a wide
//! plane's cold planes reaching into another's, does not build.
//! The design: `docs/type_registry.md`; item by item:
//! `docs/reference.md`.
//!
//! | module | what it is |
//! |---|---|
//! | `type_ids` | what an ID is of: [`LayerType`], [`Wide`] and its widths, [`EntityType`], [`AttributeType`] |
//! | `registry` | the table, a constant a row, and the check that no two rows clash |
//! | `diagnostics` | nothing yet |
//! | `transient_data` | the crate's `transient_data/` folder |

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod diagnostics;
mod registry;
pub mod transient_data;
mod type_ids;

pub use registry::*;
pub use type_ids::{AttributeType, Bits16, Bits2, Bits4, Bits8, EntityType, LayerType, Wide, Width};
