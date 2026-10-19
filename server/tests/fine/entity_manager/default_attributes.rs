//! A type's default attribute: kept without its type, used as any other.

use super::{registered_types, world, cell, GrazerState};
use crate::entity_manager::{attribute_seen, is_attribute_list, may_be_given, push_attribute, DefaultAttributes, Header, Instructions, InstructionsApplied, Layout, WholeAttribute, BLOCK_WORDS, NEVER_WAKES};
use crate::entity_manager::saved::{decode_state, encode_state};
use utilities::rng::RandomSource;

/// A grazer, whose type is registered with a default attribute -- its
/// state -- has it always: given with its type like any attribute,
/// or not given and zeroes; kept first and without its type, so a
/// grazer with nothing else is its data's blocks and no more; read,
/// compared and written whole or a member at a time by its type as
/// any other, wherever it sorts among the others; never removed;
/// moved to another chunk and saved with the grazer. States, the
/// attributes about them and the edits are drawn from the seed.
#[test]
fn a_types_default_attribute_is_kept_without_its_type_and_used_as_any_other() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (frozen_types, types) = registered_types(seed);
    // An attribute sorting before the grazer's state, and one after: what the test is about, so said.
    assert!(types.sorts_before_state.attribute_type().type_identity() < types.grazer_state.attribute_type().type_identity(), "registered before the grazer's state");
    assert!(types.one_block.attribute_type().type_identity() > types.grazer_state.attribute_type().type_identity(), "registered after the grazer's state");
    let default_attributes = DefaultAttributes::of_types(&frozen_types);
    let kept_default = default_attributes.of(types.grazer).expect("registered with one");
    assert_eq!((kept_default.attribute_type, kept_default.words, kept_default.block_count), (types.grazer_state.attribute_type(), GrazerState::WORDS, GrazerState::WORDS.div_ceil(BLOCK_WORDS)));
    assert_eq!(default_attributes.of(types.walker), None);
    let mut seeded_rng = RandomSource::new(seed);
    let draw_state = |seeded_rng: &mut RandomSource| GrazerState { words: std::array::from_fn(|_| seeded_rng.next_draw()), small: seeded_rng.next_draw() as u32 };
    for round_number in 0..200 {
        let mut world_entities = world(&frozen_types);
        let grazer_cell = cell(seeded_rng.draw_below(200) as u32, seeded_rng.draw_below(200) as u32);
        let grazer_header = Header { entity_type: types.grazer, stood_cell: grazer_cell, wake: NEVER_WAKES };
        // Given its state or not, with an attribute before it by type, one after, both or neither.
        let (drawn_state, state_given, has_before, has_after) = (draw_state(&mut seeded_rng), seeded_rng.draw_below(2) == 0, seeded_rng.draw_below(2) == 0, seeded_rng.draw_below(2) == 0);
        let mut attributes = Vec::new();
        if has_before {
            push_attribute(&mut attributes, types.sorts_before_state, 5);
        }
        if state_given {
            push_attribute(&mut attributes, types.grazer_state, drawn_state);
        }
        if has_after {
            push_attribute(&mut attributes, types.one_block, 7);
        }
        assert!(may_be_given(default_attributes.of(types.grazer), &attributes));
        world_entities.queue_add(grazer_header, &attributes);
        world_entities.apply();
        let mut held_state = if state_given { drawn_state } else { GrazerState::default() };
        let added_entity = world_entities.get(grazer_cell).expect("put");
        // Kept: its data's blocks, then the others with their types -- a block each here.
        assert_eq!(added_entity.attribute_list().len(), kept_default.block_count + usize::from(has_before) + usize::from(has_after), "round {round_number}");
        assert_eq!(added_entity.attributes.len(), usize::from(has_before) + usize::from(has_after));
        // Its first word is its data's first, no type before it.
        assert_eq!(added_entity.attribute_list()[0].0[0], held_state.words[0], "round {round_number}");
        assert_eq!((added_entity.attribute(types.grazer_state), added_entity.attribute(types.sorts_before_state), added_entity.attribute(types.one_block)), (Some(held_state), has_before.then_some(5), has_after.then_some(7)), "round {round_number}");
        assert_eq!(added_entity.default_attribute().as_deref(), Some(&*WholeAttribute::holding_value(types.grazer_state, held_state)));
        assert_eq!(added_entity.attribute_seen(types.grazer_state.attribute_type()), attribute_seen(Some(&WholeAttribute::holding_value(types.grazer_state, held_state))));
        // Edits drawn: the state whole, a member of it, its removal -- refused -- and the others', which leave it be.
        for edit in 0..8 {
            let (mut instruction_queue, mut instructions_applied) = (Instructions::default(), InstructionsApplied::default());
            let now_at = world_entities.iter().next().expect("the one grazer").header.stood_cell;
            let chosen_edit = seeded_rng.draw_below(5);
            match chosen_edit {
                0 => {
                    held_state = draw_state(&mut seeded_rng);
                    instruction_queue.set_attribute(now_at, types.grazer_state, held_state);
                }
                1 => {
                    let (member, value) = (seeded_rng.draw_below(GrazerState::MEMBERS.len() as u64) as usize, seeded_rng.next_draw());
                    let member = GrazerState::MEMBERS[member];
                    // Where the member lies in the attribute, its type counted: as an instruction says it.
                    let superchunk = world_entities.superchunks_mut().iter_mut().next().expect("the one");
                    assert!(superchunk.write_attribute_bytes_named(now_at, types.grazer_state.attribute_type(), usize::from(member.byte_offset), usize::from(member.byte_count), value));
                    let mut words = [0; GrazerState::WORDS];
                    held_state.write(&mut words);
                    let (word, first_bit, mask) = (usize::from(member.byte_offset) / 8 - 1, usize::from(member.byte_offset) % 8 * 8, u64::MAX >> (64 - u32::from(member.byte_count) * 8));
                    words[word] = words[word] & !(mask << first_bit) | (value & mask) << first_bit;
                    held_state = GrazerState::read_data(&words);
                    assert_eq!(world_entities.get(now_at).expect("there").attribute_member(types.grazer_state.attribute_type(), usize::from(member.byte_offset), usize::from(member.byte_count)), Some(value & mask));
                }
                2 => instruction_queue.unset_attribute(now_at, types.grazer_state.attribute_type()),
                3 => instruction_queue.set_attribute(now_at, types.one_block, 100 + edit),
                // To a cell of another chunk as often as not: all it has goes with it.
                _ => instruction_queue.move_entity(Header { stood_cell: cell(seeded_rng.draw_below(600) as u32, seeded_rng.draw_below(600) as u32), ..grazer_header }, now_at),
            }
            instruction_queue.apply(world_entities.superchunks_mut(), 0, &mut instructions_applied);
            // The tick over: it is named by where it stands now.
            world_entities.superchunks_mut().iter_mut().for_each(|superchunk| superchunk.names_anew());
            // Its removal alone is passed over: a default attribute is never taken.
            assert_eq!(instructions_applied.passed_over, usize::from(chosen_edit == 2), "round {round_number}, edit {edit}");
            let edited_entity = world_entities.iter().next().expect("the one grazer");
            assert_eq!(edited_entity.attribute(types.grazer_state), Some(held_state), "round {round_number}, edit {edit}");
            assert!(is_attribute_list(edited_entity.attributes) && edited_entity.attribute_list().len() == kept_default.block_count + edited_entity.attributes.len(), "round {round_number}, edit {edit}");
            assert_eq!(edited_entity.attribute(types.sorts_before_state), has_before.then_some(5), "round {round_number}, edit {edit}");
        }
        // Saved and read back: the same grazer, kept the same.
        let saved_entity = world_entities.iter().next().expect("the one grazer");
        let (words, count) = encode_state(None, world_entities.superchunk(saved_entity.header.stood_cell.superchunk()));
        let mut read_back = world(&frozen_types);
        assert_eq!((count, decode_state(&words, 0, &mut read_back).map(|saved_state| saved_state.entity_count)), (1, Ok(1)));
        read_back.apply();
        let read_entity = read_back.get(saved_entity.header.stood_cell).expect("read back");
        assert_eq!((saved_entity.header, saved_entity.attribute_list()), (read_entity.header, read_entity.attribute_list()), "round {round_number}");
    }
}
