//! Which superchunks are to be hot: the world's size, the hot entity
//! whose halos are, and whether the viewport's are too
//! (`docs/simulation.md`, "Halos").

use utilities::coordinates::{SuperchunkIndex, WORLD_MIDDLE, WORLD_SIDE_SUPERCHUNKS};
use crate::entity_manager::{WorldEntities, EntityType};

/// Which superchunks are to be hot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hot {
    /// Every entity of the kind `entity` keeps its halo hot -- and, if
    /// `viewport`, the viewport's superchunks too -- in a world of
    /// `world_side_superchunks` superchunks a side, or as far as coordinates reach.
    AboutHotEntities {
        /// The hot entity.
        entity: EntityType,
        /// Superchunks along a side of the world, if it has a size.
        world_side_superchunks: Option<u32>,
        /// Whether the viewport's superchunks are hot too.
        viewport: bool,
    },
    /// Every superchunk of a world `world_side_superchunks` superchunks along a side is
    /// hot, whatever its entities do: a fixed load. Only a world of a
    /// size can be forced: one of none has no end to be hot to.
    Forced {
        /// Superchunks along a side of the world.
        world_side_superchunks: u32,
    },
}

impl Hot {
    /// A world of no size, hot about every entity of the kind `entity`
    /// alone.
    pub const fn about_entity(entity: EntityType) -> Self {
        Self::AboutHotEntities { entity, world_side_superchunks: None, viewport: false }
    }

    /// Whether the viewport's superchunks are hot too: never forced
    /// hot, the whole of which is.
    pub const fn viewport(&self) -> bool {
        matches!(*self, Self::AboutHotEntities { viewport: true, .. })
    }

    /// Superchunks along a side of the world, if it has a size.
    pub const fn world_side_superchunks(&self) -> Option<u32> {
        match *self {
            Self::AboutHotEntities { world_side_superchunks: side, .. } => side,
            Self::Forced { world_side_superchunks: side } => Some(side),
        }
    }

    /// The world's first column and row and the one past its last, of
    /// superchunks: both the same, the world being square.
    pub fn world_span_cartesian(&self) -> std::ops::Range<u32> {
        let side_superchunks = self.world_side_superchunks().unwrap_or(WORLD_SIDE_SUPERCHUNKS).min(WORLD_SIDE_SUPERCHUNKS);
        let first_superchunk_cartesian = WORLD_MIDDLE.cartesian().0 - side_superchunks / 2;
        first_superchunk_cartesian..first_superchunk_cartesian + side_superchunks
    }

    /// Whether `superchunk` is of the world.
    pub fn is_within_world(&self, superchunk: SuperchunkIndex) -> bool {
        let ((x, y), world_span_cartesian) = (superchunk.cartesian(), self.world_span_cartesian());
        world_span_cartesian.contains(&x) && world_span_cartesian.contains(&y)
    }

    /// Every superchunk of a world of a size, sorted; none of one of no
    /// size, which has no end.
    pub fn all_superchunks(&self) -> Vec<SuperchunkIndex> {
        if self.world_side_superchunks().is_none() {
            return Vec::new();
        }
        let world_span_cartesian = self.world_span_cartesian();
        let mut world: Vec<SuperchunkIndex> = world_span_cartesian.clone().flat_map(|y| world_span_cartesian.clone().map(move |x| SuperchunkIndex::from_cartesian(x, y))).collect();
        world.sort_unstable();
        world
    }

    /// The superchunks to be hot, sorted, each once: the hot entities'
    /// halos, or every superchunk of the world if they are forced --
    /// none outside the world either way.
    pub fn wanted_hot_superchunks(&self, entity_store: &WorldEntities) -> Vec<SuperchunkIndex> {
        let mut wanted_superchunks = match *self {
            Self::AboutHotEntities { entity, .. } => halos_about(entity_store.superchunks().iter().filter(|superchunk| superchunk.iter().any(|kept_entity| kept_entity.header.entity_type == entity)).map(|superchunk| superchunk.superchunk_index())),
            Self::Forced { .. } => self.all_superchunks(),
        };
        wanted_superchunks.retain(|&superchunk| self.is_within_world(superchunk));
        wanted_superchunks
    }
}

/// The halos about the superchunks `halo_middles` -- each one's own and its
/// eight neighbours' -- as superchunk indices, sorted, each once.
pub fn halos_about(halo_middles: impl Iterator<Item = SuperchunkIndex>) -> Vec<SuperchunkIndex> {
    let mut halos: Vec<SuperchunkIndex> = halo_middles.flat_map(|halo_middle| (-1..=1).flat_map(move |dy| (-1..=1).filter_map(move |dx| halo_middle.at_cartesian_offset(dx, dy)))).collect();
    halos.sort_unstable();
    halos.dedup();
    halos
}
