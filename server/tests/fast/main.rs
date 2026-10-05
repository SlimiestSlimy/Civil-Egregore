//! The fast tier: small worlds grown from a seed and run for a few thousand ticks, judged -- seconds.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fast`

mod commands;
mod halos;
mod world;

/// A seed whose world has land about its origin, the `nth` such the
/// run's tests ask for: found from the crate's seed, rolled every few
/// runs (`utilities::seed`), so that nothing passes on one seed alone.
fn land_seed(nth: u64) -> u64 {
    ::server::seed_with_land(utilities::seed::counted().wrapping_add(nth.wrapping_mul(1_000_003)), &worldgen::Shape::DEFAULT)
}
