//! Sheep on the grass: a sheep sleeps until it next needs something --
//! a meal, its lamb, growing up -- and wakes for that alone; hungry, it
//! walks to grass, a step a wake (`docs/entity_rules.md`, "The sheep").

use instructions::around::{self, CENTRE, RING};
use instructions::entities::EntitiesBetweenTicks;
pub use instructions::entity_types::{Roaming, HUNGRY_AT, LAMB, PREGNANT, ROAMING, SHEEP};
use instructions::layers::{GRASS, WALL_EAST, WALL_SOUTH};
use instructions::{area, cells, entities, walking, AttributeBlock, CellCartesian, EntityEdit, EntityId, EntityRef, Header, Rng, RuleCounts, SuperchunkIndex, Turn, SUPERCHUNK_SIDE_CELLS};
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

/// What the rule counts, each named at its place in its [`RuleCounts`].
pub const COUNTED: [&str; 7] = ["woken", "eaten", "births", "deaths", "sought", "paths", "far"];
/// Sheep woken.
pub const WOKEN: usize = 0;
/// Cells of grass eaten.
pub const EATEN: usize = 1;
/// Lambs born.
pub const BIRTHS: usize = 2;
/// Sheep dead: starved, or of old age.
pub const DEATHS: usize = 3;
/// Paths to grass looked for, by hungry sheep with none beside them.
pub const SOUGHT: usize = 4;
/// Of those, found.
pub const PATHS: usize = 5;
/// Of those found, the ones beyond the area about the sheep.
pub const FAR: usize = 6;

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
    let (done, room, now) = (&mut flock.done, &mut flock.room, turn.now());
    done[WOKEN] += 1;
    let at = sheep.header.at;
    let mut sheep = EntityEdit::of(sheep, room);
    let grass = around::layer(turn, GRASS, at);
    // The neighbours it may step to: on the hot bitplanes, no wall before them. Where entities stand is not read.
    let mut steppable = grass.hot & RING & walking::around_unwalled(turn, at);
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
        cells::clear(turn, GRASS, at);
        sheep.set(HUNGRY_AT, now + MEAL_TICKS);
        done[EATEN] += 1;
        if let (false, Some(way)) = (lush, around::pick(turn.random(), steppable)) {
            sheep.set(ROAMING, Roaming { until: now + MEAL_TICKS + ROAM_TICKS, neighbour: way });
        }
    }
    let hungry = !fed && now >= hungry_at;
    // What it next has to wake for, were it to sleep as long as it can.
    let mut needs = if fed { now + MEAL_TICKS } else { hungry_at };
    let grown_at = sheep.get(LAMB);
    match sheep.get(PREGNANT) {
        Some(due) if now < due => needs = needs.min(due),
        // Its lamb is born on a cell seen free beside it; with none, it waits a step's time more.
        Some(_) => match around::free_beside(turn, at, steppable) {
            Some(beside) => {
                sheep.unset(PREGNANT);
                // The lamb's cell is no longer one to step to.
                steppable &= !(1 << beside);
                let (cell, wake) = (around::cell(at, beside).expect("a hot neighbour is in the world"), next_step(turn));
                entities::spawn(turn, SHEEP, cell, wake, &[AttributeBlock::holding(HUNGRY_AT, now + MEAL_TICKS), AttributeBlock::holding(LAMB, now + LAMB_TICKS)]);
                done[BIRTHS] += 1;
            }
            None => needs = now,
        },
        None if lush && grown_at.is_none() && turn.random().below(CONCEIVE_ONE_IN) == 0 => {
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
    let way = if !hungry {
        None
    } else if let Some(roaming) = roaming {
        // On the way it set off, until its time is up or it comes to the edge of the hot superchunks.
        let way = 1 << roaming.neighbour;
        if now >= roaming.until || steppable & way == 0 {
            sheep.unset(ROAMING);
        }
        around::prefer(turn.random(), way, steppable)
    } else if grass.set & steppable != 0 {
        around::pick(turn.random(), grass.set & steppable)
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
            None => around::pick(turn.random(), steppable),
        }
    };
    let to = way.and_then(|way| around::cell(at, way)).unwrap_or(at);
    let wake = if hungry { next_step(turn) } else { next_step(turn).max(needs + turn.random().below(STEP_JITTER)) };
    // Old age comes by the tick, not the wake: a long sleep is as much of a life as many short ones.
    if turn.random().below(LIFE_TICKS) < wake - now {
        entities::remove(turn, sheep.header());
        done[DEATHS] += 1;
        return;
    }
    entities::commit(turn, sheep, to, wake);
}

/// The tick a sheep taking a step now wakes next.
fn next_step(turn: &mut Turn) -> u64 {
    turn.now() + STEP_TICKS + turn.random().below(STEP_JITTER)
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
