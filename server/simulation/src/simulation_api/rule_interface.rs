//! A rule as the simulation runs it ([`RuleCode`]): registered with the
//! rule's type, given on each superchunk's turn what it runs with
//! ([`RuleContext`]) -- its master, and the libraries of the types its
//! master depends on. A rule of the cells runs on each sample of its
//! master ([`on_each_sampled`]).

use crate::simulation_api::RuleCounts;
use utilities::coordinates::CellIndex;
use crate::SimulationTickState;
use type_registry::{EntityType, LayerType, Library};
use utilities::chance::Chance;

/// What a rule is of: its master, as the world's types say it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleMaster {
    /// A layer: a rule of the cells, sampling its set cells.
    Layer(LayerType),
    /// A wide plane: a rule of the cells, sampling its every tile.
    WidePlane(LayerType),
    /// A type of entity: a rule of its entities.
    Entity(EntityType),
}

/// What a rule runs with, gathered once as its world starts: its master,
/// and the library of each type its master depends on, by the type's
/// ID.
#[derive(Clone)]
pub struct RuleContext {
    /// Its master.
    pub master: RuleMaster,
    /// The libraries of the types its master depends on, each with the
    /// type's ID.
    pub libraries: Box<[(u64, Library)]>,
}

impl std::fmt::Debug for RuleContext {
    /// Its master, and the IDs of the types whose libraries it holds: what
    /// a library is, only its rule knows.
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        let library_ids: Vec<u64> = self.libraries.iter().map(|(type_id, _)| *type_id).collect();
        formatter.debug_struct("RuleContext").field("master", &self.master).field("libraries", &library_ids).finish()
    }
}

impl RuleContext {
    /// The library of the type of ID `type_id` its master depends on, as
    /// the rule knows it, `L`: none if its master depends on no such
    /// type, or the library is not an `L`.
    pub fn library<L: 'static>(&self, type_id: u64) -> Option<&L> {
        self.libraries.iter().find(|(library_type_id, _)| *library_type_id == type_id).and_then(|(_, library)| library.downcast_ref())
    }
}

/// What runs a rule: what it counts, each named at its place in its
/// counts, and the rule on one superchunk's turn, with what it runs
/// with.
#[derive(Clone, Copy, Debug)]
pub struct RuleCode {
    /// What it counts.
    pub count_names: &'static [&'static str],
    /// The rule, on one superchunk's turn.
    pub run: fn(&mut SimulationTickState, &RuleContext) -> RuleCounts,
}

/// One superchunk's turn of a rule of the cells: `rule`, the function on
/// one sample, run by the simulation on each its master has in the
/// superchunk, chosen with `chance` -- its set cells if it is a layer,
/// its every tile if it is a wide plane.
///
/// # Panics
/// If its master is a type of entity.
#[inline]
pub fn on_each_sampled(tick_state: &mut SimulationTickState, master: RuleMaster, chance: Chance, rule: impl FnMut(&mut SimulationTickState, CellIndex, &mut RuleCounts)) -> RuleCounts {
    let mut counts = RuleCounts::default();
    match master {
        RuleMaster::Layer(layer_type) => tick_state.each_sampled(layer_type, chance, &mut counts, rule),
        RuleMaster::WidePlane(layer_type) => tick_state.each_tile_sampled(layer_type, chance, &mut counts, rule),
        RuleMaster::Entity(_) => panic!("a rule of the cells is of a plane"),
    };
    counts
}
