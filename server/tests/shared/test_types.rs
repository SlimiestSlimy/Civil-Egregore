//! The server's tests' own types, registered as any type is, by each
//! test in a registry of its own ([`TestTypes::seeded`]): never a
//! number by hand, no two of them alike, and other IDs with every seed.

// A tier uses what it needs of it.
#![allow(dead_code)]

use crate::bitplane_manager::{Bits16, Bits2, Bits4, Bits8, WidePlane};
use crate::chunk_storage::LayerType;
use crate::entity_manager::{Attribute, EntityType};
use protocol::type_registry::{Registration, TypeRegistry};
use utilities::rng::RandomSource;

/// Types registered before a test's own, at most: as many as its seed
/// draws, for its types' IDs to be other with every seed.
const MOST_FILLERS: u64 = 64;

/// The server's tests' types.
#[derive(Clone, Copy)]
pub struct TestTypes {
    /// The plane the arenas hold: what walkers seek and stand off.
    pub stone: LayerType,
    /// The mock superchunk's dirt.
    pub dirt: LayerType,
    /// The mock superchunk's grass.
    pub grass: LayerType,
    /// What stands on a cell, as walking asks it.
    pub collision: LayerType,
    /// A wall on a cell's north side.
    pub wall_north: LayerType,
    /// A wall on a cell's west side.
    pub wall_west: LayerType,
    /// A wall on a cell's south side.
    pub wall_south: LayerType,
    /// A wall on a cell's east side.
    pub wall_east: LayerType,
    /// Planes a bit a cell that are nothing but themselves: what storage
    /// is tried on. Their IDs' order is the registry's to choose; a test
    /// asking for type order sorts them.
    pub layers: [LayerType; 10],
    /// A plane of two bits a cell.
    pub two_bits: WidePlane<Bits2>,
    /// A plane of four bits a cell.
    pub four_bits: WidePlane<Bits4>,
    /// A plane of eight bits a cell.
    pub eight_bits: WidePlane<Bits8>,
    /// A plane of sixteen bits a cell.
    pub sixteen_bits: WidePlane<Bits16>,
    /// A plane of four bits a tile 8 by 8 cells.
    pub four_bit_tiles: WidePlane<Bits4>,
    /// A plane of sixteen bits a tile 8 by 8 cells.
    pub sixteen_bit_tiles: WidePlane<Bits16>,
    /// The walkers' type.
    pub walker: EntityType,
    /// An attribute counting the times an entity woke.
    pub woken_count: Attribute<u64>,
    /// An attribute present every other time an entity woke.
    pub odd_wake: Attribute<u64>,
    /// An attribute a test tells its entities apart by: a number each.
    pub entity_number: Attribute<u64>,
    /// An attribute naming an entity.
    pub name: Attribute<u64>,
    /// An attribute an entity is marked with.
    pub mark: Attribute<u64>,
    /// An attribute an entity is scarred with.
    pub scar: Attribute<u64>,
    /// An attribute an entity gives itself.
    pub own: Attribute<u64>,
    /// An attribute saying an entity was fed.
    pub fed: Attribute<u64>,
    /// Attributes others give an entity, one each.
    pub given: [Attribute<u64>; 8],
}

impl TestTypes {
    /// Every one registered in a registry of a test's own, of `seed`: as
    /// many types as it draws first, then these, in the order their
    /// fields are, but `layers` in an order drawn.
    pub fn seeded(seed: u64) -> Self {
        let (mut type_registry, mut seeded_random) = (TypeRegistry::for_new_world(), RandomSource::new(seed ^ 0x7e57_7e57));
        let mut type_registry = type_registry.open_bundle("tests").expect("the tests' bundle");
        for filler in 0..seeded_random.draw_below(MOST_FILLERS + 1) {
            type_registry.register(Registration::layer(&format!("filler {filler}"))).expect("a filler of its own name");
        }
        let mut register = |registration: Registration| type_registry.register(registration).expect("a test's type, of a name no other has");
        let mut layer = |name: &str| register(Registration::layer(name)).layer_type().expect("a layer");
        let (stone, dirt, grass, collision) = (layer("stone"), layer("dirt"), layer("grass"), layer("collision"));
        let (wall_north, wall_west, wall_south, wall_east) = (layer("wall north"), layer("wall west"), layer("wall south"), layer("wall east"));
        // In an order drawn: no test may count on `layers[0]` being of the lowest ID.
        let mut registration_order: [usize; 10] = std::array::from_fn(|place| place);
        for last in (1..registration_order.len()).rev() {
            registration_order.swap(last, seeded_random.draw_below(last as u64 + 1) as usize);
        }
        let mut layers = [stone; 10];
        for place in registration_order {
            layers[place] = layer(&format!("layer {place}"));
        }
        let two_bits = WidePlane::registered(&register(WidePlane::<Bits2>::registration("two bits"))).expect("two bits a cell");
        let four_bits = WidePlane::registered(&register(WidePlane::<Bits4>::registration("four bits"))).expect("four bits a cell");
        let eight_bits = WidePlane::registered(&register(WidePlane::<Bits8>::registration("eight bits"))).expect("eight bits a cell");
        let sixteen_bits = WidePlane::registered(&register(WidePlane::<Bits16>::registration("sixteen bits"))).expect("sixteen bits a cell");
        let four_bit_tiles = WidePlane::registered(&register(WidePlane::<Bits4>::registration_of_tiles("four bit tiles", 3))).expect("four bits a tile");
        let sixteen_bit_tiles = WidePlane::registered(&register(WidePlane::<Bits16>::registration_of_tiles("sixteen bit tiles", 3))).expect("sixteen bits a tile");
        let walker = register(Registration::entity("walker", None)).entity_type().expect("a type of entity");
        let mut attribute = |name: &str| Attribute::registered(&register(Attribute::<u64>::registration(name))).expect("a word's attribute");
        let (woken_count, odd_wake, entity_number) = (attribute("woken count"), attribute("odd wake"), attribute("entity number"));
        let (name, mark, scar, own, fed) = (attribute("name"), attribute("mark"), attribute("scar"), attribute("own"), attribute("fed"));
        let given = std::array::from_fn(|giver| attribute(&format!("given {giver}")));
        Self { stone, dirt, grass, collision, wall_north, wall_west, wall_south, wall_east, layers, two_bits, four_bits, eight_bits, sixteen_bits, four_bit_tiles, sixteen_bit_tiles, walker, woken_count, odd_wake, entity_number, name, mark, scar, own, fed, given }
    }
}
