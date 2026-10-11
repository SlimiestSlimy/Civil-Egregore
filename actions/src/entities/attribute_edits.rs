//! An entity's attributes edited: each attribute a compare of what
//! the rule saw of it and, under it, the writes -- the members that
//! differ, the attribute whole, or it taken -- so no write lands on
//! another's (`docs/actions.md`, "One entity writing another").

use coordinates::CellIndex;
use entity_manager::{attribute_blocks, attribute_bytes, each_attribute, words_sum, Attribute, AttributeBlock, AttributeType, Header, Layout, Member, WholeAttribute, ATTRIBUTE_WORDS_MOST, BLOCK_WORDS, NO_ATTRIBUTE};
use compares_and_writes::{compare, write, Write};
use simulation::Turn;

/// Queues each of `members` that differs between `was` and `is`,
/// the blocks of one attribute of the entity on `at`, as long as
/// each other: a write a member, its bytes and no more. Under
/// whatever compare is open.
fn members_differing(turn: &mut Turn, at: CellIndex, members: impl Iterator<Item = Member>, was: &[AttributeBlock], is: &[AttributeBlock]) {
    let kind = is[0].kind();
    for member in members {
        let held = |blocks: &[AttributeBlock]| attribute_bytes(blocks, usize::from(member.offset), usize::from(member.bytes));
        if let Some(value) = held(is).filter(|&value| held(was) != Some(value)) {
            write::attribute_member(turn, at, kind, member, value);
        }
    }
}

/// Queues the attribute `seen` of `entity` -- any in reach, the
/// rule's own or another -- becoming `value`: both the blocks of one
/// attribute, either none -- not there before, or removed. A compare
/// of the attribute being still as seen, and under it the writes:
/// the words that differ, each a member for want of its layout; or,
/// its size another or it not there, the attribute whole; or it
/// taken. So of two writing one attribute of one entity in a tick
/// the first applied does and the other is refused, and two writing
/// two attributes both do: an entity is written an attribute at a
/// time, never whole, and no write lands on another's.
pub fn set_attribute_blocks(turn: &mut Turn, entity: &Header, seen: Option<&[AttributeBlock]>, value: Option<&[AttributeBlock]>) {
    let (at, kind) = (entity.at, value.or(seen).expect("an attribute seen or set")[0].kind());
    compare::open(turn, compare::attribute_blocks(at, kind, seen));
    match (seen, value) {
        (_, None) => write::attribute_removed(turn, at, kind),
        (Some(seen), Some(value)) if seen.len() == value.len() => members_differing(turn, at, (0..value.len() * BLOCK_WORDS - 1).map(Member::word), seen, value),
        (_, Some(value)) => write::attribute_whole(turn, at, value),
    }
    compare::close(turn);
}

/// Queues `member` of the attribute of type `kind` of `entity`
/// becoming `value`, if it is still the `seen` the rule read there:
/// a compare of that member alone, and the write under it. So two
/// writing two members of one attribute in a tick both do, where a
/// compare of the whole attribute would refuse the second.
pub fn set_attribute_member(turn: &mut Turn, entity: &Header, kind: AttributeType, member: Member, seen: u64, value: u64) {
    compare::with_writes(turn, compare::attribute_member(entity.at, kind, member, seen), &[Write::AttributeMember { at: entity.at, kind, member, value }]);
}

/// Queues `attribute` of `other` -- the rule's own entity or
/// another, any in reach -- becoming `value`, if it is still the `seen`
/// the rule read of it, or it still has none: how one entity acts on
/// another. What is written is each member of the layout that
/// differs, its bytes and no more; the attribute whole if it had
/// none. Applied or refused whole, whatever else is written to
/// `other` in the tick -- by itself, awake, or by a third
/// (`docs/actions.md`, "One entity writing another").
pub fn set_attribute_of<L: Layout>(turn: &mut Turn, other: &Header, attribute: Attribute<L>, seen: Option<L>, value: L) {
    let Some(seen) = seen else {
        return set_attribute_blocks(turn, other, None, Some(&WholeAttribute::holding(attribute, value)));
    };
    // Both as their words, the type first: all a member is read from, and what the attribute sums to.
    let (at, kind, words) = (other.at, attribute.attribute_type(), L::BLOCKS * BLOCK_WORDS);
    let (mut was, mut is) = ([0; ATTRIBUTE_WORDS_MOST], [0; ATTRIBUTE_WORDS_MOST]);
    (was[0], is[0]) = (kind.0, kind.0);
    seen.write(&mut was[1..words]);
    value.write(&mut is[1..words]);
    compare::open(turn, compare::Compare::Attribute { at, kind, seen: words_sum(&was[..words]).max(NO_ATTRIBUTE + 1) });
    for &member in L::MEMBERS {
        let held = member.of(&is[1..words]);
        if member.of(&was[1..words]) != held {
            write::attribute_member(turn, at, kind, member, held);
        }
    }
    compare::close(turn);
}

/// Queues removing `attribute` of `other`, another entity in reach,
/// if it is still the `seen` the rule read of it.
pub fn unset_attribute_of<L: Layout>(turn: &mut Turn, other: &Header, attribute: Attribute<L>, seen: L) {
    set_attribute_blocks(turn, other, Some(&WholeAttribute::holding(attribute, seen)), None);
}

/// Queues every attribute that differs between `seen` and
/// `attributes` -- both sorted by type -- of `entity` becoming as in
/// `attributes`, each a compare-and-write of its own.
fn set_attributes_changed(turn: &mut Turn, entity: &Header, seen: &[AttributeBlock], attributes: &[AttributeBlock]) {
    // The type's default attribute is not among those seen, being kept apart: seen as it would be given.
    let default = super::on(turn, entity.at).and_then(|seen| seen.default_attribute());
    let default = default.as_deref().filter(|default| !seen.is_empty() || !default.is_empty());
    for value in each_attribute(attributes) {
        let was = default.filter(|default| default[0].kind().identity() == value[0].kind().identity()).or_else(|| attribute_blocks(seen, value[0].kind()));
        if was != Some(value) {
            set_attribute_blocks(turn, entity, was, Some(value));
        }
    }
    for was in each_attribute(seen).filter(|was| attribute_blocks(attributes, was[0].kind()).is_none()) {
        set_attribute_blocks(turn, entity, Some(was), None);
    }
}

/// Queues `before`'s entity becoming `after`, with `attributes`:
/// each attribute that differs from what the tick found a
/// compare-and-write of its own ([`set_attribute_blocks`]), then
/// moved to its cell unless that is taken by then, and its wake
/// written. It is never put whole over itself, so what another
/// entity writes to it in the same tick is not lost. To another
/// superchunk it crosses.
pub fn update(turn: &mut Turn, before: &Header, after: Header, attributes: &[AttributeBlock]) {
    let seen = super::on(turn, before.at).map_or(&[][..], |seen| seen.attributes);
    set_attributes_changed(turn, before, seen, attributes);
    if before.at.superchunk() == after.at.superchunk() {
        super::step(turn, before, after.at, after.wake);
    } else {
        // Its type's default attribute goes with it as the tick found it, unless it is among those given.
        let default = super::on(turn, before.at).and_then(|seen| seen.default_attribute());
        let default = default.as_deref().filter(|default| attribute_blocks(attributes, default[0].kind()).is_none());
        super::cross(turn, before, after, default.unwrap_or(&[]), attributes);
    }
}
