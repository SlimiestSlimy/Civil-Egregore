//! The fine tier: attributes of more blocks than one, and of a size
//! that varies -- put, found by a walk that steps over each as far as
//! its type or its block length says, edited to another length, saved
//! and read back (`docs/entity_manager.md`, "Attributes, a block each").
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, SUPERCHUNK_SIDE_CELLS};
use entity_manager::saved::{decode_state, encode_state};
use entity_manager::{default_kept, may_be_given, WholeAttribute, find_attribute, push_attribute, sorted, Attribute, AttributeBlock, AttributeType, Entities, EntityType, Header, Instructions, InstructionsApplied, Layout, BLOCK_WORDS, NEVER};

/// The entities' type.
const WALKER: EntityType = EntityType::from_number(40);

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

/// A walker on `at`, never waking.
fn walker(at: CellIndex) -> Header {
    Header { kind: WALKER, at, wake: NEVER }
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
    const WORDS: usize = 15;
    const MEMBERS: &'static [entity_manager::Member] = &[entity_manager::Member::word(0), entity_manager::Member::word(14)];

    fn write(self, data: &mut [u64]) {
        (data[0], data[14]) = (self.first, self.last);
    }

    fn read(data: &[u64]) -> Self {
        Self { first: data[0], last: data[14] }
    }
}

/// An attribute of one block: its number the lowest, so the first of an entity's.
const NAME: Attribute<u64> = Attribute::new(39);
/// One of eight blocks.
const ENDS: Attribute<BothEnds> = Attribute::new(41);
/// One whose size varies.
const NOTE: AttributeType = AttributeType::of_varying_size(42);
/// Another, after it.
const TAIL: AttributeType = AttributeType::of_varying_size(43);

/// An attribute of type `kind`, whose size varies, holding `data`: its
/// block length in its type's last four bits, its data from the
/// second word.
fn varying(kind: AttributeType, data: &[u64]) -> Vec<AttributeBlock> {
    let blocks = (1 + data.len()).div_ceil(BLOCK_WORDS);
    let mut words = vec![0; blocks * BLOCK_WORDS];
    words[0] = kind.with_blocks(blocks).0;
    words[1..1 + data.len()].copy_from_slice(data);
    words.as_chunks::<BLOCK_WORDS>().0.iter().map(|&block| AttributeBlock(block)).collect()
}

/// The attributes an entity of the tests starts with: one block,
/// eight, five by its block length, and one of two after.
fn attributes() -> Vec<AttributeBlock> {
    let mut blocks = vec![AttributeBlock::holding(NAME, 7)];
    push_attribute(&mut blocks, ENDS, BothEnds { first: 11, last: 13 });
    blocks.extend(varying(NOTE, &[5; 9]));
    blocks.extend(varying(TAIL, &[3; 3]));
    blocks
}

/// A type's blocks are in its ID, and an entity's attributes are found
/// by stepping over each: one block, two, as many as a block length
/// says.
#[test]
fn every_attribute_is_found_over_those_of_any_length() {
    assert_eq!((NAME.attribute_type().blocks(), ENDS.attribute_type().blocks(), NOTE.0 & 0xF), (1, 8, 0));
    assert_eq!((ENDS.attribute_type().number(), NOTE.number()), (41, 42));
    let blocks = attributes();
    assert_eq!(blocks.len(), 1 + 8 + 5 + 2);
    assert!(sorted(&blocks));
    assert_eq!(find_attribute(&blocks, NAME.attribute_type()), Ok(0..1));
    assert_eq!(find_attribute(&blocks, ENDS.attribute_type()), Ok(1..9));
    assert_eq!(find_attribute(&blocks, NOTE), Ok(9..14));
    assert_eq!(find_attribute(&blocks, TAIL), Ok(14..16));
    assert_eq!(find_attribute(&blocks, Attribute::<u64>::new(40).attribute_type()), Err(1), "between the first and the second, by number");
    assert_eq!(find_attribute(&blocks, Attribute::<u64>::new(50).attribute_type()), Err(16), "after them all");
    assert!(!sorted(&blocks[..12]), "an attribute cut short");
    assert!(!sorted(&[blocks[14], blocks[15], blocks[0]]), "out of order");
    assert!(!sorted(&[AttributeBlock([WALKER.0, 0])]), "an entity type is no attribute's");

    let mut entities = world();
    let header = walker(cell(40, 30));
    entities.queue_put(header, &blocks);
    entities.apply();
    let entity = entities.get(header.at).expect("put");
    assert_eq!((entity.attribute(NAME), entity.attribute(ENDS)), (Some(7), Some(BothEnds { first: 11, last: 13 })));
    assert_eq!(entity.attribute_blocks(NOTE).map(<[_]>::len), Some(5));
    assert_eq!(entity.attribute_blocks(TAIL).expect("after the note")[1].0[0], 3);
}

/// An attribute is edited to another length -- longer, shorter, as
/// long, gone -- and those about it are as they were; the entity is
/// saved and read back the same, and a file whose block length is
/// wrong is not read.
#[test]
fn an_attribute_is_edited_to_any_length_and_saved() {
    let mut entities = world();
    let target = walker(cell(40, 30));
    entities.queue_put(target, &attributes());
    entities.apply();
    let edits: [&dyn Fn(&mut Instructions); 4] = [
        &|queue| queue.set_attribute_blocks(target.at, &varying(NOTE, &[6; 21])),
        &|queue| queue.set_attribute_blocks(target.at, &varying(NOTE, &[8; 3])),
        &|queue| queue.set_attribute(target.at, ENDS, BothEnds { first: 17, last: 19 }),
        &|queue| queue.unset_attribute(target.at, NAME.attribute_type()),
    ];
    for (tick, edit) in edits.into_iter().enumerate() {
        let (mut queue, mut applied) = (Instructions::default(), InstructionsApplied::default());
        edit(&mut queue);
        queue.apply(entities.superchunks_mut(), 0, &mut applied);
        assert_eq!(applied.edits, 1, "edit {tick}");
        let edited = entities.get(target.at).expect("where it stood");
        assert!(sorted(edited.attributes), "edit {tick}");
        let note = edited.attribute_blocks(NOTE).expect("the note");
        assert_eq!((note.len(), note[1].0[0]), if tick == 0 { (11, 6) } else { (2, 8) }, "edit {tick}");
        assert_eq!(edited.attribute(ENDS), Some(if tick < 2 { BothEnds { first: 11, last: 13 } } else { BothEnds { first: 17, last: 19 } }), "edit {tick}");
        assert_eq!(edited.attribute(NAME), (tick < 3).then_some(7), "edit {tick}");
        assert_eq!(edited.attribute_blocks(TAIL).map(|tail| tail[1].0[0]), Some(3), "edit {tick}");
    }

    let superchunk = target.at.superchunk();
    let (words, count) = encode_state(None, entities.superchunk(superchunk));
    assert_eq!(count, 1);
    let mut read_back = world();
    assert_eq!(decode_state(&words, 4, &mut read_back).map(|state| state.entities), Ok(1));
    read_back.apply();
    let (was, is) = (entities.get(target.at).expect("there"), read_back.get(target.at).expect("read back"));
    assert_eq!((was.header, was.attributes), (is.header, is.attributes));

    // The note's block length said to be more than the entity has blocks: nothing is read.
    let mut wrong = words.clone();
    let length = wrong.iter().position(|&word| AttributeType(word).identity() == NOTE.identity()).expect("the note's type");
    wrong[length] = NOTE.with_blocks(16).0;
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
    let target = walker(cell(random.below(1024) as u32, random.below(1024) as u32));
    entities.queue_put(target, &[]);
    entities.apply();
    // Twelve types: numbers drawn, no two the same, eight of 1 to 3 blocks and four whose size varies.
    let mut numbers = std::collections::BTreeSet::new();
    while numbers.len() < 12 {
        numbers.insert(1 + random.below(1 << 20));
    }
    // Each with whether its size varies: the type alone does not say, its registering does.
    let kinds: Vec<(AttributeType, bool)> = numbers.into_iter().enumerate().map(|(nth, number)| if nth % 3 == 2 { (AttributeType::of_varying_size(number), true) } else { (AttributeType::of_blocks(number, 1 + nth % 4 % 3), false) }).collect();
    let mut held: BTreeMap<u64, Vec<AttributeBlock>> = BTreeMap::new();
    for edit in 0..400 {
        let (kind, varies) = kinds[random.below(kinds.len() as u64) as usize];
        let (mut queue, mut applied) = (Instructions::default(), InstructionsApplied::default());
        if random.below(3) == 0 {
            queue.unset_attribute(target.at, kind);
            held.remove(&kind.0);
        } else {
            let blocks = match (!varies).then_some(kind.blocks()) {
                Some(blocks) => {
                    let mut words: Vec<u64> = (0..blocks * BLOCK_WORDS).map(|_| random.draw()).collect();
                    words[0] = kind.0;
                    words.as_chunks::<BLOCK_WORDS>().0.iter().map(|&block| AttributeBlock(block)).collect()
                }
                None => varying(kind, &(0..1 + random.below(28)).map(|_| random.draw()).collect::<Vec<_>>()),
            };
            queue.set_attribute_blocks(target.at, &blocks);
            held.insert(kind.0, blocks);
        }
        queue.apply(entities.superchunks_mut(), 0, &mut applied);
        assert_eq!((applied.edits, applied.passed_over), (1, 0), "edit {edit}");
        let edited = entities.get(target.at).expect("where it stood");
        assert!(sorted(edited.attributes), "edit {edit}");
        let expected: Vec<AttributeBlock> = held.values().flatten().copied().collect();
        assert!(edited.attributes == expected.as_slice(), "edit {edit}: {} blocks held, {} expected", edited.attributes.len(), expected.len());
        for &(kind, _) in &kinds {
            assert_eq!(edited.attribute_blocks(kind), held.get(&kind.0).map(Vec::as_slice), "edit {edit}: {kind:?}");
        }
    }
    let (words, count) = encode_state(None, entities.superchunk(target.at.superchunk()));
    let mut read_back = world();
    assert_eq!((count, decode_state(&words, 4, &mut read_back).map(|state| state.entities)), (1, Ok(1)));
    read_back.apply();
    let (was, is) = (entities.get(target.at).expect("there"), read_back.get(target.at).expect("read back"));
    assert!((was.header, was.attributes) == (is.header, is.attributes));
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
    let mut entities = world();
    let (wanted, second, third) = (cell(40, 30), cell(41, 30), cell(42, 30));
    entities.queue_put(walker(wanted), &[]);
    entities.queue_put(walker(second), &[]);
    entities.apply();
    let (mut queue, mut applied) = (Instructions::default(), InstructionsApplied::default());
    queue.move_entity(walker(third), wanted);
    queue.set_attribute(wanted, NAME, 7);
    queue.move_entity(walker(wanted), second);
    queue.put(walker(wanted), &[]);
    queue.apply(entities.superchunks_mut(), 0, &mut applied);
    assert_eq!((applied.moves, applied.edits, applied.stayed, applied.refused, applied.passed_over), (2, 1, 1, 1, 0));
    assert_eq!(entities.get(third).and_then(|moved| moved.attribute(NAME)), Some(7), "written where it moved to");
    assert!(entities.get(second).is_some() && entities.len() == 2, "the cell left is no other's this tick");
    // Still the same tick: removed by the name it has in it, and nothing written to it after.
    let (mut queue, mut applied) = (Instructions::default(), InstructionsApplied::default());
    queue.remove(wanted);
    queue.set_attribute(wanted, NAME, 8);
    queue.apply(entities.superchunks_mut(), 0, &mut applied);
    assert_eq!((applied.removes, applied.passed_over, entities.len()), (1, 1, 1));
    // The tick over, the cells are whoever comes to them.
    entities.superchunks_mut().iter_mut().for_each(|superchunk| superchunk.names_anew(&mut ()));
    let (mut queue, mut applied) = (Instructions::default(), InstructionsApplied::default());
    queue.put(walker(wanted), &[]);
    queue.put(walker(third), &[]);
    queue.apply(entities.superchunks_mut(), 0, &mut applied);
    assert_eq!((applied.puts, entities.len()), (2, 3));
    entities.superchunks_mut().iter_mut().for_each(|superchunk| superchunk.names_anew(&mut ()));

    let (mut queue, mut applied) = (Instructions::default(), InstructionsApplied::default());
    let nobodys = cell(50, 50);
    queue.remove(nobodys);
    queue.unset_attribute(nobodys, NAME.attribute_type());
    queue.move_entity(walker(second), nobodys);
    queue.apply(entities.superchunks_mut(), 0, &mut applied);
    assert_eq!((applied.passed_over, applied.removes, applied.edits, applied.moves, entities.len()), (3, 0, 0, 0, 3));
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
    use utilities::rng::Rng;
    /// An attribute sorting before the sheep's state, and one after.
    const BEFORE: Attribute<u64> = Attribute::new(11);
    let default = default_kept(SHEEP).expect("registered with one");
    assert_eq!((default.kind, default.words, default.blocks), (SHEEP_STATE.attribute_type(), SheepState::WORDS, SheepState::WORDS.div_ceil(BLOCK_WORDS)));
    assert_eq!(default_kept(WALKER), None);
    let mut random = Rng::new(utilities::seed::counted());
    let drawn = |random: &mut Rng| SheepState { hungry_at: random.draw(), lamb_due_at: random.draw(), grown_at: random.draw(), roaming_until: random.draw(), roaming_neighbour: random.draw() as u32 };
    for round in 0..200 {
        let mut entities = world();
        let at = cell(random.below(200) as u32, random.below(200) as u32);
        let sheep = Header { kind: SHEEP, at, wake: NEVER };
        // Given its state or not, with an attribute before it by type, one after, both or neither.
        let (state, given, before, after) = (drawn(&mut random), random.below(2) == 0, random.below(2) == 0, random.below(2) == 0);
        let mut attributes = Vec::new();
        if before {
            push_attribute(&mut attributes, BEFORE, 5);
        }
        if given {
            push_attribute(&mut attributes, SHEEP_STATE, state);
        }
        if after {
            push_attribute(&mut attributes, NAME, 7);
        }
        assert!(may_be_given(SHEEP, &attributes));
        entities.queue_put(sheep, &attributes);
        entities.apply();
        let mut holds = if given { state } else { SheepState::default() };
        let put = entities.get(at).expect("put");
        // Kept: its data's blocks, then the others with their types -- a block each here.
        assert_eq!(put.attribute_list().len(), default.blocks + usize::from(before) + usize::from(after), "round {round}");
        assert_eq!(put.attributes.len(), usize::from(before) + usize::from(after));
        // Its first word is its data's first, no type before it.
        assert_eq!(put.attribute_list()[0].0[0], holds.hungry_at, "round {round}");
        assert_eq!((put.attribute(SHEEP_STATE), put.attribute(BEFORE), put.attribute(NAME)), (Some(holds), before.then_some(5), after.then_some(7)), "round {round}");
        assert_eq!(put.default_attribute().as_deref(), Some(&*WholeAttribute::holding(SHEEP_STATE, holds)));
        assert_eq!(put.attribute_seen(SHEEP_STATE.attribute_type()), entity_manager::attribute_seen(Some(&WholeAttribute::holding(SHEEP_STATE, holds))));
        // Edits drawn: the state whole, a member of it, its removal -- refused -- and the others', which leave it be.
        for edit in 0..8 {
            let (mut queue, mut applied) = (Instructions::default(), InstructionsApplied::default());
            let now_at = entities.iter().next().expect("the one sheep").header.at;
            let chosen = random.below(5);
            match chosen {
                0 => {
                    holds = drawn(&mut random);
                    queue.set_attribute(now_at, SHEEP_STATE, holds);
                }
                1 => {
                    let (member, value) = (random.below(SheepState::MEMBERS.len() as u64) as usize, random.draw());
                    let member = SheepState::MEMBERS[member];
                    // Where the member lies in the attribute, its type counted: as an instruction says it.
                    let superchunk = entities.superchunks_mut().iter_mut().next().expect("the one");
                    assert!(superchunk.write_attribute_bytes_named(now_at, SHEEP_STATE.attribute_type(), usize::from(member.offset), usize::from(member.bytes), value));
                    let mut words = [0; SheepState::WORDS];
                    holds.write(&mut words);
                    let (word, first, mask) = (usize::from(member.offset) / 8 - 1, usize::from(member.offset) % 8 * 8, u64::MAX >> (64 - u32::from(member.bytes) * 8));
                    words[word] = words[word] & !(mask << first) | (value & mask) << first;
                    holds = SheepState::read(&words);
                    assert_eq!(entities.get(now_at).expect("there").attribute_member(SHEEP_STATE.attribute_type(), usize::from(member.offset), usize::from(member.bytes)), Some(value & mask));
                }
                2 => queue.unset_attribute(now_at, SHEEP_STATE.attribute_type()),
                3 => queue.set_attribute(now_at, NAME, 100 + edit),
                // To a cell of another chunk as often as not: all it has goes with it.
                _ => queue.move_entity(Header { at: cell(random.below(600) as u32, random.below(600) as u32), ..sheep }, now_at),
            }
            queue.apply(entities.superchunks_mut(), 0, &mut applied);
            // The tick over: it is named by where it stands now.
            entities.superchunks_mut().iter_mut().for_each(|superchunk| superchunk.names_anew(&mut ()));
            // Its removal alone is passed over: a default attribute is never taken.
            assert_eq!(applied.passed_over, usize::from(chosen == 2), "round {round}, edit {edit}");
            let edited = entities.iter().next().expect("the one sheep");
            assert_eq!(edited.attribute(SHEEP_STATE), Some(holds), "round {round}, edit {edit}");
            assert!(sorted(edited.attributes) && edited.attribute_list().len() == default.blocks + edited.attributes.len(), "round {round}, edit {edit}");
            assert_eq!(edited.attribute(BEFORE), before.then_some(5), "round {round}, edit {edit}");
        }
        // Saved and read back: the same sheep, kept the same.
        let was = entities.iter().next().expect("the one sheep");
        let (words, count) = encode_state(None, entities.superchunk(was.header.at.superchunk()));
        let mut read_back = world();
        assert_eq!((count, decode_state(&words, 0, &mut read_back).map(|state| state.entities)), (1, Ok(1)));
        read_back.apply();
        let is = read_back.get(was.header.at).expect("read back");
        assert_eq!((was.header, was.attribute_list()), (is.header, is.attribute_list()), "round {round}");
    }
}
