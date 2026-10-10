//! The fine tier: attributes of more blocks than one, and of a size
//! that varies -- put, found by a walk that steps over each as far as
//! its type or its block length says, edited to another length, saved
//! and read back (`docs/entity_manager.md`, "Attributes, a block each").
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::saved::{decode_state, encode_state};
use entity_manager::{find_attribute, push_attribute, sorted, Attribute, AttributeBlock, AttributeType, Entities, EntityId, EntityType, Header, Instructions, InstructionsApplied, Layout, BLOCK_WORDS, NEVER};

/// The entities' type.
const WALKER: EntityType = EntityType(40);

/// Entities holding the superchunk `(10, 10)`, none in it.
fn world() -> Entities {
    let mut entities = Entities::new();
    assert_eq!(entities.align(&[SuperchunkIndex::from_cartesian(10, 10)]), 0);
    entities
}

/// The cell `(x, y)` cells from the top left of the superchunk `(10, 10)`.
fn cell(x: u32, y: u32) -> CellIndex {
    CellCartesian { x: 10 * SUPERCHUNK_SIDE_CELLS + x, y: 10 * SUPERCHUNK_SIDE_CELLS + y }.into()
}

/// A walker with ID `id` on `at`, never waking.
fn walker(id: u64, at: CellIndex) -> Header {
    Header { id: EntityId(id), kind: WALKER, at, wake: NEVER }
}

/// A layout of two blocks: a number in its data's first word and one
/// in its last, the second block's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BothEnds {
    /// The first word's.
    first: u64,
    /// The last word's.
    last: u64,
}

impl Layout for BothEnds {
    const BLOCKS: usize = 2;

    fn write(self, data: &mut [u64]) {
        (data[0], data[14]) = (self.first, self.last);
    }

    fn read(data: &[u64]) -> Self {
        Self { first: data[0], last: data[14] }
    }
}

/// An attribute of one block: its number the highest, its ID the lowest.
const NAME: Attribute<u64> = Attribute::new(49);
/// One of two blocks.
const ENDS: Attribute<BothEnds> = Attribute::new(41);
/// One whose size varies.
const NOTE: AttributeType = AttributeType::of_varying_size(42);
/// Another, after it.
const TAIL: AttributeType = AttributeType::of_varying_size(43);

/// An attribute of type `kind`, whose size varies, holding `data`: its
/// block length in its second word, its data from the third.
fn varying(kind: AttributeType, data: &[u64]) -> Vec<AttributeBlock> {
    let blocks = (2 + data.len()).div_ceil(BLOCK_WORDS);
    let mut words = vec![0; blocks * BLOCK_WORDS];
    (words[0], words[1]) = (kind.0, blocks as u64);
    words[2..2 + data.len()].copy_from_slice(data);
    words.as_chunks::<BLOCK_WORDS>().0.iter().map(|&block| AttributeBlock(block)).collect()
}

/// The attributes an entity of the tests starts with: one block, two,
/// three by its block length, and one after.
fn attributes() -> Vec<AttributeBlock> {
    let mut blocks = vec![AttributeBlock::holding(NAME, 7)];
    push_attribute(&mut blocks, ENDS, BothEnds { first: 11, last: 13 });
    blocks.extend(varying(NOTE, &[5; 20]));
    blocks.extend(varying(TAIL, &[3]));
    blocks
}

/// A type's blocks are in its ID, and an entity's attributes are found
/// by stepping over each: one block, two, as many as a block length
/// says.
#[test]
fn every_attribute_is_found_over_those_of_any_length() {
    assert_eq!((NAME.attribute_type().blocks(), ENDS.attribute_type().blocks(), NOTE.blocks()), (Some(1), Some(2), None));
    assert_eq!((ENDS.attribute_type().number(), NOTE.number()), (41, 42));
    let blocks = attributes();
    assert_eq!(blocks.len(), 1 + 2 + 3 + 1);
    assert!(sorted(&blocks));
    assert_eq!(find_attribute(&blocks, NAME.attribute_type()), Ok(0..1));
    assert_eq!(find_attribute(&blocks, ENDS.attribute_type()), Ok(1..3));
    assert_eq!(find_attribute(&blocks, NOTE), Ok(3..6));
    assert_eq!(find_attribute(&blocks, TAIL), Ok(6..7));
    assert_eq!(find_attribute(&blocks, Attribute::<u64>::new(50).attribute_type()), Err(1), "between the one-block types and the two-block");
    assert!(!sorted(&blocks[..5]), "an attribute cut short");
    assert!(!sorted(&[blocks[6], blocks[0]]), "out of order");
    assert!(!sorted(&[AttributeBlock([WALKER.0, 0, 0, 0, 0, 0, 0, 0])]), "a type whose top byte is 0 is no attribute's");

    let mut entities = world();
    let header = walker(1, cell(40, 30));
    entities.queue_put(header, &blocks);
    entities.apply();
    let entity = entities.get(header.id, header.at).expect("put");
    assert_eq!((entity.attribute(NAME), entity.attribute(ENDS)), (Some(7), Some(BothEnds { first: 11, last: 13 })));
    assert_eq!(entity.attribute_blocks(NOTE).map(<[_]>::len), Some(3));
    assert_eq!(entity.attribute_blocks(TAIL).expect("after the note")[0].0[2], 3);
}

/// An attribute is edited to another length -- longer, shorter, as
/// long, gone -- and those about it are as they were; the entity is
/// saved and read back the same, and a file whose block length is
/// wrong is not read.
#[test]
fn an_attribute_is_edited_to_any_length_and_saved() {
    let mut entities = world();
    let target = walker(9, cell(40, 30));
    entities.queue_put(target, &attributes());
    entities.apply();
    let edits: [&dyn Fn(&mut Instructions); 4] = [
        &|queue| queue.set_attribute_blocks(target.id, target.at, &varying(NOTE, &[6; 40])),
        &|queue| queue.set_attribute_blocks(target.id, target.at, &varying(NOTE, &[8; 2])),
        &|queue| queue.set_attribute(target.id, target.at, ENDS, BothEnds { first: 17, last: 19 }),
        &|queue| queue.unset_attribute(target.id, target.at, NAME.attribute_type()),
    ];
    for (tick, edit) in edits.into_iter().enumerate() {
        let (mut queue, mut applied) = (Instructions::default(), InstructionsApplied::default());
        edit(&mut queue);
        queue.apply(entities.superchunks_mut(), 0, &mut applied);
        assert_eq!(applied.edits, 1, "edit {tick}");
        let edited = entities.get(target.id, target.at).expect("where it stood");
        assert!(sorted(edited.attributes), "edit {tick}");
        let note = edited.attribute_blocks(NOTE).expect("the note");
        assert_eq!((note.len(), note[0].0[2]), if tick == 0 { (6, 6) } else { (1, 8) }, "edit {tick}");
        assert_eq!(edited.attribute(ENDS), Some(if tick < 2 { BothEnds { first: 11, last: 13 } } else { BothEnds { first: 17, last: 19 } }), "edit {tick}");
        assert_eq!(edited.attribute(NAME), (tick < 3).then_some(7), "edit {tick}");
        assert_eq!(edited.attribute_blocks(TAIL).map(|tail| tail[0].0[2]), Some(3), "edit {tick}");
    }

    let superchunk = target.at.superchunk();
    let (words, count) = encode_state(None, entities.superchunk(superchunk));
    assert_eq!(count, 1);
    let mut read_back = world();
    assert_eq!(decode_state(&words, 4, &mut read_back).map(|state| state.entities), Ok(1));
    read_back.apply();
    let (was, is) = (entities.get(target.id, target.at).expect("there"), read_back.get(target.id, target.at).expect("read back"));
    assert_eq!((was.header, was.attributes), (is.header, is.attributes));

    // The note's block length said to be more than the entity has blocks: nothing is read.
    let mut wrong = words.clone();
    let length = wrong.iter().position(|&word| word == NOTE.0).expect("the note's type") + 1;
    wrong[length] = 40;
    assert!(decode_state(&wrong, 4, &mut world()).is_err());
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
    let mut random = utilities::rng::Rng::new(utilities::seed::counted());
    let mut entities = world();
    let target = walker(9, cell(random.below(1024) as u32, random.below(1024) as u32));
    entities.queue_put(target, &[]);
    entities.apply();
    // Twelve types: numbers drawn, no two the same, eight of 1 to 3 blocks and four whose size varies.
    let mut numbers = std::collections::BTreeSet::new();
    while numbers.len() < 12 {
        numbers.insert(1 + random.below(1 << 20));
    }
    let kinds: Vec<AttributeType> = numbers.into_iter().enumerate().map(|(nth, number)| if nth % 3 == 2 { AttributeType::of_varying_size(number) } else { AttributeType::of_blocks(number, 1 + nth % 4 % 3) }).collect();
    let mut held: BTreeMap<u64, Vec<AttributeBlock>> = BTreeMap::new();
    for edit in 0..400 {
        let kind = kinds[random.below(kinds.len() as u64) as usize];
        let (mut queue, mut applied) = (Instructions::default(), InstructionsApplied::default());
        if random.below(3) == 0 {
            queue.unset_attribute(target.id, target.at, kind);
            held.remove(&kind.0);
        } else {
            let blocks = match kind.blocks() {
                Some(blocks) => {
                    let mut words: Vec<u64> = (0..blocks * BLOCK_WORDS).map(|_| random.draw()).collect();
                    words[0] = kind.0;
                    words.as_chunks::<BLOCK_WORDS>().0.iter().map(|&block| AttributeBlock(block)).collect()
                }
                None => varying(kind, &(0..random.below(40)).map(|_| random.draw()).collect::<Vec<_>>()),
            };
            queue.set_attribute_blocks(target.id, target.at, &blocks);
            held.insert(kind.0, blocks);
        }
        queue.apply(entities.superchunks_mut(), 0, &mut applied);
        assert_eq!((applied.edits, applied.passed_over), (1, 0), "edit {edit}");
        let edited = entities.get(target.id, target.at).expect("where it stood");
        assert!(sorted(edited.attributes), "edit {edit}");
        let expected: Vec<AttributeBlock> = held.values().flatten().copied().collect();
        assert!(edited.attributes == expected.as_slice(), "edit {edit}: {} blocks held, {} expected", edited.attributes.len(), expected.len());
        for &kind in &kinds {
            assert_eq!(edited.attribute_blocks(kind), held.get(&kind.0).map(Vec::as_slice), "edit {edit}: {kind:?}");
        }
    }
    let (words, count) = encode_state(None, entities.superchunk(target.at.superchunk()));
    let mut read_back = world();
    assert_eq!((count, decode_state(&words, 4, &mut read_back).map(|state| state.entities)), (1, Ok(1)));
    read_back.apply();
    let (was, is) = (entities.get(target.id, target.at).expect("there"), read_back.get(target.id, target.at).expect("read back"));
    assert!((was.header, was.attributes) == (is.header, is.attributes));
}

/// A new entity put on the first free of some cells takes its own if
/// it is free, the first free of the others if not -- those of
/// another superchunk passed by -- and is refused only when every one
/// is taken. An instruction for an entity that is not there is passed
/// over, and counted.
#[test]
fn a_new_entity_is_put_on_the_first_free_cell() {
    let mut entities = world();
    let (wanted, second, third) = (cell(40, 30), cell(41, 30), cell(42, 30));
    // Past the superchunk's west edge: in another superchunk.
    let outside = cell(0, 30).offset(-1, 0).expect("in the world");
    let apply = |entities: &mut Entities, id: u64, others: &[CellIndex]| {
        let (mut queue, mut applied) = (Instructions::default(), InstructionsApplied::default());
        queue.put_on_the_first_free(walker(id, wanted), others, &[]);
        queue.apply(entities.superchunks_mut(), 0, &mut applied);
        (applied.puts, applied.beside, applied.refused)
    };
    assert_eq!(apply(&mut entities, 1, &[second, third]), (1, 0, 0));
    assert_eq!(apply(&mut entities, 2, &[outside, second, third]), (1, 1, 0));
    assert_eq!(apply(&mut entities, 3, &[second, third]), (1, 1, 0));
    assert_eq!(apply(&mut entities, 4, &[outside, second, third]), (0, 0, 1));
    for (id, at) in [(1, wanted), (2, second), (3, third)] {
        assert_eq!(entities.get(EntityId(id), at).map(|entity| entity.header.at), Some(at));
    }
    assert_eq!(entities.len(), 3);

    let (mut queue, mut applied) = (Instructions::default(), InstructionsApplied::default());
    queue.remove(EntityId(9), wanted);
    queue.unset_attribute(EntityId(1), second, NAME.attribute_type());
    queue.move_entity(walker(3, wanted), second);
    queue.apply(entities.superchunks_mut(), 0, &mut applied);
    assert_eq!((applied.passed_over, applied.removes, applied.edits, applied.moves, entities.len()), (3, 0, 0, 0, 3));
}
