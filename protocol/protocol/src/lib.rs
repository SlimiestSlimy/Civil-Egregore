//! What the client and the server say to each other -- the one thing
//! both depend on, neither on the other. Today the parameters a command
//! line gives the server, and the base game's types; the messages and
//! the wire come next
//! (`docs/reference.md`).

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod default_types;
pub mod flags;
pub mod server_parameters;
