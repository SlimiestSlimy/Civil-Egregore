//! The fine tier: each test pins one behaviour, at the edges written by hand and at cases drawn from the run's seed -- instant.
//!
//! `cargo test --test fine`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;

#[path = "fine/applier.rs"]
mod applier;
#[path = "fine/hot_chunk.rs"]
mod hot_chunk;
#[path = "fine/sampling.rs"]
mod sampling;
