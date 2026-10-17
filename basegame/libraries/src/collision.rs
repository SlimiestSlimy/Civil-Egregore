//! The collision plane's API ([`Collision`]): an entity that depends on
//! the plane comes, goes and steps only where it holds no cell, and
//! keeps its own cell held there -- each a compare-and-write of the
//! entity's, made of the generic actions alone.

use simulation::simulation_api::compare_and_write::{self, SkipTagSet};
use simulation::simulation_api::{entity_reads_and_writes, AttributeBlock, CellIndex, EntityType, Header, LayerType, SimulationTickState};

/// The collision plane's library, as its registration exposes it.
pub static COLLISION: Collision = Collision { plane: base_types::COLLISION };

/// What an entity that collides does through the collision plane: what
/// stands on a cell bars it, and it bars the cell it stands on.
#[derive(Debug)]
pub struct Collision {
    /// The plane: a cell set wherever something stands that bars
    /// stepping there.
    plane: LayerType,
}

impl Collision {
    /// Queues making an entity of type `entity_type` on `entity_cell`, with
    /// `attributes` sorted by type, to wake at `wake`, its cell held:
    /// what the rule queues under the compares returned happens only if
    /// it is made -- and it is made only if no entity stands on the
    /// cell or is named by it this tick, and the plane holds it not.
    #[inline]
    pub fn begin_making_entity<'scope, 'world>(&self, tick_state: &'scope mut SimulationTickState<'world>, entity_type: EntityType, entity_cell: CellIndex, wake: u64, attributes: &[AttributeBlock]) -> SkipTagSet<'scope, 'world> {
        let mut cell_free_still = compare_and_write::open_compare(tick_state, compare_and_write::cell_free(entity_cell)).and_compare(compare_and_write::layer_lacks(self.plane, entity_cell));
        cell_free_still.make_entity(Header { entity_type, stood_cell: entity_cell, wake }, attributes);
        cell_free_still.write_cell(self.plane, entity_cell, 1);
        cell_free_still
    }

    /// Queues `entity` stepping to `to_cell` -- its own cell to stay where
    /// it stands -- to wake at `wake`: onto a cell no entity stands on nor
    /// is named by this tick, and the plane holds not, which it then
    /// holds, the cell it left let go. Its cell taken, it stays, and
    /// wakes at `wake` all the same.
    #[inline]
    pub fn step(&self, tick_state: &mut SimulationTickState, entity: &Header, to_cell: CellIndex, wake: u64) {
        if to_cell == entity.stood_cell {
            return entity_reads_and_writes::sleep(tick_state, entity, wake);
        }
        let mut entity_as_seen = compare_and_write::open_compare(tick_state, compare_and_write::entity(entity));
        {
            let mut cell_free_still = compare_and_write::open_compare(&mut entity_as_seen, compare_and_write::cell_free(to_cell)).and_compare(compare_and_write::layer_lacks(self.plane, to_cell));
            cell_free_still.move_entity(entity.stood_cell, to_cell);
            cell_free_still.write_cell(self.plane, to_cell, 1);
            cell_free_still.write_cell(self.plane, entity.stood_cell, 0);
        }
        entity_as_seen.wake_entity(entity.stood_cell, wake);
    }

    /// Queues removing `entity`, the cell it held let go.
    #[inline]
    pub fn remove(&self, tick_state: &mut SimulationTickState, entity: &Header) {
        let mut entity_as_seen = compare_and_write::open_compare(tick_state, compare_and_write::entity(entity));
        entity_as_seen.remove_entity(entity.stood_cell);
        entity_as_seen.write_cell(self.plane, entity.stood_cell, 0);
    }
}
