//! General-purpose utilities, shared by every crate in Civil Egregore
//! and owned by none: a module each.
//!
//! The design: `docs/utilities.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod bitmap;
pub mod cache;
pub mod chance;
pub mod coordinates;
pub mod csv;
pub mod dispatcher;
pub mod dynamic_library;
pub mod fixed_list;
pub mod fixed_point;
pub mod hash;
pub mod rng;
pub mod seed;
pub mod settings;
pub mod wire;
