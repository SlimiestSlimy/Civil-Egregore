//! What every part of the game shares: the kinds of types
//! ([`kinds`]) and their names ([`type_names`]), the mods' manifests
//! read and checked ([`manifests`]) in the format they are written in
//! ([`cdnf`]), and the type registry made of them ([`type_registry`]).

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod cdnf;
pub mod kinds;
pub mod manifests;
pub mod type_names;
pub mod type_registry;
