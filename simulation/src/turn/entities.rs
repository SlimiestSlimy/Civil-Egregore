//! The entities in a turn: those woken, any in reach read, and the
//! instructions queued for them.

use super::Turn;
use entity_manager::{EntityRef, SuperchunkEntities, OCCUPIED_SIDE};
use bitplane_manager::Reader;
use chunk_storage::LayerType;
use coordinates::{CellIndex, ChunkIndex};

impl<'a> Turn<'a> {
    /// The superchunk's entities waking this tick, as the tick found
    /// them, in Morton order by cell -- the order their buckets hold
    /// them in. Each that is to wake
    /// again must be put back with a later wake tick.
    pub fn woken(&self) -> impl Iterator<Item = EntityRef<'a>> + 'a {
        let entities: &'a SuperchunkEntities = self.entities;
        entities.woken(self.now)
    }

    /// [`Turn::woken`], for a rule that reads the cells of
    /// `layers` about each entity woken: they are asked of memory a few
    /// wakes ahead.
    pub fn woken_reading<const N: usize>(&self, layers: [LayerType; N]) -> impl Iterator<Item = EntityRef<'a>> + 'a {
        let (entities, reader): (&'a SuperchunkEntities, &'a Reader<'a>) = (self.entities, self.reader);
        entities.woken_prefetching(self.now, move |cell| layers.iter().for_each(|&layer_type| reader.prefetch(layer_type, cell)))
    }

    /// The entity standing on `at` -- in any hot superchunk -- as the
    /// tick found it: the cell is its name for the tick.
    pub fn entity_on(&self, at: CellIndex) -> Option<EntityRef<'a>> {
        self.entity_reader.get(at)
    }

    /// The entities on `chunk` -- in any hot superchunk -- as the tick
    /// found them, in Morton order by cell; `None` if its
    /// superchunk is not hot.
    pub fn entities_in(&self, chunk: ChunkIndex) -> Option<impl Iterator<Item = EntityRef<'a>> + 'a> {
        self.entity_reader.chunk(chunk)
    }

    /// The cells entities stand on among the `width` by `height` cells
    /// (each up to [`OCCUPIED_SIDE`]) from `origin`, as the tick found
    /// them: cell `(x, y)` at bit `x` of row `y`
    /// (`docs/simulation.md`, "Entities").
    pub fn occupied(&self, origin: CellIndex, width: u32, height: u32) -> [u16; OCCUPIED_SIDE] {
        self.entity_reader.occupied(origin, width, height)
    }
}
