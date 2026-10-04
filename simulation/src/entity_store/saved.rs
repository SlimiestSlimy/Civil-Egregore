//! A superchunk's state, as words a save keeps (`chunk_storage::disk`).
//! A first word saying what it is; whether it has random numbers, and
//! their state; how many entities; then each entity -- its ID, type,
//! cell, wake tick, how many attributes, and each attribute's type and
//! value.

use coordinates::CellIndex;
use super::entity::{Attribute, AttributeType, EntityId, EntityType, Header, NEVER};
use super::store::{Entities, SuperchunkEntities};

/// The first word: `TSstate` and the format's number, 2.
const FIRST_WORD: u64 = u64::from_le_bytes(*b"TSstate\x02");

/// What a state file held, beside the entities queued.
pub struct SavedState {
    /// Its random numbers' state, if it had ticked.
    pub random: Option<u64>,
    /// How many entities it holds.
    pub entities: usize,
}

/// The state file of a superchunk with `random` its random numbers'
/// state and `entities` its entities, either of which it may lack: its
/// words, and how many entities.
pub fn encode_state(random: Option<u64>, entities: Option<&SuperchunkEntities>) -> (Vec<u64>, usize) {
    let count = entities.map_or(0, SuperchunkEntities::len);
    let mut words = vec![FIRST_WORD, random.is_some() as u64, random.unwrap_or(0), count as u64];
    for entity in entities.into_iter().flat_map(SuperchunkEntities::iter) {
        let header = entity.header;
        words.extend([header.id.0, header.kind.0, header.at.0, header.wake, entity.attributes.len() as u64]);
        words.extend(entity.attributes.iter().flat_map(|attribute| [attribute.kind.0, attribute.value]));
    }
    (words, count)
}

/// Reads the state file `words` of a world at tick `now`: its entities
/// queued to be put in `entities`.
pub fn decode_state(words: &[u64], now: u64, entities: &mut Entities) -> Result<SavedState, &'static str> {
    let mut words = words.iter().copied();
    let mut next = || words.next().ok_or("cut short");
    if next()? != FIRST_WORD {
        return Err("not a state file of this format");
    }
    let (has_random, random, count) = (next()?, next()?, next()?);
    let mut attributes = Vec::new();
    for _ in 0..count {
        let (id, kind, at, wake, attribute_count) = (next()?, next()?, next()?, next()?, next()?);
        attributes.clear();
        for _ in 0..attribute_count {
            attributes.push(Attribute { kind: AttributeType(next()?), value: next()? });
        }
        // One whose wake passed with no rule seeing to it wakes no more: it is not woken by being loaded.
        let wake = if wake < now { NEVER } else { wake };
        entities.queue_put(Header { id: EntityId(id), kind: EntityType(kind), at: CellIndex(at), wake }, &attributes);
    }
    Ok(SavedState { random: (has_random == 1).then_some(random), entities: count as usize })
}
