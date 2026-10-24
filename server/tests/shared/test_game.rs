//! A game of the server's tests' own ([`TestGame`]): walkers wandering
//! a world of scattered stone, leaving a trail, born and dying -- made,
//! stepped and taken away through the simulation's API as any game's
//! rules are -- and keeping the world hot about them. What the tests of
//! whole worlds run: halos, saves and loads, paging.

// A tier uses what it needs of it.
#![allow(dead_code)]

use crate::collision::Collision;
use crate::simulation::simulation_api::{cell_and_its_neighbours, entity_reads_and_writes, tick_and_random_stream, CellCartesian, CellIndex, EntityRef, EntityType, GeneratingSuperchunk, LayerType, RandomSource, RuleCode, SetUpWorldgen, SimulationTickState, WorldgenCode, Write, WriteOp, WritesApplied, SUPERCHUNK_SIDE_CELLS};
use protocol::server_types::attribute_layouts::Attribute;
use protocol::server_types::attributes::WholeAttribute;
use protocol::server_types::plane_widths::{Bits4, WidePlane};
use crate::simulation::simulation_api::rule_counts::RuleCounts;
use protocol::server_types::simulation_time::SimulationTime;
use crate::World;
use protocol::server_parameters::{CleanServerParameters, ParameterValue, WORLD};
use std::path::{Path, PathBuf};
use protocol::type_registry::{FrozenTypeRegistry, Hook, Registration, TypeRegistry};
use std::sync::Arc;
use utilities::coordinates::{SuperchunkIndex, WORLD_MIDDLE};

/// The test game's bundle.
pub const BUNDLE: &str = "test game";
/// The test game's config file, in its folder of a world's mod config:
/// the walkers each superchunk generated is given, a number -- none if
/// there is no file.
pub const WALKERS_CONFIG_FILE: &str = "walkers per superchunk.txt";
/// The walkers' rule's name.
pub const WALKERS_RULE: &str = "walkers rule";
/// A walker's chance, one in this, of giving birth at a wake, and of dying.
const ONE_IN: u64 = 64;
/// The most ticks a walker sleeps between wakes.
const LONGEST_SLEEP: u64 = 8;

/// What the walkers' rule counts, each at its place.
const COUNT_NAMES: [&str; 4] = ["woken", "steps", "births", "deaths"];
/// Walkers woken.
pub const WOKEN_COUNT: usize = 0;
/// Steps taken, counted as applied.
pub const STEPS_COUNT: usize = 1;
/// Walkers born, counted as applied.
pub const BIRTHS_COUNT: usize = 2;
/// Walkers dead.
pub const DEATHS_COUNT: usize = 3;

/// The test game's types, as registering them gave each.
#[derive(Clone, Copy, Debug)]
pub struct TestGame {
    /// Stone, scattered by the seed: barred to walkers.
    pub stone: LayerType,
    /// Where a walker has stood.
    pub trail: LayerType,
    /// Whatever bars a cell: stone and walkers.
    pub collision: LayerType,
    /// A number a tile 8 by 8 cells, written by none of the game.
    pub tiles: WidePlane<Bits4>,
    /// The walker: it keeps the world hot about it.
    pub walker: EntityType,
    /// A walker's steps, its own attribute, always.
    pub steps: Attribute<u64>,
}

impl TestGame {
    /// Registers the test game in `type_registry`, in its bundle: its
    /// rule and its generation hooked as a game's are.
    pub fn registered(type_registry: &mut TypeRegistry) -> Result<Self, String> {
        let mut bundle = type_registry.open_bundle(BUNDLE).map_err(|refusal| format!("{BUNDLE}: {refusal:?}"))?;
        let mut register = |registration: Registration| bundle.register(registration).map_err(|refusal| format!("{refusal:?}"));
        let stone = register(Registration::layer("stone"))?.layer_type().expect("a layer");
        let trail = register(Registration::layer("trail"))?.layer_type().expect("a layer");
        let collision = register(Registration::layer("collision"))?.layer_type().expect("a layer");
        let tiles = WidePlane::registered(&register(WidePlane::<Bits4>::registration_of_tiles("tiles", 3))?).expect("four bits a tile");
        let steps = Attribute::registered(&register(Attribute::<u64>::registration("steps"))?).expect("a word's attribute");
        let walker = register(Registration::entity("walker", Some("steps")).depending_on(&["collision"]).with_hooks(&[Hook::KeepsTheWorldHot]))?.entity_type().expect("a type of entity");
        let test_game = Self { stone, trail, collision, tiles, walker, steps };
        register(Registration::entity_rule(WALKERS_RULE, "walker").with_library(Arc::new(test_game.rule())).with_hooks(&[Hook::RunsEveryTick]))?;
        register(Registration::worldgen("generation").depending_on(&["walker", "collision"]).with_library(Arc::new(test_game.generation())).with_hooks(&[Hook::GeneratesTheWorld]))?;
        Ok(test_game)
    }

    /// The test game's types alone, as a world of them is loaded.
    pub fn register_types(type_registry: &mut TypeRegistry) -> Result<(), String> {
        Self::registered(type_registry).map(drop)
    }

    /// The test game's types frozen for a new world, and their IDs.
    pub fn new_world_types() -> (Arc<FrozenTypeRegistry>, Self) {
        let mut type_registry = TypeRegistry::for_new_world();
        let test_game = Self::registered(&mut type_registry).expect("the test game's types");
        (Arc::new(type_registry.freeze().expect("frozen")), test_game)
    }

    /// The collision plane's API, of the game's collision plane.
    pub fn collision_library(&self) -> Collision {
        Collision { plane: self.collision }
    }

    /// The walkers' rule: each walker woken steps to a neighbour drawn,
    /// if free, leaving a trail where it stood -- or gives birth on it,
    /// or dies, one wake in [`ONE_IN`] each -- and sleeps again.
    fn rule(self) -> RuleCode {
        RuleCode::new(&COUNT_NAMES, move |tick_state, _| {
            let mut rule_counts = RuleCounts::default();
            entity_reads_and_writes::each_woken(tick_state, [self.collision, self.trail], &mut rule_counts, |tick_state, walker, rule_counts| self.wake(tick_state, walker, rule_counts));
            rule_counts
        })
    }

    /// A walker woken.
    fn wake(self, tick_state: &mut SimulationTickState, walker: EntityRef, rule_counts: &mut RuleCounts) {
        rule_counts[WOKEN_COUNT] += 1;
        let (base_data, tick_now, collision) = (walker.base_data, tick_and_random_stream::current_tick(tick_state), self.collision_library());
        let (fate, way, sleep) = {
            let random_stream = tick_and_random_stream::random_stream(tick_state);
            (random_stream.draw_below(ONE_IN), random_stream.draw_below(8) as u32, 1 + random_stream.draw_below(LONGEST_SLEEP))
        };
        if fate == 0 {
            rule_counts[DEATHS_COUNT] += 1;
            return collision.remove(tick_state, &base_data);
        }
        let neighbour = cell_and_its_neighbours::cell(base_data.stood_cell, way).filter(|&neighbour| !crate::simulation::simulation_api::cell_reads_and_writes::layer_holds(tick_state, self.collision, neighbour));
        let Some(neighbour) = neighbour else {
            return entity_reads_and_writes::sleep(tick_state, &base_data, tick_now + sleep);
        };
        if fate == 1 {
            let mut born = collision.begin_making_entity(tick_state, self.walker, neighbour, tick_now + sleep, &WholeAttribute::holding_value(self.steps, 0));
            born.count(BIRTHS_COUNT);
            drop(born);
            return entity_reads_and_writes::sleep(tick_state, &base_data, tick_now + sleep);
        }
        if crate::simulation::simulation_api::cell_reads_and_writes::layer_lacks(tick_state, self.trail, base_data.stood_cell) {
            crate::simulation::simulation_api::cell_reads_and_writes::set(tick_state, self.trail, base_data.stood_cell);
        }
        let steps_before = walker.attribute(self.steps).expect("a walker's steps, always");
        entity_reads_and_writes::set_attribute_of(tick_state, &base_data, self.steps, Some(steps_before), steps_before + 1);
        collision.step(tick_state, &base_data, neighbour, tick_now + sleep);
        rule_counts[STEPS_COUNT] += 1;
    }

    /// The generation: stone on a cell in sixteen, drawn from the seed
    /// and the cell, held in the collision plane; and on each superchunk
    /// as many walkers as its config says ([`WALKERS_CONFIG_FILE`]), on
    /// cells of their own free of it -- the superchunk generated whole.
    fn generation(self) -> WorldgenCode {
        WorldgenCode { set_up: Arc::new(move |config_folder: &Path| Ok(self.set_up_generation(walkers_in(config_folder)?))) }
    }

    /// The generation set up, `walkers` walkers given each superchunk.
    fn set_up_generation(self, walkers: u64) -> SetUpWorldgen {
        SetUpWorldgen {
            generate: Arc::new(move |seed, generating: &mut GeneratingSuperchunk| {
                let (superchunk, mut writes_applied) = (generating.superchunk_index(), WritesApplied::default());
                let corner = superchunk.top_left().cartesian();
                for (x, y) in (0..SUPERCHUNK_SIDE_CELLS).flat_map(|y| (0..SUPERCHUNK_SIDE_CELLS).map(move |x| (x, y))) {
                    let cell = CellIndex::from(CellCartesian { x: corner.x + x, y: corner.y + y });
                    if RandomSource::for_stream(seed, cell.0).draw_below(16) == 0 {
                        generating.cells_mut().apply(self.stone, Write::cell(cell, WriteOp::SetCell), &mut writes_applied);
                        generating.cells_mut().apply(self.collision, Write::cell(cell, WriteOp::SetCell), &mut writes_applied);
                    }
                }
                self.generate_walkers(generating, walkers, &mut RandomSource::for_stream(!seed, superchunk.0));
            }),
        }
    }

    /// Makes on `generating` `count` walkers on cells drawn from
    /// `random_stream`, each of its own and free of the collision plane,
    /// which then holds it, first waking within [`LONGEST_SLEEP`] of the
    /// tick after it turns hot.
    pub fn generate_walkers(self, generating: &mut GeneratingSuperchunk, count: u64, random_stream: &mut RandomSource) {
        let (corner, mut writes_applied, mut made) = (generating.superchunk_index().top_left().cartesian(), WritesApplied::default(), 0);
        while made < count {
            let drawn_cell: CellIndex = CellCartesian { x: corner.x + random_stream.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32, y: corner.y + random_stream.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32 }.into();
            if generating.cells().value_at(self.collision, drawn_cell) == Some(1) {
                continue;
            }
            let first_wake = SimulationTime::time_between_ticks(0, 1 + random_stream.draw_below(LONGEST_SLEEP));
            if generating.new_entity(self.walker, drawn_cell, first_wake).with(self.steps, 0).make() {
                generating.cells_mut().apply(self.collision, Write::cell(drawn_cell, WriteOp::SetCell), &mut writes_applied);
                made += 1;
            }
        }
    }
}

/// The walkers each superchunk generated is given, as the test game's
/// config in `config_folder` says: none if it has none. Or why not.
fn walkers_in(config_folder: &Path) -> Result<u64, String> {
    let config_path = config_folder.join(WALKERS_CONFIG_FILE);
    match std::fs::read_to_string(&config_path) {
        Ok(config_text) => config_text.trim().parse().map_err(|_| format!("{}: not a number", config_path.display())),
        Err(_) => Ok(0),
    }
}

/// `parameters` of a new world of the test game in a folder of its own
/// for the test `test_name`, emptied (`crate::whole_worlds::folder`), its
/// config giving each superchunk generated `walkers` walkers.
pub fn in_folder(parameters: CleanServerParameters, test_name: &str, walkers: u64) -> CleanServerParameters {
    let folder = crate::whole_worlds::folder(test_name);
    let config_folder = folder.join(crate::MOD_CONFIG).join(BUNDLE);
    std::fs::create_dir_all(&config_folder).expect("the test world's config folder");
    std::fs::write(config_folder.join(WALKERS_CONFIG_FILE), walkers.to_string()).expect("the test world's config written");
    parameters.with(WORLD, ParameterValue::Text(folder.display().to_string()))
}

/// The folder `parameters` name ([`WORLD`]).
pub fn folder_of(parameters: &CleanServerParameters) -> PathBuf {
    PathBuf::from(parameters.text(WORLD).expect("a test world's folder named"))
}

/// A world of the test game made in the folder `parameters` name
/// ([`in_folder`]), as they say.
pub fn made(parameters: &CleanServerParameters) -> World {
    crate::make_world(&folder_of(parameters), parameters, &TestGame::new_world_types().0).expect("the world made")
}

/// A world of the test game made in a folder of its own for the test
/// `test_name` as `parameters` say -- given no side, with no end -- its
/// generation giving no walkers, and `walkers` put at its middle by the
/// server between ticks, their halo kept hot: a flock wandering a world
/// with no end, which a flock on every superchunk generated would grow
/// for good.
pub fn one_flock_world(test_name: &str, parameters: CleanServerParameters, walkers: u64) -> (World, TestGame) {
    let (frozen_type_registry, test_game) = TestGame::new_world_types();
    let parameters = in_folder(parameters, test_name, 0);
    let mut world = crate::make_world(&folder_of(&parameters), &parameters, &frozen_type_registry).expect("the world made");
    world.keep_hot(&crate::halos_about([WORLD_MIDDLE].into_iter()));
    put_flock(&mut world, test_game, WORLD_MIDDLE, walkers);
    (world, test_game)
}

/// Puts `count` walkers on cells of `superchunk` drawn from the seed,
/// each free of the collision plane, between ticks, their cells held in it.
pub fn put_flock(world: &mut World, test_game: TestGame, superchunk: SuperchunkIndex, count: u64) {
    use crate::bitplane_manager::{Write, WriteOp};
    let mut random_stream = RandomSource::for_stream(!world.world_info().seed, superchunk.0);
    let (corner, tick_now) = (superchunk.top_left().cartesian(), world.entity_store().current_tick());
    let mut put = 0;
    while put < count {
        let drawn_cell: CellIndex = CellCartesian { x: corner.x + random_stream.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32, y: corner.y + random_stream.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32 }.into();
        if world.bitmap_arena().layer_holds_cell(test_game.collision, drawn_cell) == Ok(true) {
            continue;
        }
        let base_data = protocol::server_types::entity::EntityBaseData { entity_type: test_game.walker, stood_cell: drawn_cell, wake: tick_now + 1 + random_stream.draw_below(LONGEST_SLEEP) };
        let added = world.add_entity(base_data, &WholeAttribute::holding_value(test_game.steps, 0)).added;
        if added > 0 {
            world.write_cells(test_game.collision, [Write::cell(drawn_cell, WriteOp::SetCell)]);
        }
        put += added as u64;
    }
}


/// The place of the walkers' rule among `world`'s rules.
pub fn walkers_rule(world: &World) -> crate::RulePlace {
    let rule = world.frozen_type_registry().types().iter().find(|registered| registered.name() == protocol::type_registry::in_bundle(BUNDLE, WALKERS_RULE)).expect("the walkers' rule");
    world.rule_place(rule.type_id())
}
