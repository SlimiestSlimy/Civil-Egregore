//! What a rule does to entities, an instruction each: a step and a
//! sleep carry no attributes and keep them, across chunks and
//! superchunks; a step onto a taken cell is turned back; one entity
//! edits another, an attribute at a time.
//!
//! `cargo test`

use bitplane_manager::{BitmapArena, BucketKey};
use chunk_storage::{LayerCodec, LayerType};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::{Attribute, AttributeBlock, Entities, EntityId, EntityType, Header, NEVER};
use simulation::{Simulation, Turn};

/// The layer type the arena holds: every cell hot, none set.
const STONE: LayerType = LayerType(6);
/// The entities' type.
const WALKER: EntityType = EntityType(40);
/// An attribute.
const NAME: Attribute<u64> = Attribute::new(41);
/// Another.
const MARK: Attribute<u64> = Attribute::new(42);
/// A third.
const SCAR: Attribute<u64> = Attribute::new(43);

/// An arena with a bitmap hot over the `side` by `side` superchunks from
/// `(10, 10)`, and entities holding the same superchunks.
fn world(side: u32) -> (BitmapArena, Entities) {
    let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
    for y in 10..10 + side {
        for x in 10..10 + side {
            for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
                arena.make_hot(BucketKey { layer_type: STONE, chunk }, None, &mut codec);
            }
        }
    }
    let mut entities = Entities::new();
    assert_eq!(entities.align(&arena.superchunk_indices()), 0);
    (arena, entities)
}

/// The cell `(x, y)` cells from the top left of the superchunk `(10, 10)`.
fn cell(x: u32, y: u32) -> CellIndex {
    CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + x, y: 10 * SUPERCHUNK_SIDE_CELLS + y }.into()
}

/// A walker with ID `id` on `at`, waking at `wake`.
fn walker(id: u64, at: CellIndex, wake: u64) -> Header {
    Header { id: EntityId(id), kind: WALKER, at, wake }
}

/// Every walker woken steps a cell to the right, to wake next tick.
fn step_right(turn: &mut Turn, _: &mut Vec<CellIndex>) -> usize {
    let now = turn.now();
    for entity in turn.woken() {
        turn.step(&entity.header, entity.header.at.offset(1, 0).expect("in the world"), now + 1);
    }
    0
}

/// Entities stepping keep their attributes though none is carried: in
/// their chunk, into the next, and into the next superchunk, where they
/// go whole.
#[test]
fn a_step_carries_no_attributes_and_keeps_them() {
    let (mut arena, mut entities) = world(2);
    let attributes = [AttributeBlock::holding(NAME, 7), AttributeBlock::holding(MARK, 9)];
    // In a chunk, over a chunk's border, over a superchunk's.
    for (id, x) in [(1, 40), (2, 250), (3, 1020)] {
        entities.queue_put(walker(id, cell(x, 30), 0), &attributes);
    }
    entities.apply();
    let mut simulation = Simulation::new(2);
    let (mut moves, mut puts) = (0, 0);
    for _ in 0..10 {
        let report = simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), step_right);
        (moves, puts) = (moves + report.instructions_applied.moves, puts + report.instructions_applied.puts);
    }
    for (id, x) in [(1, 50), (2, 260), (3, 1030)] {
        let entity = entities.get(EntityId(id), cell(x, 30)).expect("ten steps on");
        assert_eq!(entity.attributes, attributes);
    }
    assert_eq!(entities.len(), 3);
    // Every step a move; the one over the superchunk's border a put there as new besides, and a sleep where it stood until settled.
    assert_eq!((moves, puts), (30, 1));
}

/// A step onto a cell an entity stands on is turned back as it is
/// applied: the stepper stays, and wakes when it was to all the
/// same.
#[test]
fn a_step_onto_a_taken_cell_is_turned_back() {
    let (mut arena, mut entities) = world(1);
    entities.queue_put(walker(1, cell(40, 30), 0), &[AttributeBlock::holding(NAME, 1)]);
    entities.queue_put(walker(2, cell(41, 30), NEVER), &[]);
    entities.apply();
    let mut simulation = Simulation::new(1);
    let (mut stayed, mut moves) = (0, 0);
    for _ in 0..3 {
        let report = simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), step_right);
        (stayed, moves) = (stayed + report.instructions_applied.stayed, moves + report.instructions_applied.moves);
    }
    assert_eq!((stayed, moves), (3, 3), "turned back each tick, and woken the next");
    let stepper = entities.get(EntityId(1), cell(40, 30)).expect("where it stood");
    assert_eq!((stepper.header.wake, stepper.attribute(NAME)), (3, Some(1)));
    assert!(entities.get(EntityId(2), cell(41, 30)).is_some());
}

/// Entities edit another, an attribute each, in one tick, each saying
/// what it saw of it: both land, neither undoing the other, and the
/// one edited keeps what it had and when it wakes. An attribute is
/// removed the same way.
#[test]
fn entities_edit_another_an_attribute_at_a_time() {
    let (mut arena, mut entities) = world(2);
    // The one edited, in another superchunk than one of the two editing it.
    let target = walker(9, cell(1030, 30), NEVER);
    entities.queue_put(target, &[AttributeBlock::holding(NAME, 7)]);
    entities.queue_put(walker(1, cell(1020, 30), 0), &[]);
    entities.queue_put(walker(2, cell(1040, 30), 0), &[]);
    entities.apply();
    let mut simulation = Simulation::new(2);
    let report = simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn, _: &mut Vec<CellIndex>| {
        for entity in turn.woken() {
            let kind = if entity.header.id == EntityId(1) { MARK } else { SCAR };
            turn.set_attribute(&target, kind, None, entity.header.id.0);
            turn.step(&entity.header, entity.header.at, NEVER);
        }
        0
    });
    assert_eq!((report.instructions_applied.edits, report.instructions_applied.puts, report.instructions_compared), (2, 0, (2, 0)));
    let edited = entities.get(target.id, target.at).expect("where it stood");
    assert_eq!(edited.attributes, [AttributeBlock::holding(NAME, 7), AttributeBlock::holding(MARK, 1), AttributeBlock::holding(SCAR, 2)]);
    assert_eq!(edited.header, target);

    entities.queue_put(walker(3, cell(1031, 31), 1), &[]);
    entities.apply();
    simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn, _: &mut Vec<CellIndex>| {
        for entity in turn.woken() {
            turn.unset_attribute(&target, MARK, 1);
            turn.set_attribute(&target, NAME, Some(7), 8);
            turn.step(&entity.header, entity.header.at, NEVER);
        }
        0
    });
    let edited = entities.get(target.id, target.at).expect("where it stood");
    assert_eq!(edited.attributes, [AttributeBlock::holding(NAME, 8), AttributeBlock::holding(SCAR, 2)]);
}

/// Two entities setting one attribute of a third in one tick, both
/// having seen it unset: the first applied is kept and the other
/// refused -- neither written over -- the same one on any number of
/// threads.
#[test]
fn of_two_edits_of_one_attribute_one_is_refused_the_same_on_any_threads() {
    let kept = [1, 4].map(|threads| {
        let (mut arena, mut entities) = world(2);
        // The one edited, and the two editing it from the two superchunks beside its border.
        let target = walker(9, cell(1030, 30), NEVER);
        entities.queue_put(target, &[]);
        entities.queue_put(walker(1, cell(1020, 30), 0), &[]);
        entities.queue_put(walker(2, cell(1040, 30), 0), &[]);
        entities.apply();
        let report = Simulation::new(threads).tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn, _: &mut Vec<CellIndex>| {
            for entity in turn.woken() {
                turn.set_attribute(&target, NAME, None, entity.header.id.0);
                turn.step(&entity.header, entity.header.at, NEVER);
            }
            0
        });
        assert_eq!((report.instructions_applied.edits, report.instructions_compared), (1, (1, 1)));
        entities.get(target.id, target.at).expect("where it stood").attribute(NAME).expect("set")
    });
    assert!(kept[0] == kept[1] && [1, 2].contains(&kept[0]), "{kept:?}");
}

/// An entity awake, changing itself, is written by another in the same
/// tick: what the other writes of an attribute it left alone stands
/// with its own change, and of the attribute both write one write is
/// applied and the other refused -- the same one on any number of
/// threads. It is never put whole over what another wrote.
#[test]
fn an_entity_changing_itself_and_another_writing_it_both_stand() {
    let kept = [1, 4].map(|threads| {
        let (mut arena, mut entities) = world(2);
        // The one written, awake, in another superchunk than the one writing it.
        let target = walker(9, cell(1030, 30), 0);
        entities.queue_put(target, &[AttributeBlock::holding(NAME, 7)]);
        entities.queue_put(walker(1, cell(1020, 30), 0), &[]);
        entities.apply();
        let report = Simulation::new(threads).tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn, _: &mut Vec<CellIndex>| {
            for entity in turn.woken() {
                if entity.header.id == target.id {
                    // Itself: its name changed, asleep from here on.
                    turn.update(&entity.header, Header { wake: NEVER, ..entity.header }, &[AttributeBlock::holding(NAME, 8)]);
                } else {
                    turn.set_attribute(&target, MARK, None, 1);
                    turn.set_attribute(&target, NAME, Some(7), 99);
                    turn.step(&entity.header, entity.header.at, NEVER);
                }
            }
            0
        });
        assert_eq!((report.instructions_applied.edits, report.instructions_compared), (2, (2, 1)), "the mark and one name applied, the other name refused");
        let written = entities.get(target.id, target.at).expect("where it stood");
        assert_eq!((written.attribute(MARK), written.header.wake), (Some(1), NEVER), "the other's mark, and its own sleep");
        written.attribute(NAME).expect("named")
    });
    assert!(kept[0] == kept[1] && [8, 99].contains(&kept[0]), "{kept:?}");
}

/// An entity crossing to another superchunk, written by another in the
/// same tick, is turned back: it stays where it stood, asleep until
/// its wake, with what was written to it -- nothing of it lost on the
/// way. The tick after, written by none, it crosses with all of it.
#[test]
fn a_crossing_written_behind_is_turned_back() {
    for threads in [1, 4] {
        let (mut arena, mut entities) = world(2);
        // On the last cell before the border, to step over it.
        let (edge, across) = (cell(SUPERCHUNK_SIDE_CELLS - 1, 30), cell(SUPERCHUNK_SIDE_CELLS, 30));
        let crosser = walker(9, edge, 0);
        entities.queue_put(crosser, &[AttributeBlock::holding(NAME, 7)]);
        entities.queue_put(walker(1, cell(1000, 30), 0), &[]);
        entities.apply();
        let mut simulation = Simulation::new(threads);
        let rule = |turn: &mut Turn, _: &mut Vec<CellIndex>| {
            let now = turn.now();
            for entity in turn.woken() {
                if entity.header.id == crosser.id {
                    turn.step(&entity.header, across, now + 1);
                } else {
                    turn.set_attribute(&crosser, MARK, None, 1);
                    turn.step(&entity.header, entity.header.at, NEVER);
                }
            }
            0
        };
        let report = simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), rule);
        assert_eq!((report.instructions_applied.crossed, report.instructions_applied.turned_back, entities.len()), (1, 1, 2), "on {threads} threads");
        assert!(entities.get(crosser.id, across).is_none(), "not across");
        let stayed = entities.get(crosser.id, edge).expect("where it stood");
        assert_eq!((stayed.attribute(NAME), stayed.attribute(MARK), stayed.header.wake), (Some(7), Some(1), 1), "written, and asleep a tick");
        let report = simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), rule);
        assert_eq!((report.instructions_applied.crossed, report.instructions_applied.turned_back, entities.len()), (1, 0, 2), "on {threads} threads");
        assert!(entities.get(crosser.id, edge).is_none(), "gone from where it stood");
        let crossed = entities.get(crosser.id, across).expect("across");
        assert_eq!((crossed.attribute(NAME), crossed.attribute(MARK)), (Some(7), Some(1)), "with all that was written to it");
    }
}
