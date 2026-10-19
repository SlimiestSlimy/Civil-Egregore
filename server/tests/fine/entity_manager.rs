//! The entity manager: attributes of more blocks than one, and of a size
//! that varies -- put, found by a walk that steps over each as far as
//! its type or its block length says, edited to another length, saved
//! and read back (`docs/entity_manager.md`, "Attributes, a block each");
//! an entity named by its cell all a tick long ([`named_by_its_cell`]);
//! and a type's default attribute ([`default_attributes`]).

mod default_attributes;
mod edited_in_any_order;
mod named_by_its_cell;

use crate::entity_manager::{find_attribute, is_attribute_list, Attribute, AttributeBlock, AttributeType, EntityType, Header, Instructions, InstructionsApplied, Layout, Member, WholeAttribute, WorldEntities, BLOCK_WORDS, NEVER_WAKES};
use crate::entity_manager::saved::{decode_state, encode_state};
use protocol::type_registry::{FrozenTypeRegistry, RegisteredType, Registration, TypeRegistry};
use utilities::coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use utilities::rng::RandomSource;

/// Types registered before a test's own, at most: as many as its seed
/// draws, for their IDs to be other with every seed.
const MOST_FILLERS: u64 = 64;

/// What a grazer keeps, its default attribute: four words and a
/// number within a fifth -- a member of each kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct GrazerState {
    /// Four words.
    words: [u64; 4],
    /// A number of four bytes, in the fifth word.
    small: u32,
}

impl Layout for GrazerState {
    const WORDS: usize = 5;
    const MEMBERS: &'static [Member] = &[Member::word(0), Member::word(1), Member::word(2), Member::word(3), Member::within_word(4, 0, 4)];

    fn write(self, attribute_data: &mut [u64]) {
        attribute_data[..4].copy_from_slice(&self.words);
        attribute_data[4] = u64::from(self.small);
    }

    fn read_data(attribute_data: &[u64]) -> Self {
        Self { words: [attribute_data[0], attribute_data[1], attribute_data[2], attribute_data[3]], small: attribute_data[4] as u32 }
    }
}

/// The types the store is tried with, registered by each test in a
/// registry of its own ([`registered_types`]) -- never a number by hand.
#[derive(Clone, Copy)]
struct TestTypes {
    /// A type of entity registered with a default attribute.
    grazer: EntityType,
    /// Its default attribute.
    grazer_state: Attribute<GrazerState>,
    /// An attribute registered before the grazer's state: of an ID below
    /// its.
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
/// as it draws, the attribute sorting before the state, the grazer and
/// its state, then the rest in an order drawn -- no test counting on
/// which of them is of the lower ID.
fn registered_types(seed: u64) -> (FrozenTypeRegistry, TestTypes) {
    let (mut type_registry, mut seeded_random) = (TypeRegistry::for_new_world(), RandomSource::new(seed ^ 0x7e57_7e57));
    let registered_alone = |type_registry: &mut TypeRegistry, registration: Registration| type_registry.register(registration).expect("a test's type, of a name no other has");
    // A bundle of its own before the grazer's, for its attribute to be of a lower ID.
    let sorts_before_state = {
        let mut before_the_grazer = type_registry.open_bundle("tests before").expect("a bundle of its own");
        for filler in 0..seeded_random.draw_below(MOST_FILLERS + 1) {
            before_the_grazer.register(Registration::layer(&format!("filler {filler}"))).expect("a filler of its own name");
        }
        Attribute::registered(&registered_alone(&mut before_the_grazer, Attribute::<u64>::registration("sorts before state"))).expect("a word's attribute")
    };
    let mut tests = type_registry.open_bundle("tests").expect("the tests' bundle");
    let grazer_state = Attribute::registered(&registered_alone(&mut tests, Attribute::<GrazerState>::registration("grazer state"))).expect("laid out as a grazer's state");
    let grazer = registered_alone(&mut tests, Registration::entity("grazer", Some("grazer state"))).entity_type().expect("a type of entity");
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
        grazer,
        grazer_state,
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
    const MEMBERS: &'static [Member] = &[Member::word(0), Member::word(14)];

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
