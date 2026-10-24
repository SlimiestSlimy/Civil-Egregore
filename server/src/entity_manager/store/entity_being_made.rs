//! An entity being made, between ticks ([`EntityBeingMade`]): its type,
//! cell and first wake, and its attributes set by type as it is made --
//! the store laying out their blocks, the type's default where it keeps
//! defaults. Made, or refused, at once ([`EntityBeingMade::make`]).

use super::SuperchunkEntities;
use crate::entity_manager::attributes::set_attribute;
use protocol::server_types::attribute_layouts::{Attribute, Layout};
use protocol::server_types::attributes::AttributeBlock;
use crate::entity_manager::bucket::EntityWritten;
use crate::entity_manager::entity::EntityType;
use protocol::server_types::entity::EntityBaseData;
use protocol::server_types::simulation_time::SimulationTime;
use utilities::coordinates::CellIndex;

/// An entity being made in a superchunk's entities: made by
/// [`EntityBeingMade::make`], nothing of it there till then.
pub struct EntityBeingMade<'store> {
    /// The entities it is made in.
    superchunk_entities: &'store mut SuperchunkEntities,
    /// The first tick it can wake at: the one it is made for.
    earliest_tick: u64,
    /// Its base data.
    base_data: EntityBaseData,
    /// Its attributes so far, whole ones sorted by type.
    attributes: Vec<AttributeBlock>,
}

impl SuperchunkEntities {
    /// An entity of `entity_type` to make on `cell`, for the tick
    /// `earliest_tick`, first waking `wakes_after` after it: never before.
    pub(crate) fn new_entity(&mut self, earliest_tick: u64, entity_type: EntityType, cell: CellIndex, wakes_after: SimulationTime) -> EntityBeingMade<'_> {
        let wake = wakes_after.tick_after(earliest_tick);
        EntityBeingMade { superchunk_entities: self, earliest_tick, base_data: EntityBaseData { entity_type, stood_cell: cell, wake }, attributes: Vec::new() }
    }
}

impl EntityBeingMade<'_> {
    /// It with `attribute` holding `value`: in place of any of its type
    /// set before.
    pub fn with<L: Layout>(mut self, attribute: Attribute<L>, value: L) -> Self {
        set_attribute(&mut self.attributes, attribute, value);
        self
    }

    /// It made: whether it was -- not on a cell of another superchunk,
    /// nor on one an entity stands on.
    pub fn make(self) -> bool {
        if self.base_data.stood_cell.superchunk() != self.superchunk_entities.superchunk_index {
            return false;
        }
        self.superchunk_entities.make_entity(self.earliest_tick, self.base_data, &self.attributes) == EntityWritten::Added
    }
}
