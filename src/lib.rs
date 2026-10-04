//! TileSim's worlds, from the command line: made from a seed, ticked,
//! and looked at -- each a folder (`world`). The program, `main.rs`,
//! reads the command line and prints what these say.
//!
//! | module | what it is |
//! |---|---|
//! | [`commands`] | the commands: `new`, `run`, `info` |
//! | `diagnostics/` | data gathered, to be measured and tested on: none yet |
//! | [`transient_data`] | the crate's `transient_data/`, out of git: the tests' worlds |
//!
//! Function by function: `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod commands;
pub mod diagnostics;
pub mod transient_data;
