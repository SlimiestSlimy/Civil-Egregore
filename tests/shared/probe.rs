//! The tests' probe ([`Probe`]): a rule of the tests' own bundle, run
//! every tick, that does on each superchunk's turn what the test
//! hands it for that tick -- its setting up and its reading of the
//! world, inside the tick, through the simulation's API as any rule
//! does: nothing is done to a world between ticks. What the turns read
//! comes back in the order of their superchunks. Registered before the
//! base game, it runs first in a turn: what it writes meets the game's
//! rules' writes of the same tick first.

// A tier uses what it needs of it.
#![allow(dead_code)]

use crate::worlds::named;
use basegame::default_types::BUNDLE as BASE_BUNDLE;
use protocol::type_registry::{in_bundle, FrozenTypeRegistry, Hook, Registration, TypeRegistry};
use basegame::libraries::collision::Collision;
use server::simulation::simulation_api::{tick_and_random_stream, CellIndex, EntityRef, LayerType, RuleCode, SimulationTickState, SuperchunkIndex, TickReport};
use protocol::server_types::entity::EntityBaseData;
use protocol::server_types::attributes::AttributeBlock;
use server::simulation::simulation_api::rule_counts::RuleCounts;
use utilities::coordinates::{ChunkIndex, CHUNK_SIDE};
use server::{ChosenRules, RulePlace, TickCounts, World, WorldTick};
use std::sync::{Arc, Mutex};

/// The bundle of the probe, registered before the base game's.
pub const PROBE_BUNDLE: &str = "tests first";
/// The probe's name in its bundle.
pub const PROBE: &str = "probe";
/// The probe's own plane, its rule's master: never written.
pub const PROBE_PLANE: &str = "probe plane";

/// What the probe does on a superchunk's turn.
type TurnWork = dyn Fn(&mut SimulationTickState) + Send + Sync;
/// What each turn of a tick gave, with its superchunk, as the turns put it.
type TurnsGave<T> = Arc<Mutex<Vec<(SuperchunkIndex, Vec<T>)>>>;

/// The probe of one world: what it is handed to do on the next tick.
/// A world of its own each, so the tests running at once never share one.
#[derive(Clone, Default)]
pub struct Probe {
    /// The work of the tick being run, none between.
    handed: Arc<Mutex<Option<Arc<TurnWork>>>>,
}

impl Probe {
    /// Registers the probe in its bundle of `type_registry`: a rule of a
    /// plane of its own, hooked to run every tick, doing nothing on a
    /// tick it is handed nothing for.
    pub fn register(&self, type_registry: &mut TypeRegistry) -> Result<(), String> {
        let handed = self.handed.clone();
        let code = RuleCode::new(&[], move |tick_state, _| {
            let work = handed.lock().expect("the probe's work").clone();
            if let Some(work) = work {
                work(tick_state);
            }
            RuleCounts::default()
        });
        let mut probe_bundle = type_registry.open_bundle(PROBE_BUNDLE).map_err(|refusal| format!("{PROBE_BUNDLE}: {refusal:?}"))?;
        probe_bundle.register(Registration::layer(PROBE_PLANE)).map_err(|refusal| format!("{PROBE_PLANE}: {refusal:?}"))?;
        probe_bundle.register(Registration::sca_rule(PROBE, PROBE_PLANE).with_library(Arc::new(code)).with_hooks(&[Hook::RunsEveryTick])).map(drop).map_err(|refusal| format!("{PROBE}: {refusal:?}"))
    }

    /// The probe and the base game's types -- then whatever `more`
    /// registers -- frozen for a new world: the probe's world's types,
    /// the base game's of the IDs [`crate::worlds::base`] holds.
    pub fn types(&self, more: impl FnOnce(&mut TypeRegistry) -> Result<(), String>) -> Arc<FrozenTypeRegistry> {
        let mut type_registry = TypeRegistry::for_new_world();
        self.register(&mut type_registry).expect("the probe");
        basegame::default_types::register_types(&mut type_registry).expect("the base game's types");
        more(&mut type_registry).expect("the test's own types");
        Arc::new(type_registry.freeze().expect("the types frozen"))
    }

    /// Ticks `world` once, running the base game's rules named
    /// `rule_names` and the probe doing `work` on every superchunk's
    /// turn: the tick's report and what each turn gave, in the order
    /// of the superchunks. Every rule sees the world as the tick found
    /// it, the probe too.
    pub fn tick<T: Send + 'static>(&self, world: &mut World, rule_names: &[&str], work: impl Fn(&mut SimulationTickState) -> Vec<T> + Send + Sync + 'static) -> (TickReport<TickCounts>, Vec<T>) {
        let mut places: Vec<RulePlace> = rule_names.iter().map(|&rule_name| rule_place(world, rule_name)).collect();
        places.push(world.rule_place(named(world.frozen_type_registry(), &in_bundle(PROBE_BUNDLE, PROBE)).expect("a world of the probe's types").type_id()));
        let given = self.hand(work);
        let report = world.tick_only(ChosenRules::from_rules(&places), false);
        (report, self.taken(&given))
    }

    /// Hands the probe `work` for the next tick: where its turns put what
    /// they give.
    fn hand<T: Send + 'static>(&self, work: impl Fn(&mut SimulationTickState) -> Vec<T> + Send + Sync + 'static) -> TurnsGave<T> {
        let given: TurnsGave<T> = Arc::default();
        let giving = given.clone();
        let turn_work: Arc<TurnWork> = Arc::new(move |tick_state: &mut SimulationTickState| {
            let (superchunk, turn_gave) = (tick_and_random_stream::superchunk(tick_state), work(tick_state));
            giving.lock().expect("what the turns gave").push((superchunk, turn_gave));
        });
        *self.handed.lock().expect("the probe's work") = Some(turn_work);
        given
    }

    /// The work handed taken back, the tick over: what the turns gave, in
    /// the order of their superchunks.
    fn taken<T>(&self, given: &Mutex<Vec<(SuperchunkIndex, Vec<T>)>>) -> Vec<T> {
        *self.handed.lock().expect("the probe's work") = None;
        let mut turns_gave = std::mem::take(&mut *given.lock().expect("what the turns gave"));
        turns_gave.sort_unstable_by_key(|&(superchunk, _)| superchunk);
        turns_gave.into_iter().flat_map(|(_, turn_gave)| turn_gave).collect()
    }

    /// A whole tick of `world` -- every rule, its halos moving -- the
    /// probe doing `work` on every superchunk's turn: what the tick did,
    /// and what each turn gave, in the order of the superchunks.
    pub fn whole_tick<T: Send + 'static>(&self, world: &mut World, work: impl Fn(&mut SimulationTickState) -> Vec<T> + Send + Sync + 'static) -> (WorldTick, Vec<T>) {
        let given = self.hand(work);
        let world_tick = world.tick();
        (world_tick, self.taken(&given))
    }

    /// [`Probe::tick`] of the probe alone: what each turn gave of `work`.
    pub fn alone<T: Send + 'static>(&self, world: &mut World, work: impl Fn(&mut SimulationTickState) -> Vec<T> + Send + Sync + 'static) -> Vec<T> {
        self.tick(world, &[], work).1
    }

    /// [`Probe::tick`] of the base game's rule named `rule_name` and the
    /// probe: the rule's report, its counts alone in it, and what each
    /// turn gave.
    pub fn tick_rule<T: Send + 'static>(&self, world: &mut World, rule_name: &str, work: impl Fn(&mut SimulationTickState) -> Vec<T> + Send + Sync + 'static) -> (TickReport<RuleCounts>, Vec<T>) {
        let rule = rule_place(world, rule_name);
        let (report, turns_gave) = self.tick(world, &[rule_name], work);
        (report_of(report, rule), turns_gave)
    }
}

/// The place among `world`'s rules of the base game's rule named `rule_name`.
pub fn rule_place(world: &World, rule_name: &str) -> RulePlace {
    let rule_id = named(world.frozen_type_registry(), &in_bundle(BASE_BUNDLE, rule_name)).unwrap_or_else(|| panic!("no rule named {rule_name}")).type_id();
    world.rule_place(rule_id)
}

/// `report`, the counts of the rule at `rule` alone in it.
pub fn report_of(report: TickReport<TickCounts>, rule: RulePlace) -> TickReport<RuleCounts> {
    TickReport { writes_applied: report.writes_applied, instructions_applied: report.instructions_applied, instructions_compared: report.instructions_compared, counted_when_applied: report.counted_when_applied, rules: report.rules.counts_of(rule), computing_time: report.computing_time, applying_time: report.applying_time, acts_over_a_border: report.acts_over_a_border }
}

// What a turn's work reads and writes, of its own superchunk: each
// turn sets up its own part, so all of it is set up once.

/// Whether `cell` is of the turn's superchunk.
pub fn of_the_turn(tick_state: &SimulationTickState, cell: CellIndex) -> bool {
    cell.superchunk() == tick_and_random_stream::superchunk(tick_state)
}

/// The entities of the turn's superchunk as the tick found them, in
/// Morton order by cell.
pub fn entities_of_the_turn<'world>(tick_state: &SimulationTickState<'world>) -> Vec<EntityRef<'world>> {
    tick_and_random_stream::superchunk(tick_state).chunks().flat_map(|chunk| tick_state.entities_in(chunk).into_iter().flatten()).collect()
}

/// The cells of `layer_type` on each chunk of the turn's superchunk, as
/// the tick found them: a chunk's words, its cells in Morton order.
pub fn words_of_the_turn(tick_state: &SimulationTickState, layer_type: LayerType) -> Vec<(ChunkIndex, Vec<u64>)> {
    tick_and_random_stream::superchunk(tick_state).chunks().map(|chunk| (chunk, tick_state.aligned_tile(layer_type, chunk.top_left(), CHUNK_SIDE as u32).expect("a hot chunk").to_vec())).collect()
}

/// The cells `layer_type` holds on the turn's superchunk.
pub fn count_of_the_turn(tick_state: &SimulationTickState, layer_type: LayerType) -> u64 {
    words_of_the_turn(tick_state, layer_type).iter().flat_map(|(_, words)| words).map(|word| u64::from(word.count_ones())).sum()
}

/// Puts an entity as `base_data` says, with `attributes`, its cell held in
/// the collision plane -- as one that collides comes -- unless the cell
/// is taken by then. Woken no sooner than the tick after.
pub fn put_entity(tick_state: &mut SimulationTickState, base_data: EntityBaseData, attributes: &[AttributeBlock]) {
    let wake = base_data.wake.max(tick_and_random_stream::current_tick(tick_state) + 1);
    drop(Collision { plane: crate::worlds::base().collision }.begin_making_entity(tick_state, base_data.entity_type, base_data.stood_cell, wake, attributes));
}

/// Takes `base_data`'s entity away, the cell it held in the collision plane
/// let go.
pub fn take_entity(tick_state: &mut SimulationTickState, base_data: &EntityBaseData) {
    Collision { plane: crate::worlds::base().collision }.remove(tick_state, base_data);
}
