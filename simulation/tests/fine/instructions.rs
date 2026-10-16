//! What a rule does to entities, an instruction each: a step and a
//! sleep carry no attributes and keep them, across chunks and
//! superchunks; a step onto a taken cell is turned back; one entity
//! edits another, an attribute at a time.
//!
//! `cargo test`

use bitplane_manager::{BitmapArena, BucketKey};
use chunk_storage::{LayerCodec, LayerType};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::{Attribute, AttributeBlock, WorldEntities, EntityType, Header, Member, NEVER_WAKES};
use simulation::{Simulation, Turn};

/// The layer type the arena holds: every cell hot, none set.
const STONE_LAYER: LayerType = LayerType::of_a_bit(6);
/// The entities' type.
const WALKER_TYPE: EntityType = EntityType::from_number(40);
/// An attribute.
const NAME_ATTRIBUTE: Attribute<u64> = Attribute::new(41);
/// Another.
const MARK_ATTRIBUTE: Attribute<u64> = Attribute::new(42);
/// A third.
const SCAR_ATTRIBUTE: Attribute<u64> = Attribute::new(43);

/// An arena with a bitmap hot over the `side_superchunks` by `side_superchunks` superchunks from
/// `(10, 10)`, and entities holding the same superchunks.
fn world(side_superchunks: u32) -> (BitmapArena, WorldEntities) {
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    for y in 10..10 + side_superchunks {
        for x in 10..10 + side_superchunks {
            for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
                bitmap_arena.make_hot(BucketKey { layer_type: STONE_LAYER, chunk }, None, &mut layer_codec);
            }
        }
    }
    let mut entity_store = WorldEntities::new();
    assert_eq!(entity_store.keep_superchunks(&bitmap_arena.superchunk_indices()), 0);
    (bitmap_arena, entity_store)
}

/// The cell `(x, y)` cells from the top left of the superchunk `(10, 10)`.
fn cell(x: u32, y: u32) -> CellIndex {
    CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + x, y: 10 * SUPERCHUNK_SIDE_CELLS + y }.into()
}

/// A walker on `stood_cell`, waking at `wake`.
fn walker_header(stood_cell: CellIndex, wake: u64) -> Header {
    Header { entity_type: WALKER_TYPE, stood_cell, wake }
}

/// The number a test knows the walker on `walker_cell` by: 1 the one on
/// `first_cell`, 2 any other.
fn known_number(walker_cell: CellIndex, first_cell: CellIndex) -> u64 {
    if walker_cell == first_cell { 1 } else { 2 }
}

/// Every walker woken steps a cell to the right, to wake next tick.
fn step_right(turn: &mut Turn) -> usize {
    let current_tick = turn.current_tick();
    for entity in turn.woken_entities() {
        actions::entities::step(turn, &entity.header, entity.header.stood_cell.at_cartesian_offset(1, 0).expect("in the world"), current_tick + 1);
    }
    0
}

/// Entities stepping keep their attributes though none is carried: in
/// their chunk, into the next, and into the next superchunk, where they
/// go whole.
#[test]
fn a_step_carries_no_attributes_and_keeps_them() {
    let (mut bitmap_arena, mut entity_store) = world(2);
    let attributes = [AttributeBlock::holding_value(NAME_ATTRIBUTE, 7), AttributeBlock::holding_value(MARK_ATTRIBUTE, 9)];
    // In a chunk, over a chunk's border, over a superchunk's.
    for x in [40, 250, 1020] {
        entity_store.queue_put(walker_header(cell(x, 30), 0), &attributes);
    }
    entity_store.apply();
    let mut world_simulation = Simulation::new(2);
    let (mut moves, mut puts) = (0, 0);
    for _ in 0..10 {
        let report = world_simulation.tick(&mut bitmap_arena, &mut entity_store, utilities::seed::counted_seed(), step_right);
        (moves, puts) = (moves + report.instructions_applied.moves, puts + report.instructions_applied.puts);
    }
    for x in [50, 260, 1030] {
        let entity = entity_store.get(cell(x, 30)).expect("ten steps on");
        assert_eq!(entity.attributes, attributes);
    }
    assert_eq!(entity_store.len(), 3);
    // Every step a move, the one over the superchunk's border as any other: nothing is put.
    assert_eq!((moves, puts), (30, 0));
}

/// A step onto a cell an entity stands on is turned back as it is
/// applied: the stepper stays, and wakes when it was to all the
/// same.
#[test]
fn a_step_onto_a_taken_cell_is_turned_back() {
    let (mut bitmap_arena, mut entity_store) = world(1);
    entity_store.queue_put(walker_header(cell(40, 30), 0), &[AttributeBlock::holding_value(NAME_ATTRIBUTE, 1)]);
    entity_store.queue_put(walker_header(cell(41, 30), NEVER_WAKES), &[]);
    entity_store.apply();
    let mut world_simulation = Simulation::new(1);
    let (mut stayed_count, mut moves) = (0, 0);
    for _ in 0..3 {
        let report = world_simulation.tick(&mut bitmap_arena, &mut entity_store, utilities::seed::counted_seed(), step_right);
        (stayed_count, moves) = (stayed_count + report.instructions_applied.turned_back, moves + report.instructions_applied.moves);
    }
    assert_eq!((stayed_count, moves), (3, 3), "turned back each tick, and woken the next");
    let stepper_entity = entity_store.get(cell(40, 30)).expect("where it stood");
    assert_eq!((stepper_entity.header.wake, stepper_entity.attribute(NAME_ATTRIBUTE)), (3, Some(1)));
    assert!(entity_store.get(cell(41, 30)).is_some());
}

/// Entities edit another, an attribute each, in one tick, each saying
/// what it saw of it: both land, neither undoing the other, and the
/// one edited keeps what it had and when it wakes. An attribute is
/// removed the same way.
#[test]
fn entities_edit_another_an_attribute_at_a_time() {
    let (mut bitmap_arena, mut entity_store) = world(2);
    // The one edited, in another superchunk than one of the two editing it.
    let edited_header = walker_header(cell(1030, 30), NEVER_WAKES);
    entity_store.queue_put(edited_header, &[AttributeBlock::holding_value(NAME_ATTRIBUTE, 7)]);
    entity_store.queue_put(walker_header(cell(1020, 30), 0), &[]);
    entity_store.queue_put(walker_header(cell(1040, 30), 0), &[]);
    entity_store.apply();
    let mut world_simulation = Simulation::new(2);
    let report = world_simulation.tick(&mut bitmap_arena, &mut entity_store, utilities::seed::counted_seed(), |turn: &mut Turn| {
        for entity in turn.woken_entities() {
            let editor_number = known_number(entity.header.stood_cell, cell(1020, 30));
            actions::entities::set_attribute_of(turn, &edited_header, if editor_number == 1 { MARK_ATTRIBUTE } else { SCAR_ATTRIBUTE }, None, editor_number);
            actions::entities::step(turn, &entity.header, entity.header.stood_cell, NEVER_WAKES);
        }
        0
    });
    // Each edit an attribute given whole, one write, under a compare of none being seen; each editor's sleep under a compare of its being there.
    assert_eq!((report.instructions_applied.edits, report.instructions_applied.puts, report.instructions_compared), (2, 0, (4, 0)));
    let edited_entity = entity_store.get(edited_header.stood_cell).expect("where it stood");
    assert_eq!(edited_entity.attributes, [AttributeBlock::holding_value(NAME_ATTRIBUTE, 7), AttributeBlock::holding_value(MARK_ATTRIBUTE, 1), AttributeBlock::holding_value(SCAR_ATTRIBUTE, 2)]);
    assert_eq!(edited_entity.header, edited_header);

    entity_store.queue_put(walker_header(cell(1031, 31), 1), &[]);
    entity_store.apply();
    world_simulation.tick(&mut bitmap_arena, &mut entity_store, utilities::seed::counted_seed(), |turn: &mut Turn| {
        for entity in turn.woken_entities() {
            actions::entities::unset_attribute_of(turn, &edited_header, MARK_ATTRIBUTE, 1);
            actions::entities::set_attribute_of(turn, &edited_header, NAME_ATTRIBUTE, Some(7), 8);
            actions::entities::step(turn, &entity.header, entity.header.stood_cell, NEVER_WAKES);
        }
        0
    });
    let edited_entity_after = entity_store.get(edited_header.stood_cell).expect("where it stood");
    assert_eq!(edited_entity_after.attributes, [AttributeBlock::holding_value(NAME_ATTRIBUTE, 8), AttributeBlock::holding_value(SCAR_ATTRIBUTE, 2)]);
}

/// Two entities setting one attribute of a third in one tick, both
/// having seen it unset: the first applied is kept and the other
/// refused -- neither written over -- the same one on any number of
/// threads.
#[test]
fn of_two_edits_of_one_attribute_one_is_refused_the_same_on_any_threads() {
    let kept_values = [1, 4].map(|thread_count| {
        let (mut bitmap_arena, mut entity_store) = world(2);
        // The one edited, and the two editing it from the two superchunks beside its border.
        let edited_header = walker_header(cell(1030, 30), NEVER_WAKES);
        entity_store.queue_put(edited_header, &[]);
        entity_store.queue_put(walker_header(cell(1020, 30), 0), &[]);
        entity_store.queue_put(walker_header(cell(1040, 30), 0), &[]);
        entity_store.apply();
        let report = Simulation::new(thread_count).tick(&mut bitmap_arena, &mut entity_store, utilities::seed::counted_seed(), |turn: &mut Turn| {
            for entity in turn.woken_entities() {
                actions::entities::set_attribute_of(turn, &edited_header, NAME_ATTRIBUTE, None, known_number(entity.header.stood_cell, cell(1020, 30)));
                actions::entities::step(turn, &entity.header, entity.header.stood_cell, NEVER_WAKES);
            }
            0
        });
        // The attribute given whole, once; the two sleeps' compares hold beside the one edit's.
        assert_eq!((report.instructions_applied.edits, report.instructions_compared), (1, (3, 1)));
        entity_store.get(edited_header.stood_cell).expect("where it stood").attribute(NAME_ATTRIBUTE).expect("set")
    });
    assert!(kept_values[0] == kept_values[1] && [1, 2].contains(&kept_values[0]), "{kept_values:?}");
}

/// An entity awake, changing itself, is written by another in the same
/// tick: what the other writes of an attribute it left alone stands
/// with its own change, and of the attribute both write one write is
/// applied and the other refused -- the same one on any number of
/// threads. It is never put whole over what another wrote.
#[test]
fn an_entity_changing_itself_and_another_writing_it_both_stand() {
    let kept_values = [1, 4].map(|thread_count| {
        let (mut bitmap_arena, mut entity_store) = world(2);
        // The one written, awake, in another superchunk than the one writing it.
        let written_header = walker_header(cell(1030, 30), 0);
        entity_store.queue_put(written_header, &[AttributeBlock::holding_value(NAME_ATTRIBUTE, 7)]);
        entity_store.queue_put(walker_header(cell(1020, 30), 0), &[]);
        entity_store.apply();
        let report = Simulation::new(thread_count).tick(&mut bitmap_arena, &mut entity_store, utilities::seed::counted_seed(), |turn: &mut Turn| {
            for entity in turn.woken_entities() {
                if entity.header.stood_cell == written_header.stood_cell {
                    // Itself: its name changed, asleep from here on.
                    actions::entities::update_entity(turn, &entity.header, Header { wake: NEVER_WAKES, ..entity.header }, &[AttributeBlock::holding_value(NAME_ATTRIBUTE, 8)]);
                } else {
                    actions::entities::set_attribute_of(turn, &written_header, MARK_ATTRIBUTE, None, 1);
                    actions::entities::set_attribute_of(turn, &written_header, NAME_ATTRIBUTE, Some(7), 99);
                    actions::entities::step(turn, &entity.header, entity.header.stood_cell, NEVER_WAKES);
                }
            }
            0
        });
        assert_eq!((report.instructions_applied.edits, report.instructions_compared), (2, (4, 1)), "the mark given whole and one name written, the other name refused; the two sleeps' compares hold");
        let written_entity = entity_store.get(written_header.stood_cell).expect("where it stood");
        assert_eq!((written_entity.attribute(MARK_ATTRIBUTE), written_entity.header.wake), (Some(1), NEVER_WAKES), "the other's mark, and its own sleep");
        written_entity.attribute(NAME_ATTRIBUTE).expect("named")
    });
    assert!(kept_values[0] == kept_values[1] && [8, 99].contains(&kept_values[0]), "{kept_values:?}");
}

/// An entity crossing to another superchunk, written by another in the
/// same tick, crosses with what was written to it: as one stepping
/// within a superchunk does, whichever of the two is applied first.
#[test]
fn a_crossing_written_behind_takes_what_was_written() {
    for thread_count in [1, 4] {
        let (mut bitmap_arena, mut entity_store) = world(2);
        // On the last cell before the border, to step over it.
        let (before_border_cell, across_cell) = (cell(SUPERCHUNK_SIDE_CELLS - 1, 30), cell(SUPERCHUNK_SIDE_CELLS, 30));
        let crosser_header = walker_header(before_border_cell, 0);
        entity_store.queue_put(crosser_header, &[AttributeBlock::holding_value(NAME_ATTRIBUTE, 7)]);
        entity_store.queue_put(walker_header(cell(1000, 30), 0), &[]);
        entity_store.apply();
        let mut world_simulation = Simulation::new(thread_count);
        let rule = |turn: &mut Turn| {
            let current_tick = turn.current_tick();
            for entity in turn.woken_entities() {
                if entity.header.stood_cell == crosser_header.stood_cell {
                    actions::entities::step(turn, &entity.header, across_cell, current_tick + 1);
                } else {
                    actions::entities::set_attribute_of(turn, &crosser_header, MARK_ATTRIBUTE, None, 1);
                    actions::entities::step(turn, &entity.header, entity.header.stood_cell, NEVER_WAKES);
                }
            }
            0
        };
        let report = world_simulation.tick(&mut bitmap_arena, &mut entity_store, utilities::seed::counted_seed(), rule);
        assert_eq!((report.instructions_applied.border_crossings, entity_store.len()), (1, 2), "on {thread_count} threads");
        assert!(entity_store.get(before_border_cell).is_none(), "gone from where it stood");
        let crossed_entity = entity_store.get(across_cell).expect("across");
        assert_eq!((crossed_entity.attribute(NAME_ATTRIBUTE), crossed_entity.attribute(MARK_ATTRIBUTE)), (Some(7), Some(1)), "with all that was written to it");
    }
}

/// Two entities writing two members of one attribute of a third in
/// one tick, each held against the member it writes alone: both
/// land, where two writes of the whole attribute would have one
/// refused. One held against a member that is not so is refused; and
/// a write is its member's bytes and no more.
#[test]
fn two_fields_of_one_attribute_are_written_by_two_in_a_tick() {
    let (mut bitmap_arena, mut entity_store) = world(2);
    let edited_header = walker_header(cell(1030, 30), NEVER_WAKES);
    // An attribute of one word of data, three members of it: four bytes holding 1, two holding 2, two holding 3.
    let (low_member, middle_member, high_member) = (Member::within_word(0, 0, 4), Member::within_word(0, 4, 2), Member::within_word(0, 6, 2));
    entity_store.queue_put(edited_header, &[AttributeBlock::holding_value(NAME_ATTRIBUTE, 1 | 2 << 32 | 3 << 48)]);
    entity_store.queue_put(walker_header(cell(1020, 30), 0), &[]);
    entity_store.queue_put(walker_header(cell(1040, 30), 0), &[]);
    entity_store.apply();
    let name_attribute_type = NAME_ATTRIBUTE.attribute_type();
    let report = Simulation::new(2).tick(&mut bitmap_arena, &mut entity_store, utilities::seed::counted_seed(), |turn: &mut Turn| {
        for entity in turn.woken_entities() {
            if known_number(entity.header.stood_cell, cell(1020, 30)) == 1 {
                // The first member, and the one after it seen wrongly.
                actions::entities::set_attribute_member(turn, &edited_header, name_attribute_type, low_member, 1, 10);
                actions::entities::set_attribute_member(turn, &edited_header, name_attribute_type, middle_member, 9, 70);
            } else {
                // The last: the two bytes beside the one refused.
                actions::entities::set_attribute_member(turn, &edited_header, name_attribute_type, high_member, 3, 30);
            }
            actions::entities::step(turn, &entity.header, entity.header.stood_cell, NEVER_WAKES);
        }
        0
    });
    // Two member writes land and one is refused; the two sleeps' compares hold beside them.
    assert_eq!((report.instructions_applied.edits, report.instructions_compared), (2, (4, 1)));
    assert_eq!(entity_store.get(edited_header.stood_cell).expect("where it stood").attribute(NAME_ATTRIBUTE), Some(10 | 2 << 32 | 30 << 48));
}
