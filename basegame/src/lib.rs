//! Civil Egregore's base game: the one crate of its content, a module
//! each -- its types ([`base_types`]), the libraries they expose
//! ([`libraries`]), the rules of its cells ([`sca_rules`]) and of its
//! entities ([`entity_rules`]), its world generation ([`worldgen`])
//! how it is drawn ([`render_rules`]) and its windows ([`gui`]) -- the
//! code of those two only with the `client` feature -- and its types
//! registered ([`default_types`]).
//! Nothing but the program's initialization should name it: the server
//! and the client are handed its types.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod base_types;
pub mod default_types;
pub mod entity_rules;
pub mod gui;
pub mod libraries;
pub mod render_rules;
pub mod sca_rules;
pub mod worldgen;
