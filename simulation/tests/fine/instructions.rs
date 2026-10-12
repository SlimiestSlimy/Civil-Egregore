//! What a rule does to entities, an instruction each: a step and a
//! sleep carry no attributes and keep them, across chunks and
//! superchunks; a step onto a taken cell is turned back; one entity
//! edits another, an attribute at a time.
//!
//! `cargo test`

use bitplane_manager::{BitmapArena, BucketKey};
use chunk_storage::{LayerCodec, LayerType};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::{Attribute, AttributeBlock, Entities, EntityType, Header, Member, NEVER};
use simulation::{Simulation, Turn};

/// The layer type the arena holds: every cell hot, none set.
const STONE: LayerType = LayerType::of_a_bit(6);
/// The entities' type.
const WALKER: EntityType = EntityType::of(40);
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

/// A walker on `at`, waking at `wake`.
fn walker(at: CellIndex, wake: u64) -> Header {
    Header { kind: WALKER, at, wake }
}

/// The number a test knows the walker on `at` by: 1 the one on
/// `first`, 2 any other.
fn number(at: CellIndex, first: CellIndex) -> u64 {
    if at == first { 1 } else { 2 }
}

/// Every walker woken steps a cell to the right, to wake next tick.
fn step_right(turn: &mut Turn) -> usize {
    let now = turn.now();
    for entity in turn.woken() {
        actions::entities::step(turn, &entity.header, entity.header.at.offset(1, 0).expect("in the world"), now + 1);
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
    for x in [40, 250, 1020] {
        entities.queue_put(walker(cell(x, 30), 0), &attributes);
    }
    entities.apply();
    let mut simulation = Simulation::new(2);
    let (mut moves, mut puts) = (0, 0);
    for _ in 0..10 {
        let report = simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), step_right);
        (moves, puts) = (moves + report.instructions_applied.moves, puts + report.instructions_applied.puts);
    }
    for x in [50, 260, 1030] {
        let entity = entities.get(cell(x, 30)).expect("ten steps on");
        assert_eq!(entity.attributes, attributes);
    }
    assert_eq!(entities.len(), 3);
    // Every step a move, the one over the superchunk's border as any other: nothing is put.
    assert_eq!((moves, puts), (30, 0));
}

/// A step onto a cell an entity stands on is turned back as it is
/// applied: the stepper stays, and wakes when it was to all the
/// same.
#[test]
fn a_step_onto_a_taken_cell_is_turned_back() {
    let (mut arena, mut entities) = world(1);
    entities.queue_put(walker(cell(40, 30), 0), &[AttributeBlock::holding(NAME, 1)]);
    entities.queue_put(walker(cell(41, 30), NEVER), &[]);
    entities.apply();
    let mut simulation = Simulation::new(1);
    let (mut stayed, mut moves) = (0, 0);
    for _ in 0..3 {
        let report = simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), step_right);
        (stayed, moves) = (stayed + report.instructions_applied.stayed, moves + report.instructions_applied.moves);
    }
    assert_eq!((stayed, moves), (3, 3), "turned back each tick, and woken the next");
    let stepper = entities.get(cell(40, 30)).expect("where it stood");
    assert_eq!((stepper.header.wake, stepper.attribute(NAME)), (3, Some(1)));
    assert!(entities.get(cell(41, 30)).is_some());
}

/// Entities edit another, an attribute each, in one tick, each saying
/// what it saw of it: both land, neither undoing the other, and the
/// one edited keeps what it had and when it wakes. An attribute is
/// removed the same way.
#[test]
fn entities_edit_another_an_attribute_at_a_time() {
    let (mut arena, mut entities) = world(2);
    // The one edited, in another superchunk than one of the two editing it.
    let target = walker(cell(1030, 30), NEVER);
    entities.queue_put(target, &[AttributeBlock::holding(NAME, 7)]);
    entities.queue_put(walker(cell(1020, 30), 0), &[]);
    entities.queue_put(walker(cell(1040, 30), 0), &[]);
    entities.apply();
    let mut simulation = Simulation::new(2);
    let report = simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn| {
        for entity in turn.woken() {
            let number = number(entity.header.at, cell(1020, 30));
            actions::entities::set_attribute_of(turn, &target, if number == 1 { MARK } else { SCAR }, None, number);
            actions::entities::step(turn, &entity.header, entity.header.at, NEVER);
        }
        0
    });
    // Each edit an attribute given whole, one write, under a compare of none being seen; each editor's sleep under a compare of its being there.
    assert_eq!((report.instructions_applied.edits, report.instructions_applied.puts, report.instructions_compared), (2, 0, (4, 0)));
    let edited = entities.get(target.at).expect("where it stood");
    assert_eq!(edited.attributes, [AttributeBlock::holding(NAME, 7), AttributeBlock::holding(MARK, 1), AttributeBlock::holding(SCAR, 2)]);
    assert_eq!(edited.header, target);

    entities.queue_put(walker(cell(1031, 31), 1), &[]);
    entities.apply();
    simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn| {
        for entity in turn.woken() {
            actions::entities::unset_attribute_of(turn, &target, MARK, 1);
            actions::entities::set_attribute_of(turn, &target, NAME, Some(7), 8);
            actions::entities::step(turn, &entity.header, entity.header.at, NEVER);
        }
        0
    });
    let edited = entities.get(target.at).expect("where it stood");
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
        let target = walker(cell(1030, 30), NEVER);
        entities.queue_put(target, &[]);
        entities.queue_put(walker(cell(1020, 30), 0), &[]);
        entities.queue_put(walker(cell(1040, 30), 0), &[]);
        entities.apply();
        let report = Simulation::new(threads).tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn| {
            for entity in turn.woken() {
                actions::entities::set_attribute_of(turn, &target, NAME, None, number(entity.header.at, cell(1020, 30)));
                actions::entities::step(turn, &entity.header, entity.header.at, NEVER);
            }
            0
        });
        // The attribute given whole, once; the two sleeps' compares hold beside the one edit's.
        assert_eq!((report.instructions_applied.edits, report.instructions_compared), (1, (3, 1)));
        entities.get(target.at).expect("where it stood").attribute(NAME).expect("set")
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
        let target = walker(cell(1030, 30), 0);
        entities.queue_put(target, &[AttributeBlock::holding(NAME, 7)]);
        entities.queue_put(walker(cell(1020, 30), 0), &[]);
        entities.apply();
        let report = Simulation::new(threads).tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn| {
            for entity in turn.woken() {
                if entity.header.at == target.at {
                    // Itself: its name changed, asleep from here on.
                    actions::entities::update(turn, &entity.header, Header { wake: NEVER, ..entity.header }, &[AttributeBlock::holding(NAME, 8)]);
                } else {
                    actions::entities::set_attribute_of(turn, &target, MARK, None, 1);
                    actions::entities::set_attribute_of(turn, &target, NAME, Some(7), 99);
                    actions::entities::step(turn, &entity.header, entity.header.at, NEVER);
                }
            }
            0
        });
        assert_eq!((report.instructions_applied.edits, report.instructions_compared), (2, (4, 1)), "the mark given whole and one name written, the other name refused; the two sleeps' compares hold");
        let written = entities.get(target.at).expect("where it stood");
        assert_eq!((written.attribute(MARK), written.header.wake), (Some(1), NEVER), "the other's mark, and its own sleep");
        written.attribute(NAME).expect("named")
    });
    assert!(kept[0] == kept[1] && [8, 99].contains(&kept[0]), "{kept:?}");
}

/// An entity crossing to another superchunk, written by another in the
/// same tick, crosses with what was written to it: as one stepping
/// within a superchunk does, whichever of the two is applied first.
#[test]
fn a_crossing_written_behind_takes_what_was_written() {
    for threads in [1, 4] {
        let (mut arena, mut entities) = world(2);
        // On the last cell before the border, to step over it.
        let (edge, across) = (cell(SUPERCHUNK_SIDE_CELLS - 1, 30), cell(SUPERCHUNK_SIDE_CELLS, 30));
        let crosser = walker(edge, 0);
        entities.queue_put(crosser, &[AttributeBlock::holding(NAME, 7)]);
        entities.queue_put(walker(cell(1000, 30), 0), &[]);
        entities.apply();
        let mut simulation = Simulation::new(threads);
        let rule = |turn: &mut Turn| {
            let now = turn.now();
            for entity in turn.woken() {
                if entity.header.at == crosser.at {
                    actions::entities::step(turn, &entity.header, across, now + 1);
                } else {
                    actions::entities::set_attribute_of(turn, &crosser, MARK, None, 1);
                    actions::entities::step(turn, &entity.header, entity.header.at, NEVER);
                }
            }
            0
        };
        let report = simulation.tick(&mut arena, &mut entities, utilities::seed::counted(), rule);
        assert_eq!((report.instructions_applied.crossed, entities.len()), (1, 2), "on {threads} threads");
        assert!(entities.get(edge).is_none(), "gone from where it stood");
        let crossed = entities.get(across).expect("across");
        assert_eq!((crossed.attribute(NAME), crossed.attribute(MARK)), (Some(7), Some(1)), "with all that was written to it");
    }
}

/// Two entities writing two members of one attribute of a third in
/// one tick, each held against the member it writes alone: both
/// land, where two writes of the whole attribute would have one
/// refused. One held against a member that is not so is refused; and
/// a write is its member's bytes and no more.
#[test]
fn two_fields_of_one_attribute_are_written_by_two_in_a_tick() {
    let (mut arena, mut entities) = world(2);
    let target = walker(cell(1030, 30), NEVER);
    // An attribute of one word of data, three members of it: four bytes holding 1, two holding 2, two holding 3.
    let (low, middle, high) = (Member::within(0, 0, 4), Member::within(0, 4, 2), Member::within(0, 6, 2));
    entities.queue_put(target, &[AttributeBlock::holding(NAME, 1 | 2 << 32 | 3 << 48)]);
    entities.queue_put(walker(cell(1020, 30), 0), &[]);
    entities.queue_put(walker(cell(1040, 30), 0), &[]);
    entities.apply();
    let kind = NAME.attribute_type();
    let report = Simulation::new(2).tick(&mut arena, &mut entities, utilities::seed::counted(), |turn: &mut Turn| {
        for entity in turn.woken() {
            if number(entity.header.at, cell(1020, 30)) == 1 {
                // The first member, and the one after it seen wrongly.
                actions::entities::set_attribute_member(turn, &target, kind, low, 1, 10);
                actions::entities::set_attribute_member(turn, &target, kind, middle, 9, 70);
            } else {
                // The last: the two bytes beside the one refused.
                actions::entities::set_attribute_member(turn, &target, kind, high, 3, 30);
            }
            actions::entities::step(turn, &entity.header, entity.header.at, NEVER);
        }
        0
    });
    // Two member writes land and one is refused; the two sleeps' compares hold beside them.
    assert_eq!((report.instructions_applied.edits, report.instructions_compared), (2, (4, 1)));
    assert_eq!(entities.get(target.at).expect("where it stood").attribute(NAME), Some(10 | 2 << 32 | 30 << 48));
}
