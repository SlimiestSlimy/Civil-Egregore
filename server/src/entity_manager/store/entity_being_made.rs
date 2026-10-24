//! An entity being made, between ticks ([`EntityBeingMade`]): its type,
//! cell and first wake, and its attributes set by type as it is made --
//! the store laying out their blocks, the type's default where it keeps
//! defaults. Made, or refused, at once ([`EntityBeingMade::make`]).

use super::SuperchunkEntities;
use crate::entity_manager::attributes::{set_attribute, Attribute, AttributeBlock, Layout};
use crate::entity_manager::bucket::EntityWritten;
use crate::entity_manager::entity::{EntityType, Header, NEVER_WAKES};
use crate::simulation::simulation_api::simulation_time::SimulationTime;
use utilities::coordinates::CellIndex;

/// When an entity made between ticks first wakes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstWake {
    /// This long after the tick it is made for.
    After(SimulationTime),
    /// Never: asleep until something wakes it.
    Never,
}

/// An entity being made in a superchunk's entities: made by
/// [`EntityBeingMade::make`], nothing of it there till then.
pub struct EntityBeingMade<'store> {
    /// The entities it is made in.
    superchunk_entities: &'store mut SuperchunkEntities,
    /// The first tick it can wake at: the one it is made for.
    earliest_tick: u64,
    /// Its header.
    header: Header,
    /// Its attributes so far, whole ones sorted by type.
    attributes: Vec<AttributeBlock>,
}

impl SuperchunkEntities {
    /// An entity of `entity_type` to make on `cell`, for the tick
    /// `earliest_tick`, first waking as `first_wake` says.
    pub(crate) fn new_entity(&mut self, earliest_tick: u64, entity_type: EntityType, cell: CellIndex, first_wake: FirstWake) -> EntityBeingMade<'_> {
        let wake = match first_wake {
            FirstWake::After(time) => time.tick_after(earliest_tick),
            FirstWake::Never => NEVER_WAKES,
        };
        EntityBeingMade { superchunk_entities: self, earliest_tick, header: Header { entity_type, stood_cell: cell, wake }, attributes: Vec::new() }
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
        if self.header.stood_cell.superchunk() != self.superchunk_entities.superchunk_index {
            return false;
        }
        self.superchunk_entities.make_entity(self.earliest_tick, self.header, &self.attributes) == EntityWritten::Added
    }
}
