//! The entity manager: attributes of more blocks than one, and of a size
//! that varies -- put, found by a walk that steps over each as far as
//! its type or its block length says, edited to another length, saved
//! and read back (`docs/entity_manager.md`, "Attributes, a block each").
//! Here and not in the server's own tests: its types are the base
//! game's, registered beside the tests' own.

// TEMPORARY: reaches the server's internals -- `server::internals_for_root_tests`
// and the world's handles to them -- until the world has reads of its
// own and this is rewritten against them (AI.txt, LATER).

use utilities::coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use server::internals_for_root_tests::{decode_state, encode_state};
use basegame::base_types::BaseTypes;
use protocol::type_registry::{FrozenTypeRegistry, RegisteredType, Registration, TypeRegistry};
use utilities::rng::RandomSource;
use server::internals_for_root_tests::{may_be_given, DefaultAttributes, WholeAttribute, find_attribute, push_attribute, is_attribute_list, Attribute, AttributeBlock, AttributeType, WorldEntities, EntityType, Header, Instructions, InstructionsApplied, Layout, BLOCK_WORDS, NEVER_WAKES};

/// Types registered before a test's own, at most: as many as its seed
/// draws, for their IDs to be other with every seed.
const MOST_FILLERS: u64 = 64;

/// The types the store is tried with: the base game's and the tests'
/// own, registered by each test in a registry of its own
/// ([`registered_types`]) -- never a number by hand.
#[derive(Clone, Copy)]
struct TestTypes {
    /// The base game's.
    base: BaseTypes,
    /// An attribute registered before the base game's types: of an ID
    /// below the sheep's state's.
    sorts_before_state: Attribute<u64>,
    /// The entities' type.
    walker: EntityType,
    /// An attribute of one block.
    one_block: Attribute<u64>,
    /// One of eight blocks.
    both_ends: Attribute<BothEnds>,
    /// One whose size varies.
    varying_note: AttributeType,
    /// Another.
    varying_tail: AttributeType,
    /// One no entity is given: what an attribute not held is looked for as.
    never_held: AttributeType,
    /// Twelve more, eight of 1 to 3 blocks and four whose size varies,
    /// each with whether it varies: the type alone does not say, its
    /// registering does.
    edited: [(AttributeType, bool); 12],
}

/// Whether the edited attribute `edited` is one whose size varies, and
/// if not its blocks.
fn edited_shape(edited: usize) -> Option<usize> {
    (edited % 3 != 2).then_some(1 + edited % 4 % 3)
}

/// The tests' types of `seed`, registered and frozen: as many fillers
/// as it draws, the attribute sorting before the state, the base game's,
/// then the rest in an order drawn -- no test counting on which of them
/// is of the lower ID.
fn registered_types(seed: u64) -> (FrozenTypeRegistry, TestTypes) {
    let (mut type_registry, mut seeded_random) = (TypeRegistry::for_new_world(), RandomSource::new(seed ^ 0x7e57_7e57));
    let registered_alone = |type_registry: &mut TypeRegistry, registration: Registration| type_registry.register(registration).expect("a test's type, of a name no other has");
    // A bundle of its own before the base game's, for its attribute to be of a lower ID.
    let sorts_before_state = {
        let mut before_the_base_game = type_registry.open_bundle("tests before").expect("a bundle of its own");
        for filler in 0..seeded_random.draw_below(MOST_FILLERS + 1) {
            before_the_base_game.register(Registration::layer(&format!("filler {filler}"))).expect("a filler of its own name");
        }
        Attribute::registered(&registered_alone(&mut before_the_base_game, Attribute::<u64>::registration("sorts before state"))).expect("a word's attribute")
    };
    let base = basegame::default_types::register(&mut type_registry).expect("the base game's types registered");
    let mut tests = type_registry.open_bundle("tests").expect("the tests' bundle");
    let varying = |name: &str| Registration::attribute(name, 1, 0);
    let mut registrations: Vec<(usize, Registration)> = [Registration::entity("walker", None), Attribute::<u64>::registration("one block"), Attribute::<BothEnds>::registration("both ends"), varying("varying note"), varying("varying tail"), varying("never held")]
        .into_iter()
        .chain((0..12).map(|edited| match edited_shape(edited) {
            Some(block_count) => Registration::attribute(&format!("edited {edited}"), block_count, block_count * BLOCK_WORDS - 1),
            None => varying(&format!("edited {edited}")),
        }))
        .enumerate()
        .collect();
    for last in (1..registrations.len()).rev() {
        registrations.swap(last, seeded_random.draw_below(last as u64 + 1) as usize);
    }
    let mut registered: Vec<(usize, RegisteredType)> = registrations.into_iter().map(|(place, registration)| (place, registered_alone(&mut tests, registration))).collect();
    registered.sort_by_key(|(place, _)| *place);
    let attribute_type = |place: usize| registered[place].1.attribute_type().expect("an attribute");
    let test_types = TestTypes {
        base,
        sorts_before_state,
        walker: registered[0].1.entity_type().expect("a type of entity"),
        one_block: Attribute::registered(&registered[1].1).expect("a word's attribute"),
        both_ends: Attribute::registered(&registered[2].1).expect("eight blocks' attribute"),
        varying_note: attribute_type(3),
        varying_tail: attribute_type(4),
        never_held: attribute_type(5),
        edited: std::array::from_fn(|edited| (attribute_type(6 + edited), edited_shape(edited).is_none())),
    };
    drop(tests);
    (type_registry.freeze().expect("frozen"), test_types)
}

/// Entities of the types `frozen_types` holding the superchunk `(10, 10)`, none in it.
fn world(frozen_types: &FrozenTypeRegistry) -> WorldEntities {
    let mut world_entities = WorldEntities::for_types(frozen_types, 0);
    assert_eq!(world_entities.keep_superchunks(&[SuperchunkIndex::from_cartesian(10, 10)]), 0);
    world_entities
}

/// The cell `(x, y)` cells from the top left of the superchunk `(10, 10)`.
fn cell(x: u32, y: u32) -> CellIndex {
    CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + x, y: 10 * SUPERCHUNK_SIDE_CELLS + y }.into()
}

/// A walker on `stood_cell`, never waking.
fn walker_header(types: TestTypes, stood_cell: CellIndex) -> Header {
    Header { entity_type: types.walker, stood_cell, wake: NEVER_WAKES }
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
    const MEMBERS: &'static [server::internals_for_root_tests::Member] = &[server::internals_for_root_tests::Member::word(0), server::internals_for_root_tests::Member::word(14)];

    fn write(self, attribute_data: &mut [u64]) {
        (attribute_data[0], attribute_data[14]) = (self.first_word, self.last_word);
    }

    fn read_data(attribute_data: &[u64]) -> Self {
        Self { first_word: attribute_data[0], last_word: attribute_data[14] }
    }
}

/// An attribute of type `attribute_type`, whose size varies, holding `attribute_data`: its
/// block length in its type's last four bits, its data from the
/// second word.
fn varying_attribute(attribute_type: AttributeType, attribute_data: &[u64]) -> Vec<AttributeBlock> {
    let block_count = (1 + attribute_data.len()).div_ceil(BLOCK_WORDS);
    let mut words = vec![0; block_count * BLOCK_WORDS];
    words[0] = attribute_type.with_blocks(block_count).header_word();
    words[1..1 + attribute_data.len()].copy_from_slice(attribute_data);
    words.as_chunks::<BLOCK_WORDS>().0.iter().map(|&block_words| AttributeBlock(block_words)).collect()
}

/// The attributes an entity of the tests starts with, in type order:
/// one of a block, one of eight, one five by its block length and one
/// two by it.
fn attributes(types: TestTypes) -> Vec<AttributeBlock> {
    let mut each_attribute = [WholeAttribute::holding_value(types.one_block, 7).to_vec(), WholeAttribute::holding_value(types.both_ends, BothEnds { first_word: 11, last_word: 13 }).to_vec(), varying_attribute(types.varying_note, &[5; 9]), varying_attribute(types.varying_tail, &[3; 3])];
    each_attribute.sort_unstable_by_key(|attribute_blocks| attribute_blocks[0].attribute_type().type_identity());
    each_attribute.concat()
}

/// Where each of `held`, types and their blocks, lies in an entity's
/// attributes: one after another in type order.
fn ranges_in_type_order(held: &[(AttributeType, usize)]) -> Vec<(AttributeType, std::ops::Range<usize>)> {
    let mut in_type_order = held.to_vec();
    in_type_order.sort_unstable_by_key(|(attribute_type, _)| attribute_type.type_identity());
    let mut start = 0;
    in_type_order.into_iter().map(|(attribute_type, block_count)| (attribute_type, start..{ start += block_count; start })).collect()
}

/// A type's blocks are in its ID, and an entity's attributes are found
/// by stepping over each: one block, two, as many as a block length
/// says.
#[test]
fn every_attribute_is_found_over_those_of_any_length() {
    let (frozen_types, types) = registered_types(crate::test_seed::TestSeedHandle::open().fine_seed());
    assert_eq!((types.one_block.attribute_type().block_count(), types.both_ends.attribute_type().block_count(), types.varying_note.header_word() & 0xF), (1, 8, 0));
    let attribute_list = attributes(types);
    assert_eq!(attribute_list.len(), 1 + 8 + 5 + 2);
    assert!(is_attribute_list(&attribute_list));
    let held_ranges = ranges_in_type_order(&[(types.one_block.attribute_type(), 1), (types.both_ends.attribute_type(), 8), (types.varying_note, 5), (types.varying_tail, 2)]);
    for (attribute_type, held_range) in &held_ranges {
        assert_eq!(find_attribute(&attribute_list, *attribute_type), Ok(held_range.clone()), "{attribute_type:?}");
    }
    // One not held: where it would go, after those of lower IDs.
    let blocks_before = held_ranges.iter().filter(|(attribute_type, _)| attribute_type.type_identity() < types.never_held.type_identity()).map(|(_, held_range)| held_range.len()).sum();
    assert_eq!(find_attribute(&attribute_list, types.never_held), Err(blocks_before), "by its ID among them");
    let both_ends_range = find_attribute(&attribute_list, types.both_ends.attribute_type()).expect("held");
    assert!(!is_attribute_list(&attribute_list[..both_ends_range.start + 4]), "an attribute cut short");
    let reversed: Vec<AttributeBlock> = held_ranges.iter().rev().flat_map(|(_, held_range)| attribute_list[held_range.clone()].iter().copied()).collect();
    assert!(!is_attribute_list(&reversed), "out of order");
    assert!(!is_attribute_list(&[AttributeBlock([types.walker.0, 0])]), "an entity type is no attribute's");

    let mut world_entities = world(&frozen_types);
    let header = walker_header(types, cell(40, 30));
    world_entities.queue_add(header, &attribute_list);
    world_entities.apply();
    let entity = world_entities.get(header.stood_cell).expect("put");
    assert_eq!((entity.attribute(types.one_block), entity.attribute(types.both_ends)), (Some(7), Some(BothEnds { first_word: 11, last_word: 13 })));
    assert_eq!(entity.attribute_blocks(types.varying_note).map(<[_]>::len), Some(5));
    assert_eq!(entity.attribute_blocks(types.varying_tail).expect("after the note")[1].0[0], 3);
}

/// An attribute is edited to another length -- longer, shorter, as
/// long, gone -- and those about it are as they were; the entity is
/// saved and read back the same, and a file whose block length is
/// wrong is not read.
#[test]
fn an_attribute_is_edited_to_any_length_and_saved() {
    let (frozen_types, types) = registered_types(crate::test_seed::TestSeedHandle::open().fine_seed());
    let mut world_entities = world(&frozen_types);
    let target_header = walker_header(types, cell(40, 30));
    world_entities.queue_add(target_header, &attributes(types));
    world_entities.apply();
    let edits: [&dyn Fn(&mut Instructions); 4] = [
        &|instruction_queue| instruction_queue.set_attribute_blocks(target_header.stood_cell, &varying_attribute(types.varying_note, &[6; 21])),
        &|instruction_queue| instruction_queue.set_attribute_blocks(target_header.stood_cell, &varying_attribute(types.varying_note, &[8; 3])),
        &|instruction_queue| instruction_queue.set_attribute(target_header.stood_cell, types.both_ends, BothEnds { first_word: 17, last_word: 19 }),
        &|instruction_queue| instruction_queue.unset_attribute(target_header.stood_cell, types.one_block.attribute_type()),
    ];
    for (tick, edit) in edits.into_iter().enumerate() {
        let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
        edit(&mut instruction_queue);
        instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
        assert_eq!(instructions_applied.edits, 1, "edit {tick}");
        let edited_entity = world_entities.get(target_header.stood_cell).expect("where it stood");
        assert!(is_attribute_list(edited_entity.attributes), "edit {tick}");
        let note_blocks = edited_entity.attribute_blocks(types.varying_note).expect("the note");
        assert_eq!((note_blocks.len(), note_blocks[1].0[0]), if tick == 0 { (11, 6) } else { (2, 8) }, "edit {tick}");
        assert_eq!(edited_entity.attribute(types.both_ends), Some(if tick < 2 { BothEnds { first_word: 11, last_word: 13 } } else { BothEnds { first_word: 17, last_word: 19 } }), "edit {tick}");
        assert_eq!(edited_entity.attribute(types.one_block), (tick < 3).then_some(7), "edit {tick}");
        assert_eq!(edited_entity.attribute_blocks(types.varying_tail).map(|tail_blocks| tail_blocks[1].0[0]), Some(3), "edit {tick}");
    }

    let superchunk = target_header.stood_cell.superchunk();
    let (words, count) = encode_state(None, world_entities.superchunk(superchunk));
    assert_eq!(count, 1);
    let mut read_back = world(&frozen_types);
    assert_eq!(decode_state(&words, 4, &mut read_back).map(|saved_state| saved_state.entity_count), Ok(1));
    read_back.apply();
    let (saved_entity, read_entity) = (world_entities.get(target_header.stood_cell).expect("there"), read_back.get(target_header.stood_cell).expect("read back"));
    assert_eq!((saved_entity.header, saved_entity.attributes), (read_entity.header, read_entity.attributes));

    // The note's block length said to be more than the entity has blocks: nothing is read.
    let mut wrong_words = words.clone();
    let note_type_index = wrong_words.iter().position(|&word| AttributeType::of_header_word(word).type_identity() == types.varying_note.type_identity()).expect("the note's type");
    wrong_words[note_type_index] = types.varying_note.with_blocks(16).header_word();
    assert!(decode_state(&wrong_words, 4, &mut world(&frozen_types)).is_err());
    assert!(decode_state(&words[..words.len() - 1], 4, &mut world(&frozen_types)).is_err(), "cut short");
}

/// Attributes of types drawn -- one block, two, three, and of sizes
/// that vary -- set, set again to another length and unset in an order
/// drawn: after every edit the entity holds exactly those set and not
/// unset since, each as last set, in the order of their types; an
/// unset of one it has not changes nothing; and saved, it is read back
/// the same.
#[test]
fn attributes_edited_in_any_order_are_those_last_set() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    use std::collections::BTreeMap;
    let (frozen_types, types) = registered_types(seed);
    let mut seeded_rng = RandomSource::new(seed);
    let mut world_entities = world(&frozen_types);
    let target_header = walker_header(types, cell(seeded_rng.draw_below(1024) as u32, seeded_rng.draw_below(1024) as u32));
    world_entities.queue_add(target_header, &[]);
    world_entities.apply();
    // Twelve types: eight of 1 to 3 blocks and four whose size varies.
    let attribute_types = types.edited;
    let mut held_attributes: BTreeMap<u64, Vec<AttributeBlock>> = BTreeMap::new();
    for edit in 0..400 {
        let (attribute_type, size_varies) = attribute_types[seeded_rng.draw_below(attribute_types.len() as u64) as usize];
        let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
        if seeded_rng.draw_below(3) == 0 {
            instruction_queue.unset_attribute(target_header.stood_cell, attribute_type);
            held_attributes.remove(&attribute_type.header_word());
        } else {
            let set_blocks = match (!size_varies).then_some(attribute_type.block_count()) {
                Some(block_count) => {
                    let mut words: Vec<u64> = (0..block_count * BLOCK_WORDS).map(|_| seeded_rng.next_draw()).collect();
                    words[0] = attribute_type.header_word();
                    words.as_chunks::<BLOCK_WORDS>().0.iter().map(|&block_words| AttributeBlock(block_words)).collect()
                }
                None => varying_attribute(attribute_type, &(0..1 + seeded_rng.draw_below(28)).map(|_| seeded_rng.next_draw()).collect::<Vec<_>>()),
            };
            instruction_queue.set_attribute_blocks(target_header.stood_cell, &set_blocks);
            held_attributes.insert(attribute_type.header_word(), set_blocks);
        }
        instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
        assert_eq!((instructions_applied.edits, instructions_applied.passed_over), (1, 0), "edit {edit}");
        let edited_entity = world_entities.get(target_header.stood_cell).expect("where it stood");
        assert!(is_attribute_list(edited_entity.attributes), "edit {edit}");
        let expected_blocks: Vec<AttributeBlock> = held_attributes.values().flatten().copied().collect();
        assert!(edited_entity.attributes == expected_blocks.as_slice(), "edit {edit}: {} blocks held, {} expected", edited_entity.attributes.len(), expected_blocks.len());
        for &(attribute_type, _) in &attribute_types {
            assert_eq!(edited_entity.attribute_blocks(attribute_type), held_attributes.get(&attribute_type.header_word()).map(Vec::as_slice), "edit {edit}: {attribute_type:?}");
        }
    }
    let (words, count) = encode_state(None, world_entities.superchunk(target_header.stood_cell.superchunk()));
    let mut read_back = world(&frozen_types);
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
    let (frozen_types, types) = registered_types(crate::test_seed::TestSeedHandle::open().fine_seed());
    let mut world_entities = world(&frozen_types);
    let (first_cell, second_cell, third_cell) = (cell(40, 30), cell(41, 30), cell(42, 30));
    world_entities.queue_add(walker_header(types, first_cell), &[]);
    world_entities.queue_add(walker_header(types, second_cell), &[]);
    world_entities.apply();
    let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
    instruction_queue.move_entity(walker_header(types, third_cell), first_cell);
    instruction_queue.set_attribute(first_cell, types.one_block, 7);
    instruction_queue.move_entity(walker_header(types, first_cell), second_cell);
    instruction_queue.add(walker_header(types, first_cell), &[]);
    instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
    assert_eq!((instructions_applied.moves, instructions_applied.edits, instructions_applied.turned_back, instructions_applied.adds_refused, instructions_applied.passed_over), (2, 1, 1, 1, 0));
    assert_eq!(world_entities.get(third_cell).and_then(|moved_entity| moved_entity.attribute(types.one_block)), Some(7), "written where it moved to");
    assert!(world_entities.get(second_cell).is_some() && world_entities.len() == 2, "the cell left is no other's this tick");
    // Still the same tick: removed by the name it has in it, and nothing written to it after.
    let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
    instruction_queue.remove(first_cell);
    instruction_queue.set_attribute(first_cell, types.one_block, 8);
    instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
    assert_eq!((instructions_applied.entities_removed, instructions_applied.passed_over, world_entities.len()), (1, 1, 1));
    // The tick over, the cells are whoever comes to them.
    world_entities.superchunks_mut().iter_mut().for_each(|superchunk| superchunk.names_anew());
    let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
    instruction_queue.add(walker_header(types, first_cell), &[]);
    instruction_queue.add(walker_header(types, third_cell), &[]);
    instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
    assert_eq!((instructions_applied.added, world_entities.len()), (2, 3));
    world_entities.superchunks_mut().iter_mut().for_each(|superchunk| superchunk.names_anew());

    let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
    let nobodys_cell = cell(50, 50);
    instruction_queue.remove(nobodys_cell);
    instruction_queue.unset_attribute(nobodys_cell, types.one_block.attribute_type());
    instruction_queue.move_entity(walker_header(types, second_cell), nobodys_cell);
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
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    use basegame::base_types::SheepState;
    let (frozen_types, types) = registered_types(seed);
    // An attribute sorting before the sheep's state, and one after: what the test is about, so said.
    assert!(types.sorts_before_state.attribute_type().type_identity() < types.base.sheep_state.attribute_type().type_identity(), "registered before the base game");
    assert!(types.one_block.attribute_type().type_identity() > types.base.sheep_state.attribute_type().type_identity(), "registered after the base game");
    let default_attributes = DefaultAttributes::of_types(&frozen_types);
    let kept_default = default_attributes.of(types.base.sheep).expect("registered with one");
    assert_eq!((kept_default.attribute_type, kept_default.words, kept_default.block_count), (types.base.sheep_state.attribute_type(), SheepState::WORDS, SheepState::WORDS.div_ceil(BLOCK_WORDS)));
    assert_eq!(default_attributes.of(types.walker), None);
    let mut seeded_rng = RandomSource::new(seed);
    let draw_state = |seeded_rng: &mut RandomSource| SheepState { hungry_at: seeded_rng.next_draw(), lamb_due_at: seeded_rng.next_draw(), grown_at: seeded_rng.next_draw(), roaming_until: seeded_rng.next_draw(), roaming_neighbour: seeded_rng.next_draw() as u32 };
    for round_number in 0..200 {
        let mut world_entities = world(&frozen_types);
        let sheep_cell = cell(seeded_rng.draw_below(200) as u32, seeded_rng.draw_below(200) as u32);
        let sheep_header = Header { entity_type: types.base.sheep, stood_cell: sheep_cell, wake: NEVER_WAKES };
        // Given its state or not, with an attribute before it by type, one after, both or neither.
        let (drawn_state, state_given, has_before, has_after) = (draw_state(&mut seeded_rng), seeded_rng.draw_below(2) == 0, seeded_rng.draw_below(2) == 0, seeded_rng.draw_below(2) == 0);
        let mut attributes = Vec::new();
        if has_before {
            push_attribute(&mut attributes, types.sorts_before_state, 5);
        }
        if state_given {
            push_attribute(&mut attributes, types.base.sheep_state, drawn_state);
        }
        if has_after {
            push_attribute(&mut attributes, types.one_block, 7);
        }
        assert!(may_be_given(default_attributes.of(types.base.sheep), &attributes));
        world_entities.queue_add(sheep_header, &attributes);
        world_entities.apply();
        let mut held_state = if state_given { drawn_state } else { SheepState::default() };
        let added_entity = world_entities.get(sheep_cell).expect("put");
        // Kept: its data's blocks, then the others with their types -- a block each here.
        assert_eq!(added_entity.attribute_list().len(), kept_default.block_count + usize::from(has_before) + usize::from(has_after), "round {round_number}");
        assert_eq!(added_entity.attributes.len(), usize::from(has_before) + usize::from(has_after));
        // Its first word is its data's first, no type before it.
        assert_eq!(added_entity.attribute_list()[0].0[0], held_state.hungry_at, "round {round_number}");
        assert_eq!((added_entity.attribute(types.base.sheep_state), added_entity.attribute(types.sorts_before_state), added_entity.attribute(types.one_block)), (Some(held_state), has_before.then_some(5), has_after.then_some(7)), "round {round_number}");
        assert_eq!(added_entity.default_attribute().as_deref(), Some(&*WholeAttribute::holding_value(types.base.sheep_state, held_state)));
        assert_eq!(added_entity.attribute_seen(types.base.sheep_state.attribute_type()), server::internals_for_root_tests::attribute_seen(Some(&WholeAttribute::holding_value(types.base.sheep_state, held_state))));
        // Edits drawn: the state whole, a member of it, its removal -- refused -- and the others', which leave it be.
        for edit in 0..8 {
            let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
            let now_at = world_entities.iter().next().expect("the one sheep").header.stood_cell;
            let chosen_edit = seeded_rng.draw_below(5);
            match chosen_edit {
                0 => {
                    held_state = draw_state(&mut seeded_rng);
                    instruction_queue.set_attribute(now_at, types.base.sheep_state, held_state);
                }
                1 => {
                    let (member, value) = (seeded_rng.draw_below(SheepState::MEMBERS.len() as u64) as usize, seeded_rng.next_draw());
                    let member = SheepState::MEMBERS[member];
                    // Where the member lies in the attribute, its type counted: as an instruction says it.
                    let superchunk = world_entities.superchunks_mut().iter_mut().next().expect("the one");
                    assert!(superchunk.write_attribute_bytes_named(now_at, types.base.sheep_state.attribute_type(), usize::from(member.byte_offset), usize::from(member.byte_count), value));
                    let mut words = [0; SheepState::WORDS];
                    held_state.write(&mut words);
                    let (word, first_bit, mask) = (usize::from(member.byte_offset) / 8 - 1, usize::from(member.byte_offset) % 8 * 8, u64::MAX >> (64 - u32::from(member.byte_count) * 8));
                    words[word] = words[word] & !(mask << first_bit) | (value & mask) << first_bit;
                    held_state = SheepState::read_data(&words);
                    assert_eq!(world_entities.get(now_at).expect("there").attribute_member(types.base.sheep_state.attribute_type(), usize::from(member.byte_offset), usize::from(member.byte_count)), Some(value & mask));
                }
                2 => instruction_queue.unset_attribute(now_at, types.base.sheep_state.attribute_type()),
                3 => instruction_queue.set_attribute(now_at, types.one_block, 100 + edit),
                // To a cell of another chunk as often as not: all it has goes with it.
                _ => instruction_queue.move_entity(Header { stood_cell: cell(seeded_rng.draw_below(600) as u32, seeded_rng.draw_below(600) as u32), ..sheep_header }, now_at),
            }
            instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
            // The tick over: it is named by where it stands now.
            world_entities.superchunks_mut().iter_mut().for_each(|superchunk| superchunk.names_anew());
            // Its removal alone is passed over: a default attribute is never taken.
            assert_eq!(instructions_applied.passed_over, usize::from(chosen_edit == 2), "round {round_number}, edit {edit}");
            let edited_entity = world_entities.iter().next().expect("the one sheep");
            assert_eq!(edited_entity.attribute(types.base.sheep_state), Some(held_state), "round {round_number}, edit {edit}");
            assert!(is_attribute_list(edited_entity.attributes) && edited_entity.attribute_list().len() == kept_default.block_count + edited_entity.attributes.len(), "round {round_number}, edit {edit}");
            assert_eq!(edited_entity.attribute(types.sorts_before_state), has_before.then_some(5), "round {round_number}, edit {edit}");
        }
        // Saved and read back: the same sheep, kept the same.
        let saved_entity = world_entities.iter().next().expect("the one sheep");
        let (words, count) = encode_state(None, world_entities.superchunk(saved_entity.header.stood_cell.superchunk()));
        let mut read_back = world(&frozen_types);
        assert_eq!((count, decode_state(&words, 0, &mut read_back).map(|saved_state| saved_state.entity_count)), (1, Ok(1)));
        read_back.apply();
        let read_entity = read_back.get(saved_entity.header.stood_cell).expect("read back");
        assert_eq!((saved_entity.header, saved_entity.attribute_list()), (read_entity.header, read_entity.attribute_list()), "round {round_number}");
    }
}
