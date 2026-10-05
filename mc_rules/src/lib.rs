//! TileSim's rules of the cells: every rule the cells run by, a file
//! each -- a tick of Monte Carlo sampling on a superchunk's turn, and
//! the writes it queues. Entities are a crate of their own
//! (`entity_rules/`); the world ticks both together (`server/`).
//!
//! | module | rule |
//! |---|---|
//! | `grass` | grass spreading over dirt, and decaying |
//! | `diagnostics/` | data gathered, to be measured and tested on: none yet |
//! | `transient_data` | the crate's `transient_data/`, out of git: what its runs leave behind |
//!
//! What the rules are: `docs/mc_rules.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod diagnostics;
pub mod transient_data;

pub mod grass;
pub mod trees;
