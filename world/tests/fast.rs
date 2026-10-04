//! The fast tier: small worlds grown from a seed and run for a few thousand ticks, judged -- seconds.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fast`

#[path = "fast/halos.rs"]
mod halos;
#[path = "fast/world.rs"]
mod world;

/// A seed whose world has land about its origin, the `nth` such the
/// run's tests ask for: found from the crate's seed, rolled every few
/// runs (`utilities::seed`), so that nothing passes on one seed alone.
fn land_seed(nth: u64) -> u64 {
    let (shape, middle) = (terrain::Shape::DEFAULT, coordinates::WORLD_MIDDLE.top_left().cartesian());
    let land = |seed: &u64| {
        let mut lands = terrain::mesh::Lands::new(&shape, *seed);
        (-3i32..=3).all(|across| (-3i32..=3).all(|down| lands.height(middle.x.wrapping_add_signed(across * 1024), middle.y.wrapping_add_signed(down * 1024)) > shape.ocean))
    };
    (utilities::seed::counted().wrapping_add(nth.wrapping_mul(1_000_003))..).find(land).expect("a seed with land about the origin")
}
