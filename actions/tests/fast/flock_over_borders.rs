//! A flock over the borders of chunks and superchunks comes to the
//! same on one thread as on all of them, tick after tick: what it
//! queues is applied in the one order of the world, Morton order of
//! its authors' cells, however the superchunks are gone over
//! (`docs/actions.md`, "The same wherever the borders fall").
//!
//! All of it is the test's own, and nothing outside it is made for it:
//! where the flock is put, drawn from the seed so that other borders
//! cross it every run, the flock's rule, and the one generator every
//! chance of the rule is read from, by the tick and the entity.
//!
//! `cargo test --test fast flock_over_borders`

use crate::tests::{cell, walker, world, STONE, WALKER};
use bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp};
use chunk_storage::LayerCodec;
use coordinates::{CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::{Attribute, AttributeBlock, Entities, Header, InstructionsApplied};
use actions::around;
use actions::{cells, compare, Turn};
use simulation::Simulation;
use std::collections::BTreeSet;
use type_registry::COLLISION;
use utilities::rng::Rng;

/// Superchunks along the side of the world.
const WORLD: u32 = 3;
/// Cells along the side of the square the flock is put on: with no
/// shift, its middle the corner four superchunks meet at.
const PASTURE: u32 = 240;
/// Walkers put.
const FLOCK: usize = 8000;
/// Ticks run.
const TICKS: u64 = 300;
/// What a walker counts: its meals, and what its neighbours count on it.
const COUNTED: Attribute<u64> = Attribute::new(41);
/// The stones a walker laid: counted only if the stone was.
const LAID: Attribute<u64> = Attribute::new(42);
/// The walkers a walker made: counted only if the walker was.
const MADE: Attribute<u64> = Attribute::new(43);
/// Cells past the pasture, each way, that what came of it is read on:
/// further than a walker comes in the ticks run.
const READ_PAST: i64 = 320;

/// The cell `(x, y)` cells east and south of the pasture's top left,
/// the pasture `shift` cells east and south of the corner.
fn pasture_cell(shift: (u32, u32), x: i64, y: i64) -> CellIndex {
    let from = |shift: u32, along: i64| (i64::from(SUPERCHUNK_SIDE_CELLS - PASTURE / 2 + shift) + along) as u32;
    cell(from(shift.0, x), from(shift.1, y))
}

/// Where `at` is from the top left of the pasture `shift` cells east
/// and south.
fn from_pasture(shift: (u32, u32), at: CellIndex) -> (i64, i64) {
    let (origin, at) = (pasture_cell(shift, 0, 0).cartesian(), at.cartesian());
    (i64::from(at.x) - i64::from(origin.x), i64::from(at.y) - i64::from(origin.y))
}

/// What the test's one generator holds, at the tick `now`, for the
/// entity that tick found on the cell `from_pasture`: every chance of
/// the flock's rule is read from it.
fn drawn_for(seed: u64, now: u64, from_pasture: (i64, i64)) -> Rng {
    Rng::for_stream(seed ^ now.wrapping_mul(0x9e37_79b9_7f4a_7c15), (from_pasture.0 as u64) << 32 | from_pasture.1 as u64 & 0xffff_ffff)
}

/// The entity on `at` as the tick found it, if one stood there.
fn standing<'a>(turn: &Turn<'a>, at: CellIndex) -> Option<entity_manager::EntityRef<'a>> {
    turn.entities_in(at.chunk())?.find(|entity| entity.header.at == at)
}

/// One superchunk's turn of the flock's rule: each walker woken may
/// die; eats the stone under it, counting the meal, or lays one beside
/// it, counting that on itself only if it is laid; counts on a
/// neighbour too; may make another beside it, counting that only if
/// it is made; and steps to a neighbour, whoever stands there. Every
/// cell and every attribute written is a compare-and-write, and what
/// is counted of a cell beside it is under a compare of that cell --
/// in another superchunk, where a border lies between.
fn flock(turn: &mut Turn, seed: u64, shift: (u32, u32)) -> u64 {
    let now = turn.now();
    for entity in turn.woken() {
        let (header, at) = (entity.header, entity.header.at);
        turn.seeing_to(at);
        let mut drawn = drawn_for(seed, now, from_pasture(shift, at));
        let about = around::about(at);
        let stone = around::layer(turn, STONE, &about);
        let (open, taken) = (stone.hot & about.ring(), around::layer(turn, COLLISION, &about).set & about.ring());
        let chance = drawn.below(100);
        if chance < 4 {
            actions::entities::remove(turn, &header);
            continue;
        }
        let meals = entity.attribute(COUNTED);
        if !(stone.set & about.centre()).is_empty() {
            // The meal is counted only if the stone is there still: another on its way here may not eat it, but one counting on this one may.
            compare::from_here(turn, compare::holds(STONE, at));
            actions::entities::set_attribute_of(turn, &header, COUNTED, meals, meals.unwrap_or(0) + 1);
            compare::end(turn);
            cells::clear(turn, STONE, at);
        } else if let Some(beside) = around::pick(&mut drawn, &about, open.without(stone.set)).filter(|_| chance < 40).and_then(|bit| around::cell(at, bit)) {
            // Laid and counted, or neither: the cell may be another's stone by then, whichever side of a border it is on.
            let laid = entity.attribute(LAID);
            compare::from_here(turn, compare::lacks(STONE, beside));
            cells::set(turn, STONE, beside);
            actions::entities::set_attribute_of(turn, &header, LAID, laid, laid.unwrap_or(0) + 1);
            compare::end(turn);
        }
        if let Some(other) = around::pick(&mut drawn, &about, taken).and_then(|bit| around::cell(at, bit)).and_then(|beside| standing(turn, beside)) {
            let seen = other.attribute(COUNTED);
            actions::entities::set_attribute_of(turn, &other.header, COUNTED, seen, seen.unwrap_or(0) + 1000);
        }
        if let Some(beside) = around::pick(&mut drawn, &about, open.without(taken)).filter(|_| chance < 9).and_then(|bit| around::cell(at, bit)) {
            // Made and counted, or neither.
            let made = entity.attribute(MADE);
            crate::tests::make(turn, Header { kind: WALKER, at: beside, wake: now + 1 + drawn.below(3) }, &[]);
            actions::entities::set_attribute_of(turn, &header, MADE, made, made.unwrap_or(0) + 1);
            actions::entities::made(turn);
        }
        let to = around::pick(&mut drawn, &about, open).and_then(|bit| around::cell(at, bit)).unwrap_or(at);
        actions::entities::step(turn, &header, to, now + 1 + drawn.below(3));
    }
    0
}

/// A world with the pasture on it, `shift` cells east and south: stone
/// on the cells the seed draws, walkers on others it draws, each set
/// in the collision plane.
fn pasture(seed: u64, shift: (u32, u32)) -> (BitmapArena, Entities) {
    let (mut arena, mut entities) = world(WORLD);
    let mut codec = LayerCodec::new();
    for (x, y) in (10..10 + WORLD).flat_map(|y| (10..10 + WORLD).map(move |x| (x, y))) {
        for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
            arena.make_hot(BucketKey { layer_type: COLLISION, chunk }, None, &mut codec);
        }
    }
    let mut random = Rng::new(seed);
    for (x, y) in (0..i64::from(PASTURE)).flat_map(|y| (0..i64::from(PASTURE)).map(move |x| (x, y))) {
        if random.below(100) < 40 {
            arena.queue(STONE, Write::cell(pasture_cell(shift, x, y), WriteOp::Set));
        }
    }
    assert_eq!(arena.apply().missed, 0, "the pasture on hot superchunks");
    let mut taken = BTreeSet::new();
    while taken.len() < FLOCK {
        let at = (random.below(u64::from(PASTURE)) as i64, random.below(u64::from(PASTURE)) as i64);
        if taken.insert(at) {
            entities.queue_put(walker(pasture_cell(shift, at.0, at.1), random.below(3)), &[]);
            arena.queue(COLLISION, Write::cell(pasture_cell(shift, at.0, at.1), WriteOp::Set));
        }
    }
    entities.apply();
    assert_eq!(arena.apply().missed, 0);
    (arena, entities)
}

/// What came of the pasture, said from its own top left: the cells of
/// stone on it and about it, those the collision plane holds, and
/// every walker by its cell -- where, when it wakes, its attributes.
type Came = (BTreeSet<(i64, i64)>, BTreeSet<(i64, i64)>, Vec<(i64, i64, u64, Vec<AttributeBlock>)>);

/// [`Came`] of a world whose pasture is `shift` cells east and south.
fn came(arena: &BitmapArena, entities: &Entities, shift: (u32, u32)) -> Came {
    let read = -READ_PAST..i64::from(PASTURE) + READ_PAST;
    let holding = |layer| read.clone().flat_map(|y| read.clone().map(move |x| (x, y))).filter(|&(x, y)| arena.holds(layer, pasture_cell(shift, x, y)) == Ok(true)).collect();
    let (stone, held) = (holding(STONE), holding(COLLISION));
    let from = |at: CellIndex| from_pasture(shift, at);
    let mut walkers: Vec<_> = entities.iter().map(|walker| (from(walker.header.at).0, from(walker.header.at).1, walker.header.wake, walker.attributes.to_vec())).collect();
    walkers.sort_unstable_by_key(|walker| (walker.0, walker.1));
    (stone, held, walkers)
}

/// The same flock, put a drawn way east and south of the corner four
/// superchunks meet at, is the same on one thread and on every thread
/// the machine has, before a tick and after each: every cell of
/// stone, every cell the collision plane holds -- the walkers' --
/// every walker, and as many acts over a border.
/// And the flock did all the test is of: walkers crossed borders, were
/// refused cells and writes, were made and removed and written.
#[test]
fn a_flock_over_borders_comes_to_the_same_on_any_number_of_threads() {
    let seed = utilities::seed::counted();
    let mut random = Rng::new(!seed);
    // Under half the pasture each way: the corner four superchunks meet at stays on it, wherever on it.
    let shift = (random.below(u64::from(PASTURE / 2 - 10)) as u32, random.below(u64::from(PASTURE / 2 - 10)) as u32);
    let ((mut arena, mut entities), (mut arena_there, mut entities_there)) = (pasture(seed, shift), pasture(seed, shift));
    let (mut simulation, mut simulation_there) = (Simulation::new(1), Simulation::for_superchunks((WORLD * WORLD) as usize));
    simulation.keep_entities_in(COLLISION);
    simulation_there.keep_entities_in(COLLISION);
    assert_eq!(came(&arena, &entities, shift), came(&arena_there, &entities_there, shift), "the same pasture put, shifted {shift:?}");
    let (mut done, mut refused, mut over_a_border) = (InstructionsApplied::default(), 0, (0, 0));
    for _ in 0..TICKS {
        let report = simulation.tick(&mut arena, &mut entities, seed, |turn: &mut Turn| flock(turn, seed, shift));
        let report_there = simulation_there.tick(&mut arena_there, &mut entities_there, seed, |turn: &mut Turn| flock(turn, seed, shift));
        over_a_border = (over_a_border.0 + report.acts_over_a_border, over_a_border.1 + report_there.acts_over_a_border);
        done += report.instructions_applied;
        refused += report.writes_applied.refused;
        let ((stone, held, walkers), (stone_there, held_there, walkers_there)) = (came(&arena, &entities, shift), came(&arena_there, &entities_there, shift));
        // The collision plane holds the cells the walkers stand on, and no other.
        assert_eq!(held, walkers.iter().map(|walker| (walker.0, walker.1)).collect(), "after tick {}", entities.now());
        let stone_apart: Vec<_> = stone.symmetric_difference(&stone_there).chain(held.symmetric_difference(&held_there)).take(4).collect();
        let walkers_apart: Vec<_> = walkers.iter().zip(&walkers_there).filter(|(here, there)| here != there).take(4).collect();
        assert!(stone_apart.is_empty() && walkers_apart.is_empty() && walkers.len() == walkers_there.len(), "after tick {}, shifted {shift:?}, seed {seed:#x}: {} walkers on one thread and {} on all; apart, stone {stone_apart:?}, walkers {walkers_apart:?}; done here {done:?}", entities.now(), walkers.len(), walkers_there.len());
    }
    assert!(done.crossed > 0 && done.stayed > 0 && done.refused > 0 && done.puts > 0 && done.removes > 0 && done.edits > 0 && refused > 0, "{done:?}, {refused} writes refused");
    // Both borders of superchunks cross the pasture wherever it was put, and acts reach over them.
    assert!(over_a_border.0 > 0 && over_a_border.0 == over_a_border.1, "acts over a border: {over_a_border:?}");
}
