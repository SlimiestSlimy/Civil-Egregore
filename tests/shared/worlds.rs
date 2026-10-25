//! Worlds as the tiers' tests make and read them: a folder beside no
//! `main.rs`, so no test program of its own -- a module of each tier
//! that uses it (`#[path = "../shared/worlds.rs"]`). A world is set up
//! and read inside its ticks alone, by the tests' probe
//! (`../shared/probe.rs`), through the public APIs.

// A tier uses what it needs of it.
#![allow(dead_code)]

use crate::probe::{entities_of_the_turn, of_the_turn, put_entity, Probe};
use utilities::coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use server::simulation::simulation_api::{cell_reads_and_writes, entity_reads_and_writes, tick_and_random_stream, EntityRef, LayerType, SimulationTickState};
use protocol::server_types::entity::EntityBaseData;
use protocol::server_types::attribute_layouts::Attribute;
use protocol::server_types::attributes::AttributeBlock;
use protocol::kinds::server::client_call::{AnswerCall, ClientCall, ClientCallLibrary};
use protocol::kinds::server::handles::ServerSetUp;
use basegame::worldgen::WorldGeneration;
use basegame::base_types::BaseTypes;
use protocol::type_registry::{in_bundle, FrozenTypeRegistry, RegisteredType, TypeRegistry};
use basegame::default_types::BUNDLE as BASE_BUNDLE;
use protocol::server_types::plane_widths::{Bits4, WidePlane};
use protocol::server_parameters::{CleanServerParameters, ParameterValue, FORCED_HOT, SIDE, THREADS, WORLD};
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

/// The IDs of the base game's types in a world of the probe's types
/// ([`Probe::types`]): the probe registered first, then the base game,
/// as in every such world.
pub fn base() -> BaseTypes {
    static PROBED: OnceLock<BaseTypes> = OnceLock::new();
    *PROBED.get_or_init(|| {
        let mut type_registry = TypeRegistry::for_new_world();
        Probe::default().register(&mut type_registry).expect("the probe");
        basegame::default_types::register(&mut type_registry).expect("the base game's types registered")
    })
}

/// The type of `frozen_type_registry` named `type_name`, if any: a type
/// a test registered found as anything would find it, by its name.
pub fn named<'frozen>(frozen_type_registry: &'frozen FrozenTypeRegistry, type_name: &str) -> Option<&'frozen RegisteredType> {
    frozen_type_registry.types().iter().find(|registered_type| registered_type.name() == type_name)
}

/// The base game's types, frozen: what initialization hands the server.
pub fn types() -> Arc<FrozenTypeRegistry> {
    registered_types().0.clone()
}

/// The IDs of the base game's types in a new world of its types alone,
/// as the game's own server registers them.
pub fn game_base() -> BaseTypes {
    registered_types().1
}

/// The tests' own bundle: every type of theirs is in it, registered after
/// the base game's.
pub const TESTS_BUNDLE: &str = "tests";

/// The tests' own type named `type_name` in their bundle, of `frozen_type_registry`, if any.
pub fn test_type_named<'frozen>(frozen_type_registry: &'frozen FrozenTypeRegistry, type_name: &str) -> Option<&'frozen RegisteredType> {
    named(frozen_type_registry, &in_bundle(TESTS_BUNDLE, type_name))
}

/// The tests' own plane of tiles: four bits a tile 8 by 8 cells,
/// registered after the base game's types -- what tiles are tried on
/// through ticks, saves and frames, the base game having none.
pub const TEST_TILES: &str = "test tiles";

/// Registers [`TEST_TILES`] in the tests' bundle.
pub fn test_tiles_registered(type_registry: &mut TypeRegistry) -> Result<(), String> {
    let mut tests = type_registry.open_bundle(TESTS_BUNDLE).map_err(|refusal| format!("{TESTS_BUNDLE}: {refusal:?}"))?;
    tests.register(WidePlane::<Bits4>::registration_of_tiles(TEST_TILES, 3)).map(drop).map_err(|refusal| format!("{TEST_TILES}: {refusal:?}"))
}

/// The tests' own function a client may call, belonging to the world
/// generation: it answers the words it was called with, last first.
pub const TEST_CALL: &str = "test call";

/// [`TEST_CALL`]'s code.
struct TestCall;

impl ClientCall for TestCall {
    /// Nothing of the world's taken.
    fn set_up(&self, _: &ServerSetUp) -> Result<Box<AnswerCall>, String> {
        Ok(Box::new(|_, arguments| Ok(arguments.iter().rev().copied().collect())))
    }
}

/// The base game's types and [`TEST_CALL`]: a game of the tests' own,
/// as initialization hands one the server.
pub fn types_with_test_call(type_registry: &mut TypeRegistry) -> Result<(), String> {
    basegame::default_types::register_types(type_registry)?;
    let mut tests = type_registry.open_bundle(TESTS_BUNDLE).map_err(|refusal| format!("{TESTS_BUNDLE}: {refusal:?}"))?;
    tests.register(protocol::type_registry::Registration::client_call(TEST_CALL, &in_bundle(BASE_BUNDLE, "world generation")).with_library(Arc::new(ClientCallLibrary(Box::new(TestCall))))).map(drop).map_err(|refusal| format!("{TEST_CALL}: {refusal:?}"))
}

/// The probe's types, the base game's and [`TEST_TILES`]: a game of the
/// tests' own, as initialization hands one the server.
pub fn types_with_test_tiles(type_registry: &mut TypeRegistry) -> Result<(), String> {
    probe_and_base_game(type_registry)?;
    test_tiles_registered(type_registry)
}

/// The plane of [`TEST_TILES`] of `world`.
pub fn test_tiles(world: &World) -> WidePlane<Bits4> {
    test_type_named(world.frozen_type_registry(), TEST_TILES).and_then(WidePlane::registered).expect("four bits a tile")
}

/// The ID of the base game's type named `type_name` in its bundle, in a
/// new world: a rule ticked alone is asked for by it.
pub fn type_id_named(type_name: &str) -> u64 {
    named(&types(), &in_bundle(BASE_BUNDLE, type_name)).unwrap_or_else(|| panic!("no type named {type_name}")).type_id()
}

/// `parameters` of a new world in a folder of its own for the test
/// `test_name`, emptied ([`folder`]), basegame's config written there as
/// `world_generation` says: what the world is made of.
pub fn in_folder(parameters: CleanServerParameters, test_name: &str, world_generation: WorldGeneration) -> CleanServerParameters {
    let folder = folder(test_name);
    world_generation.write_config(&folder.join(server::MOD_CONFIG).join(BASE_BUNDLE)).expect("the test world's config written");
    parameters.with(WORLD, ParameterValue::Text(folder.display().to_string()))
}

/// The folder `parameters` name ([`WORLD`]).
pub fn folder_of(parameters: &CleanServerParameters) -> PathBuf {
    PathBuf::from(parameters.text(WORLD).expect("a test world's folder named"))
}

/// A parameter given with nothing of its own -- forced hot, the camera
/// loading -- as a flag `[]` gives one.
pub fn given() -> ParameterValue {
    ParameterValue::Nested(CleanServerParameters::default())
}

/// The world saved in `folder`, run again -- its types `register_types`
/// frozen first as its types file says -- or what of the save is not
/// read.
pub fn opened(folder: &Path, register_types: protocol::world_types::RegisterTypes) -> Result<World, server::DiskError> {
    let frozen_type_registry = Arc::new(server::read_types(folder, register_types)?);
    server::run_world(folder, &frozen_type_registry, &CleanServerParameters::default()).map_err(|why_not| match why_not {
        server::WorldNotMade::Disk(disk_error) => disk_error,
        server::WorldNotMade::Refused(why_refused) => panic!("refused: {why_refused}"),
    })
}

/// The first seed from `first_seed_tried` on whose world, generated as
/// `world_generation` says, the base game finds land about the middle.
pub fn seed_with_land(first_seed_tried: u64, world_generation: &WorldGeneration) -> u64 {
    basegame::worldgen::seed_with_land(first_seed_tried, &world_generation.land_shape, utilities::coordinates::WORLD_MIDDLE_CELL)
}

/// A folder of its own for the test `test_name`, emptied: in the
/// workspace's `transient_data/test_saves/`.
pub fn folder(test_name: &str) -> PathBuf {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("transient_data").join("test_saves").join(test_name);
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// How a plain test world is made, in a folder of its own for the test
/// `test_name` -- a rule tried alone, the terrain taking no part: a plain
/// forced hot of at least `superchunks` superchunks -- a square of them
/// -- with grass on `grass_cover` of [`basegame::worldgen::FRACTION_ONE`]
/// of its cells and `sheep_per_flock` sheep on each superchunk --
/// generated with it --, on `thread_count` threads.
pub fn plain_start(test_name: &str, superchunks: u32, grass_cover: u64, sheep_per_flock: usize, thread_count: usize) -> CleanServerParameters {
    let parameters = CleanServerParameters::default().with(SIDE, ParameterValue::Integer(i64::from(utilities::coordinates::square_side(superchunks)))).with(FORCED_HOT, given()).with(THREADS, ParameterValue::Integer(thread_count as i64));
    in_folder(parameters, test_name, WorldGeneration { sheep_per_superchunk: sheep_per_flock as u64, ..WorldGeneration::plain(grass_cover) })
}

/// The default generation, with `sheep` sheep on each superchunk:
/// generated with it.
pub fn flocked(sheep: usize) -> WorldGeneration {
    WorldGeneration { sheep_per_superchunk: sheep as u64, ..WorldGeneration::DEFAULT }
}

/// A world made in the folder `parameters` name ([`in_folder`]), as they
/// say, of the base game's types, the probe's and those `more`
/// registers: the world, and its probe.
pub fn probed_world(parameters: &CleanServerParameters, more: impl FnOnce(&mut TypeRegistry) -> Result<(), String>) -> (World, Probe) {
    let probe = Probe::default();
    let world = server::make_world(&folder_of(parameters), parameters, &probe.types(more)).expect("the world made");
    (world, probe)
}

/// Nothing more of the tests' own to register.
pub fn nothing_more(_: &mut TypeRegistry) -> Result<(), String> {
    Ok(())
}

/// The superchunks of `world` hot, sorted, read in a tick of the probe alone.
pub fn hot_superchunks(world: &mut World, probe: &Probe) -> Vec<SuperchunkIndex> {
    probe.alone(world, |tick_state| vec![tick_and_random_stream::superchunk(tick_state)])
}

/// The top left superchunk of `world`'s hot ones, read in a tick of
/// the probe alone.
pub fn first_superchunk(world: &mut World, probe: &Probe) -> SuperchunkIndex {
    hot_superchunks(world, probe).into_iter().min_by_key(|superchunk| (superchunk.cartesian().1, superchunk.cartesian().0)).expect("a hot superchunk")
}

/// The cells of a layer on each of some chunks, as words: a chunk's cells
/// in Morton order ([`crate::probe::words_of_the_turn`]).
pub type ChunkWords = Vec<(utilities::coordinates::ChunkIndex, Vec<u64>)>;

/// The cells set and the cells cleared from `words_before` to `words_after`,
/// two readings of the words of the same hot chunks ([`crate::probe::words_of_the_turn`]).
pub fn cells_set_and_cleared(words_before: &ChunkWords, words_after: &ChunkWords) -> (u64, u64) {
    assert_eq!(words_before.len(), words_after.len(), "the same chunks hot");
    let word_pairs = words_before.iter().zip(words_after).flat_map(|((chunk, before), (same_chunk, after))| {
        assert_eq!(chunk, same_chunk);
        before.iter().zip(after)
    });
    word_pairs.fold((0, 0), |(set, cleared), (&word_before, &word_after)| (set + u64::from((word_after & !word_before).count_ones()), cleared + u64::from((word_before & !word_after).count_ones())))
}

/// The cells of the rectangle `width` by `height` whose top left is
/// `top_left_cartesian` that are of the turn's superchunk.
pub fn rectangle_in_the_turn(tick_state: &SimulationTickState, top_left_cartesian: CellCartesian, width: u32, height: u32) -> Vec<CellIndex> {
    (0..height).flat_map(|y| (0..width).map(move |x| CellCartesian { x: top_left_cartesian.x + x, y: top_left_cartesian.y + y }.into())).filter(|&cell| of_the_turn(tick_state, cell)).collect()
}

/// Sets `layer_type` on every cell of the rectangle `width` by `height`
/// whose top left is `top_left_cartesian` that does not hold it, in a
/// tick of the probe alone: grass planted, say.
pub fn set_rectangle(world: &mut World, probe: &Probe, layer_type: LayerType, top_left_cartesian: CellCartesian, width: u32, height: u32) {
    probe.alone(world, move |tick_state| {
        for cell in rectangle_in_the_turn(tick_state, top_left_cartesian, width, height) {
            if cell_reads_and_writes::layer_lacks(tick_state, layer_type, cell) {
                cell_reads_and_writes::set(tick_state, layer_type, cell);
            }
        }
        Vec::<()>::new()
    });
}

/// Puts each entity of `entities` -- its base data and its attributes -- in a
/// tick of the probe alone, as an entity that collides comes: woken
/// no sooner than the tick after.
pub fn put_entities(world: &mut World, probe: &Probe, entities: Vec<(EntityBaseData, Vec<AttributeBlock>)>) {
    probe.alone(world, move |tick_state| {
        let turn_superchunk = tick_and_random_stream::superchunk(tick_state);
        for (base_data, attributes) in entities.iter().filter(|(base_data, _)| base_data.stood_cell.superchunk() == turn_superchunk) {
            put_entity(tick_state, *base_data, attributes);
        }
        Vec::<()>::new()
    });
}

/// What `look` reads of every entity of `world`, in a tick of the probe
/// alone, in Morton order by cell.
pub fn every_entity<T: Send + 'static>(world: &mut World, probe: &Probe, look: impl Fn(&EntityRef) -> T + Send + Sync + 'static) -> Vec<T> {
    probe.alone(world, move |tick_state| entities_of_the_turn(tick_state).iter().map(&look).collect())
}

/// Cells of grass over `world`'s hot superchunks, read in a tick of the probe alone.
pub fn cells_of_grass(world: &mut World, probe: &Probe) -> u64 {
    let grass = base().grass;
    probe.alone(world, move |tick_state| vec![crate::probe::count_of_the_turn(tick_state, grass)]).into_iter().sum()
}

/// The name of a test's own attribute: a number a sheep is told from the
/// others by from a tick to the next -- an entity has no ID, and its
/// cell names it for a tick alone.
pub const SHEEP_TAG: &str = "sheep tag";

/// Registers [`SHEEP_TAG`] in the tests' bundle: with the probe's types,
/// a world whose sheep are tagged ([`tag_sheep`]).
pub fn sheep_tag_registered(type_registry: &mut TypeRegistry) -> Result<(), String> {
    let mut tests = type_registry.open_bundle(TESTS_BUNDLE).map_err(|refusal| format!("{TESTS_BUNDLE}: {refusal:?}"))?;
    tests.register(Attribute::<u64>::registration(SHEEP_TAG)).map(drop).map_err(|refusal| format!("{SHEEP_TAG}: {refusal:?}"))
}


/// The [`SHEEP_TAG`] attribute of `world`, found by its name among the world's types.
pub fn sheep_tag(world: &World) -> Attribute<u64> {
    test_type_named(world.frozen_type_registry(), SHEEP_TAG).and_then(Attribute::registered).expect("a world of the types with the sheep tag")
}

/// Gives every sheep of `world` a [`SHEEP_TAG`], each its own -- the
/// number of the cell it stands on -- in a tick of the probe alone. A
/// lamb born after has none.
pub fn tag_sheep(world: &mut World, probe: &Probe) {
    let sheep_tag = sheep_tag(world);
    probe.alone(world, move |tick_state| {
        for tagged_sheep in entities_of_the_turn(tick_state) {
            entity_reads_and_writes::set_attribute_of(tick_state, &tagged_sheep.base_data, sheep_tag, None, tagged_sheep.base_data.stood_cell.0);
        }
        Vec::<()>::new()
    });
}

/// The tag of `tagged_sheep`, its world's [`sheep_tag`], if it was given one.
pub fn tag_of(sheep_tag: Attribute<u64>, tagged_sheep: &EntityRef) -> Option<u64> {
    tagged_sheep.attribute(sheep_tag)
}

/// Every cell of the superchunk `superchunk`, row by row.
pub fn cells_of(superchunk: SuperchunkIndex) -> impl Iterator<Item = CellIndex> {
    let corner_cartesian = superchunk.top_left().cartesian();
    (0..SUPERCHUNK_SIDE_CELLS).flat_map(move |y| (0..SUPERCHUNK_SIDE_CELLS).map(move |x| CellCartesian { x: corner_cartesian.x + x, y: corner_cartesian.y + y }.into()))
}

/// One tick of the base game's rule named `rule_name` alone over
/// `world`'s hot superchunks, its halos left where they are: the tick's
/// report, the rule's counts alone in it.
pub fn tick_rule(world: &mut World, rule_name: &str) -> server::simulation::simulation_api::TickReport<server::simulation::simulation_api::rule_counts::RuleCounts> {
    let rule = crate::probe::rule_place(world, rule_name);
    crate::probe::report_of(world.tick_only(server::ChosenRules::from_rules(&[rule]), false), rule)
}

/// One tick of the sheep's rule alone.
pub fn tick_sheep(world: &mut World) -> server::simulation::simulation_api::TickReport<server::simulation::simulation_api::rule_counts::RuleCounts> {
    tick_rule(world, basegame::base_types::SHEEP_RULE)
}

/// One tick of the grass's rule alone.
pub fn tick_grass(world: &mut World) -> server::simulation::simulation_api::TickReport<server::simulation::simulation_api::rule_counts::RuleCounts> {
    tick_rule(world, basegame::base_types::GRASS_RULE)
}

/// The tick `world` is at, read in a tick of the probe alone: the tick
/// after it is the one it reads at, plus one.
pub fn tick_read_at(world: &mut World, probe: &Probe) -> u64 {
    probe.alone(world, |tick_state| vec![tick_and_random_stream::current_tick(tick_state)])[0]
}

/// The ticks after a flock is put ([`one_flock_world`]) before any of
/// it wakes: room for a test's reading and setting up in ticks of the
/// probe alone, which no sheep's rule runs in -- a sheep whose wake
/// passes so sleeps for good.
pub const FLOCK_SETTLES: u64 = 8;

/// A world made in a folder of its own for the test `test_name` as
/// `parameters` say -- given no side, with no end -- of the probe's types
/// and those `more` registers, its generation giving no sheep, and one
/// flock of `sheep` put at its middle by the
/// probe, the flock's halo kept hot: what a halo test follows, and a
/// world run long. A flock on every superchunk generated would grow a
/// world with no end for good. No sheep wakes before [`FLOCK_SETTLES`]
/// ticks on.
pub fn one_flock_world(test_name: &str, parameters: CleanServerParameters, sheep: usize, more: impl FnOnce(&mut TypeRegistry) -> Result<(), String>) -> (World, Probe) {
    use basegame::entity_rules::sheep::{STEP, TIME_BETWEEN_MEALS};
    let (mut world, probe) = probed_world(&in_folder(parameters, test_name, flocked(0)), more);
    let middle = utilities::coordinates::WORLD_MIDDLE;
    world.keep_hot(&server::halos_about([middle].into_iter()));
    // Cells of the middle drawn from the seed, each free of the collision plane as the tick found it and drawn once.
    let (report, put) = probe.tick(&mut world, &[], move |tick_state| {
        if tick_and_random_stream::superchunk(tick_state) != middle {
            return Vec::new();
        }
        let (corner, tick_now) = (middle.top_left().cartesian(), tick_and_random_stream::current_tick(tick_state));
        let mut random_stream = utilities::rng::RandomSource::for_stream(!tick_and_random_stream::random_stream(tick_state).next_draw(), middle.0);
        let mut drawn_cells = std::collections::HashSet::new();
        while drawn_cells.len() < sheep {
            let drawn_cell: CellIndex = CellCartesian { x: corner.x + random_stream.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32, y: corner.y + random_stream.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32 }.into();
            if cell_reads_and_writes::layer_holds(tick_state, base().collision, drawn_cell) || !drawn_cells.insert(drawn_cell) {
                continue;
            }
            let base_data = EntityBaseData { entity_type: base().sheep, stood_cell: drawn_cell, wake: STEP.part_drawn(&mut random_stream).tick_after(tick_now + FLOCK_SETTLES) };
            let sheep_state = basegame::base_types::SheepState { hungry_at: TIME_BETWEEN_MEALS.part_drawn(&mut random_stream).tick_after(tick_now), ..basegame::base_types::SheepState::default() };
            put_entity(tick_state, base_data, &protocol::server_types::attributes::WholeAttribute::holding_value(base().sheep_state, sheep_state));
        }
        vec![sheep]
    });
    assert_eq!((put, report.instructions_applied.added, report.instructions_applied.adds_refused), (vec![sheep], sheep, 0), "the flock put at the middle, hot");
    (world, probe)
}

/// The probe's types and the base game's, as a world of the probe's
/// types is opened again: a probe of its own, handed nothing.
pub fn probe_and_base_game(type_registry: &mut TypeRegistry) -> Result<(), String> {
    Probe::default().register(type_registry)?;
    basegame::default_types::register_types(type_registry)
}
