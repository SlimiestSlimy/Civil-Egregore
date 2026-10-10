//! Which superchunks are to be hot: the world's size, the hot entity
//! whose halos are, and whether the viewport's are too
//! (`docs/simulation.md`, "Halos").

use coordinates::{SuperchunkIndex, WORLD_MIDDLE, WORLD_SIDE_SUPERCHUNKS};
use entity_manager::{Entities, EntityType};

/// Which superchunks are to be hot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hot {
    /// Every entity of the kind `entity` keeps its halo hot -- and, if
    /// `viewport`, the viewport's superchunks too -- in a world of
    /// `side` superchunks a side, or as far as coordinates reach.
    About {
        /// The hot entity.
        entity: EntityType,
        /// Superchunks along a side of the world, if it has a size.
        side: Option<u32>,
        /// Whether the viewport's superchunks are hot too.
        viewport: bool,
    },
    /// Every superchunk of a world `side` superchunks along a side is
    /// hot, whatever its entities do: a fixed load. Only a world of a
    /// size can be forced: one of none has no end to be hot to.
    Forced {
        /// Superchunks along a side of the world.
        side: u32,
    },
}

impl Hot {
    /// A world of no size, hot about every entity of the kind `entity`
    /// alone.
    pub const fn about(entity: EntityType) -> Self {
        Self::About { entity, side: None, viewport: false }
    }

    /// Whether the viewport's superchunks are hot too: never forced
    /// hot, the whole of which is.
    pub const fn viewport(&self) -> bool {
        matches!(*self, Self::About { viewport: true, .. })
    }

    /// Superchunks along a side of the world, if it has a size.
    pub const fn side(&self) -> Option<u32> {
        match *self {
            Self::About { side, .. } => side,
            Self::Forced { side } => Some(side),
        }
    }

    /// The world's first column and row and the one past its last, of
    /// superchunks: both the same, the world being square.
    pub fn span(&self) -> std::ops::Range<u32> {
        let side = self.side().unwrap_or(WORLD_SIDE_SUPERCHUNKS).min(WORLD_SIDE_SUPERCHUNKS);
        let first = WORLD_MIDDLE.cartesian().0 - side / 2;
        first..first + side
    }

    /// Whether `superchunk` is of the world.
    pub fn within(&self, superchunk: SuperchunkIndex) -> bool {
        let ((x, y), span) = (superchunk.cartesian(), self.span());
        span.contains(&x) && span.contains(&y)
    }

    /// Every superchunk of a world of a size, sorted; none of one of no
    /// size, which has no end.
    pub fn all(&self) -> Vec<SuperchunkIndex> {
        if self.side().is_none() {
            return Vec::new();
        }
        let span = self.span();
        let mut world: Vec<SuperchunkIndex> = span.clone().flat_map(|y| span.clone().map(move |x| SuperchunkIndex::from_cartesian(x, y))).collect();
        world.sort_unstable();
        world
    }

    /// The superchunks to be hot, sorted, each once: the hot entities'
    /// halos, or every superchunk of the world if they are forced --
    /// none outside the world either way.
    pub fn wanted(&self, entities: &Entities) -> Vec<SuperchunkIndex> {
        let mut wanted = match *self {
            Self::About { entity, .. } => about(entities.superchunks().iter().filter(|superchunk| superchunk.iter().any(|kept| kept.header.kind == entity)).map(|superchunk| superchunk.index())),
            Self::Forced { .. } => self.all(),
        };
        wanted.retain(|&superchunk| self.within(superchunk));
        wanted
    }
}

/// The halos about the superchunks `of` -- each one's own and its
/// eight neighbours' -- as superchunk indices, sorted, each once.
pub fn about(of: impl Iterator<Item = SuperchunkIndex>) -> Vec<SuperchunkIndex> {
    let mut halos: Vec<SuperchunkIndex> = of.flat_map(|middle| (-1..=1).flat_map(move |dy| (-1..=1).filter_map(move |dx| middle.offset(dx, dy)))).collect();
    halos.sort_unstable();
    halos.dedup();
    halos
}
