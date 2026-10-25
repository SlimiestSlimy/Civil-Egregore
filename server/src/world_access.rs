//! A world from outside the server: every part of it read, and
//! changed only through the simulation's own calls, which keep its
//! parts agreeing (`docs/server.md`, "What a world holds together").
//! The server does none of it: each call is the simulation's.

use crate::{World, WorldInfo};
use protocol::kinds::server::handles::BetweenTicks;
use crate::bitplane_manager::{BitmapArena, Write, WritesApplied};
use crate::chunk_storage::{ChunkStorage, LayerType};
use utilities::coordinates::SuperchunkIndex;
use crate::entity_manager::{InstructionsApplied, WorldEntities};
use protocol::server_types::entity::EntityBaseData;
use protocol::server_types::attributes::AttributeBlock;
use protocol::pipe::frame::Viewport;
use crate::simulation::halos::Halos;
use crate::simulation::{ChosenRules, HaloChange, RulePlace, Simulation, TickCounts, TickReport, WorldTick};
use std::collections::BTreeMap;

/// A client's call answered ([`World::answer_call`]).
pub(crate) struct CallAnswered {
    /// The answer: or why the call is refused
    /// (`simulation::rules::call_code_of`), or why the function would
    /// not take its words.
    pub(crate) answer: Result<Vec<u64>, String>,
    /// Whether the call asked for the world to be reset: answered or
    /// not, as what it wrote stays written.
    pub(crate) reset_asked: bool,
}

/// The server between two ticks as a call reaches it: what the call
/// asked of the world, done once it is answered.
struct CallBetweenTicks {
    /// Whether the call asked for the world to be reset.
    reset_asked: bool,
}

impl BetweenTicks for CallBetweenTicks {
    /// Asked: the host resets the world once the call is answered.
    fn reset_world(&mut self) {
        self.reset_asked = true;
    }
}

impl World {
    /// What the world is, and the tick it was made or loaded at.
    ///
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub(crate) fn world_info(&self) -> &WorldInfo {
        &self.world_info
    }

    /// A client's call of the function whose ID is `call`, with
    /// `arguments`, answered between two ticks.
    pub(crate) fn answer_call(&mut self, call: u64, arguments: &[u64]) -> CallAnswered {
        let mut between_ticks = CallBetweenTicks { reset_asked: false };
        let answer = self.client_calls.find(call).and_then(|answer_call| answer_call(&mut between_ticks, arguments));
        CallAnswered { answer, reset_asked: between_ticks.reset_asked }
    }

    /// Its hot bitmaps: every layer type of every hot superchunk.
    ///
    pub(crate) fn bitmap_arena(&self) -> &BitmapArena {
        self.simulation_handle.bitmap_arena()
    }

    /// Every superchunk ever made, as stored.
    ///
    pub(crate) fn chunk_storage(&self) -> &ChunkStorage {
        self.simulation_handle.chunk_storage()
    }

    /// The hot superchunks' entities, at its tick.
    ///
    pub(crate) fn entity_store(&self) -> &WorldEntities {
        self.simulation_handle.entity_store()
    }

    /// The tick's threads, and the hot superchunks' random numbers.
    ///
    pub(crate) fn world_simulation(&self) -> &Simulation {
        self.simulation_handle.world_simulation()
    }

    /// Each cold superchunk's state, as a save keeps it.
    ///
    pub(crate) fn cold(&self) -> &BTreeMap<SuperchunkIndex, Vec<u64>> {
        self.simulation_handle.cold()
    }

    /// Its halos: the superchunks warming and cooling.
    ///
    pub(crate) fn halos(&self) -> &Halos {
        self.simulation_handle.halos()
    }

    /// The superchunks warming, each with the tick it turns hot at.
    pub fn warming(&self) -> impl Iterator<Item = (SuperchunkIndex, u64)> + '_ {
        self.simulation_handle.warming()
    }

    /// The hot superchunks cooling, each with the tick it goes cold at.
    pub fn cooling(&self) -> impl Iterator<Item = (SuperchunkIndex, u64)> + '_ {
        self.simulation_handle.cooling()
    }


    /// The place among its rules of the rule whose type's ID is `rule_id`.
    ///
    /// # Panics
    /// If it has no rule of that ID.
    pub fn rule_place(&self, rule_id: u64) -> RulePlace {
        self.simulation_handle.rule_place(rule_id)
    }

    /// What is wrong between the world's parts, if anything is.
    pub fn broken_invariant(&self) -> Option<String> {
        self.simulation_handle.broken_invariant()
    }

    /// One tick, and the halos moved (`SimulationHandle::tick`).
    pub fn tick(&mut self) -> WorldTick {
        self.simulation_handle.tick()
    }

    /// One tick of the rules `chosen_rules` alone, each one's time taken if
    /// `each_rule_timed`, the halos left where they are (`SimulationHandle::tick_only`).
    pub fn tick_only(&mut self, chosen_rules: ChosenRules, each_rule_timed: bool) -> TickReport<TickCounts> {
        self.simulation_handle.tick_only(chosen_rules, each_rule_timed)
    }

    /// Makes `wanted_hot` -- sorted -- the hot superchunks now.
    pub fn keep_hot(&mut self, wanted_hot: &[SuperchunkIndex]) -> HaloChange {
        self.simulation_handle.keep_hot(wanted_hot)
    }

    /// Keeps the superchunks `viewport` shows hot, if the world's
    /// camera forces them hot: asked for the next time the halos move.
    pub fn keep_viewport(&mut self, viewport: Option<Viewport>) {
        self.simulation_handle.keep_viewport(viewport);
    }

    /// Has it keep `bytes_kept` bytes of its cold superchunks' images
    /// in memory at most, in place of [`crate::COLD_POOL_BYTES_KEPT`].
    pub fn keep_cold_pool_within(&mut self, bytes_kept: u64) {
        self.simulation_handle.keep_cold_pool_within(bytes_kept);
    }

    /// Puts `base_data`'s entity on the world between two ticks: input from
    /// outside, made by the simulation (`SimulationHandle::add_entity`).
    ///
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub(crate) fn add_entity(&mut self, base_data: EntityBaseData, attributes: &[AttributeBlock]) -> InstructionsApplied {
        self.simulation_handle.add_entity(base_data, attributes)
    }

    /// Removes `base_data`'s entity from the world between two ticks.
    ///
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub(crate) fn remove_entity(&mut self, base_data: &EntityBaseData) -> InstructionsApplied {
        self.simulation_handle.remove_entity(base_data)
    }

    /// Applies `writes` to the cells of `layer_type` between two ticks.
    ///
    #[cfg_attr(not(test), allow(dead_code))] // only the crate's tests ask it yet
    pub(crate) fn write_cells(&mut self, layer_type: LayerType, writes: impl IntoIterator<Item = Write>) -> WritesApplied {
        self.simulation_handle.write_cells(layer_type, writes)
    }

    /// Flushes what went cold: each cold superchunk's image its cells.
    pub fn flush_what_went_cold(&mut self) {
        self.simulation_handle.flush_what_went_cold();
    }

    /// Writes back every hot superchunk's changed bitmaps, and flushes.
    pub fn write_back_and_flush_all(&mut self) {
        self.simulation_handle.write_back_and_flush_all();
    }
}
