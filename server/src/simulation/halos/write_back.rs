//! Write-backs and flushes landed: what the jobs' threads encoded put
//! in chunk storage's ring, and the ring's tail flushed; and the cold
//! pool's images paged out of memory.

use super::{Halos, LentWorld};
use crate::chunk_storage::disk::DiskError;
use crate::chunk_storage::jobs::{JobDone, Job};

impl Halos {
    /// Puts the write-backs the jobs have encoded into the
    /// writeback ring, in the order they were taken -- each one waited
    /// for, if `wait_for_jobs`, else up to the first not yet encoded -- the
    /// superchunk at the ring's tail sent to be flushed whenever it needs
    /// the room; then puts the images flushed so far in the cold pool.
    pub(crate) fn land_write_backs(&mut self, lent_world: &mut LentWorld<'_>, wait_for_jobs: bool) {
        while let Some(&(superchunk, encode_ticket)) = self.writing_back.front() {
            let maybe_encode_done = if wait_for_jobs { Some(self.jobs.take_waiting(encode_ticket)) } else { self.jobs.try_take(encode_ticket) };
            let Some(JobDone::WriteBackEncoded(encoded_layers)) = maybe_encode_done else {
                break;
            };
            for (bucket_key, words) in &encoded_layers {
                while !lent_world.chunk_storage.try_write_back(bucket_key.chunk, bucket_key.layer_type, words) {
                    self.flush_tail(lent_world);
                }
            }
            lent_world.bitmap_arena.written_back(superchunk, encoded_layers.iter().map(|(written_bucket_key, _)| *written_bucket_key));
            self.writing_back.pop_front();
        }
        self.land_flushes(lent_world, wait_for_jobs);
    }

    /// Takes the changes of the superchunk at the ring's tail out of it,
    /// to rewrite its image by a job -- its flush before, if
    /// still on its way, put in the cold pool first, so each rewrites the
    /// image the one before made. Its buckets, hot or cooled, hold its
    /// newest cells until the image is in ([`Halos::land_flushes`]).
    fn flush_tail(&mut self, lent_world: &mut LentWorld<'_>) {
        let superchunk = lent_world.chunk_storage.tail_superchunk().expect("a full ring has a tail");
        if let Some(flushing_index) = self.flushing_superchunks.iter().position(|&(flushing_superchunk, _)| flushing_superchunk == superchunk) {
            let (_, flush_ticket) = self.flushing_superchunks.remove(flushing_index);
            let JobDone::ImageFlushed(image) = self.jobs.take_waiting(flush_ticket) else {
                unreachable!("a flush's job makes an image");
            };
            lent_world.chunk_storage.insert(superchunk, image);
        }
        let flush = lent_world.chunk_storage.take_entries(superchunk).expect("the tail's changes");
        self.flushing_superchunks.push((superchunk, self.jobs.send_job(Job::Flush(flush))));
    }

    /// Puts the images the jobs have rewritten in the cold pool --
    /// each waited for, if `wait_for_jobs` -- and tells the arena of each
    /// superchunk with none of its changes left in the ring, so its
    /// buckets waiting there may go.
    fn land_flushes(&mut self, lent_world: &mut LentWorld<'_>, wait_for_jobs: bool) {
        let mut flushed_superchunks = Vec::new();
        for (superchunk, flush_ticket) in std::mem::take(&mut self.flushing_superchunks) {
            let maybe_flush_done = if wait_for_jobs { Some(self.jobs.take_waiting(flush_ticket)) } else { self.jobs.try_take(flush_ticket) };
            let Some(flush_done) = maybe_flush_done else {
                self.flushing_superchunks.push((superchunk, flush_ticket));
                continue;
            };
            let JobDone::ImageFlushed(image) = flush_done else {
                unreachable!("a flush's job makes an image");
            };
            lent_world.chunk_storage.insert(superchunk, image);
            if !lent_world.chunk_storage.holds_changes(superchunk) {
                flushed_superchunks.push(superchunk);
            }
        }
        lent_world.bitmap_arena.flushed_from_ring(&flushed_superchunks);
    }

    /// Flushes every superchunk with changes in the ring, rewritten on
    /// every thread: the ring empty, and the cold pool's
    /// images the cells written back.
    pub(crate) fn flush_all(&mut self, lent_world: &mut LentWorld<'_>) {
        while lent_world.chunk_storage.tail_superchunk().is_some() {
            self.flush_tail(lent_world);
        }
        self.land_flushes(lent_world, true);
    }

    /// Writes back every hot superchunk's changed bitmaps, encoded on
    /// every thread, and puts them, and every write-back
    /// still on its way, into the writeback ring.
    pub(crate) fn write_back_all(&mut self, lent_world: &mut LentWorld<'_>) {
        for superchunk in lent_world.bitmap_arena.superchunk_indices() {
            let dirty = lent_world.bitmap_arena.take_dirty(superchunk);
            if !dirty.is_empty() {
                self.writing_back.push_back((superchunk, self.jobs.send_job(Job::EncodeWriteBack(dirty))));
            }
        }
        self.land_write_backs(lent_world, true);
    }

    /// Pages the cold pool's images out while more of them are in
    /// memory than it keeps (`ChunkStorage::page_out`): how many went.
    /// None of a superchunk hot -- cooling too -- warming or cooled
    /// goes, so a hot superchunk's image is always in memory. If that
    /// is not enough, the ring is flushed first -- a cold superchunk
    /// whose changes wait there is held by them -- and the rest paged.
    pub(crate) fn page_cold_pool_out(&mut self, lent_world: &mut LentWorld<'_>) -> Result<usize, DiskError> {
        if !lent_world.chunk_storage.over_memory_kept() {
            return Ok(0);
        }
        let mut paged = self.page_out_unheld(lent_world)?;
        if lent_world.chunk_storage.over_memory_kept() && !lent_world.chunk_storage.nothing_to_flush() {
            self.flush_all(lent_world);
            paged += self.page_out_unheld(lent_world)?;
        }
        Ok(paged)
    }

    /// Pages out the images of the superchunks neither hot, warming
    /// nor cooled, until the pool keeps no more than it may.
    fn page_out_unheld(&self, lent_world: &mut LentWorld<'_>) -> Result<usize, DiskError> {
        let (hot, bitmap_arena, warming) = (lent_world.bitmap_arena.superchunk_indices(), &*lent_world.bitmap_arena, &self.warming);
        lent_world.chunk_storage.page_out(|superchunk| hot.binary_search(&superchunk).is_ok() || bitmap_arena.is_cooled(superchunk) || warming.binary_search_by_key(&superchunk, |warming| warming.superchunk).is_ok())
    }
}
