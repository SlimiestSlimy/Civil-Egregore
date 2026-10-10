//! A mock world to tick a rule on: a square of superchunks from the
//! world's middle, each of dirt with grass scattered on it, every chunk
//! of both hot. What holds it -- the hot bitmaps, the entities, the
//! storage, the simulation -- is its own: a rule's tests and tools ask
//! it in the instructions' terms.

use crate::between_ticks::EntitiesBetweenTicks;
use crate::layers::{DIRT, GRASS};
use crate::{TickReport, Turn};
use bitplane_manager::{BitmapArena, Shape, Write, WriteOp};
use chunk_storage::mock::grass_on_dirt;
use chunk_storage::{ChunkStorage, LayerCodec, LayerType};
use coordinates::{square_from_middle, CellCartesian, CellIndex, ChunkIndex, SuperchunkIndex};
use entity_manager::{Attribute, Entities, EntityRef, EntityType, Header};
use simulation::Simulation;
use std::ops::AddAssign;

/// A mock world: superchunks of grass on dirt, all hot, and whatever
/// entities are put on them.
pub struct MockWorld {
    /// The hot bitmaps.
    arena: BitmapArena,
    /// The entities, holding the same superchunks as the arena.
    entities: Entities,
    /// The superchunks as stored.
    storage: ChunkStorage,
    /// What ticks it.
    simulation: Simulation,
    /// The superchunks, row by row.
    superchunks: Vec<SuperchunkIndex>,
}

impl MockWorld {
    /// `count` superchunks in a square, row by row, each with grass drawn
    /// on `grass_cells` cells -- fewer where a cell is drawn twice --
    /// ticked on one thread.
    pub fn grass_on_dirt(count: u32, grass_cells: usize) -> Self {
        let (mut codec, mut arena, mut storage) = (LayerCodec::new(), BitmapArena::new(), ChunkStorage::new(1 << 16));
        let superchunks: Vec<SuperchunkIndex> = square_from_middle(count).collect();
        for (seed, &superchunk) in superchunks.iter().enumerate() {
            storage.insert(superchunk, grass_on_dirt(seed as u64 + 1, grass_cells, &mut codec));
            arena.make_hot_superchunk(superchunk, &[DIRT, GRASS], &storage, &mut codec);
        }
        let mut entities = Entities::new();
        entities.align(&arena.superchunk_indices());
        Self { arena, entities, storage, simulation: Simulation::new(1), superchunks }
    }

    /// The same world, ticked on `threads` threads.
    pub fn on_threads(mut self, threads: usize) -> Self {
        self.simulation = Simulation::new(threads);
        self
    }

    /// Its superchunks, row by row.
    pub fn superchunks(&self) -> &[SuperchunkIndex] {
        &self.superchunks
    }

    /// One tick of `rule` over every superchunk: the first phase runs it
    /// on each one's turn -- with room for samples -- and the second
    /// applies what it queued. `seed` seeds a superchunk's random
    /// numbers the first tick it is in.
    pub fn tick<R, F>(&mut self, seed: u64, rule: F) -> TickReport<R>
    where
        R: Default + AddAssign + Send,
        F: Fn(&mut Turn, &mut Vec<CellIndex>) -> R + Sync,
    {
        self.simulation.tick(&mut self.arena, &mut self.entities, seed, rule)
    }

    /// Turns to grass the rectangle of cells `width` by `height` whose
    /// top left is `at`, all of it on the world's superchunks.
    pub fn plant_grass(&mut self, at: CellCartesian, width: u8, height: u8) {
        let shape = if (width, height) == (1, 1) { Shape::Cell } else { Shape::Rect { width, height } };
        self.arena.queue(GRASS, Write { at: at.into(), op: WriteOp::Set, shape });
        self.arena.queue(DIRT, Write { at: at.into(), op: WriteOp::Unset, shape });
        assert_eq!(self.arena.apply().missed, 0, "grass planted off the world's superchunks");
    }

    /// Cells of `layer_type` over every superchunk.
    pub fn count(&self, layer_type: LayerType) -> u64 {
        self.superchunks.iter().map(|&superchunk| self.arena.superchunk_count(layer_type, superchunk) as u64).sum()
    }

    /// Every chunk's cells of `layer_type`, as its bitmap's words, the
    /// chunks in Morton order.
    pub fn words(&self, layer_type: LayerType) -> Vec<(ChunkIndex, Vec<u64>)> {
        self.arena.run(layer_type).map(|(chunk, bucket)| (chunk, bucket.cells().to_vec())).collect()
    }

    /// Puts an entity on it, whole: there before the next tick.
    pub fn put(&mut self, header: Header, attributes: &[Attribute]) {
        self.between_ticks().put(header, attributes);
        self.settle();
    }

    /// Its entities, to put more on between two ticks: there once
    /// settled ([`MockWorld::settle`]).
    pub fn between_ticks(&mut self) -> EntitiesBetweenTicks<'_> {
        EntitiesBetweenTicks::of(&mut self.entities)
    }

    /// Places the entities put between two ticks.
    pub fn settle(&mut self) {
        self.entities.apply();
    }

    /// Every entity on it, in the world's order.
    pub fn entities(&self) -> impl Iterator<Item = EntityRef<'_>> {
        self.entities.iter()
    }

    /// Entities of `kind` over every superchunk.
    pub fn count_entities(&self, kind: EntityType) -> usize {
        self.entities().filter(|entity| entity.header.kind == kind).count()
    }

    /// What holds it -- the hot bitmaps, the entities, the storage -- for
    /// whoever measures what a world takes: nothing a rule asks.
    pub fn held(&self) -> (&BitmapArena, &Entities, &ChunkStorage) {
        (&self.arena, &self.entities, &self.storage)
    }
}
