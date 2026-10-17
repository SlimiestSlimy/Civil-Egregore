//! A chunk's entities: their headers, sorted by cell in Morton order,
//! and their attributes in one list beside them. A cell holds at most
//! one entity, enforced here (`docs/entity_manager.md`, "A chunk's
//! bucket").

use crate::attributes::{find_attribute, DefaultAttributes, write_attribute_bytes, write_bytes, write_default, AttributeBlock, AttributeType};
use crate::entity::{EntityRef, Header};
use coordinates::CellIndex;
use utilities::cache::prefetch_cache_line;

/// Attribute blocks in a cache line.
const CACHE_LINE_BLOCKS: usize = 64 / crate::attributes::BLOCK_BYTES;

/// The chunk is searched in 64x64 tiles of cells: a place is first
/// narrowed to its tile, whose places are a run of the sorted list.
const SEARCH_TILES: usize = 16;
/// The bits of a place below its search tile's.
const PLACE_IN_SEARCH_TILE_BITS: u32 = u16::BITS - SEARCH_TILES.trailing_zeros();
/// Cells in an aligned 8x8 tile, which is a run of places.
const WORD_TILE_CELLS: u16 = 64;

/// The search tile `place` is in.
const fn search_tile(place: u16) -> usize {
    (place >> PLACE_IN_SEARCH_TILE_BITS) as usize
}

/// A cell's place in its chunk, as a Morton index: what a bucket is
/// sorted and searched by.
pub(crate) fn place(cell: CellIndex) -> u16 {
    cell.place() as u16
}

/// An entity as its bucket keeps it: its header, and where its
/// attributes' blocks are in the bucket's list of them.
#[derive(Clone, Copy, Debug)]
struct StoredEntity {
    /// The entity's fixed part.
    header: Header,
    /// Where its blocks start in the list.
    first_block: u32,
    /// How many blocks they are.
    count: u16,
    /// How many of them, the first, are its type's default
    /// attribute, kept without its type: 0 if its type has none.
    default_blocks: u16,
}

/// What putting an entity in a bucket did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityWritten {
    /// It is new, and now stands on its cell.
    Added,
    /// It was changed where it stands.
    InPlace,
    /// It moved to its cell.
    MovedToCell,
    /// Its cell was occupied: it was changed, and stays where it was.
    TurnedBack,
    /// It is new and its cell was occupied: it is not added.
    RefusedOccupied,
    /// It was to be changed or moved, but no entity stands where it
    /// was: it moved on, or was removed.
    PassedOver,
}

impl EntityWritten {
    /// What it is of an entity that was to move and was put where it
    /// stood instead, its cell not to be come to: it stayed.
    pub(crate) fn as_turned_back(self) -> Self {
        if self == Self::InPlace { Self::TurnedBack } else { self }
    }
}

/// A chunk's entities.
#[derive(Default)]
pub(crate) struct Bucket {
    /// Each entity's [`place`], sorted, each once: what is searched.
    places: Vec<u16>,
    /// Where each search tile's places start in `places`, and, last, how
    /// many places there are: a place is searched for only among its
    /// search tile's, a few, not all the chunk's.
    tile_starts: [u32; SEARCH_TILES + 1],
    /// The entities, in the same order as `places`.
    stored_entities: Vec<StoredEntity>,
    /// Every entity's attribute list, one after another, and garbage.
    attributes: Vec<AttributeBlock>,
    /// How many blocks of `attributes` belong to no entity.
    garbage_blocks: usize,
}

impl Bucket {
    /// How many entities it holds.
    pub(crate) fn len(&self) -> usize {
        self.stored_entities.len()
    }

    /// The entity standing on `stood_cell`, if one does.
    #[inline]
    pub(crate) fn get<'a>(&'a self, stood_cell: CellIndex, default_attributes: &'a DefaultAttributes) -> Option<EntityRef<'a>> {
        let entity_index = self.find_place(place(stood_cell)).ok()?;
        Some(self.entity(&self.stored_entities[entity_index], default_attributes))
    }

    /// Asks memory ahead for the entity standing on `stood_cell`.
    pub(crate) fn prefetch_entity(&self, stood_cell: CellIndex) {
        if let Some(stored_entity) = self.stored_entities.get(self.index_of(place(stood_cell)).0) {
            prefetch_cache_line(stored_entity);
        }
    }

    /// Asks memory ahead for the attributes of the entity standing on
    /// `stood_cell`: its first block, and the one a cache line on, blocks of
    /// sixteen bytes lying over two lines as often as not. This reads
    /// the entity, so ask for it first.
    pub(crate) fn prefetch_attributes(&self, stood_cell: CellIndex) {
        let Some(stored_entity) = self.stored_entities.get(self.index_of(place(stood_cell)).0) else {
            return;
        };
        let (first_block, last_block) = (stored_entity.first_block as usize, (stored_entity.first_block as usize + stored_entity.count as usize).saturating_sub(1));
        if let Some(nearer_block) = self.attributes.get(first_block) {
            prefetch_cache_line(nearer_block);
        }
        if let Some(farther_block) = self.attributes.get(last_block.min(first_block + CACHE_LINE_BLOCKS - 1)).filter(|_| last_block > first_block) {
            prefetch_cache_line(farther_block);
        }
    }

    /// Every entity here, by cell in Morton order.
    pub(crate) fn iter<'a>(&'a self, default_attributes: &'a DefaultAttributes) -> impl Iterator<Item = EntityRef<'a>> {
        self.stored_entities.iter().map(move |stored_entity| self.entity(stored_entity, default_attributes))
    }

    /// The places of the entities standing on the aligned 8x8 tile whose
    /// first cell is at `first_place`. An aligned tile is a run of cells in
    /// Morton order, so these are a run of `places`.
    pub(crate) fn in_word_tile(&self, first_place: u16) -> &[u16] {
        let start_index = self.index_of(first_place).0;
        // A word tile lies within one search tile.
        let end_index = self.tile_starts[search_tile(first_place) + 1] as usize;
        let candidate_places = &self.places[start_index..end_index];
        &candidate_places[..candidate_places.partition_point(|&other_place| other_place - first_place < WORD_TILE_CELLS)]
    }

    /// Whether an entity stands on the cell at `place`.
    pub(crate) fn occupied(&self, place: u16) -> bool {
        self.index_of(place).1
    }

    /// Makes `header`'s entity on its cell, with `attributes` as a
    /// bucket keeps them ([`crate::attributes::append_kept`]): new, or
    /// refused, an entity standing there
    /// (`docs/entity_manager.md`, "A chunk's bucket", Putting).
    pub(crate) fn make_entity(&mut self, header: Header, attributes: &[AttributeBlock], default_attributes: &DefaultAttributes) -> EntityWritten {
        let target_place = place(header.stood_cell);
        let Err(entity_index) = self.find_place(target_place) else {
            return EntityWritten::RefusedOccupied;
        };
        self.places.insert(entity_index, target_place);
        self.tile_starts[search_tile(target_place) + 1..].iter_mut().for_each(|tile_start| *tile_start += 1);
        let default_blocks = default_attributes.of(header.entity_type).map_or(0, |kept_default| kept_default.block_count as u16);
        debug_assert!(attributes.len() >= default_blocks as usize && attributes.len() <= u16::MAX as usize, "an entity's blocks as a bucket keeps them");
        self.stored_entities.insert(entity_index, StoredEntity { header, first_block: self.attributes.len() as u32, count: attributes.len() as u16, default_blocks });
        self.attributes.extend_from_slice(attributes);
        EntityWritten::Added
    }

    /// Gives the entity standing on the cell at `former_place` `header` -- its
    /// cell, when it wakes -- and `attributes`, or, with none given,
    /// leaves it those it has: moved to `header`'s cell, or left
    /// where it stands, that one taken; passed over, none standing on
    /// `former_place` (`docs/entity_manager.md`, "A chunk's bucket", Putting).
    pub(crate) fn write_entity(&mut self, header: Header, former_place: u16, attributes: Option<&[AttributeBlock]>) -> EntityWritten {
        let target_place = place(header.stood_cell);
        let (entity_index, written) = match (self.find_place(former_place), former_place == target_place) {
            (Ok(found_index), true) => (found_index, EntityWritten::InPlace),
            (Ok(from_index), false) => {
                let (goes_index, occupied) = self.index_of(target_place);
                if occupied {
                    // Its cell is occupied: it stays on the cell it was on.
                    let turned_back_header = Header { stood_cell: CellIndex(header.stood_cell.0 - target_place as u64 + former_place as u64), ..header };
                    self.rewrite_entity(from_index, turned_back_header, attributes);
                    return EntityWritten::TurnedBack;
                }
                (self.shift_entity(from_index, goes_index, target_place), EntityWritten::MovedToCell)
            }
            (Err(_), _) => return EntityWritten::PassedOver,
        };
        self.rewrite_entity(entity_index, header, attributes);
        written
    }

    /// Gives the entity at `entity_index` `header`, and `attributes` if given:
    /// written over its attribute list when they are as many blocks,
    /// else as a new attribute list at the end of the bucket's blocks.
    fn rewrite_entity(&mut self, entity_index: usize, header: Header, attributes: Option<&[AttributeBlock]>) {
        let stored_entity = &mut self.stored_entities[entity_index];
        stored_entity.header = header;
        let Some(attributes) = attributes else {
            return;
        };
        if stored_entity.count as usize == attributes.len() {
            let first_block = stored_entity.first_block as usize;
            self.attributes[first_block..first_block + attributes.len()].copy_from_slice(attributes);
            return;
        }
        self.garbage_blocks += stored_entity.count as usize;
        (stored_entity.first_block, stored_entity.count) = (self.attributes.len() as u32, attributes.len() as u16);
        self.attributes.extend_from_slice(attributes);
        self.sweep_garbage();
    }

    /// Sets the attribute of type `attribute_type` of the entity standing on the
    /// cell at `place` to `whole_attribute`, or removes it if they are none.
    /// Returns whether an entity stands there. One as long as it was
    /// is written in place; any other makes the entity's attribute
    /// list anew at the end of the bucket's blocks.
    pub(crate) fn edit(&mut self, place: u16, attribute_type: AttributeType, whole_attribute: &[AttributeBlock], default_attributes: &DefaultAttributes) -> bool {
        let Ok(entity_index) = self.find_place(place) else {
            return false;
        };
        let (first_block, count) = (self.stored_entities[entity_index].first_block as usize, self.stored_entities[entity_index].count as usize);
        // The type's default attribute is written where it is kept, its type left off, and never removed.
        let first_other_block = first_block + self.stored_entities[entity_index].default_blocks as usize;
        if let Some(kept_default) = default_attributes.of(self.stored_entities[entity_index].header.entity_type).filter(|candidate_default| first_other_block > first_block && candidate_default.attribute_type.type_identity() == attribute_type.type_identity()) {
            return write_default(&mut self.attributes[first_block..first_other_block], kept_default, whole_attribute);
        }
        let (former_range, new_start) = match find_attribute(&self.attributes[first_other_block..first_block + count], attribute_type) {
            Ok(found_range) => (first_other_block + found_range.start..first_other_block + found_range.end, self.attributes.len()),
            Err(insert_index) => (first_other_block + insert_index..first_other_block + insert_index, self.attributes.len()),
        };
        if former_range.len() == whole_attribute.len() {
            self.attributes[former_range].copy_from_slice(whole_attribute);
            return true;
        }
        self.attributes.extend_from_within(first_block..former_range.start);
        self.attributes.extend_from_slice(whole_attribute);
        self.attributes.extend_from_within(former_range.end..first_block + count);
        let stored_entity = &mut self.stored_entities[entity_index];
        (stored_entity.first_block, stored_entity.count) = (new_start as u32, (self.attributes.len() - new_start) as u16);
        self.garbage_blocks += count;
        self.sweep_garbage();
        true
    }

    /// Writes the low `byte_count` bytes of `value` over the member from
    /// the byte `byte_offset` of the attribute of type `attribute_type`, of the
    /// entity standing on the cell at `place`: in place, nothing
    /// moved. Whether it was written -- not if nobody stands there,
    /// it has no such attribute, or the member is not within it.
    pub(crate) fn write_bytes(&mut self, place: u16, attribute_type: AttributeType, byte_offset: usize, byte_count: usize, value: u64, default_attributes: &DefaultAttributes) -> bool {
        let Ok(entity_index) = self.find_place(place) else {
            return false;
        };
        let (first_block, count) = (self.stored_entities[entity_index].first_block as usize, self.stored_entities[entity_index].count as usize);
        let first_other_block = first_block + self.stored_entities[entity_index].default_blocks as usize;
        // A member of the type's default attribute is where it would be were its type kept before it.
        if first_other_block > first_block && default_attributes.of(self.stored_entities[entity_index].header.entity_type).is_some_and(|kept_default| kept_default.attribute_type.type_identity() == attribute_type.type_identity()) {
            return byte_offset >= 8 && write_bytes(&mut self.attributes[first_block..first_other_block], byte_offset - 8, byte_count, value);
        }
        let Ok(found_range) = find_attribute(&self.attributes[first_other_block..first_block + count], attribute_type) else {
            return false;
        };
        write_attribute_bytes(&mut self.attributes[first_other_block + found_range.start..first_other_block + found_range.end], byte_offset, byte_count, value)
    }

    /// Sets the tick the entity standing on the cell at `place` next
    /// wakes at: whether one stands there.
    pub(crate) fn set_wake(&mut self, place: u16, wake: u64) -> bool {
        let Ok(entity_index) = self.find_place(place) else {
            return false;
        };
        self.stored_entities[entity_index].header.wake = wake;
        true
    }

    /// Removes the entity standing on `stood_cell`. Returns whether one did.
    pub(crate) fn remove(&mut self, stood_cell: CellIndex) -> bool {
        let Ok(entity_index) = self.find_place(place(stood_cell)) else {
            return false;
        };
        let place = self.places.remove(entity_index);
        self.tile_starts[search_tile(place) + 1..].iter_mut().for_each(|tile_start| *tile_start -= 1);
        self.garbage_blocks += self.stored_entities.remove(entity_index).count as usize;
        self.sweep_garbage();
        true
    }

    /// Where the entity on the cell at `place` is in the bucket's order,
    /// or would go, and whether one stands there.
    #[inline]
    fn index_of(&self, place: u16) -> (usize, bool) {
        let tile = search_tile(place);
        let (tile_first_index, tile_end_index) = (self.tile_starts[tile] as usize, self.tile_starts[tile + 1] as usize);
        let entity_index = tile_first_index + self.places[tile_first_index..tile_end_index].partition_point(|&other_place| other_place < place);
        (entity_index, self.places.get(entity_index) == Some(&place))
    }

    /// Where the entity on the cell at `place` is in the bucket's
    /// order; or, none standing there, where it would go.
    #[inline]
    fn find_place(&self, place: u16) -> Result<usize, usize> {
        let (entity_index, occupied) = self.index_of(place);
        if occupied { Ok(entity_index) } else { Err(entity_index) }
    }

    /// Moves the entity at `from_index` to `goes_index`, where an entity on the free
    /// cell at `target_place` belongs, shifting the entities between by one.
    /// Returns where it now is.
    fn shift_entity(&mut self, from_index: usize, goes_index: usize, target_place: u16) -> usize {
        let former_place = self.places[from_index];
        let entity_index = if goes_index > from_index {
            self.places[from_index..goes_index].rotate_left(1);
            self.stored_entities[from_index..goes_index].rotate_left(1);
            goes_index - 1
        } else {
            self.places[goes_index..=from_index].rotate_right(1);
            self.stored_entities[goes_index..=from_index].rotate_right(1);
            goes_index
        };
        self.places[entity_index] = target_place;
        // The search tiles between the two cells now start one earlier, or one later.
        if search_tile(former_place) < search_tile(target_place) {
            self.tile_starts[search_tile(former_place) + 1..=search_tile(target_place)].iter_mut().for_each(|tile_start| *tile_start -= 1);
        } else {
            self.tile_starts[search_tile(target_place) + 1..=search_tile(former_place)].iter_mut().for_each(|tile_start| *tile_start += 1);
        }
        entity_index
    }

    /// The entity `stored_entity` is, with its attributes.
    #[inline]
    fn entity<'a>(&'a self, stored_entity: &StoredEntity, default_attributes: &'a DefaultAttributes) -> EntityRef<'a> {
        let first_block = stored_entity.first_block as usize;
        let kept_blocks = &self.attributes[first_block..first_block + stored_entity.count as usize];
        EntityRef { header: stored_entity.header, attributes: &kept_blocks[stored_entity.default_blocks as usize..], kept_blocks, default_attributes }
    }

    /// Sweeps the garbage out of the list of blocks, once there is as
    /// much garbage as blocks in use (and enough to be worth it).
    fn sweep_garbage(&mut self) {
        if self.garbage_blocks < 64 || self.garbage_blocks < self.attributes.len() - self.garbage_blocks {
            return;
        }
        let mut swept_blocks = Vec::with_capacity(self.attributes.len() - self.garbage_blocks);
        for stored_entity in &mut self.stored_entities {
            let first_block = stored_entity.first_block as usize;
            stored_entity.first_block = swept_blocks.len() as u32;
            swept_blocks.extend_from_slice(&self.attributes[first_block..first_block + stored_entity.count as usize]);
        }
        self.attributes = swept_blocks;
        self.garbage_blocks = 0;
    }
}
