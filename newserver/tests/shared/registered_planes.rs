//! Plane types for tests, one of every shape, registered as any type
//! is, in a registry of the test's own ([`TestPlanes::registered`]): after
//! a seed-drawn number of fillers, so their IDs differ from seed to seed
//! and no test counts on one.

// A tier uses what it needs of it.
#![allow(dead_code)]

use protocol::type_registry::{BundleSide, PlaneTileSize, PlaneType, PlaneWidth, Registration, TypeRegistry};
use utilities::rng::RandomSource;

/// The most fillers registered before a test's own planes.
const MOST_FILLERS: u64 = 64;

/// Every width a plane has.
const WIDTHS: [PlaneWidth; 6] = [PlaneWidth::Bits1, PlaneWidth::Bits2, PlaneWidth::Bits4, PlaneWidth::Bits8, PlaneWidth::Bits16, PlaneWidth::Bits32];

/// Every tile size a plane has.
const TILE_SIZES: [PlaneTileSize; 9] = [PlaneTileSize::Side1, PlaneTileSize::Side2, PlaneTileSize::Side4, PlaneTileSize::Side8, PlaneTileSize::Side16, PlaneTileSize::Side32, PlaneTileSize::Side64, PlaneTileSize::Side128, PlaneTileSize::Side256];

/// A plane of every shape -- every width at every tile size -- and one
/// more of a bit a cell.
#[derive(Clone, Copy)]
pub struct TestPlanes {
    /// A bit a cell.
    pub one_bit: PlaneType,
    /// Another, for reads of two planes at once.
    pub other_bit: PlaneType,
    /// 4 bits a cell.
    pub four_bits: PlaneType,
    /// One of every shape, `one_bit` and `four_bits` among them.
    every_shape: [PlaneType; WIDTHS.len() * TILE_SIZES.len()],
}

impl TestPlanes {
    /// The planes registered in a registry of their own, after the
    /// fillers `seed` draws.
    pub fn registered(seed: u64) -> Self {
        let (mut type_registry, mut seeded_random) = (TypeRegistry::for_new_world(), RandomSource::new(seed ^ 0x9_1a7e));
        let mut type_registry = type_registry.open_bundle("tests", Some(BundleSide::Server), &[]).expect("the tests' bundle");
        let mut register = |name: &str, width, tile_size| type_registry.register(Registration::plane(name, width, tile_size)).expect("a test's plane, of a name no other has").plane_type().expect("a plane");
        for filler in 0..seeded_random.draw_below(MOST_FILLERS + 1) {
            register(&format!("filler {filler}"), PlaneWidth::Bits1, PlaneTileSize::Side1);
        }
        let every_shape: [PlaneType; WIDTHS.len() * TILE_SIZES.len()] = std::array::from_fn(|shape| {
            let (width, tile_size) = (WIDTHS[shape / TILE_SIZES.len()], TILE_SIZES[shape % TILE_SIZES.len()]);
            register(&format!("{} bits a tile {} a side", width.bits(), tile_size.side()), width, tile_size)
        });
        let other_bit = register("other bit", PlaneWidth::Bits1, PlaneTileSize::Side1);
        let shape_of = |width, tile_size| *every_shape.iter().find(|plane_type| (plane_type.width(), plane_type.tile_size()) == (width, tile_size)).expect("every shape");
        Self { one_bit: shape_of(PlaneWidth::Bits1, PlaneTileSize::Side1), other_bit, four_bits: shape_of(PlaneWidth::Bits4, PlaneTileSize::Side1), every_shape }
    }

    /// One of every shape.
    pub fn every(&self) -> &[PlaneType] {
        &self.every_shape
    }
}

/// The largest number a cell -- or a tile -- of `plane_type` holds.
pub fn most_value(plane_type: PlaneType) -> u32 {
    (u64::MAX >> (u64::BITS - plane_type.width().bits())) as u32
}
