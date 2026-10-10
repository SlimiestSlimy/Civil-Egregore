//! Attributes of more blocks than one, and of a size that varies: put,
//! found by a walk that steps over each as far as its type or its
//! block length says, edited to another length, saved and read back
//! (`entity_manager/docs/entity_manager.md`, "Attributes, a block each").
//!
//! `cargo test`

use crate::tests::{cell, walker, world, WALKER};
use coordinates::CellIndex;
use entity_manager::saved::{decode_state, encode_state};
use entity_manager::{find_attribute, push_attribute, sorted, Attribute, AttributeBlock, AttributeType, Layout, BLOCK_WORDS, NEVER};
use simulation::{Simulation, Turn};

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

    let (_, mut entities) = world(1);
    let header = walker(1, cell(40, 30), NEVER);
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
    let (mut arena, mut entities) = world(1);
    let target = walker(9, cell(40, 30), NEVER);
    entities.queue_put(target, &attributes());
    entities.queue_put(walker(1, cell(50, 30), 0), &[]);
    entities.apply();
    let mut simulation = Simulation::new(1);
    let edits: [&(dyn Fn(&mut Turn) + Sync); 4] = [
        &|turn| turn.set_attribute_blocks(&target, &varying(NOTE, &[6; 40])),
        &|turn| turn.set_attribute_blocks(&target, &varying(NOTE, &[8; 2])),
        &|turn| turn.set_attribute(&target, ENDS, BothEnds { first: 17, last: 19 }),
        &|turn| turn.unset_attribute(&target, NAME.attribute_type()),
    ];
    for (tick, edit) in edits.into_iter().enumerate() {
        let report = simulation.tick(&mut arena, &mut entities, tick as u64, |turn: &mut Turn, _: &mut Vec<CellIndex>| {
            for entity in turn.woken() {
                edit(turn);
                turn.step(&entity.header, entity.header.at, turn.now() + 1);
            }
            0
        });
        assert_eq!(report.instructions_applied.edits, 1, "tick {tick}");
        let edited = entities.get(target.id, target.at).expect("where it stood");
        assert!(sorted(edited.attributes), "tick {tick}");
        let note = edited.attribute_blocks(NOTE).expect("the note");
        assert_eq!((note.len(), note[0].0[2]), if tick == 0 { (6, 6) } else { (1, 8) }, "tick {tick}");
        assert_eq!(edited.attribute(ENDS), Some(if tick < 2 { BothEnds { first: 11, last: 13 } } else { BothEnds { first: 17, last: 19 } }), "tick {tick}");
        assert_eq!(edited.attribute(NAME), (tick < 3).then_some(7), "tick {tick}");
        assert_eq!(edited.attribute_blocks(TAIL).map(|tail| tail[0].0[2]), Some(3), "tick {tick}");
    }

    let superchunk = target.at.superchunk();
    let (words, count) = encode_state(None, entities.superchunk(superchunk));
    assert_eq!(count, 2);
    let (_, mut read_back) = world(1);
    assert_eq!(decode_state(&words, 4, &mut read_back).map(|state| state.entities), Ok(2));
    read_back.apply();
    let (was, is) = (entities.get(target.id, target.at).expect("there"), read_back.get(target.id, target.at).expect("read back"));
    assert_eq!((was.header, was.attributes), (is.header, is.attributes));

    // The note's block length said to be more than the entity has blocks: nothing is read.
    let mut wrong = words.clone();
    let length = wrong.iter().position(|&word| word == NOTE.0).expect("the note's type") + 1;
    wrong[length] = 40;
    assert!(decode_state(&wrong, 4, &mut world(1).1).is_err());
    assert!(decode_state(&words[..words.len() - 1], 4, &mut world(1).1).is_err(), "cut short");
}
