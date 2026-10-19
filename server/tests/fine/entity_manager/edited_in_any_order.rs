//! Attributes of types drawn, set, set again and unset in an order
//! drawn, saved and read back.

use super::{registered_types, varying_attribute, walker_header, world, cell};
use crate::entity_manager::{is_attribute_list, AttributeBlock, Instructions, InstructionsApplied, BLOCK_WORDS};
use crate::entity_manager::saved::{decode_state, encode_state};
use utilities::rng::RandomSource;

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
