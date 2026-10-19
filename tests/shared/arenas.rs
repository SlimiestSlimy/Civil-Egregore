//! TEMPORARY: a copy of the server's `tests/shared/arenas.rs` -- hot
//! arenas of stone and walkers on them -- for the root's tests that
//! still build on the server's internals (`server::internals_for_root_tests`),
//! until the world has reads of its own (AI.txt, LATER).

// A tier uses what it needs of it.
#![allow(dead_code)]

use basegame::base_types::BaseTypes;
use protocol::type_registry::{Registration, TypeRegistry};
use server::internals_for_root_tests::{Attribute, BitmapArena, BucketKey, LayerCodec, WorldEntities, Write, WriteOp};
use server::simulation::simulation_api::{EntityType, Header, LayerType};
use utilities::coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use utilities::rng::RandomSource;

/// Types registered between the base game's and a test's own, at most:
/// as many as its seed draws, for its types' IDs to be other with
/// every seed.
const MOST_FILLERS: u64 = 64;

/// The arenas' tests' types: the base game's, registered first as a new
/// world's are, and their own after, by each test in a registry of its
/// own ([`TestTypes::seeded`]) -- never a number by hand.
#[derive(Clone, Copy)]
pub struct TestTypes {
    /// The base game's: the same IDs as every new world's.
    pub base: BaseTypes,
    /// The plane the arena holds: what walkers eat, lay and seek.
    pub stone: LayerType,
    /// The walkers' type.
    pub walker: EntityType,
    /// What a walker counts: its meals, and what its neighbours count on it.
    pub counted: Attribute<u64>,
    /// The stones a walker laid: counted only if the stone was.
    pub laid: Attribute<u64>,
    /// The walkers a walker made: counted only if the walker was.
    pub made: Attribute<u64>,
}

impl TestTypes {
    /// Every one registered in a registry of a test's own, of `seed`:
    /// the base game's, as many fillers as it draws, then these.
    pub fn seeded(seed: u64) -> Self {
        let mut type_registry = TypeRegistry::for_new_world();
        let base = basegame::default_types::register(&mut type_registry).expect("the base game's types registered");
        let mut type_registry = type_registry.open_bundle("tests").expect("the tests' bundle");
        for filler in 0..RandomSource::new(seed ^ 0x7e57_7e57).draw_below(MOST_FILLERS + 1) {
            type_registry.register(Registration::layer(&format!("filler {filler}"))).expect("a filler of its own name");
        }
        let mut register = |registration: Registration| type_registry.register(registration).expect("a test's type, of a name no other has");
        let stone = register(Registration::layer("stone")).layer_type().expect("a layer");
        let walker = register(Registration::entity("walker", None)).entity_type().expect("a type of entity");
        let mut attribute = |name: &str| Attribute::registered(&register(Attribute::<u64>::registration(name))).expect("a word's attribute");
        let (counted, laid, made) = (attribute("counted"), attribute("laid"), attribute("made"));
        Self { base, stone, walker, counted, laid, made }
    }
}

/// An arena with `types.stone` hot over the `side_superchunks` by `side_superchunks` superchunks from
/// `(10, 10)`, the cells `cells` set.
pub fn stone_arena(types: TestTypes, side_superchunks: u32, cells: impl Iterator<Item = CellIndex>) -> BitmapArena {
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    for (x, y) in (10..10 + side_superchunks).flat_map(|y| (10..10 + side_superchunks).map(move |x| (x, y))) {
        for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
            bitmap_arena.make_hot(BucketKey { layer_type: types.stone, chunk }, None, &mut layer_codec);
        }
    }
    cells.for_each(|cell| bitmap_arena.queue_write(types.stone, Write::cell(cell, WriteOp::SetCell)));
    assert_eq!(bitmap_arena.apply().missed, 0);
    bitmap_arena
}

/// [`bitmap_arena`], no cell set, and entities holding the same superchunks.
pub fn world(types: TestTypes, side_superchunks: u32) -> (BitmapArena, WorldEntities) {
    let (bitmap_arena, mut entity_store) = (stone_arena(types, side_superchunks, std::iter::empty()), WorldEntities::new());
    assert_eq!(entity_store.keep_superchunks(&bitmap_arena.superchunk_indices()), 0);
    (bitmap_arena, entity_store)
}

/// The cell `(x, y)` cells from the top left of the superchunk `(10, 10)`.
pub fn cell(x: u32, y: u32) -> CellIndex {
    CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + x, y: 10 * SUPERCHUNK_SIDE_CELLS + y }.into()
}

/// A walker on `walker_cell`, waking at `wake`.
pub fn walker_header(types: TestTypes, walker_cell: CellIndex, wake: u64) -> Header {
    Header { entity_type: types.walker, stood_cell: walker_cell, wake }
}
