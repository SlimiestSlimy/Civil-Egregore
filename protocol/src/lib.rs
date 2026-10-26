//! What every part of the game shares: the kinds of types
//! ([`type_kinds`]) and their names ([`type_names`]), the mods' manifests
//! read and checked ([`manifests`]) -- written in CDNF
//! ([`cdnf`]), shipped in packages ([`mod_packages`]) -- and the type
//! registry made of them
//! ([`type_registry`]); and simulated time ([`simulation_time`]).

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod type_kinds;
pub mod manifest_format;
pub mod manifests;
pub mod mod_packages;
pub mod simulation_time;
pub mod type_names;
pub mod type_registry;
