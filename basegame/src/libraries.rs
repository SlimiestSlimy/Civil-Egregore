//! The base game's libraries: the API each of its types exposes to the
//! types depending on it, made as the type is registered, with the IDs
//! it works on (`protocol::type_registry::Library`). A rule takes the one
//! it needs from what it runs with (`server::simulation::simulation_api::RuleContext::library`),
//! by the type's ID, and casts it to the API here.


pub mod collision;
