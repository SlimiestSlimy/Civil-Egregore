//! Chunk storage's slow work, done off the tick on the dispatcher's
//! threads: layers encoded, images rewritten, superchunks generated and
//! decoded (`docs/chunk_storage.md`, "Jobs").

use crate::layer_codec::BucketKey;
use crate::{wide, Flush, LayerCodec, LayerType, SuperchunkImage};
use coordinates::SuperchunkIndex;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use utilities::dispatcher::Dispatcher;

/// What makes a superchunk's image the first time it is wanted: the
/// world's generator, which chunk storage knows nothing of.
pub type Generate = Box<dyn FnOnce(&mut LayerCodec) -> SuperchunkImage + Send>;

/// Work for the dispatcher's threads.
pub enum Job {
    /// A superchunk's write-back to encode: its changed buckets'
    /// words.
    Encode(Vec<(BucketKey, Box<[u64]>)>),
    /// A superchunk's changes taken from the ring, to rewrite its image
    /// with ([`crate::ChunkStorage::take`]).
    Flush(Flush),
    /// A superchunk to make hot: every layer of `types` of each of its
    /// chunks decoded from its `image` -- generated first, if it has
    /// none.
    Warm {
        /// The superchunk.
        superchunk: SuperchunkIndex,
        /// Its image in the cold pool, if it has one.
        image: Option<Arc<SuperchunkImage>>,
        /// What generates it, if it has none.
        generate: Generate,
        /// The layer types to decode.
        types: Vec<LayerType>,
    },
}

/// What a job made.
pub enum Done {
    /// The write-back, encoded: each bucket's layer words, a wide
    /// bucket's a plane at a time, under the planes' types
    /// ([`LayerCodec::encode_layer`]).
    Encoded(Vec<(BucketKey, Vec<u64>)>),
    /// The image rewritten.
    Flushed(SuperchunkImage),
    /// The superchunk's cells.
    Warmed {
        /// Its image, if it had none: generated.
        generated: Option<SuperchunkImage>,
        /// Each bitmap's cells, chunk by chunk, type by type: `None` for
        /// no cell set.
        cells: Vec<(BucketKey, Option<Box<[u64]>>)>,
    },
    /// The job panicked, with what it said.
    Failed(String),
}

impl Job {
    /// Does the job, with `codec`.
    fn run(self, codec: &mut LayerCodec) -> Done {
        match self {
            Self::Encode(dirty) => {
                let mut encoded = Vec::with_capacity(dirty.len());
                for (key, cells) in dirty {
                    match key.layer_type.bits() {
                        1 => encoded.push((key, codec.encode_layer(cells[..].try_into().expect("a bitmap's words")).to_vec())),
                        // A wide bucket is encoded a plane at a time: its bits' bitmaps, each under its own type.
                        bits => encoded.extend((0..bits).map(|bit| (BucketKey { layer_type: key.layer_type.plane(bit), chunk: key.chunk }, codec.encode_layer(&wide::plane(&cells, bits, bit)).to_vec()))),
                    }
                }
                Done::Encoded(encoded)
            }
            Self::Flush(flush) => Done::Flushed(flush.rewritten()),
            Self::Warm { superchunk, image, generate, types } => {
                let generated = image.is_none().then(|| generate(codec));
                let image = generated.as_ref().or(image.as_deref()).expect("an image, kept or generated");
                let mut cells = Vec::with_capacity(types.len() * superchunk.chunks().count());
                for chunk in superchunk.chunks() {
                    for &layer_type in &types {
                        // A wide layer's planes decoded and put together: none of them there, no cell set.
                        let (bits, mut plane, mut bucket) = (layer_type.bits(), [0; bitmap::WORDS], None);
                        for bit in 0..bits {
                            if let Some(layer) = image.layer(chunk.place(), layer_type.plane(bit)) {
                                codec.decode(layer, &mut plane);
                                wide::spread(&plane, bits, bit, bucket.get_or_insert_with(|| vec![0; bitmap::WORDS * bits as usize].into_boxed_slice()));
                            }
                        }
                        cells.push((BucketKey { layer_type, chunk }, bucket));
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

thread_local! {
    /// The thread's codec: one a thread that does a job.
    static CODEC: RefCell<LayerCodec> = RefCell::new(LayerCodec::new());
}

/// The jobs sent to the dispatcher's threads, and what they made.
pub struct Jobs {
    /// The threads.
    dispatcher: Arc<Dispatcher>,
    /// Where a job done leaves what it made.
    made_by: Sender<(Ticket, Done)>,
    /// What the threads made, as they made it.
    done: Receiver<(Ticket, Done)>,
    /// What was made and not yet taken.
    made: BTreeMap<Ticket, Done>,
    /// The next job's ticket.
    next: u64,
}

impl Jobs {
    /// Jobs done on `dispatcher`'s threads, whichever is free: shared
    /// with the tick, which a thread busy here sits out.
    pub fn new(dispatcher: Arc<Dispatcher>) -> Self {
        let (made_by, done) = channel();
        Self { dispatcher, made_by, done, made: BTreeMap::new(), next: 0 }
    }

    /// Sends `job` to the threads: its ticket.
    pub fn send(&mut self, job: Job) -> Ticket {
        let ticket = Ticket(self.next);
        self.next += 1;
        let made_by = self.made_by.clone();
        self.dispatcher.queue(move || {
            let done = catch_unwind(AssertUnwindSafe(|| CODEC.with(|codec| job.run(&mut codec.borrow_mut())))).unwrap_or_else(|panic| {
                let said = panic.downcast_ref::<&str>().map(|said| said.to_string()).or_else(|| panic.downcast_ref::<String>().cloned());
                Done::Failed(said.unwrap_or_default())
            });
            // Nobody waiting any more: the world it was for is gone.
            let _ = made_by.send((ticket, done));
        });
        ticket
    }

    /// What `ticket`'s job made, if it has been.
    pub fn try_take(&mut self, ticket: Ticket) -> Option<Done> {
        while let Ok((made, done)) = self.done.try_recv() {
            self.made.insert(made, done);
        }
        self.made.remove(&ticket).map(checked)
    }

    /// What `ticket`'s job made, waiting for it.
    pub fn take(&mut self, ticket: Ticket) -> Done {
        loop {
            if let Some(done) = self.made.remove(&ticket) {
                return checked(done);
            }
            let (made, done) = self.done.recv().expect("its own sender kept");
            self.made.insert(made, done);
        }
    }
}

/// `done`, unless its job panicked: then the panic carried on here.
fn checked(done: Done) -> Done {
    match done {
        Done::Failed(said) => panic!("a job of chunk storage's panicked: {said}"),
        done => done,
    }
}
