//! The world's tick: every rule on each hot superchunk's turn -- the
//! cells' rules, then the entities', all reading the world as the tick
//! found it -- then the halos moved, and the cold pool paged.

use super::SimulationHandle;
use crate::simulation::halos::HaloChange;
use crate::simulation::rules::{with_counts_applied, ChosenRules, TickCounts};
use crate::simulation::TickReport;

/// What a world's tick did: the rules, then the halos moved.
#[derive(Clone, Copy, Debug)]
pub struct WorldTick {
    /// What the rules did, and how long each phase took.
    pub rules: TickReport<TickCounts>,
    /// What moving the halos did.
    pub halos: HaloChange,
}

impl SimulationHandle {
    /// One tick of the rules over the hot superchunks, then the halos
    /// moved to where the hot entities came to.
    pub fn tick(&mut self) -> WorldTick {
        let rules = self.tick_only(ChosenRules::EVERY_RULE, false);
        let halos = self.move_halos();
        self.page_cold_pool_out();
        debug_assert_eq!(self.broken_invariant(), None, "after tick {}", self.entity_store.current_tick());
        WorldTick { rules, halos }
    }

    /// One tick of the rules `chosen_rules` alone over the hot superchunks,
    /// each one's time taken if `each_rule_timed`, the halos left where they are:
    /// a rule tried or measured by itself.
    pub fn tick_only(&mut self, chosen_rules: ChosenRules, each_rule_timed: bool) -> TickReport<TickCounts> {
        let (seed, rules) = (self.seed, &self.rules);
        let report = self.world_simulation.tick(&mut self.bitmap_arena, &mut self.entity_store, seed, |tick_state| match each_rule_timed {
            true => chosen_rules.timed_turn(rules, tick_state),
            false => chosen_rules.tick_state(rules, tick_state),
        });
        with_counts_applied(report)
    }
}
