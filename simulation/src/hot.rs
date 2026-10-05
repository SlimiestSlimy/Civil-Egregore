//! Which superchunks are to be hot: the world's size, if it has one,
//! and the **hot entity** -- the kind of entity that keeps the
//! superchunks about it hot, its **halo**. Whoever holds the world
//! gives both ([`Hot`]) to its halos ([`crate::Halos`]), which make
//! hot what is wanted ([`Hot::wanted`]).

use coordinates::{SuperchunkIndex, WORLD_MIDDLE, WORLD_SIDE_SUPERCHUNKS};
use entity_manager::{Entities, EntityType};

/// Which superchunks are to be hot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hot {
    /// Superchunks along a side of the world, a square with the
    /// world's origin superchunk in its middle: nothing outside it is
    /// ever hot, so nothing is made there and nothing goes there.
    /// `None`: as far as coordinates reach.
    pub side: Option<u32>,
    /// The hot entity: every one of this kind keeps its halo hot.
    /// `None`: superchunks are forced hot -- every one of a world with
    /// a size; of one without, those its holder makes hot.
    pub entity: Option<EntityType>,
}

impl Hot {
    /// A world of no size, hot about every entity of the kind `entity`.
    pub const fn about(entity: EntityType) -> Self {
        Self { side: None, entity: Some(entity) }
    }

    /// The world's first column and row and the one past its last, of
    /// superchunks: both the same, the world being square.
    fn span(&self) -> std::ops::Range<u32> {
        let side = self.side.unwrap_or(WORLD_SIDE_SUPERCHUNKS).min(WORLD_SIDE_SUPERCHUNKS);
        let first = WORLD_MIDDLE.cartesian().0 - side / 2;
        first..first + side
    }

    /// Whether `superchunk` is of the world.
    pub fn within(&self, superchunk: SuperchunkIndex) -> bool {
        let ((x, y), span) = (superchunk.cartesian(), self.span());
        span.contains(&x) && span.contains(&y)
    }

    /// The superchunks to be hot, sorted, each once: the hot
    /// entities' halos, or every superchunk of a world with a size if
    /// they are forced -- none outside the world either way. `None`
    /// where they are forced in a world of no size: those hot stay so.
    pub fn wanted(&self, entities: &Entities) -> Option<Vec<SuperchunkIndex>> {
        let mut wanted = match self.entity {
            Some(hot) => about(entities.superchunks().iter().filter(|superchunk| superchunk.iter().any(|entity| entity.header.kind == hot)).map(|superchunk| superchunk.index())),
            None => {
                let span = self.side.map(|_| self.span())?;
                let mut world: Vec<SuperchunkIndex> = span.clone().flat_map(|y| span.clone().map(move |x| SuperchunkIndex::from_cartesian(x, y))).collect();
                world.sort_unstable();
                world
            }
        };
        wanted.retain(|&superchunk| self.within(superchunk));
        Some(wanted)
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
