//! The world's tick: every rule of the cells and every entity, on each
//! hot superchunk's turn, then the halos moved. So far grass and sheep
//! together: grass spreading and decaying over dirt (`mc_rules::grass`),
//! sheep eating it (`entity_rules::sheep`), one tick running both on
//! each superchunk -- the grass first, then the sheep, all reading the
//! world as the tick found it.

use crate::{HaloChange, World};
use mc_rules::grass::{self, GrassCounts};
use mc_rules::trees::{self, TreeCounts};
use entity_rules::sheep::{self, SheepCounts};
use bitplane_manager::BitmapArena;
use entity_manager::Entities;
use simulation::{Simulation, TickReport};
use std::ops::AddAssign;

/// What grass, trees and sheep did in a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TickCounts {
    /// What the grass did.
    pub grass: GrassCounts,
    /// What the trees did.
    pub trees: TreeCounts,
    /// What the sheep did.
    pub sheep: SheepCounts,
}

impl AddAssign for TickCounts {
    /// Both added up.
    fn add_assign(&mut self, other: Self) {
        self.grass += other.grass;
        self.trees += other.trees;
        self.sheep += other.sheep;
    }
}

/// What a world's tick did: the rules, then the halos moved.
#[derive(Clone, Copy, Debug)]
pub struct WorldTick {
    /// What the rules did, and how long each phase took.
    pub rules: TickReport<TickCounts>,
    /// What moving the halos did.
    pub halos: HaloChange,
}

/// One tick of grass and sheep over every superchunk with a bitmap in
/// use, on `simulation`'s threads -- `seed`, the world's, seeding a
/// superchunk's random stream the first tick it is in. The halos are
/// not moved: for a mock world's superchunks, hot all the while.
pub fn tick_rules(simulation: &mut Simulation, arena: &mut BitmapArena, entities: &mut Entities, seed: u64) -> TickReport<TickCounts> {
    simulation.tick(arena, entities, seed, |turn, samples| TickCounts { grass: grass::rule(turn, samples), trees: trees::rule(turn, samples), sheep: sheep::rule(turn) })
}

impl World {
    /// One tick of the rules over the hot superchunks, then the halos
    /// moved to where their keepers came to.
    pub fn tick(&mut self) -> WorldTick {
        let rules = tick_rules(&mut self.simulation, &mut self.arena, &mut self.entities, self.info.seed);
        WorldTick { rules, halos: self.move_halos() }
    }
}
