//! Chunk storage's slow work, done off the tick on the dispatcher's
//! threads: layers encoded, images rewritten, superchunks generated and
//! decoded (`docs/chunk_storage.md`, "Jobs").

use crate::chunk_storage::layer_codec::BucketKey;
use crate::chunk_storage::{read_back, wide, Flush, LayerCodec, LayerType, ImageStored, SuperchunkImage};
use utilities::coordinates::SuperchunkIndex;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use utilities::dispatcher::Dispatcher;

/// What makes a superchunk's image the first time it is wanted: the
/// world's generator, which chunk storage knows nothing of.
pub type GenerateImage = Box<dyn FnOnce(&mut LayerCodec) -> SuperchunkImage + Send>;

/// Work for the dispatcher's threads.
pub enum Job {
    /// A superchunk's write-back to encode: its changed buckets'
    /// words.
    EncodeWriteBack(Vec<(BucketKey, Box<[u64]>)>),
    /// A superchunk's changes taken from the ring, to rewrite its image
    /// with ([`crate::chunk_storage::ChunkStorage::take_waiting`]).
    Flush(Flush),
    /// A superchunk to make hot: every layer of `types` of each of its
    /// chunks decoded from its `image` -- generated first, if it has
    /// none.
    WarmSuperchunk {
        /// The superchunk.
        superchunk: SuperchunkIndex,
        /// Its image as the cold pool has it, if it has one: read here,
        /// off the tick, if it is paged to disk.
        image: Option<ImageStored>,
        /// What generates it, if it has none.
        generate_image: GenerateImage,
        /// The layer types to decode.
        layer_types: Vec<LayerType>,
    },
}

/// What a job made.
pub enum JobDone {
    /// The write-back, encoded: each bucket's layer words, a wide
    /// bucket's a plane at a time, under the planes' types
    /// ([`LayerCodec::encode_layer`]).
    WriteBackEncoded(Vec<(BucketKey, Vec<u64>)>),
    /// The image rewritten.
    ImageFlushed(SuperchunkImage),
    /// The superchunk's cells.
    SuperchunkWarmed {
        /// Its image, if it had none: generated.
        generated_image: Option<SuperchunkImage>,
        /// Its image, if it was paged to disk: read back, to be in
        /// memory again ([`crate::chunk_storage::ChunkStorage::bring_in`]).
        image_read_back: Option<SuperchunkImage>,
        /// Each bitmap's cells, chunk by chunk, type by type: `None` for
        /// no cell set.
        cells: Vec<(BucketKey, Option<Box<[u64]>>)>,
    },
    /// The job panicked, with what it said.
    JobPanicked(String),
}

impl Job {
    /// Does the job, with `layer_codec`.
    fn execute_with_codec(self, layer_codec: &mut LayerCodec) -> JobDone {
        match self {
            Self::EncodeWriteBack(dirty) => {
                let mut encoded_layers = Vec::with_capacity(dirty.len());
                for (bucket_key, cells) in dirty {
                    match bucket_key.layer_type.width_bits() {
                        1 => encoded_layers.push((bucket_key, layer_codec.encode_layer(cells[..].try_into().expect("a bitmap's words")).to_vec())),
                        // A wide bucket is encoded a plane at a time: its bits' bitmaps, each under its own type.
                        bits_a_cell => encoded_layers.extend((0..bits_a_cell).map(|bit_of_cell| (BucketKey { layer_type: bucket_key.layer_type.bitplane_of(bit_of_cell), chunk: bucket_key.chunk }, layer_codec.encode_layer(&wide::bitmap_of_bit(&cells, bits_a_cell, bit_of_cell)).to_vec()))),
                    }
                }
                JobDone::WriteBackEncoded(encoded_layers)
            }
            Self::Flush(flush) => JobDone::ImageFlushed(flush.rewritten_image()),
            Self::WarmSuperchunk { superchunk, image, generate_image: generate, layer_types: types } => {
                let (mut generated_image, mut image_read_back, mut shared_image) = (None, None, None);
                match image {
                    None => generated_image = Some(generate(layer_codec)),
                    Some(ImageStored::InMemory(image)) => shared_image = Some(image),
                    Some(ImageStored::OnDisk(folder)) => image_read_back = Some(read_back(&folder, superchunk)),
                }
                let image = generated_image.as_ref().or(image_read_back.as_ref()).or(shared_image.as_deref()).expect("an image, kept, read or generated");
                let mut cells = Vec::with_capacity(types.len() * superchunk.chunks().count());
                for chunk in superchunk.chunks() {
                    for &layer_type in &types {
                        // A wide layer's planes decoded and put together: none of them there, no cell set.
                        let (layer_bits_a_cell, mut one_bit_bitmap, mut bucket) = (layer_type.width_bits(), [0; utilities::bitmap::WORDS], None);
                        for bit_decoded in 0..layer_bits_a_cell {
                            if let Some(layer) = image.layer(chunk.place(), layer_type.bitplane_of(bit_decoded)) {
                                layer_codec.decode_cells(layer, &mut one_bit_bitmap);
                                wide::spread_into_wide(&one_bit_bitmap, layer_bits_a_cell, bit_decoded, bucket.get_or_insert_with(|| vec![0; wide::bucket_words(layer_type)].into_boxed_slice()));
                            }
                        }
                        cells.push((BucketKey { layer_type, chunk }, bucket));
                    }
                }
                JobDone::SuperchunkWarmed { generated_image, image_read_back, cells }
            }
        }
    }
}

/// A job sent, to take what it made by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct JobTicket(u64);

thread_local! {
    /// The thread's codec: one a thread that does a job.
    static THREAD_CODEC: RefCell<LayerCodec> = RefCell::new(LayerCodec::new());
}

/// The jobs sent to the dispatcher's threads, and what they made.
pub struct Jobs {
    /// The threads.
    dispatcher: Arc<Dispatcher>,
    /// Where a job done leaves what it made.
    made_by: Sender<(JobTicket, JobDone)>,
    /// What the threads made, as they made it.
    done_receiver: Receiver<(JobTicket, JobDone)>,
    /// What was made and not yet taken.
    made_not_taken: BTreeMap<JobTicket, JobDone>,
    /// The next job's ticket.
    next_ticket: u64,
}

impl Jobs {
    /// Jobs done on `dispatcher`'s threads, whichever is free: shared
    /// with the tick, which a thread busy here sits out.
    pub fn new(dispatcher: Arc<Dispatcher>) -> Self {
        let (made_by, done_receiver) = channel();
        Self { dispatcher, made_by, done_receiver, made_not_taken: BTreeMap::new(), next_ticket: 0 }
    }

    /// Sends `job` to the threads: its ticket.
    pub fn send_job(&mut self, job: Job) -> JobTicket {
        let job_ticket = JobTicket(self.next_ticket);
        self.next_ticket += 1;
        let made_by = self.made_by.clone();
        self.dispatcher.queue_job(move || {
            let job_done = catch_unwind(AssertUnwindSafe(|| THREAD_CODEC.with(|thread_codec| job.execute_with_codec(&mut thread_codec.borrow_mut())))).unwrap_or_else(|panic_payload| {
                let panic_message = panic_payload.downcast_ref::<&str>().map(|message_str| message_str.to_string()).or_else(|| panic_payload.downcast_ref::<String>().cloned());
                JobDone::JobPanicked(panic_message.unwrap_or_default())
            });
            // Nobody waiting any more: the world it was for is gone.
            let _ = made_by.send((job_ticket, job_done));
        });
        job_ticket
    }

    /// What `job_ticket`'s job made, if it has been.
    pub fn try_take(&mut self, job_ticket: JobTicket) -> Option<JobDone> {
        while let Ok((made_ticket, made_done)) = self.done_receiver.try_recv() {
            self.made_not_taken.insert(made_ticket, made_done);
        }
        self.made_not_taken.remove(&job_ticket).map(panic_if_failed)
    }

    /// What `job_ticket`'s job made, waiting for it.
    pub fn take_waiting(&mut self, job_ticket: JobTicket) -> JobDone {
        loop {
            if let Some(taken_done) = self.made_not_taken.remove(&job_ticket) {
                return panic_if_failed(taken_done);
            }
            let (made_ticket, made_done) = self.done_receiver.recv().expect("its own sender kept");
            self.made_not_taken.insert(made_ticket, made_done);
        }
    }
}

/// `job_done`, unless its job panicked: then the panic carried on here.
fn panic_if_failed(job_done: JobDone) -> JobDone {
    match job_done {
        JobDone::JobPanicked(panic_message) => panic!("a job of chunk storage's panicked: {panic_message}"),
        job_succeeded => job_succeeded,
    }
}
