//! General-purpose utilities, shared by every crate in Civil Egregore
//! and owned by none: a module each.
//!
//! The design: `docs/utilities.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod cache;
pub mod chance;
pub mod commands;
pub mod csv;
pub mod diagnostics;
pub mod dispatcher;
pub mod fixed_list;
pub mod fixed_point;
pub mod hash;
pub mod rng;
pub mod seed;
pub mod settings;
pub mod stale_docs;
pub mod transient_data;
pub mod tuning;
