//! The simulation: what a server mod's procs are to the server
//! ([`sca_proc`]), and what they see of it ([`server_handle`]); a
//! chunk as it is ticked ([`hot_chunk`]), the tiles of it sampled
//! ([`sampling`]), and its queue of instructions ([`instruction`])
//! applied to it ([`applier`]).

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod applier;
pub mod hot_chunk;
pub mod instruction;
pub mod sampling;
pub mod sca_proc;
pub mod server_handle;
