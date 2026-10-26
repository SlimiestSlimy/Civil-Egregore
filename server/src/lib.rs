//! The simulation: what a server mod's procs are to the server
//! ([`sca_proc`]), and what they see of it ([`server_handle`]).

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod sca_proc;
pub mod server_handle;
