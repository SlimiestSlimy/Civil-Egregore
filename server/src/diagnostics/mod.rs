//! Diagnostics: data gathered from Civil Egregore's ticks, one kind of data to
//! a file. They only gather: nothing here judges a result or prints one
//! -- the tests (`tests/`) judge, and the tools
//! (`tool.rs`, functions `Civil_Egregore world` runs) print and keep -- as in Tessera's. The mock
//! world they tick is the entities' (`entity_rules/src/diagnostics/world.rs`).
//!
//! | file | what it gathers |
//! |---|---|
//! | `throughput.rs` | grass ticked flat out: each phase's time, the writes, the memory held |
//! | `pasture.rs` | grass and sheep ticked flat out: the flock, what the sheep did, each rule's time, the memory held |
//! | `tool.rs` | the tools: each runs one of the others, prints and keeps what it gathers |

pub mod pasture;
pub mod throughput;
pub mod tool;
