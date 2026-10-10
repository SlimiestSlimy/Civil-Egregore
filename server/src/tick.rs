//! The world's tick: every rule of the table (`rules::RULES`) on each
//! hot superchunk's turn -- the cells' rules, then the entities', all
//! reading the world as the tick found it -- then the halos moved.

use crate::rules::{with_counts_applied, Chosen, TickCounts};
use crate::{HaloChange, World};
use bitplane_manager::BitmapArena;
use entity_manager::Entities;
use simulation::{Simulation, TickReport};

/// What a world's tick did: the rules, then the halos moved.
#[derive(Clone, Copy, Debug)]
pub struct WorldTick {
    /// What the rules did, and how long each phase took.
    pub rules: TickReport<TickCounts>,
    /// What moving the halos did.
    pub halos: HaloChange,
}

/// One tick of the rules `chosen` over every superchunk with a bitmap
/// in use, on `simulation`'s threads -- `seed`, the world's, seeding a
/// superchunk's random stream the first tick it is in. The halos are
/// not moved.
fn tick_chosen(simulation: &mut Simulation, arena: &mut BitmapArena, entities: &mut Entities, seed: u64, chosen: Chosen) -> TickReport<TickCounts> {
    with_counts_applied(simulation.tick(arena, entities, seed, |turn, samples| chosen.turn(turn, samples)))
}

impl World {
    /// One tick of the rules over the hot superchunks, then the halos
    /// moved to where the hot entities came to.
    pub fn tick(&mut self) -> WorldTick {
        let rules = tick_chosen(&mut self.simulation, &mut self.arena, &mut self.entities, self.info.seed, Chosen::ALL);
        let halos = self.move_halos();
        self.page_cold_pool_out();
        // What a world holds together (`docs/server.md`), checked in a debug build.
        debug_assert_eq!(self.broken_invariant(), None, "after tick {}", self.entities.now());
        WorldTick { rules, halos }
    }

    /// One tick of the rules `chosen` alone over the hot superchunks,
    /// each one's time taken if `timed`, the halos left where they are:
    /// a rule tried or measured by itself.
    pub fn tick_only(&mut self, chosen: Chosen, timed: bool) -> TickReport<TickCounts> {
        let (simulation, seed) = (&mut self.simulation, self.info.seed);
        match timed {
            true => with_counts_applied(simulation.tick(&mut self.arena, &mut self.entities, seed, |turn, samples| chosen.timed_turn(turn, samples))),
            false => tick_chosen(simulation, &mut self.arena, &mut self.entities, seed, chosen),
        }
    }
}
