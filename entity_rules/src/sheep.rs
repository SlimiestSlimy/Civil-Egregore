//! Sheep on the grass: a sheep sleeps until it next needs something --
//! a meal, its lamb, growing up -- and wakes for that alone; hungry, it
//! walks to grass, a step a wake (`docs/entity_rules.md`, "The sheep").

use actions::around;
use actions::entities::EntitiesBetweenTicks;
pub use actions::types::{SheepState, SHEEP, SHEEP_STATE};
use actions::simulation_time::SimulationTime;
use actions::types::{COLLISION, GRASS, WALL_EAST, WALL_NORTH, WALL_SOUTH, WALL_WEST};
use actions::{WholeAttribute, Chance, area, cells, compare, entities, place_counted, this_tick, walking, CellCartesian, CellIndex, EntityRef, Header, RandomSource, RuleCounts, SuperchunkIndex, Turn, SUPERCHUNK_SIDE_CELLS};
use std::collections::HashSet;

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

/// What the rule keeps over a superchunk's turn: what the sheep did.
#[derive(Default)]
struct Flock {
    /// What the sheep did.
    rule_counts: RuleCounts,
}

/// The rule, on one superchunk's turn: every sheep waking, each seen to
/// by [`wake`].
pub fn rule(turn: &mut Turn) -> RuleCounts {
    let mut flock = Flock::default();
    entities::each_woken(turn, [GRASS, WALL_NORTH, WALL_WEST, WALL_SOUTH, WALL_EAST, COLLISION], &mut flock, wake);
    flock.rule_counts
}

/// The rule, on one sheep waking: it sees to what it woke for -- a
/// meal, a lamb, growing up -- and sleeps again, as long as it can.
#[inline]
fn wake(turn: &mut Turn, woken_sheep: EntityRef, flock: &mut Flock) {
    let (rule_counts, tick_now) = (&mut flock.rule_counts, this_tick::current_tick(turn));
    rule_counts[WOKEN_COUNT] += 1;
    let (sheep_cell, header_before) = (woken_sheep.header.stood_cell, woken_sheep.header);
    let about_tiles = around::tiles_about_cell(sheep_cell);
    let grass_about = around::read_layer(turn, GRASS, &about_tiles);
    // The neighbours it may step to: on the hot bitplanes, nothing standing there, no wall before them.
    let steppable_cells = grass_about.hot & walking::around_steppable(turn, &about_tiles);
    // All it keeps, as the tick found it, and as this wake leaves it: written once, if it changed.
    let state_seen = woken_sheep.attribute(SHEEP_STATE);
    let state_was = state_seen.unwrap_or(SheepState { hungry_at: tick_now, ..SheepState::default() });
    let mut state_after = state_was;
    let (hungry_at, is_roaming) = (state_was.hungry_at, state_was.roaming_until != 0);
    // On its way, roaming, it does not stop to eat.
    let eats_now = tick_now >= hungry_at && !(grass_about.set_cells & about_tiles.centre_cell()).is_empty() && !is_roaming;
    if !eats_now && tick_now >= TIME_TO_STARVE.tick_after(hungry_at) {
        entities::remove(turn, &header_before);
        rule_counts[DEATHS_COUNT] += 1;
        return;
    }
    // The pasture about it, looked at as it eats: thin, it will leave when next hungry; lush, it may.
    let on_lush_pasture = eats_now && area::read_layer(turn, GRASS, sheep_cell).count() >= LUSH_CELLS;
    if eats_now {
        // All the sheep comes to this wake hangs on the grass being there still when it is applied: each thing queued from here is held
        // against it, and the grass cleared last. Should it be gone by then -- decayed in this very tick -- none of it happens, and
        // what is held against the grass being gone does instead: the sheep sleeps a step, hungry as it was, and wakes to look again.
        compare::from_here(turn, compare::layer_holds(GRASS, sheep_cell));
        state_after.hungry_at = TIME_BETWEEN_MEALS.tick_after(tick_now);
        // Off thin pasture always; off lush, now and then: a flock spreads over the land it could stay on.
        let sets_off_roaming = !on_lush_pasture || this_tick::random_stream(turn).chance(ROAM_CHANCE);
        if let (true, Some(way)) = (sets_off_roaming, around::pick_way(this_tick::random_stream(turn), &about_tiles, steppable_cells)) {
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
            birth_cell = around::pick_way(this_tick::random_stream(turn), &about_tiles, steppable_cells).and_then(|birth_way| around::cell(sheep_cell, birth_way));
            next_need_tick = tick_now;
        }
        None if on_lush_pasture && grown_at.is_none() && this_tick::random_stream(turn).chance(CONCEIVE_CHANCE) => {
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
        around::prefer_way(this_tick::random_stream(turn), &about_tiles, way, steppable_cells)
    } else if !(grass_about.set_cells & steppable_cells).is_empty() {
        around::pick_way(this_tick::random_stream(turn), &about_tiles, grass_about.set_cells & steppable_cells)
    } else if steppable_cells.is_empty() {
        // Hemmed in: no step to take, and no path to look for.
        None
    } else {
        rule_counts[SOUGHT_COUNT] += 1;
        match walking::seek(turn, sheep_cell, GRASS) {
            Some(found_step) => {
                rule_counts[PATHS] += 1;
                rule_counts[FAR_COUNT] += u64::from(found_step.scale > 0);
                Some(around::way_of(sheep_cell, found_step.step_cell))
            }
            None => around::pick_way(this_tick::random_stream(turn), &about_tiles, steppable_cells),
        }
    };
    let step_cell = way.and_then(|way| around::cell(sheep_cell, way)).unwrap_or(sheep_cell);
    let wake = if is_hungry {
        next_step(turn)
    } else {
        next_step(turn).max(STEP_JITTER.part_drawn(this_tick::random_stream(turn)).tick_after(next_need_tick))
    };
    // Old age comes by the time slept, not the wake: a long sleep is as much of a life as many short ones.
    if MEAN_LIFETIME.part_drawn(this_tick::random_stream(turn)) < SimulationTime::time_between_ticks(tick_now, wake) {
        entities::remove(turn, &header_before);
        count(turn, rule_counts, eats_now.then_some(sheep_cell), DEATHS_COUNT);
    } else {
        // Written from where it is worked out, the words that changed alone: nothing of the sheep is copied to be changed.
        if state_after != state_was {
            entities::set_attribute_of(turn, &header_before, SHEEP_STATE, state_seen, state_after);
        }
        entities::step(turn, &header_before, step_cell, wake);
        if let Some(lamb_cell) = birth_cell {
            bear_lamb(turn, &header_before, state_after, lamb_cell);
        }
    }
    if eats_now {
        // One of the two is applied, never both: the sheep is written once.
        let hungry_still = next_step(turn);
        compare::from_here(turn, compare::layer_lacks(GRASS, sheep_cell));
        entities::sleep(turn, &header_before, hungry_still);
        compare::end_from_here(turn);
        cells::clear_counted(turn, GRASS, sheep_cell, EATEN_COUNT);
    }
}

/// Queues the sheep `mother_sheep`, whose state is `mother_state` once what its
/// wake queued is applied, bearing its lamb on `lamb_cell`: the lamb
/// made, the mother no longer pregnant and the birth counted, all of
/// it or none -- under one compare of the mother being as this wake
/// left it and, within it, one of the lamb's cell being free. So no
/// lamb is born twice and none is lost, whichever superchunk its
/// cell is in: the cell taken first, the mother is pregnant still.
fn bear_lamb(turn: &mut Turn, mother_sheep: &Header, mother_state: SheepState, lamb_cell: CellIndex) {
    let tick_now = this_tick::current_tick(turn);
    let (wake, lamb_state) = (next_step(turn), SheepState { hungry_at: TIME_BETWEEN_MEALS.tick_after(tick_now), grown_at: LAMBHOOD_TIME.tick_after(tick_now), ..SheepState::default() });
    let mother_delivered = SheepState { lamb_due_at: 0, ..mother_state };
    compare::from_here(turn, compare::attribute(mother_sheep, SHEEP_STATE, Some(mother_state)));
    entities::begin_making_entity(turn, SHEEP, lamb_cell, wake, &WholeAttribute::holding_value(SHEEP_STATE, lamb_state));
    entities::set_attribute_of(turn, mother_sheep, SHEEP_STATE, Some(mother_state), mother_delivered);
    compare::count(turn, compare::attribute(mother_sheep, SHEEP_STATE, Some(mother_delivered)), BIRTHS_COUNT);
    entities::end_making_entity(turn);
    compare::end_from_here(turn);
}

/// Counts one at `place` for the wake of a sheep: if it is `meal_cell`, on
/// the cell given, as it is applied, the grass there still -- what it
/// did then happens with the meal or not at all -- else here.
fn count(turn: &mut Turn, rule_counts: &mut RuleCounts, meal_cell: Option<CellIndex>, place: usize) {
    match meal_cell {
        Some(grass_cell) => compare::count(turn, compare::layer_holds(GRASS, grass_cell), place),
        None => rule_counts[place] += 1,
    }
}

/// The tick a sheep taking a step now wakes next.
fn next_step(turn: &mut Turn) -> u64 {
    let tick_now = this_tick::current_tick(turn);
    STEP_JITTER.part_drawn(this_tick::random_stream(turn)).tick_after(STEP.tick_after(tick_now))
}

/// Queues `count` grown sheep, each some way from its next meal, each
/// on a cell of its own of `superchunk` drawn from `random_stream` -- at most
/// half its cells' worth of them -- waking over the next [`STEP`]: in the world once whoever runs it places what was put. A
/// cell that is not free -- a tree's -- is drawn again; fewer are put
/// only where a superchunk has almost no free cell left.
pub fn flock(entities_between_ticks: &mut EntitiesBetweenTicks, superchunk: SuperchunkIndex, count: usize, random_stream: &mut RandomSource) {
    let CellCartesian { x: superchunk_left_x, y: superchunk_top_y } = superchunk.top_left().cartesian();
    let side_in_cells = SUPERCHUNK_SIDE_CELLS as u64;
    assert!(count as u64 <= side_in_cells * side_in_cells / 2, "{count} sheep on a superchunk: too many to draw a cell each");
    let tick_now = entities_between_ticks.current_tick();
    let mut cells_drawn = HashSet::with_capacity(count);
    let (mut put, mut draws_left) = (0, 16 * count as u64 + side_in_cells * side_in_cells);
    while put < count && draws_left > 0 {
        draws_left -= 1;
        let drawn_cartesian = CellCartesian { x: superchunk_left_x + random_stream.draw_below(side_in_cells) as u32, y: superchunk_top_y + random_stream.draw_below(side_in_cells) as u32 };
        // A cell drawn twice is drawn again: a cell holds one sheep.
        if !cells_drawn.insert((drawn_cartesian.x, drawn_cartesian.y)) {
            continue;
        }
        let header = Header { entity_type: SHEEP, stood_cell: drawn_cartesian.into(), wake: STEP.part_drawn(random_stream).tick_after(tick_now) };
        put += usize::from(entities_between_ticks.put(header, &WholeAttribute::holding_value(SHEEP_STATE, SheepState { hungry_at: TIME_BETWEEN_MEALS.part_drawn(random_stream).tick_after(tick_now), ..SheepState::default() })));
    }
}
