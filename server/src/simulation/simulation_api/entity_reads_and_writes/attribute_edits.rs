//! An entity's attributes edited: each attribute a compare of what
//! the rule saw of it and, under it, the writes -- the members that
//! differ, the attribute whole, or it taken -- so no write lands on
//! another's (`docs/actions.md`, "One entity writing another").

use utilities::coordinates::CellIndex;
use crate::entity_manager::{attribute_blocks, attribute_bytes, each_attribute, words_sum, AttributeType, NO_ATTRIBUTE};
use protocol::server_types::entity::EntityBaseData;
use protocol::server_types::attribute_layouts::{Attribute, Layout, Member, ATTRIBUTE_WORDS_MOST, BLOCK_WORDS};
use protocol::server_types::attributes::{AttributeBlock, WholeAttribute};
use crate::simulation::simulation_api::compare_and_write::{self, SkipTagSet};
use crate::simulation::{Compare, Write};
use crate::simulation::SimulationTickState;

/// Queues each of `members` that differs between `blocks_before` and `blocks_after`,
/// the blocks of one attribute of the entity on `entity_cell`, as long as
/// each other: a write a member, its bytes and no more. Under
/// whatever compare is open.
fn members_differing(attribute_as_seen: &mut SkipTagSet, entity_cell: CellIndex, members: impl Iterator<Item = Member>, blocks_before: &[AttributeBlock], blocks_after: &[AttributeBlock]) {
    let type_of_attribute = blocks_after[0].attribute_type();
    for member in members {
        let member_bytes = |blocks_read: &[AttributeBlock]| attribute_bytes(blocks_read, usize::from(member.byte_offset), usize::from(member.byte_count));
        if let Some(value) = member_bytes(blocks_after).filter(|&value| member_bytes(blocks_before) != Some(value)) {
            attribute_as_seen.write_attribute_member(entity_cell, type_of_attribute, member, value);
        }
    }
}

/// Queues the attribute `blocks_seen` of `entity` -- any in reach, the
/// rule's own or another -- becoming `value`: both the blocks of one
/// attribute, either none -- not there before, or removed. A compare
/// of the attribute being still as seen, and under it the writes:
/// the words that differ, each a member for want of its layout; or,
/// its size another or it not there, the attribute whole; or it
/// taken. So of two writing one attribute of one entity in a tick
/// the first applied does and the other is refused, and two writing
/// two attributes both do: an entity is written an attribute at a
/// time, never whole, and no write lands on another's.
pub fn set_attribute_blocks(tick_state: &mut SimulationTickState, entity: &EntityBaseData, blocks_seen: Option<&[AttributeBlock]>, value: Option<&[AttributeBlock]>) {
    let (entity_cell, type_of_attribute) = (entity.stood_cell, value.or(blocks_seen).expect("an attribute seen or set")[0].attribute_type());
    let mut attribute_as_seen = compare_and_write::open_compare(tick_state, compare_and_write::attribute_blocks(entity_cell, type_of_attribute, blocks_seen));
    match (blocks_seen, value) {
        (_, None) => attribute_as_seen.remove_attribute(entity_cell, type_of_attribute),
        (Some(blocks_seen_there), Some(value)) if blocks_seen_there.len() == value.len() => members_differing(&mut attribute_as_seen, entity_cell, (0..value.len() * BLOCK_WORDS - 1).map(Member::word), blocks_seen_there, value),
        (_, Some(value)) => attribute_as_seen.write_attribute(entity_cell, value),
    }
}

/// Queues `member` of the attribute of type `type_of_attribute` of `entity`
/// becoming `value`, if it is still the `member_seen` the rule read there:
/// a compare of that member alone, and the write under it. So two
/// writing two members of one attribute in a tick both do, where a
/// compare of the whole attribute would refuse the second.
pub fn set_attribute_member(tick_state: &mut SimulationTickState, entity: &EntityBaseData, type_of_attribute: AttributeType, member: Member, member_seen: u64, value: u64) {
    compare_and_write::with_writes(tick_state, compare_and_write::attribute_member(entity.stood_cell, type_of_attribute, member, member_seen), &[Write::AttributeMember { named_cell: entity.stood_cell, attribute_type: type_of_attribute, member, value }]);
}

/// Queues `attribute` of `target_entity` -- the rule's own entity or
/// another, any in reach -- becoming `value`, if it is still the `attribute_seen`
/// the rule read of it, or it still has none: how one entity acts on
/// another. What is written is each member of the layout that
/// differs, its bytes and no more; the attribute whole if it had
/// none. Applied or refused whole, whatever else is written to
/// `target_entity` in the tick -- by itself, awake, or by a third
/// (`docs/actions.md`, "One entity writing another").
pub fn set_attribute_of<L: Layout>(tick_state: &mut SimulationTickState, target_entity: &EntityBaseData, attribute: Attribute<L>, attribute_seen: Option<L>, value: L) {
    let Some(seen_layout) = attribute_seen else {
        return set_attribute_blocks(tick_state, target_entity, None, Some(&WholeAttribute::holding_value(attribute, value)));
    };
    // Both as their words, the type first: all a member is read from, and what the attribute sums to.
    let (entity_cell, type_of_attribute, words) = (target_entity.stood_cell, attribute.attribute_type(), L::BLOCK_COUNT * BLOCK_WORDS);
    let (mut words_seen, mut words_becoming) = ([0; ATTRIBUTE_WORDS_MOST], [0; ATTRIBUTE_WORDS_MOST]);
    (words_seen[0], words_becoming[0]) = (type_of_attribute.header_word(), type_of_attribute.header_word());
    seen_layout.write(&mut words_seen[1..words]);
    value.write(&mut words_becoming[1..words]);
    let mut attribute_as_seen = compare_and_write::open_compare(tick_state, Compare::Attribute { named_cell: entity_cell, attribute_type: type_of_attribute, seen_value: words_sum(&words_seen[..words]).max(NO_ATTRIBUTE + 1) });
    for &member in L::MEMBERS {
        let member_value = member.value_in(&words_becoming[1..words]);
        if member.value_in(&words_seen[1..words]) != member_value {
            attribute_as_seen.write_attribute_member(entity_cell, type_of_attribute, member, member_value);
        }
    }
}

/// Queues removing `attribute` of `target_entity`, another entity in reach,
/// if it is still the `attribute_seen` the rule read of it.
pub fn unset_attribute_of<L: Layout>(tick_state: &mut SimulationTickState, target_entity: &EntityBaseData, attribute: Attribute<L>, attribute_seen: L) {
    set_attribute_blocks(tick_state, target_entity, Some(&WholeAttribute::holding_value(attribute, attribute_seen)), None);
}

/// Queues every attribute that differs between `blocks_seen` and
/// `attributes` -- both sorted by type -- of `entity` becoming as in
/// `attributes`, each a compare-and-write of its own.
fn set_attributes_changed(tick_state: &mut SimulationTickState, entity: &EntityBaseData, blocks_seen: &[AttributeBlock], attributes: &[AttributeBlock]) {
    // The type's default attribute is not among those seen, being kept apart: seen as it would be given.
    let default_attribute_seen = super::entity_on(tick_state, entity.stood_cell).and_then(|entity_seen| entity_seen.default_attribute());
    let default_blocks = default_attribute_seen.as_deref().filter(|default_blocks_given| !blocks_seen.is_empty() || !default_blocks_given.is_empty());
    for value in each_attribute(attributes) {
        let blocks_were = default_blocks.filter(|default_blocks_found| default_blocks_found[0].attribute_type().type_identity() == value[0].attribute_type().type_identity()).or_else(|| attribute_blocks(blocks_seen, value[0].attribute_type()));
        if blocks_were != Some(value) {
            set_attribute_blocks(tick_state, entity, blocks_were, Some(value));
        }
    }
    for blocks_gone in each_attribute(blocks_seen).filter(|blocks_checked| attribute_blocks(attributes, blocks_checked[0].attribute_type()).is_none()) {
        set_attribute_blocks(tick_state, entity, Some(blocks_gone), None);
    }
}

/// Queues `base_data_before`'s entity becoming `base_data_after`, with `attributes`:
/// each attribute that differs from what the tick found a
/// compare-and-write of its own ([`set_attribute_blocks`]), then
/// moved to its cell unless that is taken by then, and its wake
/// written. It is never put whole over itself, so what another
/// entity writes to it in the same tick is not lost. The same
/// whether its cell is in its superchunk or over a border.
pub fn update_entity(tick_state: &mut SimulationTickState, base_data_before: &EntityBaseData, base_data_after: EntityBaseData, attributes: &[AttributeBlock]) {
    let attributes_seen = super::entity_on(tick_state, base_data_before.stood_cell).map_or(&[][..], |entity_seen| entity_seen.attributes);
    set_attributes_changed(tick_state, base_data_before, attributes_seen, attributes);
    super::step(tick_state, base_data_before, base_data_after.stood_cell, base_data_after.wake);
}
