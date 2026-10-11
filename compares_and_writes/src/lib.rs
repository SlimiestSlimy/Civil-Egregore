//! Civil Egregore's compares and writes -- its instructions: what a turn queues for the tick's
//! second phase, one function an instruction -- a compare, a write,
//! the end of a compare -- and nothing made of several. What is made
//! of several, and all a turn is asked, is `../actions/`.
//!
//! The design: `docs/compares_and_writes.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod compare;
pub mod entity;
pub mod write;

pub use simulation::{Turn, Write};
