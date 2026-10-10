//! Diagnostics: what the window draws, gathered with no window. They
//! only gather: the tools (`tool.rs`, functions `Civil_Egregore
//! renderer` runs) print and keep.
//!
//! | file | what it gathers |
//! |---|---|
//! | `stills.rs` | pictures of one place of a world at every zoom, painted as the window's own threads paint them |
//! | `tool.rs` | the tools: each runs one of the others and keeps what it gathers |

pub mod stills;
pub mod tool;
