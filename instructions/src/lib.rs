//! Civil Egregore's instructions: what a rule is made of. Each is one
//! small thing asked or done on a superchunk's turn; a rule is a few
//! of them put together, and its crate depends on this one alone. A
//! module a subject -- the cells, the entities, the nine cells about
//! one, the area, a mask, walking -- its shape, what reads it and what
//! writes it together.
//!
//! The design: `docs/instructions.md`; function by function:
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
pub mod walking;

// The words the instructions are asked in: where a cell and an entity
// are, what a layer and an attribute are, a turn, a tick's report, the
// lot drawn.
pub use chunk_storage::{Bits4, LayerType, Wide};
pub use coordinates::{CellCartesian, CellIndex, SuperchunkIndex, NEIGHBOURS, SUPERCHUNK_SIDE_CELLS};
pub use entity_manager::{Attribute, AttributeBlock, AttributeType, EntityEdit, EntityId, EntityRef, EntityType, Header, Layout};
pub use simulation::{TickReport, Turn};
pub use utilities::chance::Chance;
pub use utilities::rng::Rng;

use std::ops::{AddAssign, Index, IndexMut};

/// The layers a world's cells have, before any rule adds its own: what
/// a rule reads and writes by name, from the type registry
/// (`../../type_registry/`). A cell with nothing on it is dirt, which
/// has no layer.
pub mod layers {
    pub use type_registry::{GRASS, OLDEST_TREE_STAGE, TREE, TREE_STAGE, WALL_EAST, WALL_SOUTH, WET};
}

/// The types of entity a world has and the attributes they carry: what
/// an entity's rule names its own by, from the type registry.
pub mod entity_types {
    pub use type_registry::{Roaming, BEARING, HUNGRY_AT, LAMB, PREGNANT, ROAMING, SHEEP};
}

/// The most counts a rule keeps.
pub const COUNTS_OF_A_RULE: usize = 8;

/// The place of the count named `name` among `counted`, the names of
/// what a rule counts in the order it keeps them: the one order, from
/// which each place is worked out as the rule is compiled.
///
/// # Panics
/// If nothing is named so -- for a constant, as it is compiled.
pub const fn place_counted(counted: &[&str], name: &str) -> usize {
    let mut place = 0;
    while place < counted.len() {
        let (one, other) = (counted[place].as_bytes(), name.as_bytes());
        let mut same = one.len() == other.len();
        let mut byte = 0;
        while same && byte < one.len() {
            same = one[byte] == other[byte];
            byte += 1;
        }
        if same {
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
    fn add_assign(&mut self, other: Self) {
        for (count, more) in self.0.iter_mut().zip(other.0) {
            *count += more;
        }
    }
}
