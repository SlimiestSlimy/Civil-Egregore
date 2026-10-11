//! A world shifted is the same world: a flock put some cells east and
//! south comes to the same, tick after tick, seen from itself -- so
//! the borders of chunks and superchunks, which the shift moves across
//! it, change nothing of what is not chance
//! (`docs/instructions.md`, "The same wherever the borders fall").
//!
//! All of it is the test's own, and nothing outside it is made for it:
//! the shift, the flock's rule, and the one generator every chance of
//! the rule is read from -- by the tick and the entity, not as the
//! next number of a stream, since the order entities are seen to in is
//! the superchunks' and so moves with the borders too.
//!
//! `cargo test --test fast shifted`

use crate::tests::{cell, walker, world, STONE, WALKER};
use bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp};
use chunk_storage::LayerCodec;
use coordinates::{CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::{AttributeBlock, Entities, EntityId, Header, InstructionsApplied};
use instructions::around::{self, CENTRE, RING};
use instructions::{cells, compare, Turn};
use simulation::Simulation;
use std::collections::BTreeSet;
use type_registry::{COLLISION, HUNGRY_AT};
use utilities::rng::Rng;

/// Superchunks along the side of the world.
const WORLD: u32 = 3;
/// Cells along the side of the square the flock is put on: unshifted,
/// its middle the corner four superchunks meet at.
const PASTURE: u32 = 240;
/// Walkers put.
const FLOCK: usize = 8000;
/// Ticks run.
const TICKS: u64 = 300;
/// Cells past the pasture, each way, that what came of it is read on:
/// further than a walker comes in the ticks run.
const READ_PAST: i64 = 320;

/// The cell `(x, y)` cells east and south of the pasture's top left,
/// the pasture `shift` cells east and south of where it is unshifted.
fn pasture_cell(shift: (u32, u32), x: i64, y: i64) -> CellIndex {
    let from = |shift: u32, along: i64| (i64::from(SUPERCHUNK_SIDE_CELLS - PASTURE / 2 + shift) + along) as u32;
    cell(from(shift.0, x), from(shift.1, y))
}

/// What the test's one generator holds for the entity `id` at the tick
/// `now`: every chance of the flock's rule is read from it.
fn drawn_for(seed: u64, now: u64, id: EntityId) -> Rng {
    Rng::for_stream(seed ^ now.wrapping_mul(0x9e37_79b9_7f4a_7c15), id.0)
}

/// The entity on `at` as the tick found it, if one stood there.
fn standing<'a>(turn: &Turn<'a>, at: CellIndex) -> Option<entity_manager::EntityRef<'a>> {
    turn.entities_in(at.chunk())?.find(|entity| entity.header.at == at)
}

/// One superchunk's turn of the flock's rule: each walker woken may
/// die; eats the stone under it, counting the meal, or lays one beside
/// it; counts on a neighbour too; may make another beside it; and
/// steps to a neighbour, whoever stands there. Every cell and every
/// attribute written is a compare-and-write.
fn flock(turn: &mut Turn, seed: u64) -> u64 {
    let now = turn.now();
    for entity in turn.woken() {
        let (header, at) = (entity.header, entity.header.at);
        turn.seeing_to(at);
        let mut drawn = drawn_for(seed, now, header.id);
        let stone = around::layer(turn, STONE, at);
        let (open, taken) = (stone.hot & RING, around::occupied(turn, at) & RING);
        let chance = drawn.below(100);
        if chance < 4 {
            turn.remove(&header);
            continue;
        }
        let meals = entity.attribute(HUNGRY_AT);
        if stone.set & CENTRE != 0 {
            // The meal is counted only if the stone is there still: another on its way here may not eat it, but one counting on this one may.
            compare::entities_from_here(turn, compare::holds(STONE, at));
            turn.set_attribute(&header, HUNGRY_AT, meals, meals.unwrap_or(0) + 1);
            compare::entities_as_ever(turn);
            cells::clear(turn, STONE, at);
        } else if let Some(beside) = around::pick(&mut drawn, open & !stone.set).filter(|_| chance < 40).and_then(|bit| around::cell(at, bit)) {
            cells::set(turn, STONE, beside);
        }
        if let Some(other) = around::pick(&mut drawn, taken).and_then(|bit| around::cell(at, bit)).and_then(|beside| standing(turn, beside)) {
            let seen = other.attribute(HUNGRY_AT);
            turn.set_attribute(&other.header, HUNGRY_AT, seen, seen.unwrap_or(0) + 1000);
        }
        if let Some(beside) = around::pick(&mut drawn, open & !taken).filter(|_| chance < 9).and_then(|bit| around::cell(at, bit)) {
            turn.put(Header { id: EntityId(drawn.draw()), kind: WALKER, at: beside, wake: now + 1 + drawn.below(3) }, &[]);
        }
        let to = around::pick(&mut drawn, open).and_then(|bit| around::cell(at, bit)).unwrap_or(at);
        turn.step(&header, to, now + 1 + drawn.below(3));
    }
    0
}

/// A world with the pasture on it, `shift` cells east and south: stone
/// on the cells the seed draws, walkers on others it draws, each set
/// in the collision plane -- the same whatever the shift.
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
            entities.queue_put(walker(random.draw(), pasture_cell(shift, at.0, at.1), random.below(3)), &[]);
            arena.queue(COLLISION, Write::cell(pasture_cell(shift, at.0, at.1), WriteOp::Set));
        }
    }
    entities.apply();
    assert_eq!(arena.apply().missed, 0);
    (arena, entities)
}

/// What came of the pasture, said from its own top left: the cells of
/// stone on it and about it, those the collision plane holds, and
/// every walker by its ID -- where, when it wakes, its attributes.
type Came = (BTreeSet<(i64, i64)>, BTreeSet<(i64, i64)>, Vec<(u64, i64, i64, u64, Vec<AttributeBlock>)>);

/// [`Came`] of a world whose pasture is `shift` cells east and south.
fn came(arena: &BitmapArena, entities: &Entities, shift: (u32, u32)) -> Came {
    let read = -READ_PAST..i64::from(PASTURE) + READ_PAST;
    let holding = |layer| read.clone().flat_map(|y| read.clone().map(move |x| (x, y))).filter(|&(x, y)| arena.holds(layer, pasture_cell(shift, x, y)) == Ok(true)).collect();
    let (stone, held) = (holding(STONE), holding(COLLISION));
    let origin = pasture_cell(shift, 0, 0).cartesian();
    let from = |at: CellIndex| (i64::from(at.cartesian().x) - i64::from(origin.x), i64::from(at.cartesian().y) - i64::from(origin.y));
    let mut walkers: Vec<_> = entities.iter().map(|walker| (walker.header.id.0, from(walker.header.at).0, from(walker.header.at).1, walker.header.wake, walker.attributes.to_vec())).collect();
    walkers.sort_unstable_by_key(|walker| walker.0);
    (stone, held, walkers)
}

/// The same flock here and shifted a drawn way east and south --
/// across other borders of chunks and superchunks -- is the same
/// before a tick and after each: every cell of stone, every cell the
/// collision plane holds -- the walkers' -- every walker.
/// And the flock did all the test is of: walkers crossed borders, were
/// refused cells and writes, were made and removed and written.
#[test]
fn a_world_shifted_comes_to_the_same() {
    let seed = utilities::seed::counted();
    let mut random = Rng::new(!seed);
    let shift = (1 + random.below(u64::from(SUPERCHUNK_SIDE_CELLS) - 1) as u32, 1 + random.below(u64::from(SUPERCHUNK_SIDE_CELLS) - 1) as u32);
    let ((mut arena, mut entities), (mut arena_there, mut entities_there)) = (pasture(seed, (0, 0)), pasture(seed, shift));
    let (mut simulation, mut simulation_there) = (Simulation::for_superchunks((WORLD * WORLD) as usize), Simulation::for_superchunks((WORLD * WORLD) as usize));
    simulation.keep_entities_in(COLLISION);
    simulation_there.keep_entities_in(COLLISION);
    assert_eq!(came(&arena, &entities, (0, 0)), came(&arena_there, &entities_there, shift), "the same pasture put, shifted {shift:?}");
    let (mut done, mut refused) = (InstructionsApplied::default(), 0);
    for _ in 0..TICKS {
        let report = simulation.tick(&mut arena, &mut entities, seed, |turn: &mut Turn, _: &mut Vec<CellIndex>| flock(turn, seed));
        simulation_there.tick(&mut arena_there, &mut entities_there, seed, |turn: &mut Turn, _: &mut Vec<CellIndex>| flock(turn, seed));
        done += report.instructions_applied;
        refused += report.writes_applied.refused;
        let ((stone, held, walkers), (stone_there, held_there, walkers_there)) = (came(&arena, &entities, (0, 0)), came(&arena_there, &entities_there, shift));
        // The collision plane holds the cells the walkers stand on, and no other.
        assert_eq!(held, walkers.iter().map(|walker| (walker.1, walker.2)).collect(), "after tick {}", entities.now());
        let stone_apart: Vec<_> = stone.symmetric_difference(&stone_there).chain(held.symmetric_difference(&held_there)).take(4).collect();
        let walkers_apart: Vec<_> = walkers.iter().zip(&walkers_there).filter(|(here, there)| here != there).take(4).collect();
        assert!(stone_apart.is_empty() && walkers_apart.is_empty() && walkers.len() == walkers_there.len(), "after tick {}, shifted {shift:?}, seed {seed:#x}: {} walkers here and {} there; apart, stone {stone_apart:?}, walkers {walkers_apart:?}; done here {done:?}", entities.now(), walkers.len(), walkers_there.len());
    }
    assert!(done.crossed > 0 && done.stayed > 0 && done.refused > 0 && done.puts > 0 && done.removes > 0 && done.edits > 0 && refused > 0, "{done:?}, {refused} writes refused");
}
