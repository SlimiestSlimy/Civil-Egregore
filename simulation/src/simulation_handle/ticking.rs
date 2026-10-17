//! The world's tick: every rule on each hot superchunk's turn -- the
//! cells' rules, then the entities', all reading the world as the tick
//! found it -- a superchunk not yet populated populated by the world's
//! generation first; then the halos moved, and the cold pool paged.

use super::SimulationHandle;
use crate::halos::HaloChange;
use crate::rules::{with_counts_applied, ChosenRules, TickCounts};
use crate::TickReport;
use coordinates::SuperchunkIndex;

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
    /// moved to where the hot entities came to, each superchunk they
    /// generated left to be populated its first tick hot.
    pub fn tick(&mut self) -> WorldTick {
        let rules = self.tick_only(ChosenRules::EVERY_RULE, false);
        let halos = self.move_halos();
        self.page_cold_pool_out();
        debug_assert_eq!(self.broken_invariant(), None, "after tick {}", self.entity_store.current_tick());
        WorldTick { rules, halos }
    }

    /// One tick of the rules `chosen_rules` alone over the hot superchunks,
    /// each one's time taken if `each_rule_timed`, the halos left where they are:
    /// a rule tried or measured by itself. A superchunk not yet populated
    /// is populated on its turn first, by the world's generation: the tick
    /// makes what lives there, under compares as a rule does.
    pub fn tick_only(&mut self, chosen_rules: ChosenRules, each_rule_timed: bool) -> TickReport<TickCounts> {
        let (seed, worldgen, rules, generation_numbers, unpopulated) = (self.seed, self.worldgen, &self.rules, &self.generation_numbers, &self.unpopulated);
        // Those hot now have their turn: each of them unpopulated is populated on it.
        let hot_now = (!unpopulated.is_empty()).then(|| self.bitmap_arena.superchunk_indices());
        let report = self.world_simulation.tick(&mut self.bitmap_arena, &mut self.entity_store, seed, |tick_state| {
            if unpopulated.binary_search(&tick_state.superchunk().0).is_ok() {
                (worldgen.populate)(tick_state, generation_numbers, seed);
            }
            match each_rule_timed {
                true => chosen_rules.timed_turn(rules, tick_state),
                false => chosen_rules.tick_state(rules, tick_state),
            }
        });
        if let Some(hot_now) = hot_now {
            self.unpopulated.retain(|&superchunk| hot_now.binary_search(&SuperchunkIndex(superchunk)).is_err());
        }
        with_counts_applied(report)
    }

    /// Keeps `superchunks` as unpopulated: each populated by the world's
    /// generation the first tick it is hot.
    pub fn mark_unpopulated(&mut self, superchunks: &[SuperchunkIndex]) {
        self.unpopulated.extend(superchunks.iter().map(|superchunk| superchunk.0));
        self.unpopulated.sort_unstable();
        self.unpopulated.dedup();
    }
}
