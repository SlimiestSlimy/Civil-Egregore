//! Halos: the superchunks kept hot. Every entity of a kind that keeps a
//! halo ([`HALO_KEEPERS`]) keeps its own superchunk and the eight about
//! it hot -- its **halo** -- so whatever it reaches in a tick, as far as
//! the speed of light, is hot. Every other superchunk is cold: its
//! cells in its image in chunk storage, its entities and random
//! numbers kept as a save keeps them ([`crate::World::cold`]). A
//! superchunk a halo reaches is made hot from storage if it was ever
//! made, else generated; one no halo reaches any more goes cold. The
//! halos move after every tick ([`crate::World::tick`]), so between
//! ticks the hot superchunks are the halos, exactly.

use crate::{generate_image, World};
use coordinates::SuperchunkIndex;
use entity_rules::sheep::SHEEP;
use simulation::entity_store::{saved, EntityType};
use std::ops::AddAssign;

/// The kinds of entity that keep a halo: the ones that matter, about
/// which the world is ticked. For now the sheep -- the only kind there
/// is -- stand in for the people to come.
pub const HALO_KEEPERS: [EntityType; 1] = [SHEEP];

/// What moving the halos did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HaloChange {
    /// Superchunks made hot for the first time: generated.
    pub generated: usize,
    /// Superchunks made hot again, from storage.
    pub warmed: usize,
    /// Superchunks gone cold.
    pub cooled: usize,
}

impl AddAssign for HaloChange {
    /// Both added up.
    fn add_assign(&mut self, other: Self) {
        self.generated += other.generated;
        self.warmed += other.warmed;
        self.cooled += other.cooled;
    }
}

/// The halos about the superchunks `keepers` -- each one's own and its
/// eight neighbours' -- as superchunk indices, sorted, each once.
pub fn about(keepers: impl Iterator<Item = SuperchunkIndex>) -> Vec<SuperchunkIndex> {
    let mut halos: Vec<SuperchunkIndex> = keepers.flat_map(|keeper| (-1..=1).flat_map(move |dy| (-1..=1).filter_map(move |dx| keeper.offset(dx, dy)))).collect();
    halos.sort_unstable();
    halos.dedup();
    halos
}

impl World {
    /// The hot superchunks holding an entity that keeps a halo.
    fn keepers(&self) -> impl Iterator<Item = SuperchunkIndex> + '_ {
        self.entities.superchunks().iter().filter(|superchunk| superchunk.iter().any(|entity| HALO_KEEPERS.contains(&entity.header.kind))).map(|superchunk| superchunk.index())
    }

    /// Moves the halos to where their keepers stand: the superchunks
    /// they reach made hot, the rest cold.
    pub fn move_halos(&mut self) -> HaloChange {
        let wanted = about(self.keepers());
        self.keep_hot(&wanted)
    }

    /// Makes `wanted` -- sorted -- the hot superchunks: every other one
    /// made cold, its state kept; every one of them not hot made hot,
    /// from storage and its kept state, or generated. The halos are kept
    /// so ([`World::move_halos`]); between two ticks, anything else
    /// needing superchunks hot a while may ask too.
    pub fn keep_hot(&mut self, wanted: &[SuperchunkIndex]) -> HaloChange {
        let hot = self.arena.superchunk_indices();
        let random: Vec<(SuperchunkIndex, u64)> = self.simulation.random_states().collect();
        let mut change = HaloChange::default();
        for &superchunk in hot.iter().filter(|superchunk| wanted.binary_search(superchunk).is_err()) {
            let state = random.binary_search_by_key(&superchunk, |state| state.0).ok().map(|at| random[at].1);
            self.cold.insert(superchunk, saved::encode_state(state, self.entities.superchunk(superchunk)).0);
            self.arena.make_cold_superchunk(superchunk, &mut self.storage, &mut self.codec);
            change.cooled += 1;
        }
        let warming: Vec<SuperchunkIndex> = wanted.iter().filter(|superchunk| hot.binary_search(superchunk).is_err()).copied().collect();
        for &superchunk in &warming {
            if self.storage.image(superchunk).is_some() {
                change.warmed += 1;
            } else {
                let image = generate_image(self.info.seed, superchunk, &mut self.codec);
                self.storage.insert(superchunk, image);
                change.generated += 1;
            }
            self.arena.make_hot_superchunk(superchunk, &self.info.layers, &self.storage, &mut self.codec);
        }
        if change == HaloChange::default() {
            return change;
        }
        let now_hot = self.arena.superchunk_indices();
        self.entities.align(&now_hot);
        let mut restored = Vec::new();
        for superchunk in &warming {
            if let Some(words) = self.cold.remove(superchunk) {
                let state = saved::decode_state(&words, self.entities.now(), &mut self.entities).expect("a cold superchunk's state, as it was kept");
                restored.extend(state.random.map(|state| (*superchunk, state)));
            }
        }
        self.entities.apply();
        // The random streams of the superchunks hot all along, and of those warmed the ones kept: none of one gone cold.
        let kept = random.into_iter().filter(|state| now_hot.binary_search(&state.0).is_ok() && warming.binary_search(&state.0).is_err());
        let mut states: Vec<(SuperchunkIndex, u64)> = kept.chain(restored).collect();
        states.sort_unstable();
        self.simulation.restore_random(&states);
        change
    }
}
