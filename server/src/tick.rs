//! The world's tick: every rule of the table (`rules::RULES`) on each
//! hot superchunk's turn -- the cells' rules, then the entities', all
//! reading the world as the tick found it -- then the halos moved.

use crate::rules::{with_counts_applied, ChosenRules, TickCounts};
use crate::{HaloChange, World};
use bitplane_manager::BitmapArena;
use entity_manager::WorldEntities;
use simulation::{Simulation, TickReport};

/// What a world's tick did: the rules, then the halos moved.
#[derive(Clone, Copy, Debug)]
pub struct WorldTick {
    /// What the rules did, and how long each phase took.
    pub rules: TickReport<TickCounts>,
    /// What moving the halos did.
    pub halos: HaloChange,
}

/// One tick of the rules `chosen_rules` over every superchunk with a bitmap
/// in use, on `world_simulation`'s threads -- `seed`, the world's, seeding a
/// superchunk's random stream the first tick it is in. The halos are
/// not moved.
fn tick_chosen(world_simulation: &mut Simulation, bitmap_arena: &mut BitmapArena, entity_store: &mut WorldEntities, seed: u64, chosen_rules: ChosenRules) -> TickReport<TickCounts> {
    with_counts_applied(world_simulation.tick(bitmap_arena, entity_store, seed, |turn| chosen_rules.turn(turn)))
}

impl World {
    /// One tick of the rules over the hot superchunks, then the halos
    /// moved to where the hot entities came to.
    pub fn tick(&mut self) -> WorldTick {
        let rules = tick_chosen(&mut self.world_simulation, &mut self.bitmap_arena, &mut self.entity_store, self.world_info.seed, ChosenRules::EVERY_RULE);
        let halos = self.move_halos();
        self.page_cold_pool_out();
        // What a world holds together (`docs/server.md`), checked in a debug build.
        debug_assert_eq!(self.broken_invariant(), None, "after tick {}", self.entity_store.current_tick());
        WorldTick { rules, halos }
    }

    /// One tick of the rules `chosen_rules` alone over the hot superchunks,
    /// each one's time taken if `each_rule_timed`, the halos left where they are:
    /// a rule tried or measured by itself.
    pub fn tick_only(&mut self, chosen_rules: ChosenRules, each_rule_timed: bool) -> TickReport<TickCounts> {
        let (world_simulation, seed) = (&mut self.world_simulation, self.world_info.seed);
        match each_rule_timed {
            true => with_counts_applied(world_simulation.tick(&mut self.bitmap_arena, &mut self.entity_store, seed, |turn| chosen_rules.timed_turn(turn))),
            false => tick_chosen(world_simulation, &mut self.bitmap_arena, &mut self.entity_store, seed, chosen_rules),
        }
    }
}
