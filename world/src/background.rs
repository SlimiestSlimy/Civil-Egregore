//! The background: threads doing chunk storage's slow work off the
//! tick -- encoding the changed layers of superchunks gone cold, and
//! generating and decoding the superchunks warming. A thread a
//! core, each with its own codec, asleep while there is nothing to do.
//!
//! A job sent ([`Background::send`]) is a [`Ticket`]; what it made is
//! taken by it ([`Background::take`], waiting if not yet made, or
//! [`Background::try_take`]), or forgotten ([`Background::forget`]).
//! What a job makes depends on nothing but the job, so the world is the
//! same however fast the threads are.

use crate::generate_image;
use bitmap::CellWords;
use bitplane_manager::BucketKey;
use chunk_storage::{LayerCodec, LayerType, SuperchunkImage};
use coordinates::SuperchunkIndex;
use std::collections::{BTreeMap, BTreeSet};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Work for the background.
pub enum Job {
    /// A superchunk's write-back to encode
    /// ([`bitplane_manager::BitmapArena::take_dirty`]).
    Encode(Vec<(BucketKey, Box<CellWords>)>),
    /// A superchunk to make hot: every layer of `types` of each of its
    /// chunks decoded from its `image` -- generated from `seed` first,
    /// if it has none.
    Warm {
        /// The superchunk.
        superchunk: SuperchunkIndex,
        /// Its image in the cold pool, if it has one.
        image: Option<Arc<SuperchunkImage>>,
        /// The world's seed.
        seed: u64,
        /// The layer types to decode.
        types: Vec<LayerType>,
    },
}

/// What a job made.
pub enum Done {
    /// The write-back, encoded: each bucket's layer words
    /// ([`LayerCodec::encode_layer`]).
    Encoded(Vec<(BucketKey, Vec<u64>)>),
    /// The superchunk's cells.
    Warmed {
        /// Its image, if it had none: generated.
        generated: Option<SuperchunkImage>,
        /// Each bitmap's cells, chunk by chunk, type by type: `None` for
        /// no cell set.
        cells: Vec<(BucketKey, Option<Box<CellWords>>)>,
    },
    /// The job panicked, with what it said.
    Failed(String),
}

impl Job {
    /// Does the job, with `codec`.
    fn run(self, codec: &mut LayerCodec) -> Done {
        match self {
            Self::Encode(dirty) => Done::Encoded(dirty.into_iter().map(|(key, cells)| (key, codec.encode_layer(&cells).to_vec())).collect()),
            Self::Warm { superchunk, image, seed, types } => {
                let generated = image.is_none().then(|| generate_image(seed, superchunk, codec));
                let image = generated.as_ref().or(image.as_deref()).expect("an image, kept or generated");
                let mut cells = Vec::with_capacity(types.len() * superchunk.chunks().count());
                for chunk in superchunk.chunks() {
                    for &layer_type in &types {
                        let decoded = image.layer(chunk.place(), layer_type).map(|layer| {
                            let mut bucket = Box::new([0; bitmap::WORDS]);
                            codec.decode(layer, &mut bucket);
                            bucket
                        });
                        cells.push((BucketKey { layer_type, chunk }, decoded));
                    }
                }
                Done::Warmed { generated, cells }
            }
        }
    }
}

/// A job sent, to take what it made by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ticket(u64);

/// The background's threads, the jobs sent them, and what they made.
pub struct Background {
    /// Where jobs are sent: dropped first, so the threads stop.
    jobs: Option<Sender<(Ticket, Job)>>,
    /// What the threads made, as they made it.
    done: Receiver<(Ticket, Done)>,
    /// What was made and not yet taken.
    made: BTreeMap<Ticket, Done>,
    /// Jobs forgotten before they were made: what they make is dropped.
    forgotten: BTreeSet<Ticket>,
    /// The next job's ticket.
    next: u64,
    /// The threads.
    threads: Vec<JoinHandle<()>>,
}

impl Background {
    /// A thread a core, waiting for jobs.
    pub fn new() -> Self {
        let (jobs, waiting) = channel::<(Ticket, Job)>();
        let (made, done) = channel();
        let waiting = Arc::new(Mutex::new(waiting));
        let threads = (0..std::thread::available_parallelism().map_or(1, usize::from))
            .map(|_| {
                let (waiting, made) = (Arc::clone(&waiting), made.clone());
                std::thread::spawn(move || {
                    let mut codec = LayerCodec::new();
                    loop {
                        // The next job, the lock let go before it is done; none will come once the sender is dropped.
                        let next = waiting.lock().expect("the jobs").recv();
                        let Ok((ticket, job)) = next else {
                            return;
                        };
                        let done = catch_unwind(AssertUnwindSafe(|| job.run(&mut codec))).unwrap_or_else(|panic| {
                            let said = panic.downcast_ref::<&str>().map(|said| said.to_string()).or_else(|| panic.downcast_ref::<String>().cloned());
                            Done::Failed(said.unwrap_or_default())
                        });
                        if made.send((ticket, done)).is_err() {
                            return;
                        }
                    }
                })
            })
            .collect();
        Self { jobs: Some(jobs), done, made: BTreeMap::new(), forgotten: BTreeSet::new(), next: 0, threads }
    }

    /// Sends `job` to the threads: its ticket.
    pub fn send(&mut self, job: Job) -> Ticket {
        let ticket = Ticket(self.next);
        self.next += 1;
        self.jobs.as_ref().expect("the threads running").send((ticket, job)).expect("the threads running");
        ticket
    }

    /// Keeps `done`, made for `ticket`, unless it was forgotten.
    fn keep(&mut self, (ticket, done): (Ticket, Done)) {
        if !self.forgotten.remove(&ticket) {
            self.made.insert(ticket, done);
        }
    }

    /// What `ticket`'s job made, if it has been.
    pub fn try_take(&mut self, ticket: Ticket) -> Option<Done> {
        while let Ok(made) = self.done.try_recv() {
            self.keep(made);
        }
        self.made.remove(&ticket).map(checked)
    }

    /// What `ticket`'s job made, waiting for it.
    pub fn take(&mut self, ticket: Ticket) -> Done {
        loop {
            if let Some(done) = self.made.remove(&ticket) {
                return checked(done);
            }
            let made = self.done.recv().expect("the threads running");
            self.keep(made);
        }
    }

    /// Forgets `ticket`'s job: what it makes is dropped.
    pub fn forget(&mut self, ticket: Ticket) {
        if self.made.remove(&ticket).is_none() {
            self.forgotten.insert(ticket);
        }
    }
}

/// `done`, unless its job panicked: then the panic carried on here.
fn checked(done: Done) -> Done {
    match done {
        Done::Failed(said) => panic!("a background job panicked: {said}"),
        done => done,
    }
}

impl Default for Background {
    /// The same as [`Background::new`].
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Background {
    /// Stops the threads, each once its job is done.
    fn drop(&mut self) {
        self.jobs = None;
        self.threads.drain(..).for_each(|thread| {
            let _ = thread.join();
        });
    }
}
