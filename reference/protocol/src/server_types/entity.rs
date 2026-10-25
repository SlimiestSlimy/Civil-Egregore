//! What every entity is, before its attributes: its base data
//! ([`EntityBaseData`]) -- its type, the cell it stands on, when it next
//! wakes -- or never ([`NEVER_WAKES`]). It has no ID: no two stand on one
//! cell, so the cell it stands on names it (`docs/entity_manager.md`,
//! "Instructions", An entity's name in a tick).

use crate::type_registry::EntityType;
use utilities::coordinates::CellIndex;

/// The tick an entity that never wakes wakes at.
pub const NEVER_WAKES: u64 = u64::MAX;

/// An entity's base data: its type and the tick it next wakes at, which
/// every entity has, and the cell it stands on, which names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityBaseData {
    /// Its type.
    pub entity_type: EntityType,
    /// The cell it stands on.
    pub stood_cell: CellIndex,
    /// The tick it next wakes at, or [`NEVER_WAKES`].
    pub wake: u64,
}
