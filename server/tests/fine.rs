//! The fine tier: each test pins one behaviour, at the edges written by hand and at cases drawn from the run's seed -- instant.
//!
//! `cargo test --test fine`

#[allow(dead_code)] // a shared file: each test program uses a part of it
#[path = "../../utilities/test_seed.rs"]
mod test_seed;

#[path = "fine/hot_chunk.rs"]
mod hot_chunk;
#[path = "fine/plane_sampling.rs"]
mod plane_sampling;
#[path = "fine/queue_applier.rs"]
mod queue_applier;
