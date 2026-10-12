//! Sheep on the grass: a sheep sleeps until it next needs something --
//! a meal, its lamb, growing up -- and wakes for that alone; hungry, it
//! walks to grass, a step a wake (`docs/entity_rules.md`, "The sheep").

use actions::around;
use actions::entities::EntitiesBetweenTicks;
pub use actions::types::{SheepState, SHEEP, SHEEP_STATE};
use actions::simulation_time::SimulationTime;
use actions::types::{COLLISION, GRASS, WALL_EAST, WALL_NORTH, WALL_SOUTH, WALL_WEST};
use actions::{WholeAttribute, Chance, area, cells, compare, entities, place_counted, this_tick, walking, CellCartesian, CellIndex, EntityRef, Header, Rng, RuleCounts, SuperchunkIndex, Turn, SUPERCHUNK_SIDE_CELLS};
use std::collections::HashSet;

/// Every time below is said in simulation time: the rule adds them to
/// the tick it runs in and never says how many ticks they are -- the
/// same sheep at any tick rate.
///
/// The time between a walking sheep's steps, at the least: a quarter
/// of a second...
pub const STEP: SimulationTime = SimulationTime::milliseconds(250);
/// ...and up to this much more, a sixteenth of a second, drawn each
/// wake, so the flock's wakes spread over the ticks.
pub const STEP_JITTER: SimulationTime = SimulationTime::milliseconds(62);
/// The time from a meal to being hungry again: until then a sheep
/// eats nothing, though it stands on grass, and sleeps.
pub const MEAL: SimulationTime = SimulationTime::seconds(15);
/// The time a hungry sheep finds no meal in before it starves.
pub const STARVATION: SimulationTime = SimulationTime::seconds(90);
/// Cells of grass among the 16 by 16 about a sheep for the pasture
/// to be lush enough to breed on: a quarter of them
/// (`docs/entity_rules.md`, "The sheep").
pub const LUSH_CELLS: u32 = 64;
/// The simulation time a grown sheep eating on lush pasture takes to
/// fall pregnant, on average: with its gestation, four lambs or so
/// in the grown part of a life spent on lush pasture -- every sheep
/// bears, so that is a ewe's eight.
pub const MEAN_TIME_TO_CONCEIVE: SimulationTime = SimulationTime::seconds(800);
/// The chance, each meal of a grown sheep on lush pasture, that it
/// falls pregnant.
pub const CONCEIVE_CHANCE: Chance = MEAN_TIME_TO_CONCEIVE.chance_each(MEAL);
/// The sleep there is to a sheep's life, on average: before a sleep
/// it dies of old age at that sleep's part of this. An hour and a
/// quarter: a sheep's ten years or so, a year of them seven and a
/// half minutes -- what the times below are worked out from.
pub const LIFE: SimulationTime = SimulationTime::seconds(4_500);
/// The time a sheep that set off roaming walks for, one way, once it
/// is hungry again, before it looks for grass: 96 steps or so, out
/// of the patch its flock has grazed.
pub const ROAM: SimulationTime = SimulationTime::seconds(27);
/// The simulation time a sheep eating on lush pasture takes to set
/// off roaming all the same, on average: on thin pasture it sets off
/// at every meal.
pub const MEAN_TIME_TO_ROAM: SimulationTime = SimulationTime::seconds(54);
/// The chance, each meal on lush pasture, that the sheep sets off
/// roaming when next hungry.
pub const ROAM_CHANCE: Chance = MEAN_TIME_TO_ROAM.chance_each(MEAL);
/// The time from falling pregnant to giving birth: five months of
/// such a year.
pub const GESTATION: SimulationTime = SimulationTime::seconds(190);
/// The time a lamb takes to grow: a year.
pub const LAMBHOOD: SimulationTime = SimulationTime::seconds(450);

/// What the rule counts, each named at its place in its
/// [`RuleCounts`]: the one order, the places below worked out from it.
pub const COUNTED: [&str; 7] = ["woken", "eaten", "births", "deaths", "sought", "paths", "far"];
/// Sheep woken.
pub const WOKEN: usize = place_counted(&COUNTED, "woken");
/// Cells of grass eaten: counted as each meal is applied.
pub const EATEN: usize = place_counted(&COUNTED, "eaten");
/// Lambs born.
pub const BIRTHS: usize = place_counted(&COUNTED, "births");
/// Sheep dead: starved, or of old age.
pub const DEATHS: usize = place_counted(&COUNTED, "deaths");
/// Paths to grass looked for, by hungry sheep with none beside them.
pub const SOUGHT: usize = place_counted(&COUNTED, "sought");
/// Of those, found.
pub const PATHS: usize = place_counted(&COUNTED, "paths");
/// Of those found, the ones beyond the area about the sheep.
pub const FAR: usize = place_counted(&COUNTED, "far");

/// What the rule keeps over a superchunk's turn: what the sheep did.
#[derive(Default)]
struct Flock {
    /// What the sheep did.
    done: RuleCounts,
}

/// The rule, on one superchunk's turn: every sheep waking, each seen to
/// by [`wake`].
pub fn rule(turn: &mut Turn) -> RuleCounts {
    let mut flock = Flock::default();
    entities::each_woken(turn, [GRASS, WALL_NORTH, WALL_WEST, WALL_SOUTH, WALL_EAST, COLLISION], &mut flock, wake);
    flock.done
}

/// The rule, on one sheep waking: it sees to what it woke for -- a
/// meal, a lamb, growing up -- and sleeps again, as long as it can.
#[inline]
fn wake(turn: &mut Turn, sheep: EntityRef, flock: &mut Flock) {
    let (done, now) = (&mut flock.done, this_tick::now(turn));
    done[WOKEN] += 1;
    let (at, before) = (sheep.header.at, sheep.header);
    let about = around::about(at);
    let grass = around::layer(turn, GRASS, &about);
    // The neighbours it may step to: on the hot bitplanes, nothing standing there, no wall before them.
    let steppable = grass.hot & walking::around_steppable(turn, &about);
    // All it keeps, as the tick found it, and as this wake leaves it: written once, if it changed.
    let seen = sheep.attribute(SHEEP_STATE);
    let was = seen.unwrap_or(SheepState { hungry_at: now, ..SheepState::default() });
    let mut state = was;
    let (hungry_at, roaming) = (was.hungry_at, was.roaming_until != 0);
    // On its way, roaming, it does not stop to eat.
    let fed = now >= hungry_at && !(grass.set & about.centre()).is_empty() && !roaming;
    if !fed && now >= STARVATION.after(hungry_at) {
        entities::remove(turn, &before);
        done[DEATHS] += 1;
        return;
    }
    // The pasture about it, looked at as it eats: thin, it will leave when next hungry; lush, it may.
    let lush = fed && area::layer(turn, GRASS, at).count() >= LUSH_CELLS;
    if fed {
        // All the sheep comes to this wake hangs on the grass being there still when it is applied: each thing queued from here is held
        // against it, and the grass cleared last. Should it be gone by then -- decayed in this very tick -- none of it happens, and
        // what is held against the grass being gone does instead: the sheep sleeps a step, hungry as it was, and wakes to look again.
        compare::from_here(turn, compare::holds(GRASS, at));
        state.hungry_at = MEAL.after(now);
        // Off thin pasture always; off lush, now and then: a flock spreads over the land it could stay on.
        let roams = !lush || this_tick::random(turn).chance(ROAM_CHANCE);
        if let (true, Some(way)) = (roams, around::pick(this_tick::random(turn), &about, steppable)) {
            (state.roaming_until, state.roaming_neighbour) = (ROAM.after(MEAL.after(now)), way);
        }
    }
    let hungry = !fed && now >= hungry_at;
    // What it next has to wake for, were it to sleep as long as it can.
    let mut needs = if fed { MEAL.after(now) } else { hungry_at };
    let grown_at = (was.grown_at != 0).then_some(was.grown_at);
    // The cell it bears its lamb on this tick, if it does: it stays where it is, the cell not to be the one it steps to.
    let mut bearing = None;
    match (state.lamb_due_at != 0).then_some(state.lamb_due_at) {
        Some(due) if now < due => needs = needs.min(due),
        // Eating, it bears at its next wake: the meal is one thing to happen whole, the birth another.
        Some(_) if fed => needs = now,
        // Its lamb is born on a cell seen free beside it, drawn here and queued last ([`bear`]); with none free, or that one taken
        // first, it is pregnant still, and tries again a step's time on.
        Some(_) => {
            bearing = around::pick(this_tick::random(turn), &about, steppable).and_then(|beside| around::cell(at, beside));
            needs = now;
        }
        None if lush && grown_at.is_none() && this_tick::random(turn).chance(CONCEIVE_CHANCE) => {
            state.lamb_due_at = GESTATION.after(now);
            needs = needs.min(state.lamb_due_at);
        }
        None => {}
    }
    match grown_at {
        Some(grown_at) if now >= grown_at => state.grown_at = 0,
        Some(grown_at) => needs = needs.min(grown_at),
        None => {}
    }
    // Hungry, it walks; satisfied, it stays, and sleeps until it needs something.
    let way = if !hungry || bearing.is_some() {
        None
    } else if roaming {
        // On the way it set off, until its time is up or it comes to the edge of the hot superchunks.
        let way = about.towards(was.roaming_neighbour);
        if now >= was.roaming_until || (steppable & way).is_empty() {
            (state.roaming_until, state.roaming_neighbour) = (0, 0);
        }
        around::prefer(this_tick::random(turn), &about, way, steppable)
    } else if !(grass.set & steppable).is_empty() {
        around::pick(this_tick::random(turn), &about, grass.set & steppable)
    } else if steppable.is_empty() {
        // Hemmed in: no step to take, and no path to look for.
        None
    } else {
        done[SOUGHT] += 1;
        match walking::seek(turn, at, GRASS) {
            Some(found) => {
                done[PATHS] += 1;
                done[FAR] += u64::from(found.scale > 0);
                Some(around::way_of(at, found.to))
            }
            None => around::pick(this_tick::random(turn), &about, steppable),
        }
    };
    let to = way.and_then(|way| around::cell(at, way)).unwrap_or(at);
    let wake = if hungry {
        next_step(turn)
    } else {
        next_step(turn).max(STEP_JITTER.part_drawn(this_tick::random(turn)).after(needs))
    };
    // Old age comes by the time slept, not the wake: a long sleep is as much of a life as many short ones.
    if LIFE.part_drawn(this_tick::random(turn)) < SimulationTime::between(now, wake) {
        entities::remove(turn, &before);
        count(turn, done, fed.then_some(at), DEATHS);
    } else {
        // Written from where it is worked out, the words that changed alone: nothing of the sheep is copied to be changed.
        if state != was {
            entities::set_attribute_of(turn, &before, SHEEP_STATE, seen, state);
        }
        entities::step(turn, &before, to, wake);
        if let Some(beside) = bearing {
            bear(turn, &before, state, beside);
        }
    }
    if fed {
        // One of the two is applied, never both: the sheep is written once.
        let hungry_still = next_step(turn);
        compare::from_here(turn, compare::lacks(GRASS, at));
        entities::sleep(turn, &before, hungry_still);
        compare::end(turn);
        cells::clear_counted(turn, GRASS, at, EATEN);
    }
}

/// Queues the sheep `mother`, whose state is `state` once what its
/// wake queued is applied, bearing its lamb on `beside`: the lamb
/// made, the mother no longer pregnant and the birth counted, all of
/// it or none -- under one compare of the mother being as this wake
/// left it and, within it, one of the lamb's cell being free. So no
/// lamb is born twice and none is lost, whichever superchunk its
/// cell is in: the cell taken first, the mother is pregnant still.
fn bear(turn: &mut Turn, mother: &Header, state: SheepState, beside: CellIndex) {
    let now = this_tick::now(turn);
    let (wake, born) = (next_step(turn), SheepState { hungry_at: MEAL.after(now), grown_at: LAMBHOOD.after(now), ..SheepState::default() });
    let delivered = SheepState { lamb_due_at: 0, ..state };
    compare::from_here(turn, compare::attribute(mother, SHEEP_STATE, Some(state)));
    entities::make(turn, SHEEP, beside, wake, &WholeAttribute::holding(SHEEP_STATE, born));
    entities::set_attribute_of(turn, mother, SHEEP_STATE, Some(state), delivered);
    compare::count(turn, compare::attribute(mother, SHEEP_STATE, Some(delivered)), BIRTHS);
    entities::made(turn);
    compare::end(turn);
}

/// Counts one at `place` for the wake of a sheep: if it is `fed`, on
/// the cell given, as it is applied, the grass there still -- what it
/// did then happens with the meal or not at all -- else here.
fn count(turn: &mut Turn, done: &mut RuleCounts, fed: Option<CellIndex>, place: usize) {
    match fed {
        Some(at) => compare::count(turn, compare::holds(GRASS, at), place),
        None => done[place] += 1,
    }
}

/// The tick a sheep taking a step now wakes next.
fn next_step(turn: &mut Turn) -> u64 {
    let now = this_tick::now(turn);
    STEP_JITTER.part_drawn(this_tick::random(turn)).after(STEP.after(now))
}

/// Queues `count` grown sheep, each some way from its next meal, each
/// on a cell of its own of `superchunk` drawn from `random` -- at most
/// half its cells' worth of them -- waking over the next [`STEP`]: in the world once whoever runs it places what was put. A
/// cell that is not free -- a tree's -- is drawn again; fewer are put
/// only where a superchunk has almost no free cell left.
pub fn flock(entities: &mut EntitiesBetweenTicks, superchunk: SuperchunkIndex, count: usize, random: &mut Rng) {
    let CellCartesian { x: left, y: top } = superchunk.top_left().cartesian();
    let side = SUPERCHUNK_SIDE_CELLS as u64;
    assert!(count as u64 <= side * side / 2, "{count} sheep on a superchunk: too many to draw a cell each");
    let now = entities.now();
    let mut taken = HashSet::with_capacity(count);
    let (mut put, mut draws_left) = (0, 16 * count as u64 + side * side);
    while put < count && draws_left > 0 {
        draws_left -= 1;
        let at = CellCartesian { x: left + random.below(side) as u32, y: top + random.below(side) as u32 };
        // A cell drawn twice is drawn again: a cell holds one sheep.
        if !taken.insert((at.x, at.y)) {
            continue;
        }
        let header = Header { kind: SHEEP, at: at.into(), wake: STEP.part_drawn(random).after(now) };
        put += usize::from(entities.put(header, &WholeAttribute::holding(SHEEP_STATE, SheepState { hungry_at: MEAL.part_drawn(random).after(now), ..SheepState::default() })));
    }
}
