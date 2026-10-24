//! A superchunk's state, as words a save keeps
//! (`docs/entity_manager.md`, "Saved").

use utilities::coordinates::CellIndex;
use crate::entity_manager::attributes::{may_be_given, AttributeBlock, DefaultAttributes, BLOCK_WORDS};
use crate::entity_manager::entity::{EntityType, Header};
use crate::entity_manager::store::{EntityWritten, SuperchunkEntities};

/// The first word: `TSstate` and the format's number, 5.
const FIRST_WORD: u64 = u64::from_le_bytes(*b"TSstate\x05");

/// What a state file held, beside the entities queued.
pub struct SavedState {
    /// Its random numbers' state, if it had ticked.
    pub random_stream_state: Option<u64>,
    /// How many entities it holds.
    pub entity_count: usize,
}

/// The state file of a superchunk with `random_stream_state` its random numbers'
/// state and `superchunk_entities` its entities, either of which it may lack: its
/// words, and how many entities.
pub fn encode_state(random_stream_state: Option<u64>, superchunk_entities: Option<&SuperchunkEntities>) -> (Vec<u64>, usize) {
    let count = superchunk_entities.map_or(0, SuperchunkEntities::len);
    let mut words = vec![FIRST_WORD, random_stream_state.is_some() as u64, random_stream_state.unwrap_or(0), count as u64];
    for entity in superchunk_entities.into_iter().flat_map(SuperchunkEntities::iter) {
        let header = entity.header;
        // Its type's default attribute first, its type before it as any other's: a file says what each of its blocks is.
        let whole_default = entity.default_attribute();
        let default_blocks = whole_default.as_deref().unwrap_or(&[]);
        words.extend([header.entity_type.0, header.stood_cell.0, header.wake, (default_blocks.len() + entity.attributes.len()) as u64]);
        words.extend(default_blocks.iter().chain(entity.attributes).flat_map(|saved_block| saved_block.0));
    }
    (words, count)
}

/// Reads the state file `words` into `superchunk_entities`, its
/// superchunk's, warming to be hot at `first_hot_tick`, against the
/// world's `default_attributes`: each entity made there, each whose wake
/// passed while it was cold waking at `first_hot_tick`. Its random
/// numbers' state, if it had one; or what is wrong with the file.
pub fn decode_into(words: &[u64], first_hot_tick: u64, default_attributes: &DefaultAttributes, superchunk_entities: &mut SuperchunkEntities) -> Result<Option<u64>, &'static str> {
    let saved_state = read_state(words, default_attributes, |header, attributes| {
        let made = superchunk_entities.make_entity(first_hot_tick, Header { wake: header.wake.max(first_hot_tick), ..header }, attributes);
        debug_assert_eq!(made, EntityWritten::Added, "a state keeps one entity a cell");
    })?;
    Ok(saved_state.random_stream_state)
}

/// Reads the state file `words`, each entity handed to `each_entity` with its
/// attributes: what it held beside them, or what is wrong with it.
fn read_state(words: &[u64], default_attributes: &DefaultAttributes, mut each_entity: impl FnMut(Header, &[AttributeBlock])) -> Result<SavedState, &'static str> {
    let mut words = words.iter().copied();
    let mut next_word = || words.next().ok_or("cut short");
    if next_word()? != FIRST_WORD {
        return Err("not a state file of this format");
    }
    let (has_random, random_stream_state, count) = (next_word()?, next_word()?, next_word()?);
    let mut attributes = Vec::new();
    for _ in 0..count {
        let (type_word, cell_word, wake, block_count) = (next_word()?, next_word()?, next_word()?, next_word()?);
        attributes.clear();
        for _ in 0..block_count {
            let mut read_block = AttributeBlock([0; BLOCK_WORDS]);
            for word in &mut read_block.0 {
                *word = next_word()?;
            }
            attributes.push(read_block);
        }
        if !may_be_given(default_attributes.of(EntityType(type_word)), &attributes) {
            return Err("an entity's attributes are not whole ones sorted by type");
        }
        each_entity(Header { entity_type: EntityType(type_word), stood_cell: CellIndex(cell_word), wake }, &attributes);
    }
    Ok(SavedState { random_stream_state: (has_random == 1).then_some(random_stream_state), entity_count: count as usize })
}

/// How many entities the state file `words` holds, every one read as of a
/// world whose default attributes are `default_attributes`: or what is
/// wrong with it.
pub fn entity_count(words: &[u64], default_attributes: &DefaultAttributes) -> Result<usize, &'static str> {
    read_state(words, default_attributes, |_, _| {}).map(|saved_state| saved_state.entity_count)
}
