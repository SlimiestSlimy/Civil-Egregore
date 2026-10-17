//! The base game's libraries: the API each of its types exposes to the
//! types depending on it, a `static` the type's registration hands a
//! copyable handle to (`type_registry::Library`). A rule takes the one
//! it needs from what it runs with (`simulation::simulation_api::RuleContext::library`),
//! by the type's ID, and casts it to the API here.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod collision;
