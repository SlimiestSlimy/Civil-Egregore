//! What the server, the client and the mods share -- the one thing
//! all depend on, none on another, over the utilities: the world's
//! types as the game registers them ([`type_registry`]), the Rust types
//! of the server's side ([`server_types`]), the parameters a command
//! line gives the server, and the messages and their wire ([`pipe`]).
//! It depends on nothing of the game's (`docs/reference.md`).

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod flags;
pub mod pipe;
pub mod server_parameters;
pub mod type_registry;
pub mod world_types;
