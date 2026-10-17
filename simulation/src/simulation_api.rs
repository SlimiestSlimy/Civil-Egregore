//! The simulation's API, what the base game is written in: every
//! read a rule makes of its turn, and every change made of several
//! compares and writes, each a function over the turn. A rule is a
//! few of them put together, and its interface to the simulation
//! ([`RuleCode`]) is here too. No type of the game is named here. A
//! module a subject -- its shape, what reads it and what writes it
//! together.

pub mod square_around_a_cell;
pub mod cell_and_its_neighbours;
pub mod cell_reads_and_writes;
pub mod compare_and_write;
pub mod entity_reads_and_writes;
pub mod tile_masks;
mod rule_interface;
pub mod tick_and_random_stream;
// Composed by the actions, never by a rule: a rule asks what is about a cell, and is not shown the tiles.
pub(crate) mod four_tiles_under_the_neighbours;
pub mod steps_and_paths;

// The words the actions are asked in: where a cell and an entity
// are, what a layer and an attribute are, a turn, a tick's report, the
// lot drawn.
pub use chunk_storage::{Bits4, LayerType, WidePlane};
pub use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, NEIGHBOUR_OFFSETS, SUPERCHUNK_SIDE_CELLS};
pub use entity_manager::{Attribute, AttributeBlock, AttributeType, EntityRef, EntityType, Header, Layout, Member, WholeAttribute};
pub use rule_interface::{on_each_sampled, RuleCode, RuleContext, RuleMaster};
pub use crate::{TickReport, SuperchunkTickState};
pub use utilities::chance::Chance;
pub use utilities::rng::RandomSource;

use std::ops::{AddAssign, Index, IndexMut};

// Simulation time: a rule says how long a thing takes in seconds of
// the simulation, never in ticks, so it is the same rule at any tick
// rate (`docs/simulation.md`, "Simulation time").
pub use crate::simulation_time;

/// The most counts a rule keeps.
pub const COUNTS_OF_A_RULE: usize = 8;

/// The place of the count named `count_name` among `count_names`, the names of
/// what a rule counts in the order it keeps them: the one order, from
/// which each place is worked out as the rule is compiled.
///
/// # Panics
/// If nothing is named so -- for a constant, as it is compiled.
pub const fn place_counted(count_names: &[&str], count_name: &str) -> usize {
    let mut place = 0;
    while place < count_names.len() {
        let (listed_bytes, name_bytes) = (count_names[place].as_bytes(), count_name.as_bytes());
        let mut bytes_same = listed_bytes.len() == name_bytes.len();
        let mut byte_index = 0;
        while bytes_same && byte_index < listed_bytes.len() {
            bytes_same = listed_bytes[byte_index] == name_bytes[byte_index];
            byte_index += 1;
        }
        if bytes_same {
            return place;
        }
        place += 1;
    }
    panic!("the rule counts nothing named so")
}

/// What a rule did, on a turn or added up over many: its counts, each
/// at the place the rule names it by, the rest 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleCounts(pub [u64; COUNTS_OF_A_RULE]);

impl Index<usize> for RuleCounts {
    type Output = u64;

    /// The count at `place`.
    #[inline(always)]
    fn index(&self, place: usize) -> &u64 {
        &self.0[place]
    }
}

impl IndexMut<usize> for RuleCounts {
    /// The count at `place`, to add to.
    #[inline(always)]
    fn index_mut(&mut self, place: usize) -> &mut u64 {
        &mut self.0[place]
    }
}

impl AddAssign for RuleCounts {
    /// Each count added to its like.
    #[inline]
    fn add_assign(&mut self, counts_added: Self) {
        for (count, count_added) in self.0.iter_mut().zip(counts_added.0) {
            *count += count_added;
        }
    }
}
