//! A world as the simulation holds it ([`SimulationHandle`]): every part
//! of it, hot and cold, and what it runs with -- its rules, what
//! generates it, what it is hot about -- each found by its types'
//! hooks. Ticked whole (`ticking.rs`); changed between two ticks only
//! through its own calls (`between_ticks.rs`), which whoever runs it
//! makes for its user and its loads; its halos moved by it alone
//! (`halo_work.rs`).

mod between_ticks;
mod halo_work;
mod ticking;

pub use between_ticks::SavedImage;
pub use ticking::WorldTick;

use crate::simulation::halos::Halos;
use crate::simulation::hot::Hot;
use crate::simulation::rules::{hot_entity_of, place_of, rules_of, Rule, RulePlace};
use crate::simulation::simulation_api::SetUpWorldgen;
use crate::simulation::Simulation;
use crate::bitplane_manager::BitmapArena;
use crate::chunk_storage::disk::WorldInfo;
use crate::chunk_storage::{ChunkStorage, LayerType};
use utilities::coordinates::SuperchunkIndex;
use crate::entity_manager::WorldEntities;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use protocol::type_registry::FrozenTypeRegistry;
use utilities::dispatcher::Dispatcher;

/// Where a world pages its cold superchunks' images, and how many bytes
/// of them it keeps in memory before it does.
#[derive(Clone, Debug)]
pub struct ColdPoolPaging {
    /// The folder paged into, gone with the world.
    pub folder: PathBuf,
    /// Bytes of images kept in memory at most.
    pub bytes_kept: u64,
}

/// A world as the simulation holds it: its superchunks hot about the
/// entities that keep a halo, the rest cold.
pub struct SimulationHandle {
    /// The seed its superchunks are generated from.
    seed: u64,
    /// Its layer types: made hot on every chunk.
    layers: Vec<LayerType>,
    /// Its hot bitmaps: every layer type of every hot superchunk.
    bitmap_arena: BitmapArena,
    /// Every superchunk ever made, as stored: the cold ones' cells, and
    /// the hot ones' as last written back.
    chunk_storage: ChunkStorage,
    /// The hot superchunks' entities, at its tick.
    entity_store: WorldEntities,
    /// The tick's threads, and the hot superchunks' random numbers.
    world_simulation: Simulation,
    /// Each cold superchunk's state -- its entities and random numbers
    /// -- as a save keeps it.
    cold: BTreeMap<SuperchunkIndex, Vec<u64>>,
    /// Its halos: the superchunks warming and cooling, and the jobs
    /// making them.
    halos: Halos,
    /// Its rules, in the order a turn runs them.
    rules: Arc<[Rule]>,
    /// What generates it, set up from its config.
    worldgen: SetUpWorldgen,
}

impl SimulationHandle {
    /// The world `world_info` says, generated as `worldgen` says,
    /// with nothing in it yet, nothing hot, at its tick: what running it
    /// builds on. Of `frozen_type_registry`, whose hooks give its rules and
    /// what it is hot about -- or why it cannot run. On `thread_count`
    /// threads, every one the machine has if none is given; its cold pool
    /// paged as `cold_pool_paging` says.
    pub fn new(world_info: &WorldInfo, worldgen: SetUpWorldgen, frozen_type_registry: &FrozenTypeRegistry, thread_count: Option<usize>, cold_pool_paging: ColdPoolPaging) -> Result<Self, String> {
        let hot = match (world_info.side_in_superchunks, world_info.forced_hot) {
            (Some(side_superchunks), true) => Hot::Forced { world_side_superchunks: side_superchunks },
            (side_if_sized, _) => Hot::AboutHotEntities { entity: hot_entity_of(frozen_type_registry)?, world_side_superchunks: side_if_sized, viewport: world_info.camera_forces_superchunks_hot },
        };
        // The world's superchunks not counted, as it grows: one set of threads, the tick's and chunk storage's jobs' alike.
        let dispatcher = Arc::new(thread_count.map_or_else(Dispatcher::of_the_machine, Dispatcher::new));
        let mut chunk_storage = ChunkStorage::new(1 << 16);
        chunk_storage.page_under(cold_pool_paging.folder, cold_pool_paging.bytes_kept);
        Ok(Self {
            seed: world_info.seed,
            layers: frozen_type_registry.layer_types(),
            bitmap_arena: BitmapArena::new(),
            chunk_storage,
            entity_store: WorldEntities::for_types(frozen_type_registry, world_info.tick),
            world_simulation: Simulation::on_dispatcher(Arc::clone(&dispatcher)),
            cold: BTreeMap::new(),
            halos: Halos::new(hot, dispatcher),
            rules: rules_of(frozen_type_registry)?,
            worldgen,
        })
    }

    /// The seed its superchunks are generated from.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Its hot bitmaps: every layer type of every hot superchunk.
    pub fn bitmap_arena(&self) -> &BitmapArena {
        &self.bitmap_arena
    }

    /// Every superchunk ever made, as stored.
    pub fn chunk_storage(&self) -> &ChunkStorage {
        &self.chunk_storage
    }

    /// The hot superchunks' entities, at its tick.
    pub fn entity_store(&self) -> &WorldEntities {
        &self.entity_store
    }

    /// The tick's threads, and the hot superchunks' random numbers.
    pub fn world_simulation(&self) -> &Simulation {
        &self.world_simulation
    }

    /// Each cold superchunk's state, as a save keeps it.
    pub fn cold(&self) -> &BTreeMap<SuperchunkIndex, Vec<u64>> {
        &self.cold
    }

    /// Its halos: the superchunks warming and cooling.
    pub fn halos(&self) -> &Halos {
        &self.halos
    }


    /// The place among its rules of the rule whose type's ID is `rule_id`.
    ///
    /// # Panics
    /// If it has no rule of that ID.
    pub fn rule_place(&self, rule_id: u64) -> RulePlace {
        place_of(&self.rules, rule_id).unwrap_or_else(|| panic!("no rule of ID {rule_id:016x}"))
    }
}
