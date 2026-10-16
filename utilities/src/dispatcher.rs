//! The thread dispatcher: worker threads started once and kept, a job
//! run on all at once ([`Dispatcher::run_in_parts`]) and jobs queued for
//! whichever is free ([`Dispatcher::queue_job`]): `docs/utilities.md`,
//! "The dispatcher".

use std::collections::VecDeque;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;

/// A job run: once a part, given the part's number.
type Job = dyn Fn(usize) + Sync;

/// A job queued: done once, by one worker.
type QueuedJob = Box<dyn FnOnce() + Send>;

/// A job run, handed to the workers for no longer than
/// [`Dispatcher::run_in_parts`] waits on them.
#[derive(Clone, Copy)]
struct JobPointer(*const Job);

// Safety: the job is `Sync`, so its parts may run on any thread; the
// pointer is only followed while `Dispatcher::run_in_parts` waits.
unsafe impl Send for JobPointer {}

/// What the dispatcher and its workers share, behind the lock.
struct DispatcherState {
    /// The job running, if any.
    job: Option<JobPointer>,
    /// Counts the jobs run: a worker runs its part of each new one once.
    run_generation: u64,
    /// Workers still running their part of the job.
    parts_running: usize,
    /// Whether a worker's part panicked.
    part_panicked: bool,
    /// Whether the workers are to stop.
    quit_requested: bool,
    /// The jobs queued, the oldest first.
    queued_jobs: VecDeque<QueuedJob>,
    /// How many workers are busy with a queued job: they sit a job run
    /// out.
    busy_workers: usize,
}

/// The lock and the two waits on it.
struct DispatcherShared {
    /// The state.
    locked_state: Mutex<DispatcherState>,
    /// Signalled when a job is run or queued, or the workers are to stop.
    work_given: Condvar,
    /// Signalled when the last worker finishes its part.
    job_done: Condvar,
}

/// Worker threads, kept between jobs.
pub struct Dispatcher {
    /// What the dispatcher and the workers share.
    shared_state: Arc<DispatcherShared>,
    /// The workers: one fewer than the threads, the caller's being one.
    worker_threads: Vec<JoinHandle<()>>,
    /// Held for the whole of a job run: one at a time, whoever calls.
    one_run: Mutex<()>,
}

impl Dispatcher {
    /// A dispatcher of `thread_count` threads: this one, and `thread_count - 1`
    /// workers started now and kept. At least one.
    pub fn new(thread_count: usize) -> Self {
        let shared_state = Arc::new(DispatcherShared {
            locked_state: Mutex::new(DispatcherState { job: None, run_generation: 0, parts_running: 0, part_panicked: false, quit_requested: false, queued_jobs: VecDeque::new(), busy_workers: 0 }),
            work_given: Condvar::new(),
            job_done: Condvar::new(),
        });
        let worker_threads = (1..thread_count.max(1))
            .map(|part| {
                let worker_shared = Arc::clone(&shared_state);
                std::thread::spawn(move || worker_loop(&worker_shared, part))
            })
            .collect();
        Self { shared_state, worker_threads, one_run: Mutex::new(()) }
    }

    /// A dispatcher of every thread the machine has.
    pub fn of_the_machine() -> Self {
        Self::new(std::thread::available_parallelism().map_or(1, usize::from))
    }

    /// How many threads there are: the most parts a job run is split
    /// into.
    pub fn thread_count(&self) -> usize {
        self.worker_threads.len() + 1
    }

    /// Runs `job` once a part, part 0 on this thread and each other on
    /// a worker not busy with a queued job, and returns once every part
    /// started has finished: a part's panic is raised here, after. One
    /// job is run at a time: a second caller waits for the first. A
    /// job run must not itself call this -- it would wait on itself.
    pub fn run_in_parts(&self, job: &(dyn Fn(usize) + Sync)) {
        if self.worker_threads.is_empty() {
            job(0);
            return;
        }
        // The state holds one job: two runs at once would hand a worker the wrong one, and one already let go of.
        // A run that panicked left nothing half done behind the lock.
        let _one_run = self.one_run.lock().unwrap_or_else(PoisonError::into_inner);
        // Safety: the lifetime is erased only for the workers to hold the
        // job while this call waits for them: it returns after the last
        // has finished with it, so the job outlives every use -- and
        // no other call hands them another meanwhile (`one_run`).
        let erased_job: *const Job = unsafe { std::mem::transmute::<*const (dyn Fn(usize) + Sync + '_), *const Job>(job) };
        {
            let mut start_guard = self.shared_state.locked_state.lock().expect("the dispatcher's lock");
            start_guard.job = Some(JobPointer(erased_job));
            start_guard.run_generation += 1;
            start_guard.parts_running = self.worker_threads.len() - start_guard.busy_workers;
            start_guard.part_panicked = false;
        }
        self.shared_state.work_given.notify_all();
        let own_part_result = catch_unwind(AssertUnwindSafe(|| job(0)));
        let mut finish_guard = self.shared_state.locked_state.lock().expect("the dispatcher's lock");
        while finish_guard.parts_running > 0 {
            finish_guard = self.shared_state.job_done.wait(finish_guard).expect("the dispatcher's lock");
        }
        finish_guard.job = None;
        let worker_panicked = finish_guard.part_panicked;
        drop(finish_guard);
        if let Err(panic_payload) = own_part_result {
            resume_unwind(panic_payload);
        }
        assert!(!worker_panicked, "a part of the job panicked on a worker");
    }

    /// Queues `job`, done once by a worker when one is free -- or here
    /// and now, there being no worker. What it makes it hands over
    /// itself; its panic is its own to catch, and is dropped here.
    pub fn queue_job(&self, job: impl FnOnce() + Send + 'static) {
        if self.worker_threads.is_empty() {
            let _ = catch_unwind(AssertUnwindSafe(job));
            return;
        }
        self.shared_state.locked_state.lock().expect("the dispatcher's lock").queued_jobs.push_back(Box::new(job));
        self.shared_state.work_given.notify_one();
    }
}

/// What a worker found to do.
enum FoundWork {
    /// Its part of a job run.
    Part(JobPointer),
    /// A job queued.
    JobQueued(QueuedJob),
}

/// A worker's life: waits for work, a job run before one queued, does
/// it, says so.
fn worker_loop(dispatcher_shared: &DispatcherShared, part: usize) {
    let mut seen_generation = 0;
    loop {
        let found_work = {
            let mut wait_guard = dispatcher_shared.locked_state.lock().expect("the dispatcher's lock");
            loop {
                if wait_guard.quit_requested {
                    return;
                }
                if wait_guard.run_generation != seen_generation {
                    seen_generation = wait_guard.run_generation;
                    break FoundWork::Part(wait_guard.job.expect("a job handed out"));
                }
                if let Some(queued_job) = wait_guard.queued_jobs.pop_front() {
                    wait_guard.busy_workers += 1;
                    break FoundWork::JobQueued(queued_job);
                }
                wait_guard = dispatcher_shared.work_given.wait(wait_guard).expect("the dispatcher's lock");
            }
        };
        match found_work {
            FoundWork::Part(job) => {
                // Safety: `Dispatcher::run_in_parts` holds the job until this part is done.
                let part_result = catch_unwind(AssertUnwindSafe(|| unsafe { (*job.0)(part) }));
                let mut part_guard = dispatcher_shared.locked_state.lock().expect("the dispatcher's lock");
                part_guard.part_panicked |= part_result.is_err();
                part_guard.parts_running -= 1;
                if part_guard.parts_running == 0 {
                    dispatcher_shared.job_done.notify_one();
                }
            }
            FoundWork::JobQueued(job_to_run) => {
                let _ = catch_unwind(AssertUnwindSafe(job_to_run));
                let mut queued_guard = dispatcher_shared.locked_state.lock().expect("the dispatcher's lock");
                queued_guard.busy_workers -= 1;
                // A job run meanwhile did not count on this worker: its part is not run.
                seen_generation = queued_guard.run_generation;
            }
        }
    }
}

impl Drop for Dispatcher {
    /// Stops the workers, each once its job is done, and waits for
    /// them; jobs queued and not begun are dropped.
    fn drop(&mut self) {
        self.shared_state.locked_state.lock().expect("the dispatcher's lock").quit_requested = true;
        self.shared_state.work_given.notify_all();
        for worker_thread in self.worker_threads.drain(..) {
            let _ = worker_thread.join();
        }
    }
}
