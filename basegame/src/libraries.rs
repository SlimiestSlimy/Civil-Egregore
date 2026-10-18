//! The base game's libraries: the API each of its types exposes to the
//! types depending on it, a `static` the type's registration hands a
//! copyable handle to (`protocol::type_registry::Library`). A rule takes the one
//! it needs from what it runs with (`server::simulation::simulation_api::RuleContext::library`),
//! by the type's ID, and casts it to the API here.


pub mod collision;
