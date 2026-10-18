//! Worlds as the tiers' tests make and read them: a folder beside no
//! `main.rs`, so no test program of its own -- a module of each tier
//! that uses it (`#[path = "../shared/worlds.rs"]`).

// TEMPORARY: reaches the server's internals -- `server::internals_for_root_tests`
// and the world's handles to them -- until the world has reads of its
// own and this is rewritten against them (AI.txt, LATER).

// A tier uses what it needs of it.
#![allow(dead_code)]

use server::internals_for_root_tests::SuperchunkImage;
use utilities::coordinates::SuperchunkIndex;
use server::internals_for_root_tests::{set_attribute, Attribute, AttributeBlock, EntityRef, Header};
use basegame::worldgen::WorldGeneration;
use basegame::base_types::BaseTypes;
use protocol::type_registry::{FrozenTypeRegistry, TypeRegistry};
use server::simulation::simulation_api::{Bits4, WidePlane};
use protocol::server_parameters::{CleanServerParameters, ParameterValue, FORCED_HOT, SIDE, THREADS, WORLD, WORLDGEN};
use server::World;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// The base game's types of a new world, registered and frozen once for
/// every test of a tier -- what initialization hands the server -- and
/// the IDs registering them gave each: a new world's IDs are handed out
/// in registration order, so every new world's are these.
fn registered_types() -> &'static (Arc<FrozenTypeRegistry>, BaseTypes) {
    static REGISTERED: OnceLock<(Arc<FrozenTypeRegistry>, BaseTypes)> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let mut type_registry = TypeRegistry::for_new_world();
        let base_types = basegame::default_types::register(&mut type_registry).expect("the base game's types registered");
        (Arc::new(type_registry.freeze().expect("the base game's types frozen")), base_types)
    })
}

/// The base game's types, frozen: what initialization hands the server.
pub fn types() -> Arc<FrozenTypeRegistry> {
    registered_types().0.clone()
}

/// The IDs of the base game's types in a new world.
pub fn base() -> BaseTypes {
    registered_types().1
}

/// The tests' own plane of tiles: four bits a tile 8 by 8 cells,
/// registered after the base game's types -- what tiles are tried on
/// through ticks, saves and frames, the base game having none.
pub const TEST_TILES: &str = "test tiles";

/// The base game's types and [`TEST_TILES`]: a game of the tests' own,
/// as initialization hands one the server.
pub fn types_with_test_tiles(type_registry: &mut TypeRegistry) -> Result<(), String> {
    basegame::default_types::register_types(type_registry)?;
    type_registry.register(WidePlane::<Bits4>::registration_of_tiles(TEST_TILES, 3)).map(drop).map_err(|refusal| format!("{TEST_TILES}: {refusal:?}"))
}

/// The types of [`types_with_test_tiles`], frozen for a new world, and
/// the plane of tiles among them.
pub fn with_test_tiles() -> (Arc<FrozenTypeRegistry>, WidePlane<Bits4>) {
    let frozen_type_registry = protocol::world_types::of_a_new_world(types_with_test_tiles).expect("the tests' types");
    let test_tiles = frozen_type_registry.types().iter().find(|registered| registered.name() == TEST_TILES).and_then(WidePlane::registered).expect("four bits a tile");
    (Arc::new(frozen_type_registry), test_tiles)
}

/// The ID of the base game's type named `type_name`, in a new world:
/// a rule ticked alone is asked for by it.
pub fn type_id_named(type_name: &str) -> u64 {
    types().types().iter().find(|registered_type| registered_type.name() == type_name).unwrap_or_else(|| panic!("no type named {type_name}")).type_id()
}

/// Worldgen parameters, each by its name, as [`WORLDGEN`] holds them.
pub fn worldgen(worldgen_parameters: Vec<(String, u64)>) -> ParameterValue {
    ParameterValue::Nested(worldgen_parameters.into_iter().fold(CleanServerParameters::default(), |worldgen, (parameter_name, value)| worldgen.with(parameter_name, ParameterValue::Hex(value))))
}

/// A parameter given with nothing of its own -- forced hot, the camera
/// loading -- as a flag `[]` gives one.
pub fn given() -> ParameterValue {
    ParameterValue::Nested(CleanServerParameters::default())
}

/// The parameters of the world saved in `folder` ([`WORLD`]).
pub fn world_at(folder: &Path) -> CleanServerParameters {
    CleanServerParameters::default().with(WORLD, ParameterValue::Text(folder.display().to_string()))
}

/// The world saved in `folder`, started again -- its types `register_types`
/// frozen first as its types file says -- or what of the save is not
/// read.
pub fn opened(folder: &Path, register_types: protocol::world_types::RegisterTypes) -> Result<World, server::DiskError> {
    let frozen_type_registry = Arc::new(server::read_types(folder, register_types)?);
    server::start_world(&world_at(folder), &frozen_type_registry).map_err(|why_not| match why_not {
        server::WorldNotStarted::Unreadable(disk_error) => disk_error,
        server::WorldNotStarted::Refused(why_refused) => panic!("refused: {why_refused}"),
        server::WorldNotStarted::Unsaved(disk_error) => panic!("an opened world saved: {disk_error}"),
    })
}

/// The first seed from `first_seed_tried` on whose world, generated as
/// `world_generation` says, the base game finds land about the middle.
pub fn seed_with_land(first_seed_tried: u64, world_generation: &WorldGeneration) -> u64 {
    server::seed_fit_to_start(first_seed_tried, &world_generation.worldgen_parameters(), &basegame::worldgen::world_generation::code(base()))
}

/// A folder of its own for the test `test_name`, emptied: in the
/// workspace's `transient_data/test_saves/`.
pub fn folder(test_name: &str) -> PathBuf {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("transient_data").join("test_saves").join(test_name);
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// Every hot cell of grass and dirt, every entity with its attributes,
/// the tick, every random stream, every cold superchunk's image and
/// kept state, and the superchunks warming and cooling with when each is due: what
/// two worlds the same hold the same.
pub type WorldContents = (Vec<u64>, Vec<(Header, Vec<AttributeBlock>)>, u64, Vec<(SuperchunkIndex, u64)>, Vec<(SuperchunkIndex, SuperchunkImage, Vec<u64>)>, Vec<(SuperchunkIndex, u64)>, Vec<(SuperchunkIndex, u64)>);

/// [`WorldContents`] `world` holds.
pub fn world_contents(world: &World) -> WorldContents {
    let cells = world.world_info().layers.clone().into_iter().flat_map(|layer| world.bitmap_arena().buckets_of(layer)).flat_map(|(_, bucket)| bucket.words().to_vec()).collect();
    let entities_with_attributes = world.entity_store().iter().map(|entity| (entity.header, entity.attributes.to_vec())).collect();
    let cold = world.cold().iter().map(|(&superchunk, words)| (superchunk, SuperchunkImage::clone(&world.chunk_storage().shared_image(superchunk).expect("a cold superchunk's image")), words.clone())).collect();
    (cells, entities_with_attributes, world.entity_store().current_tick(), world.world_simulation().random_states().collect(), cold, world.warming().collect(), world.cooling().collect())
}

/// How a plain test world is started -- a rule tried alone, the
/// terrain taking no part: a plain forced hot of at least
/// `superchunks` superchunks -- a square of them -- with grass on
/// `grass_cover` of [`basegame::worldgen::FRACTION_ONE`] of its cells and
/// `sheep_per_flock` sheep on each superchunk -- made the first tick --, on
/// `thread_count` threads.
pub fn plain_start(superchunks: u32, grass_cover: u64, sheep_per_flock: usize, thread_count: usize) -> CleanServerParameters {
    let worldgen_parameters = WorldGeneration { sheep_per_superchunk: sheep_per_flock as u64, ..WorldGeneration::plain(grass_cover) }.worldgen_parameters();
    CleanServerParameters::default().with(WORLDGEN, worldgen(worldgen_parameters)).with(SIDE, ParameterValue::Integer(i64::from(utilities::coordinates::square_side(superchunks)))).with(FORCED_HOT, given()).with(THREADS, ParameterValue::Integer(thread_count as i64))
}

/// The numbers of the default generation, with `sheep` sheep on each
/// superchunk: made the first tick it is hot.
pub fn flocked(sheep: usize) -> Vec<(String, u64)> {
    WorldGeneration { sheep_per_superchunk: sheep as u64, ..WorldGeneration::DEFAULT }.worldgen_parameters()
}

/// A world started as `parameters` say -- given no side, with no end -- and one flock of
/// `sheep` at its origin, the flock's halo hot: what a halo test follows.
/// Its generation gives no sheep -- a flock on every superchunk
/// generated would grow a world with no end for good -- so the one
/// flock is put between ticks ([`put_flock`]).
pub fn one_flock_world(parameters: CleanServerParameters, sheep: usize) -> World {
    let mut world = server::start_world(&parameters.with(WORLDGEN, worldgen(flocked(0))), &types()).expect("the world started");
    put_flock(&mut world, utilities::coordinates::WORLD_MIDDLE, sheep);
    world.keep_hot(&server::halos_about([utilities::coordinates::WORLD_MIDDLE].into_iter()));
    world
}

/// The top left superchunk of `world`'s hot ones.
pub fn first_superchunk(world: &World) -> SuperchunkIndex {
    world.bitmap_arena().superchunk_indices().into_iter().min_by_key(|superchunk| (superchunk.cartesian().1, superchunk.cartesian().0)).expect("a hot superchunk")
}

/// The cells of `layer_type` on every hot chunk of `world`, as words: what
/// two of, a tick apart, tell what the tick did to the layer
/// ([`cells_set_and_cleared`]).
pub fn layer_words(world: &World, layer_type: protocol::type_registry::LayerType) -> Vec<(utilities::coordinates::ChunkIndex, Vec<u64>)> {
    world.bitmap_arena().buckets_of(layer_type).map(|(chunk, bucket)| (chunk, bucket.cells().to_vec())).collect()
}

/// The cells set and the cells cleared from `words_before` to `words_after`,
/// two [`layer_words`] of the same hot chunks.
pub fn cells_set_and_cleared(words_before: &[(utilities::coordinates::ChunkIndex, Vec<u64>)], words_after: &[(utilities::coordinates::ChunkIndex, Vec<u64>)]) -> (u64, u64) {
    assert_eq!(words_before.len(), words_after.len(), "the same chunks hot");
    let word_pairs = words_before.iter().zip(words_after).flat_map(|((chunk, before), (same_chunk, after))| {
        assert_eq!(chunk, same_chunk);
        before.iter().zip(after)
    });
    word_pairs.fold((0, 0), |(set, cleared), (&word_before, &word_after)| (set + u64::from((word_after & !word_before).count_ones()), cleared + u64::from((word_before & !word_after).count_ones())))
}

/// Cells of grass over `world`'s hot superchunks.
pub fn cells_of_grass(world: &World) -> u64 {
    world.bitmap_arena().superchunk_indices().into_iter().map(|superchunk| world.bitmap_arena().superchunk_count(base().grass, superchunk) as u64).sum()
}

/// Turns to grass the rectangle of cells `width` by `height` whose top
/// left is `top_left_cartesian`, all of it on `world`'s hot superchunks.
pub fn plant_grass(world: &mut World, top_left_cartesian: utilities::coordinates::CellCartesian, width: u8, height: u8) {
    use server::internals_for_root_tests::{WriteShape, Write, WriteOp};
    let write_shape = if (width, height) == (1, 1) { WriteShape::Cell } else { WriteShape::RectangleOfCells { width, height } };
    let planted_writes = world.write_cells(base().grass, [Write { anchor_cell: top_left_cartesian.into(), write_op: WriteOp::SetCell, write_shape }]);
    assert_eq!(planted_writes.missed, 0, "grass planted off the hot superchunks");
}

/// Puts an entity on `world`, whole, its cell held in the collision
/// plane -- as an entity that collides holds it: there before the next
/// tick.
pub fn add_entity(world: &mut World, header: Header, attributes: &[AttributeBlock]) -> server::internals_for_root_tests::InstructionsApplied {
    let applied = world.add_entity(header, attributes);
    if applied.added > 0 {
        world.write_cells(base().collision, [server::internals_for_root_tests::Write::cell(header.stood_cell, server::internals_for_root_tests::WriteOp::SetCell)]);
    }
    applied
}

/// Puts `count` sheep on cells of `superchunk` drawn from the seed,
/// each free of the collision plane, between ticks: one flock on a
/// world whose generation gives none ([`flocked`] 0) -- a halo test's
/// one flock wandering a world with no end, which a flock on every
/// superchunk generated would grow for good.
pub fn put_flock(world: &mut World, superchunk: SuperchunkIndex, count: usize) {
    use utilities::coordinates::{CellCartesian, SUPERCHUNK_SIDE_CELLS};
    use basegame::entity_rules::sheep::{STEP, TIME_BETWEEN_MEALS};
    let mut random_stream = utilities::rng::RandomSource::for_stream(!world.world_info().seed, superchunk.0);
    let (corner, side_in_cells, tick_now) = (superchunk.top_left().cartesian(), SUPERCHUNK_SIDE_CELLS as u64, world.entity_store().current_tick());
    let mut put = 0;
    while put < count {
        let drawn_cell = CellCartesian { x: corner.x + random_stream.draw_below(side_in_cells) as u32, y: corner.y + random_stream.draw_below(side_in_cells) as u32 }.into();
        if world.bitmap_arena().layer_holds_cell(base().collision, drawn_cell) == Ok(true) {
            continue;
        }
        let header = Header { entity_type: base().sheep, stood_cell: drawn_cell, wake: STEP.part_drawn(&mut random_stream).tick_after(tick_now) };
        let sheep_state = basegame::base_types::SheepState { hungry_at: TIME_BETWEEN_MEALS.part_drawn(&mut random_stream).tick_after(tick_now), ..basegame::base_types::SheepState::default() };
        put += add_entity(world, header, &server::internals_for_root_tests::WholeAttribute::holding_value(base().sheep_state, sheep_state)).added;
    }
}

/// Takes `header`'s entity off `world`, the cell it held in the collision
/// plane let go.
pub fn remove_entity(world: &mut World, header: &Header) -> server::internals_for_root_tests::InstructionsApplied {
    let remove_applied = world.remove_entity(header);
    if remove_applied.entities_removed > 0 {
        world.write_cells(base().collision, [server::internals_for_root_tests::Write::cell(header.stood_cell, server::internals_for_root_tests::WriteOp::ClearCell)]);
    }
    remove_applied
}

/// A test's own attribute: a number a sheep is told from the others
/// by from a tick to the next -- an entity has no ID, and its cell
/// names it for a tick alone.
pub const SHEEP_TAG: Attribute<u64> = Attribute::new(1 << 20);

/// Gives every sheep of `world` a [`SHEEP_TAG`], each its own: taken off the
/// world and put back with it. A lamb born after has none.
pub fn tag_sheep(world: &mut World) {
    let every_sheep: Vec<(Header, Vec<AttributeBlock>)> = world.entity_store().iter().map(|one_sheep| (one_sheep.header, one_sheep.attributes.to_vec())).collect();
    for (tag_number, (header, mut attributes)) in every_sheep.into_iter().enumerate() {
        set_attribute(&mut attributes, SHEEP_TAG, tag_number as u64);
        assert_eq!(world.remove_entity(&header).entities_removed, 1);
        assert_eq!(world.add_entity(header, &attributes).added, 1);
    }
}

/// The [`SHEEP_TAG`] of `tagged_sheep`, if it was given one.
pub fn tag_of(tagged_sheep: &EntityRef) -> Option<u64> {
    tagged_sheep.attribute(SHEEP_TAG)
}

/// One tick of the rule whose type's ID is `rule_id` alone over `world`'s hot
/// superchunks, its halos left where they are: the tick's report, the
/// rule's counts alone in it.
pub fn tick_rule(world: &mut World, rule_id: u64) -> server::internals_for_root_tests::TickReport<server::simulation::simulation_api::RuleCounts> {
    let rule = world.rule_place(rule_id);
    let report = world.tick_only(server::ChosenRules::from_rules(&[rule]), false);
    server::internals_for_root_tests::TickReport { writes_applied: report.writes_applied, instructions_applied: report.instructions_applied, instructions_compared: report.instructions_compared, counted_when_applied: report.counted_when_applied, rules: report.rules.counts_of(rule), computing_time: report.computing_time, applying_time: report.applying_time, acts_over_a_border: report.acts_over_a_border }
}

/// One tick of the sheep's rule alone over `world`'s hot superchunks,
/// its halos left where they are.
pub fn tick_sheep(world: &mut World) -> server::internals_for_root_tests::TickReport<server::simulation::simulation_api::RuleCounts> {
    tick_rule(world, type_id_named(basegame::base_types::SHEEP_RULE))
}

/// One tick of the grass's rule alone over `world`'s hot superchunks.
pub fn tick_grass(world: &mut World) -> server::internals_for_root_tests::TickReport<server::simulation::simulation_api::RuleCounts> {
    tick_rule(world, type_id_named(basegame::base_types::GRASS_RULE))
}
