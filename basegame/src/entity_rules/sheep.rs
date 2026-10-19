//! Sheep on the grass: a sheep sleeps until it next needs something --
//! a meal, its lamb, growing up -- and wakes for that alone; hungry, it
//! walks to grass, a step a wake (`docs/entity_rules.md`, "The sheep").

use server::simulation::simulation_api::cell_and_its_neighbours;
pub use crate::base_types::SheepState;
use server::simulation::simulation_api::simulation_time::SimulationTime;
use crate::base_types::BaseTypes;
use server::simulation::simulation_api::steps_and_paths::PlanesInTheWay;
use server::simulation::simulation_api::RuleCode;
use crate::libraries::collision::Collision;
use server::simulation::simulation_api::{WholeAttribute, Chance, square_around_a_cell, cell_reads_and_writes, compare_and_write, entity_reads_and_writes, place_counted, tick_and_random_stream, steps_and_paths, CellIndex, EntityRef, Header, RuleCounts, SimulationTickState};

/// Every time below is said in simulation time: the rule adds them to
/// the tick it runs in and never says how many ticks they are -- the
/// same sheep at any tick rate.
///
/// The time between a walking sheep's steps, at the least: a quarter
/// of a second...
pub const STEP: SimulationTime = SimulationTime::from_milliseconds(250);
/// ...and up to this much more, a sixteenth of a second, drawn each
/// wake, so the flock's wakes spread over the ticks.
pub const STEP_JITTER: SimulationTime = SimulationTime::from_milliseconds(62);
/// The time from a meal to being hungry again: until then a sheep
/// eats nothing, though it stands on grass, and sleeps.
pub const TIME_BETWEEN_MEALS: SimulationTime = SimulationTime::from_seconds(15);
/// The time a hungry sheep finds no meal in before it starves.
pub const TIME_TO_STARVE: SimulationTime = SimulationTime::from_seconds(90);
/// Cells of grass among the 16 by 16 about a sheep for the pasture
/// to be lush enough to breed on: a quarter of them
/// (`docs/entity_rules.md`, "The sheep").
pub const LUSH_CELLS: u32 = 64;
/// The simulation time a grown sheep eating on lush pasture takes to
/// fall pregnant, on average: with its gestation, four lambs or so
/// in the grown part of a life spent on lush pasture -- every sheep
/// bears, so that is a ewe's eight.
pub const MEAN_TIME_TO_CONCEIVE: SimulationTime = SimulationTime::from_seconds(800);
/// The chance, each meal of a grown sheep on lush pasture, that it
/// falls pregnant.
pub const CONCEIVE_CHANCE: Chance = MEAN_TIME_TO_CONCEIVE.chance_each(TIME_BETWEEN_MEALS);
/// The sleep there is to a sheep's life, on average: before a sleep
/// it dies of old age at that sleep's part of this. An hour and a
/// quarter: a sheep's ten years or so, a year of them seven and a
/// half minutes -- what the times below are worked out from.
pub const MEAN_LIFETIME: SimulationTime = SimulationTime::from_seconds(4_500);
/// The time a sheep that set off roaming walks for, one way, once it
/// is hungry again, before it looks for grass: 96 steps or so, out
/// of the patch its flock has grazed.
pub const ROAMING_TIME: SimulationTime = SimulationTime::from_seconds(27);
/// The simulation time a sheep eating on lush pasture takes to set
/// off roaming all the same, on average: on thin pasture it sets off
/// at every meal.
pub const MEAN_TIME_TO_ROAM: SimulationTime = SimulationTime::from_seconds(54);
/// The chance, each meal on lush pasture, that the sheep sets off
/// roaming when next hungry.
pub const ROAM_CHANCE: Chance = MEAN_TIME_TO_ROAM.chance_each(TIME_BETWEEN_MEALS);
/// The time from falling pregnant to giving birth: five months of
/// such a year.
pub const GESTATION_TIME: SimulationTime = SimulationTime::from_seconds(190);
/// The time a lamb takes to grow: a year.
pub const LAMBHOOD_TIME: SimulationTime = SimulationTime::from_seconds(450);

/// What the rule counts, each named at its place in its
/// [`RuleCounts`]: the one order, the places below worked out from it.
pub const COUNT_NAMES: [&str; 7] = ["woken", "eaten", "births", "deaths", "sought", "paths", "far"];
/// What runs the sheep's rule, as it is registered with `base_types`:
/// [`rule`], on every sheep due, through the collision plane's library.
pub fn code(base_types: BaseTypes) -> RuleCode {
    RuleCode::new(&COUNT_NAMES, move |tick_state, context| rule(&base_types, tick_state, context.library(base_types.collision.type_id()).expect("a sheep depends on the collision plane")))
}

/// What is in a sheep's way, of `base_types`: what stands on a cell, and
/// the terrain's walls.
pub const fn in_the_way(base_types: &BaseTypes) -> PlanesInTheWay {
    PlanesInTheWay { collision: base_types.collision, wall_north: base_types.wall_north, wall_west: base_types.wall_west, wall_south: base_types.wall_south, wall_east: base_types.wall_east }
}
/// Sheep woken.
pub const WOKEN_COUNT: usize = place_counted(&COUNT_NAMES, "woken");
/// Cells of grass eaten: counted as each meal is applied.
pub const EATEN_COUNT: usize = place_counted(&COUNT_NAMES, "eaten");
/// Lambs born.
pub const BIRTHS_COUNT: usize = place_counted(&COUNT_NAMES, "births");
/// Sheep dead: starved, or of old age.
pub const DEATHS_COUNT: usize = place_counted(&COUNT_NAMES, "deaths");
/// Paths to grass looked for, by hungry sheep with none beside them.
pub const SOUGHT_COUNT: usize = place_counted(&COUNT_NAMES, "sought");
/// Of those, found.
pub const PATHS: usize = place_counted(&COUNT_NAMES, "paths");
/// Of those found, the ones beyond the area about the sheep.
pub const FAR_COUNT: usize = place_counted(&COUNT_NAMES, "far");

/// What the rule keeps over a superchunk's turn: the types it works on,
/// what the sheep did, and the collision plane's API, which a sheep
/// comes, goes and steps through.
struct Flock<'library> {
    /// The types it works on.
    base_types: &'library BaseTypes,
    /// What the sheep did.
    rule_counts: RuleCounts,
    /// The collision plane's API.
    collision: &'library Collision,
}

/// The rule, on one superchunk's turn: every sheep waking, each seen to
/// by [`wake`], coming, going and stepping through `collision`.
pub fn rule(base_types: &BaseTypes, tick_state: &mut SimulationTickState, collision: &Collision) -> RuleCounts {
    let mut flock = Flock { base_types, rule_counts: RuleCounts::default(), collision };
    entity_reads_and_writes::each_woken(tick_state, [base_types.grass, base_types.wall_north, base_types.wall_west, base_types.wall_south, base_types.wall_east, base_types.collision], &mut flock, wake);
    flock.rule_counts
}

/// The rule, on one sheep waking: it sees to what it woke for -- a
/// meal, a lamb, growing up -- and sleeps again, as long as it can.
#[inline]
fn wake(tick_state: &mut SimulationTickState, woken_sheep: EntityRef, flock: &mut Flock) {
    let (base_types, collision, rule_counts, tick_now) = (flock.base_types, flock.collision, &mut flock.rule_counts, tick_and_random_stream::current_tick(tick_state));
    let grass = base_types.grass;
    rule_counts[WOKEN_COUNT] += 1;
    let (sheep_cell, header_before) = (woken_sheep.header.stood_cell, woken_sheep.header);
    let about_tiles = cell_and_its_neighbours::tiles_about_cell(sheep_cell);
    let grass_about = cell_and_its_neighbours::read_layer(tick_state, grass, &about_tiles);
    // The neighbours it may step to: on the hot bitplanes, nothing standing there, no wall before them.
    let steppable_cells = grass_about.hot & steps_and_paths::around_steppable(tick_state, in_the_way(base_types), &about_tiles);
    // All it keeps, as the tick found it, and as this wake leaves it: written once, if it changed.
    let state_seen = woken_sheep.attribute(base_types.sheep_state);
    let state_was = state_seen.unwrap_or(SheepState { hungry_at: tick_now, ..SheepState::default() });
    let mut state_after = state_was;
    let (hungry_at, is_roaming) = (state_was.hungry_at, state_was.roaming_until != 0);
    // On its way, roaming, it does not stop to eat.
    let eats_now = tick_now >= hungry_at && !(grass_about.set_cells & about_tiles.centre_cell()).is_empty() && !is_roaming;
    if !eats_now && tick_now >= TIME_TO_STARVE.tick_after(hungry_at) {
        collision.remove(tick_state, &header_before);
        rule_counts[DEATHS_COUNT] += 1;
        return;
    }
    // The pasture about it, looked at as it eats: thin, it will leave when next hungry; lush, it may.
    let on_lush_pasture = eats_now && square_around_a_cell::read_layer(tick_state, grass, sheep_cell).count() >= LUSH_CELLS;
    if eats_now {
        state_after.hungry_at = TIME_BETWEEN_MEALS.tick_after(tick_now);
        // Off thin pasture always; off lush, now and then: a flock spreads over the land it could stay on.
        let sets_off_roaming = !on_lush_pasture || tick_and_random_stream::random_stream(tick_state).chance(ROAM_CHANCE);
        if let (true, Some(way)) = (sets_off_roaming, cell_and_its_neighbours::pick_way(tick_and_random_stream::random_stream(tick_state), &about_tiles, steppable_cells)) {
            (state_after.roaming_until, state_after.roaming_neighbour) = (ROAMING_TIME.tick_after(TIME_BETWEEN_MEALS.tick_after(tick_now)), way);
        }
    }
    let is_hungry = !eats_now && tick_now >= hungry_at;
    // What it next has to wake for, were it to sleep as long as it can.
    let mut next_need_tick = if eats_now { TIME_BETWEEN_MEALS.tick_after(tick_now) } else { hungry_at };
    let grown_at = (state_was.grown_at != 0).then_some(state_was.grown_at);
    // The cell it bears its lamb on this tick, if it does: it stays where it is, the cell not to be the one it steps to.
    let mut birth_cell = None;
    match (state_after.lamb_due_at != 0).then_some(state_after.lamb_due_at) {
        Some(due_tick) if tick_now < due_tick => next_need_tick = next_need_tick.min(due_tick),
        // Eating, it bears at its next wake: the meal is one thing to happen whole, the birth another.
        Some(_) if eats_now => next_need_tick = tick_now,
        // Its lamb is born on a cell seen free beside it, drawn here and queued last ([`bear_lamb`]); with none free, or that one taken
        // first, it is pregnant still, and tries again a step's time on.
        Some(_) => {
            birth_cell = cell_and_its_neighbours::pick_way(tick_and_random_stream::random_stream(tick_state), &about_tiles, steppable_cells).and_then(|birth_way| cell_and_its_neighbours::cell(sheep_cell, birth_way));
            next_need_tick = tick_now;
        }
        None if on_lush_pasture && grown_at.is_none() && tick_and_random_stream::random_stream(tick_state).chance(CONCEIVE_CHANCE) => {
            state_after.lamb_due_at = GESTATION_TIME.tick_after(tick_now);
            next_need_tick = next_need_tick.min(state_after.lamb_due_at);
        }
        None => {}
    }
    match grown_at {
        Some(grown_at) if tick_now >= grown_at => state_after.grown_at = 0,
        Some(grown_at) => next_need_tick = next_need_tick.min(grown_at),
        None => {}
    }
    // Hungry, it walks; satisfied, it stays, and sleeps until it needs something.
    let way = if !is_hungry || birth_cell.is_some() {
        None
    } else if is_roaming {
        // On the way it set off, until its time is up or it comes to the edge of the hot superchunks.
        let way = about_tiles.cell_towards(state_was.roaming_neighbour);
        if tick_now >= state_was.roaming_until || (steppable_cells & way).is_empty() {
            (state_after.roaming_until, state_after.roaming_neighbour) = (0, 0);
        }
        cell_and_its_neighbours::prefer_way(tick_and_random_stream::random_stream(tick_state), &about_tiles, way, steppable_cells)
    } else if !(grass_about.set_cells & steppable_cells).is_empty() {
        cell_and_its_neighbours::pick_way(tick_and_random_stream::random_stream(tick_state), &about_tiles, grass_about.set_cells & steppable_cells)
    } else if steppable_cells.is_empty() {
        // Hemmed in: no step to take, and no path to look for.
        None
    } else {
        rule_counts[SOUGHT_COUNT] += 1;
        match steps_and_paths::seek(tick_state, in_the_way(base_types), sheep_cell, grass) {
            Some(found_step) => {
                rule_counts[PATHS] += 1;
                rule_counts[FAR_COUNT] += u64::from(found_step.scale > 0);
                Some(cell_and_its_neighbours::way_of(sheep_cell, found_step.step_cell))
            }
            None => cell_and_its_neighbours::pick_way(tick_and_random_stream::random_stream(tick_state), &about_tiles, steppable_cells),
        }
    };
    let step_cell = way.and_then(|way| cell_and_its_neighbours::cell(sheep_cell, way)).unwrap_or(sheep_cell);
    let wake = if is_hungry {
        next_step(tick_state)
    } else {
        next_step(tick_state).max(STEP_JITTER.part_drawn(tick_and_random_stream::random_stream(tick_state)).tick_after(next_need_tick))
    };
    // Old age comes by the time slept, not the wake: a long sleep is as much of a life as many short ones.
    let dies_of_age = MEAN_LIFETIME.part_drawn(tick_and_random_stream::random_stream(tick_state)) < SimulationTime::time_between_ticks(tick_now, wake);
    let wake_end = WakeEnd { dies_of_age, state_seen, state_was, state_after, step_cell, wake, birth_cell };
    if eats_now {
        // All the sheep comes to this wake hangs on the grass being there still when it is applied: each thing queued under it is held
        // against it, and the grass cleared last. Should it be gone by then -- decayed in this very tick -- none of it happens, and
        // what is held against the grass being gone does instead: the sheep sleeps a step, hungry as it was, and wakes to look again.
        queue_wake_end(base_types, &mut compare_and_write::open_compare(tick_state, compare_and_write::layer_holds(grass, sheep_cell)), collision, rule_counts, &header_before, wake_end, Some(sheep_cell));
        // One of the two is applied, never both: the sheep is written once.
        let hungry_still = next_step(tick_state);
        entity_reads_and_writes::sleep(&mut compare_and_write::open_compare(tick_state, compare_and_write::layer_lacks(grass, sheep_cell)), &header_before, hungry_still);
        cell_reads_and_writes::clear_counted(tick_state, grass, sheep_cell, EATEN_COUNT);
    } else {
        queue_wake_end(base_types, tick_state, collision, rule_counts, &header_before, wake_end, None);
    }
}

/// How a sheep's wake ends, worked out: what [`queue_wake_end`] queues.
#[derive(Clone, Copy)]
struct WakeEnd {
    /// Whether it dies of old age.
    dies_of_age: bool,
    /// Its state, as the tick found it.
    state_seen: Option<SheepState>,
    /// Its state as the tick found it, or as a sheep starts if it had none.
    state_was: SheepState,
    /// Its state as the wake leaves it.
    state_after: SheepState,
    /// The cell it steps to: its own, to stay.
    step_cell: CellIndex,
    /// The tick it wakes next.
    wake: u64,
    /// The cell it bears its lamb on, if it does.
    birth_cell: Option<CellIndex>,
}

/// Queues `wake_end` for the sheep `sheep_header`: dead of old age, or its state
/// written if it changed, its step taken and its lamb born. A death
/// is counted on `meal_cell`, if it ate ([`count`]).
fn queue_wake_end(base_types: &BaseTypes, tick_state: &mut SimulationTickState, collision: &Collision, rule_counts: &mut RuleCounts, sheep_header: &Header, wake_end: WakeEnd, meal_cell: Option<CellIndex>) {
    if wake_end.dies_of_age {
        collision.remove(tick_state, sheep_header);
        return count(base_types, tick_state, rule_counts, meal_cell, DEATHS_COUNT);
    }
    // Written from where it is worked out, the words that changed alone: nothing of the sheep is copied to be changed.
    if wake_end.state_after != wake_end.state_was {
        entity_reads_and_writes::set_attribute_of(tick_state, sheep_header, base_types.sheep_state, wake_end.state_seen, wake_end.state_after);
    }
    collision.step(tick_state, sheep_header, wake_end.step_cell, wake_end.wake);
    if let Some(lamb_cell) = wake_end.birth_cell {
        bear_lamb(base_types, tick_state, collision, sheep_header, wake_end.state_after, lamb_cell);
    }
}

/// Queues the sheep `mother_sheep`, whose state is `mother_state` once what its
/// wake queued is applied, bearing its lamb on `lamb_cell`: the lamb
/// made, the mother no longer pregnant and the birth counted, all of
/// it or none -- under one compare of the mother being as this wake
/// left it and, within it, one of the lamb's cell being free. So no
/// lamb is born twice and none is lost, whichever superchunk its
/// cell is in: the cell taken first, the mother is pregnant still.
fn bear_lamb(base_types: &BaseTypes, tick_state: &mut SimulationTickState, collision: &Collision, mother_sheep: &Header, mother_state: SheepState, lamb_cell: CellIndex) {
    let tick_now = tick_and_random_stream::current_tick(tick_state);
    let (wake, lamb_state) = (next_step(tick_state), SheepState { hungry_at: TIME_BETWEEN_MEALS.tick_after(tick_now), grown_at: LAMBHOOD_TIME.tick_after(tick_now), ..SheepState::default() });
    let mother_delivered = SheepState { lamb_due_at: 0, ..mother_state };
    let sheep_state = base_types.sheep_state;
    let mut mother_as_left = compare_and_write::open_compare(tick_state, compare_and_write::attribute(mother_sheep, sheep_state, Some(mother_state)));
    let mut lamb_made = collision.begin_making_entity(&mut mother_as_left, base_types.sheep, lamb_cell, wake, &WholeAttribute::holding_value(sheep_state, lamb_state));
    entity_reads_and_writes::set_attribute_of(&mut lamb_made, mother_sheep, sheep_state, Some(mother_state), mother_delivered);
    compare_and_write::count(&mut lamb_made, compare_and_write::attribute(mother_sheep, sheep_state, Some(mother_delivered)), BIRTHS_COUNT);
}

/// Counts one at `place` for the wake of a sheep: if it is `meal_cell`, on
/// the cell given, as it is applied, the grass there still -- what it
/// did then happens with the meal or not at all -- else here.
fn count(base_types: &BaseTypes, tick_state: &mut SimulationTickState, rule_counts: &mut RuleCounts, meal_cell: Option<CellIndex>, place: usize) {
    match meal_cell {
        Some(grass_cell) => compare_and_write::count(tick_state, compare_and_write::layer_holds(base_types.grass, grass_cell), place),
        None => rule_counts[place] += 1,
    }
}

/// The tick a sheep taking a step now wakes next.
fn next_step(tick_state: &mut SimulationTickState) -> u64 {
    let tick_now = tick_and_random_stream::current_tick(tick_state);
    STEP_JITTER.part_drawn(tick_and_random_stream::random_stream(tick_state)).tick_after(STEP.tick_after(tick_now))
}
