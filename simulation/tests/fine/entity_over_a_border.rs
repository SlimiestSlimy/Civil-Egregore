//! An entity that crosses a border is one entity, moved once: what
//! other authors queued for it in the same tick -- by its name, the
//! cell the tick found it on -- lands on it wherever it stands by
//! then, as it would within a superchunk
//! (`docs/simulation.md`, "An entity over a border").
//!
//! `cargo test --test fine entity_over_a_border`

use crate::tests::{cell, walker, world};
use coordinates::{CellIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::{Attribute, Entities, Header, NEVER};
use simulation::{Simulation, Turn};
use utilities::rng::Rng;

/// What the crosser has from the start.
const NAME: Attribute<u64> = Attribute::new(50);
/// What a writer writes of itself.
const OWN: Attribute<u64> = Attribute::new(51);
/// The first of the attributes the writers give the crosser, one each.
const GIVEN: u64 = 60;

/// What a writer does to the crosser.
#[derive(Clone, Copy, PartialEq)]
enum Does {
    /// Gives it an attribute, and queues nothing else.
    Gives,
    /// Gives it an attribute, and itself one: it lands in two.
    GivesAndKeeps,
    /// Removes it.
    Removes,
}

/// The scene: the crosser on the last column of the middle superchunk
/// of three by three, a row drawn from the seed, stepping right over
/// the border; and the writers about it, each with what it does and
/// whether it is after the crosser in the world's order -- before it,
/// beside it in its superchunk, over the border it crosses, and in
/// the superchunk down and left, which is no neighbour of the one it
/// comes to.
fn scene(row: u32, last: Does) -> (CellIndex, CellIndex, Vec<(CellIndex, Does)>) {
    let edge = 2 * SUPERCHUNK_SIDE_CELLS - 1;
    let writers = vec![
        // Before it in Morton order: across and down both less.
        (cell(edge - 3, row - 3), Does::Gives),
        // After it, both no less: in its own superchunk, and in the one it crosses to.
        (cell(edge, row + 2), Does::Gives),
        (cell(edge + 2, row + 1), Does::Gives),
        (cell(edge, row + 3), Does::GivesAndKeeps),
        (cell(edge + 3, row + 2), Does::GivesAndKeeps),
        // After it, a superchunk down and one left: the bit that tells them apart is the row's.
        (cell(edge - SUPERCHUNK_SIDE_CELLS, row + SUPERCHUNK_SIDE_CELLS), Does::Gives),
        (cell(edge - SUPERCHUNK_SIDE_CELLS - 1, row + SUPERCHUNK_SIDE_CELLS), Does::GivesAndKeeps),
        (cell(edge + 1, row + 4), last),
    ];
    (cell(edge, row), cell(edge + 1, row), writers)
}

/// One tick of the scene: the crosser steps over the border, every
/// writer does what it does to it. What the tick applied, and the
/// entities after.
fn tick(threads: usize, row: u32, last: Does, blocked: bool) -> (entity_manager::InstructionsApplied, Entities) {
    let (mut arena, mut entities) = world(3);
    let (from, to, writers) = scene(row, last);
    let crosser = walker(from, 0);
    entities.queue_put(crosser, &[entity_manager::AttributeBlock::holding(NAME, 7)]);
    for &(at, _) in &writers {
        entities.queue_put(walker(at, 0), &[]);
    }
    if blocked {
        entities.queue_put(walker(to, NEVER), &[]);
    }
    entities.apply();
    for &(at, _) in &writers[1..] {
        assert!(at.0 > from.0, "a writer meant to come after the crosser in the world's order");
    }
    assert!(writers[0].0 .0 < from.0);
    let report = Simulation::new(threads).tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn, _: &mut Vec<CellIndex>| {
        let now = turn.now();
        actions::entities::each_woken(turn, [], &mut (), |turn, entity, _| {
            let header: Header = entity.header;
            if header.at == from {
                actions::entities::step(turn, &header, to, now + 1);
                return;
            }
            let Some(which) = writers.iter().position(|&(at, _)| at == header.at) else {
                return;
            };
            match writers[which].1 {
                Does::Removes => actions::entities::remove(turn, &crosser),
                does => {
                    actions::entities::set_attribute_of(turn, &crosser, Attribute::<u64>::new(GIVEN + which as u64), None, which as u64 + 1);
                    if does == Does::GivesAndKeeps {
                        actions::entities::set_attribute_of(turn, &header, OWN, None, 1);
                    }
                }
            }
        });
        0
    });
    (report.instructions_applied, entities)
}

/// The crosser steps over a border, and eight others write to it in
/// the same tick, before it and after it in the world's order, from
/// its superchunk, from the one it comes to and from one that is no
/// neighbour of that, alone or with a write of their own: it stands
/// over the border with every one of their attributes, and each kept
/// its own -- the same on one thread and on all. Its cell over the
/// border taken, it stays, and has them all the same.
#[test]
fn what_others_write_to_an_entity_follows_it_over_a_border() {
    let mut random = Rng::new(utilities::seed::counted());
    let row = 4 + random.below(900) as u32;
    for (threads, blocked) in [(1, false), (simulation::threads_for(9), false), (1, true), (simulation::threads_for(9), true)] {
        let (applied, entities) = tick(threads, row, Does::Gives, blocked);
        let (from, to, writers) = scene(row, Does::Gives);
        assert_eq!((applied.crossed, applied.stayed), (usize::from(!blocked), usize::from(blocked)), "on {threads} threads, row {row}");
        let stands = entities.get(if blocked { from } else { to }).expect("the crosser, where it should stand");
        assert_eq!((stands.attribute(NAME), entities.get(from).is_some()), (Some(7), blocked));
        for (which, &(at, does)) in writers.iter().enumerate() {
            assert_eq!(stands.attribute(Attribute::<u64>::new(GIVEN + which as u64)), Some(which as u64 + 1), "writer {which} on {threads} threads, row {row}, blocked {blocked}");
            assert_eq!(entities.get(at).expect("the writer").attribute(OWN), (does == Does::GivesAndKeeps).then_some(1), "writer {which}'s own");
        }
        assert_eq!(entities.len(), writers.len() + 1 + usize::from(blocked));
    }
}

/// One of those after it removes the crosser instead: it is gone,
/// from the cell it left and from the one it came to, on one thread
/// and on all.
#[test]
fn an_entity_removed_after_it_crossed_is_gone() {
    let mut random = Rng::new(utilities::seed::counted());
    let row = 4 + random.below(900) as u32;
    for threads in [1, simulation::threads_for(9)] {
        let (applied, entities) = tick(threads, row, Does::Removes, false);
        let (from, to, writers) = scene(row, Does::Removes);
        assert_eq!((applied.crossed, applied.removes), (1, 1), "on {threads} threads, row {row}");
        assert!(entities.get(from).is_none() && entities.get(to).is_none(), "on {threads} threads, row {row}");
        assert_eq!(entities.len(), writers.len());
    }
}
