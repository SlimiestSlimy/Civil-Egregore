//! The fine tier: attributes of more blocks than one, and of a size
//! that varies -- put, found by a walk that steps over each as far as
//! its type or its block length says, edited to another length, saved
//! and read back (`docs/entity_manager.md`, "Attributes, a block each").
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::saved::{decode_state, encode_state};
use entity_manager::{default_kept, may_be_given, WholeAttribute, find_attribute, push_attribute, is_attribute_list, Attribute, AttributeBlock, AttributeType, WorldEntities, EntityType, Header, Instructions, InstructionsApplied, Layout, BLOCK_WORDS, NEVER_WAKES};

/// The entities' type.
const WALKER_TYPE: EntityType = EntityType::from_number(40);

/// Entities holding the superchunk `(10, 10)`, none in it.
fn world() -> WorldEntities {
    let mut world_entities = WorldEntities::new();
    assert_eq!(world_entities.keep_superchunks(&[SuperchunkIndex::from_cartesian(10, 10)]), 0);
    world_entities
}

/// The cell `(x, y)` cells from the top left of the superchunk `(10, 10)`.
fn cell(x: u32, y: u32) -> CellIndex {
    CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + x, y: 10 * SUPERCHUNK_SIDE_CELLS + y }.into()
}

/// A walker on `stood_cell`, never waking.
fn walker_header(stood_cell: CellIndex) -> Header {
    Header { entity_type: WALKER_TYPE, stood_cell, wake: NEVER_WAKES }
}

/// A layout of two blocks: a number in its data's first word and one
/// in its last, the second block's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BothEnds {
    /// The first word's.
    first_word: u64,
    /// The last word's.
    last_word: u64,
}

impl Layout for BothEnds {
    const WORDS: usize = 15;
    const MEMBERS: &'static [entity_manager::Member] = &[entity_manager::Member::word(0), entity_manager::Member::word(14)];

    fn write(self, attribute_data: &mut [u64]) {
        (attribute_data[0], attribute_data[14]) = (self.first_word, self.last_word);
    }

    fn read_data(attribute_data: &[u64]) -> Self {
        Self { first_word: attribute_data[0], last_word: attribute_data[14] }
    }
}

/// An attribute of one block: its number the lowest, so the first of an entity's.
const ONE_BLOCK_ATTRIBUTE: Attribute<u64> = Attribute::new(39);
/// One of eight blocks.
const BOTH_ENDS_ATTRIBUTE: Attribute<BothEnds> = Attribute::new(41);
/// One whose size varies.
const VARYING_NOTE: AttributeType = AttributeType::of_varying_size(42);
/// Another, after it.
const VARYING_TAIL: AttributeType = AttributeType::of_varying_size(43);

/// An attribute of type `attribute_type`, whose size varies, holding `attribute_data`: its
/// block length in its type's last four bits, its data from the
/// second word.
fn varying_attribute(attribute_type: AttributeType, attribute_data: &[u64]) -> Vec<AttributeBlock> {
    let block_count = (1 + attribute_data.len()).div_ceil(BLOCK_WORDS);
    let mut words = vec![0; block_count * BLOCK_WORDS];
    words[0] = attribute_type.with_blocks(block_count).0;
    words[1..1 + attribute_data.len()].copy_from_slice(attribute_data);
    words.as_chunks::<BLOCK_WORDS>().0.iter().map(|&block_words| AttributeBlock(block_words)).collect()
}

/// The attributes an entity of the tests starts with: one block,
/// eight, five by its block length, and one of two after.
fn attributes() -> Vec<AttributeBlock> {
    let mut attribute_list = vec![AttributeBlock::holding_value(ONE_BLOCK_ATTRIBUTE, 7)];
    push_attribute(&mut attribute_list, BOTH_ENDS_ATTRIBUTE, BothEnds { first_word: 11, last_word: 13 });
    attribute_list.extend(varying_attribute(VARYING_NOTE, &[5; 9]));
    attribute_list.extend(varying_attribute(VARYING_TAIL, &[3; 3]));
    attribute_list
}

/// A type's blocks are in its ID, and an entity's attributes are found
/// by stepping over each: one block, two, as many as a block length
/// says.
#[test]
fn every_attribute_is_found_over_those_of_any_length() {
    assert_eq!((ONE_BLOCK_ATTRIBUTE.attribute_type().block_count(), BOTH_ENDS_ATTRIBUTE.attribute_type().block_count(), VARYING_NOTE.0 & 0xF), (1, 8, 0));
    assert_eq!((BOTH_ENDS_ATTRIBUTE.attribute_type().type_number(), VARYING_NOTE.type_number()), (41, 42));
    let attribute_list = attributes();
    assert_eq!(attribute_list.len(), 1 + 8 + 5 + 2);
    assert!(is_attribute_list(&attribute_list));
    assert_eq!(find_attribute(&attribute_list, ONE_BLOCK_ATTRIBUTE.attribute_type()), Ok(0..1));
    assert_eq!(find_attribute(&attribute_list, BOTH_ENDS_ATTRIBUTE.attribute_type()), Ok(1..9));
    assert_eq!(find_attribute(&attribute_list, VARYING_NOTE), Ok(9..14));
    assert_eq!(find_attribute(&attribute_list, VARYING_TAIL), Ok(14..16));
    assert_eq!(find_attribute(&attribute_list, Attribute::<u64>::new(40).attribute_type()), Err(1), "between the first and the second, by number");
    assert_eq!(find_attribute(&attribute_list, Attribute::<u64>::new(50).attribute_type()), Err(16), "after them all");
    assert!(!is_attribute_list(&attribute_list[..12]), "an attribute cut short");
    assert!(!is_attribute_list(&[attribute_list[14], attribute_list[15], attribute_list[0]]), "out of order");
    assert!(!is_attribute_list(&[AttributeBlock([WALKER_TYPE.0, 0])]), "an entity type is no attribute's");

    let mut world_entities = world();
    let header = walker_header(cell(40, 30));
    world_entities.queue_put(header, &attribute_list);
    world_entities.apply();
    let entity = world_entities.get(header.stood_cell).expect("put");
    assert_eq!((entity.attribute(ONE_BLOCK_ATTRIBUTE), entity.attribute(BOTH_ENDS_ATTRIBUTE)), (Some(7), Some(BothEnds { first_word: 11, last_word: 13 })));
    assert_eq!(entity.attribute_blocks(VARYING_NOTE).map(<[_]>::len), Some(5));
    assert_eq!(entity.attribute_blocks(VARYING_TAIL).expect("after the note")[1].0[0], 3);
}

/// An attribute is edited to another length -- longer, shorter, as
/// long, gone -- and those about it are as they were; the entity is
/// saved and read back the same, and a file whose block length is
/// wrong is not read.
#[test]
fn an_attribute_is_edited_to_any_length_and_saved() {
    let mut world_entities = world();
    let target_header = walker_header(cell(40, 30));
    world_entities.queue_put(target_header, &attributes());
    world_entities.apply();
    let edits: [&dyn Fn(&mut Instructions); 4] = [
        &|instruction_queue| instruction_queue.set_attribute_blocks(target_header.stood_cell, &varying_attribute(VARYING_NOTE, &[6; 21])),
        &|instruction_queue| instruction_queue.set_attribute_blocks(target_header.stood_cell, &varying_attribute(VARYING_NOTE, &[8; 3])),
        &|instruction_queue| instruction_queue.set_attribute(target_header.stood_cell, BOTH_ENDS_ATTRIBUTE, BothEnds { first_word: 17, last_word: 19 }),
        &|instruction_queue| instruction_queue.unset_attribute(target_header.stood_cell, ONE_BLOCK_ATTRIBUTE.attribute_type()),
    ];
    for (tick, edit) in edits.into_iter().enumerate() {
        let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
        edit(&mut instruction_queue);
        instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
        assert_eq!(instructions_applied.edits, 1, "edit {tick}");
        let edited_entity = world_entities.get(target_header.stood_cell).expect("where it stood");
        assert!(is_attribute_list(edited_entity.attributes), "edit {tick}");
        let note_blocks = edited_entity.attribute_blocks(VARYING_NOTE).expect("the note");
        assert_eq!((note_blocks.len(), note_blocks[1].0[0]), if tick == 0 { (11, 6) } else { (2, 8) }, "edit {tick}");
        assert_eq!(edited_entity.attribute(BOTH_ENDS_ATTRIBUTE), Some(if tick < 2 { BothEnds { first_word: 11, last_word: 13 } } else { BothEnds { first_word: 17, last_word: 19 } }), "edit {tick}");
        assert_eq!(edited_entity.attribute(ONE_BLOCK_ATTRIBUTE), (tick < 3).then_some(7), "edit {tick}");
        assert_eq!(edited_entity.attribute_blocks(VARYING_TAIL).map(|tail_blocks| tail_blocks[1].0[0]), Some(3), "edit {tick}");
    }

    let superchunk = target_header.stood_cell.superchunk();
    let (words, count) = encode_state(None, world_entities.superchunk(superchunk));
    assert_eq!(count, 1);
    let mut read_back = world();
    assert_eq!(decode_state(&words, 4, &mut read_back).map(|saved_state| saved_state.entity_count), Ok(1));
    read_back.apply();
    let (saved_entity, read_entity) = (world_entities.get(target_header.stood_cell).expect("there"), read_back.get(target_header.stood_cell).expect("read back"));
    assert_eq!((saved_entity.header, saved_entity.attributes), (read_entity.header, read_entity.attributes));

    // The note's block length said to be more than the entity has blocks: nothing is read.
    let mut wrong_words = words.clone();
    let note_type_index = wrong_words.iter().position(|&word| AttributeType(word).type_identity() == VARYING_NOTE.type_identity()).expect("the note's type");
    wrong_words[note_type_index] = VARYING_NOTE.with_blocks(16).0;
    assert!(decode_state(&wrong_words, 4, &mut world()).is_err());
    assert!(decode_state(&words[..words.len() - 1], 4, &mut world()).is_err(), "cut short");
}

/// Attributes of types drawn -- one block, two, three, and of sizes
/// that vary -- set, set again to another length and unset in an order
/// drawn: after every edit the entity holds exactly those set and not
/// unset since, each as last set, in the order of their types; an
/// unset of one it has not changes nothing; and saved, it is read back
/// the same.
#[test]
fn attributes_edited_in_any_order_are_those_last_set() {
    use std::collections::BTreeMap;
    let mut seeded_rng = utilities::rng::RandomSource::new(utilities::seed::counted_seed());
    let mut world_entities = world();
    let target_header = walker_header(cell(seeded_rng.draw_below(1024) as u32, seeded_rng.draw_below(1024) as u32));
    world_entities.queue_put(target_header, &[]);
    world_entities.apply();
    // Twelve types: numbers drawn, no two the same, eight of 1 to 3 blocks and four whose size varies.
    let mut type_numbers = std::collections::BTreeSet::new();
    while type_numbers.len() < 12 {
        type_numbers.insert(1 + seeded_rng.draw_below(1 << 20));
    }
    // Each with whether its size varies: the type alone does not say, its registering does.
    let attribute_types: Vec<(AttributeType, bool)> = type_numbers.into_iter().enumerate().map(|(type_index, type_number)| if type_index % 3 == 2 { (AttributeType::of_varying_size(type_number), true) } else { (AttributeType::of_blocks(type_number, 1 + type_index % 4 % 3), false) }).collect();
    let mut held_attributes: BTreeMap<u64, Vec<AttributeBlock>> = BTreeMap::new();
    for edit in 0..400 {
        let (attribute_type, size_varies) = attribute_types[seeded_rng.draw_below(attribute_types.len() as u64) as usize];
        let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
        if seeded_rng.draw_below(3) == 0 {
            instruction_queue.unset_attribute(target_header.stood_cell, attribute_type);
            held_attributes.remove(&attribute_type.0);
        } else {
            let set_blocks = match (!size_varies).then_some(attribute_type.block_count()) {
                Some(block_count) => {
                    let mut words: Vec<u64> = (0..block_count * BLOCK_WORDS).map(|_| seeded_rng.next_draw()).collect();
                    words[0] = attribute_type.0;
                    words.as_chunks::<BLOCK_WORDS>().0.iter().map(|&block_words| AttributeBlock(block_words)).collect()
                }
                None => varying_attribute(attribute_type, &(0..1 + seeded_rng.draw_below(28)).map(|_| seeded_rng.next_draw()).collect::<Vec<_>>()),
            };
            instruction_queue.set_attribute_blocks(target_header.stood_cell, &set_blocks);
            held_attributes.insert(attribute_type.0, set_blocks);
        }
        instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
        assert_eq!((instructions_applied.edits, instructions_applied.passed_over), (1, 0), "edit {edit}");
        let edited_entity = world_entities.get(target_header.stood_cell).expect("where it stood");
        assert!(is_attribute_list(edited_entity.attributes), "edit {edit}");
        let expected_blocks: Vec<AttributeBlock> = held_attributes.values().flatten().copied().collect();
        assert!(edited_entity.attributes == expected_blocks.as_slice(), "edit {edit}: {} blocks held, {} expected", edited_entity.attributes.len(), expected_blocks.len());
        for &(attribute_type, _) in &attribute_types {
            assert_eq!(edited_entity.attribute_blocks(attribute_type), held_attributes.get(&attribute_type.0).map(Vec::as_slice), "edit {edit}: {attribute_type:?}");
        }
    }
    let (words, count) = encode_state(None, world_entities.superchunk(target_header.stood_cell.superchunk()));
    let mut read_back = world();
    assert_eq!((count, decode_state(&words, 4, &mut read_back).map(|saved_state| saved_state.entity_count)), (1, Ok(1)));
    read_back.apply();
    let (saved_entity, read_entity) = (world_entities.get(target_header.stood_cell).expect("there"), read_back.get(target_header.stood_cell).expect("read back"));
    assert!((saved_entity.header, saved_entity.attributes) == (read_entity.header, read_entity.attributes));
}

/// An entity is named, all a tick long, by the cell the tick found it
/// on: an attribute written to it after it moved lands on it where it
/// now stands, as does its removal; and that cell is no other's until
/// the tick is over -- one stepping to it stays, one new is refused.
/// Each instruction is applied as it is come to, none before another
/// for its kind. An instruction for an entity that is not there is
/// passed over, and counted.
#[test]
fn an_entity_is_named_by_the_cell_the_tick_found_it_on() {
    let mut world_entities = world();
    let (first_cell, second_cell, third_cell) = (cell(40, 30), cell(41, 30), cell(42, 30));
    world_entities.queue_put(walker_header(first_cell), &[]);
    world_entities.queue_put(walker_header(second_cell), &[]);
    world_entities.apply();
    let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
    instruction_queue.move_entity(walker_header(third_cell), first_cell);
    instruction_queue.set_attribute(first_cell, ONE_BLOCK_ATTRIBUTE, 7);
    instruction_queue.move_entity(walker_header(first_cell), second_cell);
    instruction_queue.put(walker_header(first_cell), &[]);
    instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
    assert_eq!((instructions_applied.moves, instructions_applied.edits, instructions_applied.turned_back, instructions_applied.puts_refused, instructions_applied.passed_over), (2, 1, 1, 1, 0));
    assert_eq!(world_entities.get(third_cell).and_then(|moved_entity| moved_entity.attribute(ONE_BLOCK_ATTRIBUTE)), Some(7), "written where it moved to");
    assert!(world_entities.get(second_cell).is_some() && world_entities.len() == 2, "the cell left is no other's this tick");
    // Still the same tick: removed by the name it has in it, and nothing written to it after.
    let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
    instruction_queue.remove(first_cell);
    instruction_queue.set_attribute(first_cell, ONE_BLOCK_ATTRIBUTE, 8);
    instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
    assert_eq!((instructions_applied.entities_removed, instructions_applied.passed_over, world_entities.len()), (1, 1, 1));
    // The tick over, the cells are whoever comes to them.
    world_entities.superchunks_mut().iter_mut().for_each(|superchunk| superchunk.names_anew(&mut ()));
    let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
    instruction_queue.put(walker_header(first_cell), &[]);
    instruction_queue.put(walker_header(third_cell), &[]);
    instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
    assert_eq!((instructions_applied.puts, world_entities.len()), (2, 3));
    world_entities.superchunks_mut().iter_mut().for_each(|superchunk| superchunk.names_anew(&mut ()));

    let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
    let nobodys_cell = cell(50, 50);
    instruction_queue.remove(nobodys_cell);
    instruction_queue.unset_attribute(nobodys_cell, ONE_BLOCK_ATTRIBUTE.attribute_type());
    instruction_queue.move_entity(walker_header(second_cell), nobodys_cell);
    instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
    assert_eq!((instructions_applied.passed_over, instructions_applied.entities_removed, instructions_applied.edits, instructions_applied.moves, world_entities.len()), (3, 0, 0, 0, 3));
}

/// A sheep, whose type is registered with a default attribute -- its
/// state -- has it always: given with its type like any attribute,
/// or not given and zeroes; kept first and without its type, so a
/// sheep with nothing else is its data's blocks and no more; read,
/// compared and written whole or a member at a time by its type as
/// any other, wherever it sorts among the others; never removed;
/// moved to another chunk and saved with the sheep. States, the
/// attributes about them and the edits are drawn from the seed.
#[test]
fn a_types_default_attribute_is_kept_without_its_type_and_used_as_any_other() {
    use type_registry::{SheepState, SHEEP, SHEEP_STATE};
    use utilities::rng::RandomSource;
    /// An attribute sorting before the sheep's state, and one after.
    const SORTS_BEFORE_STATE: Attribute<u64> = Attribute::new(11);
    let kept_default = default_kept(SHEEP).expect("registered with one");
    assert_eq!((kept_default.attribute_type, kept_default.words, kept_default.block_count), (SHEEP_STATE.attribute_type(), SheepState::WORDS, SheepState::WORDS.div_ceil(BLOCK_WORDS)));
    assert_eq!(default_kept(WALKER_TYPE), None);
    let mut seeded_rng = RandomSource::new(utilities::seed::counted_seed());
    let draw_state = |seeded_rng: &mut RandomSource| SheepState { hungry_at: seeded_rng.next_draw(), lamb_due_at: seeded_rng.next_draw(), grown_at: seeded_rng.next_draw(), roaming_until: seeded_rng.next_draw(), roaming_neighbour: seeded_rng.next_draw() as u32 };
    for round_number in 0..200 {
        let mut world_entities = world();
        let sheep_cell = cell(seeded_rng.draw_below(200) as u32, seeded_rng.draw_below(200) as u32);
        let sheep_header = Header { entity_type: SHEEP, stood_cell: sheep_cell, wake: NEVER_WAKES };
        // Given its state or not, with an attribute before it by type, one after, both or neither.
        let (drawn_state, state_given, has_before, has_after) = (draw_state(&mut seeded_rng), seeded_rng.draw_below(2) == 0, seeded_rng.draw_below(2) == 0, seeded_rng.draw_below(2) == 0);
        let mut attributes = Vec::new();
        if has_before {
            push_attribute(&mut attributes, SORTS_BEFORE_STATE, 5);
        }
        if state_given {
            push_attribute(&mut attributes, SHEEP_STATE, drawn_state);
        }
        if has_after {
            push_attribute(&mut attributes, ONE_BLOCK_ATTRIBUTE, 7);
        }
        assert!(may_be_given(SHEEP, &attributes));
        world_entities.queue_put(sheep_header, &attributes);
        world_entities.apply();
        let mut held_state = if state_given { drawn_state } else { SheepState::default() };
        let put = world_entities.get(sheep_cell).expect("put");
        // Kept: its data's blocks, then the others with their types -- a block each here.
        assert_eq!(put.attribute_list().len(), kept_default.block_count + usize::from(has_before) + usize::from(has_after), "round {round_number}");
        assert_eq!(put.attributes.len(), usize::from(has_before) + usize::from(has_after));
        // Its first word is its data's first, no type before it.
        assert_eq!(put.attribute_list()[0].0[0], held_state.hungry_at, "round {round_number}");
        assert_eq!((put.attribute(SHEEP_STATE), put.attribute(SORTS_BEFORE_STATE), put.attribute(ONE_BLOCK_ATTRIBUTE)), (Some(held_state), has_before.then_some(5), has_after.then_some(7)), "round {round_number}");
        assert_eq!(put.default_attribute().as_deref(), Some(&*WholeAttribute::holding_value(SHEEP_STATE, held_state)));
        assert_eq!(put.attribute_seen(SHEEP_STATE.attribute_type()), entity_manager::attribute_seen(Some(&WholeAttribute::holding_value(SHEEP_STATE, held_state))));
        // Edits drawn: the state whole, a member of it, its removal -- refused -- and the others', which leave it be.
        for edit in 0..8 {
            let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
            let now_at = world_entities.iter().next().expect("the one sheep").header.stood_cell;
            let chosen_edit = seeded_rng.draw_below(5);
            match chosen_edit {
                0 => {
                    held_state = draw_state(&mut seeded_rng);
                    instruction_queue.set_attribute(now_at, SHEEP_STATE, held_state);
                }
                1 => {
                    let (member, value) = (seeded_rng.draw_below(SheepState::MEMBERS.len() as u64) as usize, seeded_rng.next_draw());
                    let member = SheepState::MEMBERS[member];
                    // Where the member lies in the attribute, its type counted: as an instruction says it.
                    let superchunk = world_entities.superchunks_mut().iter_mut().next().expect("the one");
                    assert!(superchunk.write_attribute_bytes_named(now_at, SHEEP_STATE.attribute_type(), usize::from(member.byte_offset), usize::from(member.byte_count), value));
                    let mut words = [0; SheepState::WORDS];
                    held_state.write(&mut words);
                    let (word, first_bit, mask) = (usize::from(member.byte_offset) / 8 - 1, usize::from(member.byte_offset) % 8 * 8, u64::MAX >> (64 - u32::from(member.byte_count) * 8));
                    words[word] = words[word] & !(mask << first_bit) | (value & mask) << first_bit;
                    held_state = SheepState::read_data(&words);
                    assert_eq!(world_entities.get(now_at).expect("there").attribute_member(SHEEP_STATE.attribute_type(), usize::from(member.byte_offset), usize::from(member.byte_count)), Some(value & mask));
                }
                2 => instruction_queue.unset_attribute(now_at, SHEEP_STATE.attribute_type()),
                3 => instruction_queue.set_attribute(now_at, ONE_BLOCK_ATTRIBUTE, 100 + edit),
                // To a cell of another chunk as often as not: all it has goes with it.
                _ => instruction_queue.move_entity(Header { stood_cell: cell(seeded_rng.draw_below(600) as u32, seeded_rng.draw_below(600) as u32), ..sheep_header }, now_at),
            }
            instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
            // The tick over: it is named by where it stands now.
            world_entities.superchunks_mut().iter_mut().for_each(|superchunk| superchunk.names_anew(&mut ()));
            // Its removal alone is passed over: a default attribute is never taken.
            assert_eq!(instructions_applied.passed_over, usize::from(chosen_edit == 2), "round {round_number}, edit {edit}");
            let edited_entity = world_entities.iter().next().expect("the one sheep");
            assert_eq!(edited_entity.attribute(SHEEP_STATE), Some(held_state), "round {round_number}, edit {edit}");
            assert!(is_attribute_list(edited_entity.attributes) && edited_entity.attribute_list().len() == kept_default.block_count + edited_entity.attributes.len(), "round {round_number}, edit {edit}");
            assert_eq!(edited_entity.attribute(SORTS_BEFORE_STATE), has_before.then_some(5), "round {round_number}, edit {edit}");
        }
        // Saved and read back: the same sheep, kept the same.
        let saved_entity = world_entities.iter().next().expect("the one sheep");
        let (words, count) = encode_state(None, world_entities.superchunk(saved_entity.header.stood_cell.superchunk()));
        let mut read_back = world();
        assert_eq!((count, decode_state(&words, 0, &mut read_back).map(|saved_state| saved_state.entity_count)), (1, Ok(1)));
        read_back.apply();
        let read_entity = read_back.get(saved_entity.header.stood_cell).expect("read back");
        assert_eq!((saved_entity.header, saved_entity.attribute_list()), (read_entity.header, read_entity.attribute_list()), "round {round_number}");
    }
}
