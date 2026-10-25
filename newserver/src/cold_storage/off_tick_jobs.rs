//! Slow work done off the tick on the dispatcher's threads, each with
//! its thread's codec, what it made taken by ticket ([`Jobs`]) -- and
//! cold storage's own: plane chunks encoded, cold superchunks rewritten ([`Job`])
//! (`docs/chunk_storage.md`, "Jobs").

use crate::cold_storage::bitmap_codec::PlaneChunk;
use crate::cold_storage::{plane_chunk_encoding, Flush, BitmapCodec, ColdSuperchunk};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use utilities::dispatcher::Dispatcher;

/// Cold storage's work for the dispatcher's threads.
pub enum Job {
    /// A superchunk's write-back to encode: its changed plane chunks'
    /// words.
    EncodeWriteBack(Vec<(PlaneChunk, Box<[u64]>)>),
    /// A superchunk's changes taken from the ring, to rewrite its cold superchunk
    /// with ([`crate::cold_storage::ColdStorage::take_waiting`]).
    Flush(Flush),
}

/// What one of cold storage's jobs made.
pub enum JobDone {
    /// The write-back, encoded: each plane chunk's words, a wide
    /// plane chunk's a plane at a time, under the planes' types
    /// ([`BitmapCodec::encode_unless_empty`]).
    WriteBackEncoded(Vec<(PlaneChunk, Vec<u64>)>),
    /// The cold superchunk rewritten.
    ColdSuperchunkFlushed(ColdSuperchunk),
}

impl Job {
    /// Does the job, with `bitmap_codec`.
    fn execute_with_codec(self, bitmap_codec: &mut BitmapCodec) -> JobDone {
        match self {
            Self::EncodeWriteBack(dirty) => {
                let mut encoded_plane_chunks = Vec::with_capacity(dirty.len());
                for (plane_chunk, cells) in dirty {
                    encoded_plane_chunks.push((plane_chunk, plane_chunk_encoding::encode_plane_chunk(plane_chunk.plane_type, &cells, bitmap_codec)));
                }
                JobDone::WriteBackEncoded(encoded_plane_chunks)
            }
            Self::Flush(flush) => JobDone::ColdSuperchunkFlushed(flush.rewritten_cold_superchunk()),
        }
    }
}

/// A job sent, to take what it made by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct JobTicket(u64);

thread_local! {
    /// The thread's codec: one a thread that does a job.
    static THREAD_CODEC: RefCell<BitmapCodec> = RefCell::new(BitmapCodec::new());
}

/// What a job made, or what it said as it panicked.
type Made<Done> = Result<Done, String>;

/// Jobs sent to the dispatcher's threads, each making a `Done`, and
/// what they made.
pub struct Jobs<Done> {
    /// The threads.
    dispatcher: Arc<Dispatcher>,
    /// Where a job done leaves what it made.
    made_by: Sender<(JobTicket, Made<Done>)>,
    /// What the threads made, as they made it.
    done_receiver: Receiver<(JobTicket, Made<Done>)>,
    /// What was made and not yet taken.
    made_not_taken: BTreeMap<JobTicket, Made<Done>>,
    /// The next job's ticket.
    next_ticket: u64,
}

impl<Done: Send + 'static> Jobs<Done> {
    /// Jobs done on `dispatcher`'s threads, whichever is free: shared
    /// with the tick, which a thread busy here sits out.
    pub fn new(dispatcher: Arc<Dispatcher>) -> Self {
        let (made_by, done_receiver) = channel();
        Self { dispatcher, made_by, done_receiver, made_not_taken: BTreeMap::new(), next_ticket: 0 }
    }

    /// Sends `work` to the threads, done with the thread's codec: its ticket.
    pub fn send_work(&mut self, work: impl FnOnce(&mut BitmapCodec) -> Done + Send + 'static) -> JobTicket {
        let job_ticket = JobTicket(self.next_ticket);
        self.next_ticket += 1;
        let made_by = self.made_by.clone();
        self.dispatcher.queue_job(move || {
            let made = catch_unwind(AssertUnwindSafe(|| THREAD_CODEC.with(|thread_codec| work(&mut thread_codec.borrow_mut())))).map_err(|panic_payload| {
                panic_payload.downcast_ref::<&str>().map(|message_str| message_str.to_string()).or_else(|| panic_payload.downcast_ref::<String>().cloned()).unwrap_or_default()
            });
            // Nobody waiting any more: the world it was for is gone.
            let _ = made_by.send((job_ticket, made));
        });
        job_ticket
    }

    /// What `job_ticket`'s job made, if it has been.
    pub fn try_take(&mut self, job_ticket: JobTicket) -> Option<Done> {
        while let Ok((made_ticket, made)) = self.done_receiver.try_recv() {
            self.made_not_taken.insert(made_ticket, made);
        }
        self.made_not_taken.remove(&job_ticket).map(panic_if_failed)
    }

    /// What `job_ticket`'s job made, waiting for it.
    pub fn take_waiting(&mut self, job_ticket: JobTicket) -> Done {
        loop {
            if let Some(taken) = self.made_not_taken.remove(&job_ticket) {
                return panic_if_failed(taken);
            }
            let (made_ticket, made) = self.done_receiver.recv().expect("its own sender kept");
            self.made_not_taken.insert(made_ticket, made);
        }
    }
}

impl Jobs<JobDone> {
    /// Sends cold storage's `job` to the threads: its ticket.
    pub fn send_job(&mut self, job: Job) -> JobTicket {
        self.send_work(move |bitmap_codec| job.execute_with_codec(bitmap_codec))
    }
}

/// What a job made, unless it panicked: then the panic carried on here.
fn panic_if_failed<Done>(made: Made<Done>) -> Done {
    made.unwrap_or_else(|panic_message| panic!("a job of cold storage's panicked: {panic_message}"))
}
