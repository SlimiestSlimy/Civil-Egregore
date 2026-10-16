//! Civil Egregore's actions: what a rule is made of. Each is one
//! small thing asked or done on a superchunk's turn -- a read, or a
//! change made of several instructions (`../compares_and_writes/`: a
//! compare, a write, the end of a compare); a rule is a few of them
//! put together, and its crate depends on this one alone. A module a
//! subject -- the cells, the entities, the nine cells about one, the
//! area, a mask, walking -- its shape, what reads it and what writes
//! it together.
//!
//! The design: `docs/actions.md`; function by function:
//! `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod area;
pub mod around;
pub mod cells;
pub mod compare;
pub mod entities;
pub mod mask;
pub mod this_tick;
// Composed by the actions, never by a rule: a rule asks what is about a cell, and is not shown the tiles.
pub(crate) mod tiles_about;
pub mod walking;

// The words the actions are asked in: where a cell and an entity
// are, what a layer and an attribute are, a turn, a tick's report, the
// lot drawn.
pub use chunk_storage::{Bits4, LayerType, WidePlane};
pub use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, NEIGHBOUR_OFFSETS, SUPERCHUNK_SIDE_CELLS};
pub use entity_manager::{Attribute, AttributeBlock, AttributeType, EntityRef, EntityType, Header, Layout, Member, WholeAttribute};
pub use simulation::{TickReport, Turn};
pub use utilities::chance::Chance;
pub use utilities::rng::RandomSource;

use std::ops::{AddAssign, Index, IndexMut};

/// Every type a world has, as the type registry has it
/// (`../../type_registry/`): the layers and planes of its cells, the
/// types of its entities and their attributes, what the climate's number
/// is made of -- what a rule reads and writes by name. Nothing is listed
/// here: a type registered is a type a rule may name. A cell with
/// nothing on it is dirt, which has no layer.
pub mod types {
    pub use type_registry::*;
}

/// Simulation time: a rule says how long a thing takes in seconds of
/// the simulation, never in ticks, so it is the same rule at any tick
/// rate (`../../simulation/docs/simulation.md`, "Simulation time").
pub mod simulation_time {
    pub use simulation::simulation_time::{SimulationTime, TICKS_A_SIMULATION_SECOND};
}

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
