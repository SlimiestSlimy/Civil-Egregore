//! Civil Egregore's client: a window connected to a server of its own
//! ([`window`]), and what game content's code reaches in it
//! ([`client_api`]). A skeleton: the client is written afresh from the
//! world's frozen types, and draws nothing yet.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod client_api;
// The skeleton's: told, asked and shown as the fresh client will be, read by none of it yet.
#[allow(dead_code)]
mod server_end;
#[allow(dead_code)]
mod window;

pub use window::run_window;
