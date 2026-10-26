//! The simulation: what a server mod's procs are to the server
//! ([`sca_proc`]), and what they see of it ([`server_handle`]); a
//! chunk as it is ticked ([`hot_chunk`]), the tiles of it sampled
//! ([`plane_sampling`]), and its queue of instructions
//! ([`queued_instruction`]) applied to it ([`queue_applier`]) by its
//! planes' shapes ([`plane_shapes`]).

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod hot_chunk;
pub mod plane_sampling;
pub mod plane_shapes;
pub mod queue_applier;
pub mod queued_instruction;
pub mod sca_proc;
pub mod server_handle;
