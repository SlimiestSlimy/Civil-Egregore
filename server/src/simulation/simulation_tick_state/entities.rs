//! The entities in a turn: those woken, any in reach read, and the
//! instructions queued for them.

use super::SimulationTickState;
use crate::entity_manager::{EntityRef, SuperchunkEntities};
use crate::bitplane_manager::CellReader;
use crate::chunk_storage::LayerType;
use utilities::coordinates::{CellIndex, ChunkIndex};

impl<'a> SimulationTickState<'a> {
    /// The superchunk's entities waking this tick, as the tick found
    /// them, in Morton order by cell -- the order their buckets hold
    /// them in. Each that is to wake
    /// again must be put back with a later wake tick.
    pub fn woken_entities(&self) -> impl Iterator<Item = EntityRef<'a>> + 'a {
        let superchunk_entities: &'a SuperchunkEntities = self.superchunk_entities;
        superchunk_entities.woken_entities(self.current_tick)
    }

    /// [`SimulationTickState::woken_entities`], for a rule that reads the cells of
    /// `layers` about each entity woken: they are asked of memory a few
    /// wakes ahead.
    pub fn woken_reading<const N: usize>(&self, layers: [LayerType; N]) -> impl Iterator<Item = EntityRef<'a>> + 'a {
        let (superchunk_entities, bitmap_reader): (&'a SuperchunkEntities, &'a CellReader<'a>) = (self.superchunk_entities, self.bitmap_reader);
        superchunk_entities.woken_prefetching(self.current_tick, move |cell| layers.iter().for_each(|&layer_type| bitmap_reader.prefetch_cell(layer_type, cell)))
    }

    /// The entity standing on `named_cell` -- in any hot superchunk -- as the
    /// tick found it: the cell is its name for the tick.
    pub fn entity_on(&self, named_cell: CellIndex) -> Option<EntityRef<'a>> {
        self.entity_reader.get(named_cell)
    }

    /// The entities on `chunk` -- in any hot superchunk -- as the tick
    /// found them, in Morton order by cell; `None` if its
    /// superchunk is not hot.
    pub fn entities_in(&self, chunk: ChunkIndex) -> Option<impl Iterator<Item = EntityRef<'a>> + 'a> {
        self.entity_reader.chunk(chunk)
    }
}
