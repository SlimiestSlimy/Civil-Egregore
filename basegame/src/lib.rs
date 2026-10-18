//! Civil Egregore's base game: the one crate of its content, a module
//! each -- its types ([`base_types`]), the libraries they expose
//! ([`libraries`]), the rules of its cells ([`sca_rules`]) and of its
//! entities ([`entity_rules`]), and its world generation
//! ([`worldgen`]) -- and its types registered ([`default_types`]).
//! Nothing but the program's initialization should name it: the server
//! and the client are handed its types.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod base_types;
pub mod default_types;
pub mod entity_rules;
pub mod libraries;
pub mod sca_rules;
pub mod worldgen;
