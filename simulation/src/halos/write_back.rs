//! Write-backs and flushes landed: what the jobs' threads encoded put
//! in chunk storage's ring, and the ring's tail flushed; and the cold
//! pool's images paged out of memory.

use super::{Halos, Held};
use chunk_storage::disk::DiskError;
use chunk_storage::jobs::{Done, Job};

impl Halos {
    /// Puts the write-backs the jobs have encoded into the
    /// writeback ring, in the order they were taken -- each one waited
    /// for, if `wait`, else up to the first not yet encoded -- the
    /// superchunk at the ring's tail sent to be flushed whenever it needs
    /// the room; then puts the images flushed so far in the cold pool.
    pub fn land_write_backs(&mut self, held: &mut Held<'_>, wait: bool) {
        while let Some(&(superchunk, ticket)) = self.writing_back.front() {
            let done = if wait { Some(self.jobs.take(ticket)) } else { self.jobs.try_take(ticket) };
            let Some(Done::Encoded(encoded)) = done else {
                break;
            };
            for (key, words) in &encoded {
                while !held.storage.try_write_back(key.chunk, key.layer_type, words) {
                    self.flush_tail(held);
                }
            }
            held.arena.written_back(superchunk, encoded.iter().map(|(key, _)| *key));
            self.writing_back.pop_front();
        }
        self.land_flushes(held, wait);
    }

    /// Takes the changes of the superchunk at the ring's tail out of it,
    /// to rewrite its image by a job -- its flush before, if
    /// still on its way, put in the cold pool first, so each rewrites the
    /// image the one before made. Its buckets, hot or lingering, hold its
    /// newest cells until the image is in ([`Halos::land_flushes`]).
    fn flush_tail(&mut self, held: &mut Held<'_>) {
        let superchunk = held.storage.tail_superchunk().expect("a full ring has a tail");
        if let Some(at) = self.flushing.iter().position(|&(flushing, _)| flushing == superchunk) {
            let (_, ticket) = self.flushing.remove(at);
            let Done::Flushed(image) = self.jobs.take(ticket) else {
                unreachable!("a flush's job makes an image");
            };
            held.storage.insert(superchunk, image);
        }
        let flush = held.storage.take(superchunk).expect("the tail's changes");
        self.flushing.push((superchunk, self.jobs.send(Job::Flush(flush))));
    }

    /// Puts the images the jobs have rewritten in the cold pool --
    /// each waited for, if `wait` -- and tells the arena of each
    /// superchunk with none of its changes left in the ring, so its
    /// buckets waiting there may go.
    fn land_flushes(&mut self, held: &mut Held<'_>, wait: bool) {
        let mut flushed = Vec::new();
        for (superchunk, ticket) in std::mem::take(&mut self.flushing) {
            let done = if wait { Some(self.jobs.take(ticket)) } else { self.jobs.try_take(ticket) };
            let Some(done) = done else {
                self.flushing.push((superchunk, ticket));
                continue;
            };
            let Done::Flushed(image) = done else {
                unreachable!("a flush's job makes an image");
            };
            held.storage.insert(superchunk, image);
            if !held.storage.holds_changes(superchunk) {
                flushed.push(superchunk);
            }
        }
        held.arena.flushed(&flushed);
    }

    /// Flushes every superchunk with changes in the ring, rewritten on
    /// every thread: the ring empty, and the cold pool's
    /// images the cells written back.
    pub fn flush_all(&mut self, held: &mut Held<'_>) {
        while held.storage.tail_superchunk().is_some() {
            self.flush_tail(held);
        }
        self.land_flushes(held, true);
    }

    /// Writes back every hot superchunk's changed bitmaps, encoded on
    /// every thread, and puts them, and every write-back
    /// still on its way, into the writeback ring.
    pub fn write_back_all(&mut self, held: &mut Held<'_>) {
        for superchunk in held.arena.superchunk_indices() {
            let dirty = held.arena.take_dirty(superchunk);
            if !dirty.is_empty() {
                self.writing_back.push_back((superchunk, self.jobs.send(Job::Encode(dirty))));
            }
        }
        self.land_write_backs(held, true);
    }

    /// Pages the cold pool's images out while more of them are in
    /// memory than it keeps (`ChunkStorage::page_out`): how many went.
    /// None of a superchunk hot -- cooling too -- warming or lingering
    /// goes, so a hot superchunk's image is always in memory. If that
    /// is not enough, the ring is flushed first -- a cold superchunk
    /// whose changes wait there is held by them -- and the rest paged.
    pub fn page_cold_pool_out(&mut self, held: &mut Held<'_>) -> Result<usize, DiskError> {
        if !held.storage.over_memory_kept() {
            return Ok(0);
        }
        let mut paged = self.page_out_unheld(held)?;
        if held.storage.over_memory_kept() && !held.storage.nothing_to_flush() {
            self.flush_all(held);
            paged += self.page_out_unheld(held)?;
        }
        Ok(paged)
    }

    /// Pages out the images of the superchunks neither hot, warming
    /// nor lingering, until the pool keeps no more than it may.
    fn page_out_unheld(&self, held: &mut Held<'_>) -> Result<usize, DiskError> {
        let (hot, arena, warming) = (held.arena.superchunk_indices(), &*held.arena, &self.warming);
        held.storage.page_out(|superchunk| hot.binary_search(&superchunk).is_ok() || arena.lingers(superchunk) || warming.binary_search_by_key(&superchunk, |warming| warming.superchunk).is_ok())
    }
}
