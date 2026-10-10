//! Sheep on the grass: a sheep sleeps until it next needs something --
//! a meal, its lamb, growing up -- and wakes for that alone; hungry, it
//! walks to grass, a step a wake (`docs/entity_rules.md`, "The sheep").

use instructions::around::{self, CENTRE, RING};
use instructions::entities::EntitiesBetweenTicks;
pub use instructions::entity_types::{Roaming, BEARING, HUNGRY_AT, LAMB, PREGNANT, ROAMING, SHEEP};
use instructions::layers::{GRASS, WALL_EAST, WALL_SOUTH};
use instructions::{area, cells, compare, entities, place_counted, this_tick, walking, AttributeBlock, CellCartesian, CellIndex, EntityEdit, EntityId, EntityRef, Header, Rng, RuleCounts, SuperchunkIndex, Turn, SUPERCHUNK_SIDE_CELLS};
use std::collections::HashSet;

/// Ticks between a walking sheep's steps, at the least...
pub const STEP_TICKS: u64 = 64;
/// ...and up to this many more, drawn each wake, so the flock's wakes
/// spread over the ticks.
pub const STEP_JITTER: u64 = 16;
/// Ticks after a meal a sheep is hungry again at: until then it eats
/// nothing, though it stands on grass, and sleeps.
pub const MEAL_TICKS: u64 = 6912;
/// Ticks a hungry sheep finds no meal in before it starves.
pub const STARVE_TICKS: u64 = 13_824;
/// Cells of grass among the 16 by 16 about a sheep for the pasture
/// to be lush enough to breed on: a quarter of them
/// (`docs/entity_rules.md`, "The sheep").
pub const LUSH_CELLS: u32 = 64;
/// A grown sheep falls pregnant at one meal on lush pasture in this
/// many.
pub const CONCEIVE_ONE_IN: u64 = 5;
/// Ticks of sleep to a sheep's life, on average: before a sleep of so
/// many ticks it dies of old age at so many in these.
pub const LIFE_TICKS: u64 = 172_800;
/// Ticks a sheep that ate on thin pasture walks for, one way, once it
/// is hungry again, before it looks for grass: 48 steps or so, out of
/// the patch its flock has grazed.
pub const ROAM_TICKS: u64 = 3456;
/// Ticks from falling pregnant to giving birth.
pub const GESTATION_TICKS: u64 = 1152;
/// Ticks a lamb takes to grow.
pub const LAMB_TICKS: u64 = 4608;

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

/// What the rule keeps over a superchunk's turn: what the sheep did,
/// and room for a sheep's attributes as they are changed, made once.
#[derive(Default)]
struct Flock {
    /// What the sheep did.
    done: RuleCounts,
    /// Room for the attributes of the sheep being changed.
    room: Vec<AttributeBlock>,
}

/// The rule, on one superchunk's turn: every sheep waking, each seen to
/// by [`wake`].
pub fn rule(turn: &mut Turn) -> RuleCounts {
    let mut flock = Flock::default();
    entities::each_woken(turn, [GRASS, WALL_EAST, WALL_SOUTH], &mut flock, wake);
    flock.done
}

/// The rule, on one sheep waking: it sees to what it woke for -- a
/// meal, a lamb, growing up -- and sleeps again, as long as it can.
#[inline]
fn wake(turn: &mut Turn, sheep: EntityRef, flock: &mut Flock) {
    let (done, room, now) = (&mut flock.done, &mut flock.room, this_tick::now(turn));
    done[WOKEN] += 1;
    let (at, before) = (sheep.header.at, sheep.header);
    let mut sheep = EntityEdit::of(sheep, room);
    let grass = around::layer(turn, GRASS, at);
    // The neighbours it may step to: on the hot bitplanes, no wall before them. Where entities stand is not read.
    let steppable = grass.hot & RING & walking::around_unwalled(turn, at);
    let hungry_at = sheep.get(HUNGRY_AT).unwrap_or(now);
    let roaming = sheep.get(ROAMING);
    // On its way out of thin pasture it does not stop to eat.
    let fed = now >= hungry_at && grass.set & CENTRE != 0 && roaming.is_none();
    if !fed && now >= hungry_at + STARVE_TICKS {
        entities::remove(turn, sheep.header());
        done[DEATHS] += 1;
        return;
    }
    // The pasture about it, looked at as it eats: thin, it will leave when next hungry.
    let lush = fed && area::layer(turn, GRASS, at).count() >= LUSH_CELLS;
    if fed {
        // All the sheep comes to this wake hangs on the grass being there still when it is applied: each thing queued from here is held
        // against it, and the grass cleared last. Should it be gone by then -- decayed in this very tick -- none of it happens, and
        // what is held against the grass being gone does instead: the sheep sleeps a step, hungry as it was, and wakes to look again.
        compare::entities_from_here(turn, compare::holds(GRASS, at));
        sheep.set(HUNGRY_AT, now + MEAL_TICKS);
        if let (false, Some(way)) = (lush, around::pick(this_tick::random(turn), steppable)) {
            sheep.set(ROAMING, Roaming { until: now + MEAL_TICKS + ROAM_TICKS, neighbour: way });
        }
    }
    let hungry = !fed && now >= hungry_at;
    // What it next has to wake for, were it to sleep as long as it can.
    let mut needs = if fed { now + MEAL_TICKS } else { hungry_at };
    let grown_at = sheep.get(LAMB);
    // The lamb it put beside it last tick stands there, and is born; or its cell was taken first, and it is pregnant still.
    if let Some(lamb) = sheep.unset(BEARING)
        && entities::stands_beside(turn, EntityId(lamb), at, RING).is_some()
    {
        sheep.unset(PREGNANT);
        count(turn, done, fed.then_some(at), BIRTHS);
    }
    // Whether it put a lamb this tick: it stays, and wakes the next to see it.
    let mut bearing = false;
    match sheep.get(PREGNANT) {
        Some(due) if now < due => needs = needs.min(due),
        // Eating, it bears at its next wake: a lamb may land in the next superchunk, where the grass under its mother cannot be held against it.
        Some(_) if fed => needs = now,
        // Its lamb is put on a cell seen free beside it; with none, it waits a step's time more.
        Some(_) => match around::free_beside(turn, at, steppable) {
            Some(beside) => {
                // Beside it on the cell drawn, or, that taken first, on another it may step to.
                let wake = next_step(turn);
                let lamb = entities::spawn_beside(turn, SHEEP, at, beside, steppable, wake, &[AttributeBlock::holding(HUNGRY_AT, now + MEAL_TICKS), AttributeBlock::holding(LAMB, now + LAMB_TICKS)]);
                sheep.set(BEARING, lamb.0);
                bearing = true;
            }
            None => needs = now,
        },
        None if lush && grown_at.is_none() && this_tick::random(turn).below(CONCEIVE_ONE_IN) == 0 => {
            sheep.set(PREGNANT, now + GESTATION_TICKS);
            needs = needs.min(now + GESTATION_TICKS);
        }
        None => {}
    }
    match grown_at {
        Some(grown_at) if now >= grown_at => _ = sheep.unset(LAMB),
        Some(grown_at) => needs = needs.min(grown_at),
        None => {}
    }
    // Hungry, it walks; satisfied, it stays, and sleeps until it needs something.
    let way = if !hungry || bearing {
        None
    } else if let Some(roaming) = roaming {
        // On the way it set off, until its time is up or it comes to the edge of the hot superchunks.
        let way = 1 << roaming.neighbour;
        if now >= roaming.until || steppable & way == 0 {
            sheep.unset(ROAMING);
        }
        around::prefer(this_tick::random(turn), way, steppable)
    } else if grass.set & steppable != 0 {
        around::pick(this_tick::random(turn), grass.set & steppable)
    } else if steppable == 0 {
        // Hemmed in: no step to take, and no path to look for.
        None
    } else {
        done[SOUGHT] += 1;
        match walking::seek(turn, at, GRASS) {
            Some(found) => {
                done[PATHS] += 1;
                done[FAR] += u64::from(found.scale > 0);
                Some(around::bit_of(at, found.to))
            }
            None => around::pick(this_tick::random(turn), steppable),
        }
    };
    let to = way.and_then(|way| around::cell(at, way)).unwrap_or(at);
    let wake = if bearing {
        now + 1
    } else if hungry {
        next_step(turn)
    } else {
        next_step(turn).max(needs + this_tick::random(turn).below(STEP_JITTER))
    };
    // Old age comes by the tick, not the wake: a long sleep is as much of a life as many short ones.
    if this_tick::random(turn).below(LIFE_TICKS) < wake - now {
        entities::remove(turn, sheep.header());
        count(turn, done, fed.then_some(at), DEATHS);
    } else {
        entities::commit(turn, sheep, to, wake);
    }
    if fed {
        // One of the two is applied, never both: the sheep is written once.
        let hungry_still = next_step(turn);
        compare::entities_from_here(turn, compare::lacks(GRASS, at));
        entities::sleep(turn, &before, hungry_still);
        compare::entities_as_ever(turn);
        cells::clear_counted(turn, GRASS, at, EATEN);
    }
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
    this_tick::now(turn) + STEP_TICKS + this_tick::random(turn).below(STEP_JITTER)
}

/// Queues `count` grown sheep, each some way from its next meal, each
/// on a cell of its own of `superchunk` drawn from `random` -- at most
/// half its cells' worth of them -- waking over the next [`STEP_TICKS`]
/// ticks: in the world once whoever runs it places what was put.
pub fn flock(entities: &mut EntitiesBetweenTicks, superchunk: SuperchunkIndex, count: usize, random: &mut Rng) {
    let CellCartesian { x: left, y: top } = superchunk.top_left().cartesian();
    let side = SUPERCHUNK_SIDE_CELLS as u64;
    assert!(count as u64 <= side * side / 2, "{count} sheep on a superchunk: too many to draw a cell each");
    let now = entities.now();
    let mut taken = HashSet::with_capacity(count);
    while taken.len() < count {
        let at = CellCartesian { x: left + random.below(side) as u32, y: top + random.below(side) as u32 };
        // A cell drawn twice is drawn again: a cell holds one sheep.
        if !taken.insert((at.x, at.y)) {
            continue;
        }
        let header = Header { id: EntityId(random.draw()), kind: SHEEP, at: at.into(), wake: now + random.below(STEP_TICKS) };
        entities.put(header, &[AttributeBlock::holding(HUNGRY_AT, now + random.below(MEAL_TICKS))]);
    }
}
