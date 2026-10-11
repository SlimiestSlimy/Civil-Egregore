//! A tick's crossings settled: an entity that stepped over a border
//! was put in the superchunk it came to and stayed, written to, in
//! the one it left until the second phase was over
//! (`docs/simulation.md`, "Entities").

use super::{neighbours, Simulation};
use coordinates::SuperchunkIndex;
use entity_manager::{Entities, Settled, SuperchunkEntities};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// A part of the superchunks' entities, and where those of each
/// settled of a crossing goes.
type LeaversPart<'a> = (&'a mut [SuperchunkEntities], &'a mut [Settled]);

impl Simulation {
    /// Settles the tick's crossings, on the threads, the superchunks
    /// split as for the second phase -- `per_part` a part: each
    /// superchunk removes its leavers from the cells they left
    /// ([`SuperchunkEntities::settle_leavers`]); then each gives those
    /// it was put what they ended the tick with where they left, and
    /// takes back any removed there meanwhile
    /// ([`SuperchunkEntities::settle_arrivals`]) -- how many those
    /// were.
    pub(super) fn settle_crossings(&mut self, entities: &mut Entities, superchunk_indices: &[SuperchunkIndex], per_part: usize) -> usize {
        self.arrived.resize_with(superchunk_indices.len(), Vec::new);
        self.settled.resize_with(superchunk_indices.len(), Settled::default);
        let mut crossed = false;
        for (superchunk, arrived) in entities.superchunks_mut().iter_mut().zip(&mut self.arrived) {
            superchunk.take_arrived(arrived);
            crossed |= !arrived.is_empty();
        }
        if !crossed {
            return 0;
        }
        let arrived = &self.arrived;
        let neighbour = |here: SuperchunkIndex, (dx, dy): (i32, i32)| here.offset(dx, dy).and_then(|there| superchunk_indices.binary_search(&there).ok());
        {
            let parts: Vec<Mutex<LeaversPart>> = entities.superchunks_mut().chunks_mut(per_part).zip(self.settled.chunks_mut(per_part)).map(Mutex::new).collect();
            let claimed = AtomicUsize::new(0);
            self.dispatcher.run(&|_| {
                while let Some(work) = parts.get(claimed.fetch_add(1, Ordering::Relaxed)) {
                    let (superchunks, settled) = &mut *work.lock().expect("a piece's superchunks");
                    for (superchunk, settled) in superchunks.iter_mut().zip(settled.iter_mut()) {
                        let here = superchunk.index();
                        for there in neighbours().filter_map(|offset| neighbour(here, offset)) {
                            superchunk.settle_leavers(&arrived[there], settled);
                        }
                    }
                }
            });
        }
        let mut taken_back = 0;
        if self.settled.iter().any(|settled| !settled.is_empty()) {
            let settled = &self.settled;
            let parts: Vec<Mutex<&mut [SuperchunkEntities]>> = entities.superchunks_mut().chunks_mut(per_part).map(Mutex::new).collect();
            let (claimed, counted) = (AtomicUsize::new(0), AtomicUsize::new(0));
            self.dispatcher.run(&|_| {
                while let Some(work) = parts.get(claimed.fetch_add(1, Ordering::Relaxed)) {
                    for superchunk in work.lock().expect("a piece's superchunks").iter_mut() {
                        let here = superchunk.index();
                        for there in neighbours().filter_map(|offset| neighbour(here, offset)) {
                            counted.fetch_add(superchunk.settle_arrivals(&settled[there]), Ordering::Relaxed);
                        }
                    }
                }
            });
            taken_back = counted.into_inner();
        }
        self.arrived.iter_mut().for_each(Vec::clear);
        self.settled.iter_mut().for_each(Settled::clear);
        taken_back
    }
}
