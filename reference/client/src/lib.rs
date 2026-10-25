//! Civil Egregore's client: a window connected to a server of its own
//! ([`window`]), and what game content's code reaches in it
//! ([`client_api`]): written afresh from the world's frozen types, each
//! world drawn by its render rules.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod client_api;
mod painting;
// Told, asked and shown as the client will be, not all of it read yet.
#[allow(dead_code)]
mod server_end;
mod window;

// The crate's own tests, its internals theirs to reach: the tiers in `tests/`.
#[cfg(test)]
#[path = "../tests/fine/main.rs"]
mod fine_tests;

pub use window::run_window;
