//! The thread dispatcher: worker threads started once and kept, a job
//! run on all at once ([`Dispatcher::run`]) and jobs queued for
//! whichever is free ([`Dispatcher::queue`]): `docs/utilities.md`,
//! "The dispatcher".

use std::collections::VecDeque;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;

/// A job run: once a part, given the part's number.
type Job = dyn Fn(usize) + Sync;

/// A job queued: done once, by one worker.
type Queued = Box<dyn FnOnce() + Send>;

/// A job run, handed to the workers for no longer than
/// [`Dispatcher::run`] waits on them.
#[derive(Clone, Copy)]
struct JobPointer(*const Job);

// Safety: the job is `Sync`, so its parts may run on any thread; the
// pointer is only followed while `Dispatcher::run` waits.
unsafe impl Send for JobPointer {}

/// What the dispatcher and its workers share, behind the lock.
struct State {
    /// The job running, if any.
    job: Option<JobPointer>,
    /// Counts the jobs run: a worker runs its part of each new one once.
    generation: u64,
    /// Workers still running their part of the job.
    running: usize,
    /// Whether a worker's part panicked.
    panicked: bool,
    /// Whether the workers are to stop.
    quit: bool,
    /// The jobs queued, the oldest first.
    queued: VecDeque<Queued>,
    /// How many workers are busy with a queued job: they sit a job run
    /// out.
    busy: usize,
}

/// The lock and the two waits on it.
struct Shared {
    /// The state.
    state: Mutex<State>,
    /// Signalled when a job is run or queued, or the workers are to stop.
    work_given: Condvar,
    /// Signalled when the last worker finishes its part.
    job_done: Condvar,
}

/// Worker threads, kept between jobs.
pub struct Dispatcher {
    /// What the dispatcher and the workers share.
    shared: Arc<Shared>,
    /// The workers: one fewer than the threads, the caller's being one.
    workers: Vec<JoinHandle<()>>,
    /// Held for the whole of a job run: one at a time, whoever calls.
    one_run: Mutex<()>,
}

impl Dispatcher {
    /// A dispatcher of `threads` threads: this one, and `threads - 1`
    /// workers started now and kept. At least one.
    pub fn new(threads: usize) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State { job: None, generation: 0, running: 0, panicked: false, quit: false, queued: VecDeque::new(), busy: 0 }),
            work_given: Condvar::new(),
            job_done: Condvar::new(),
        });
        let workers = (1..threads.max(1))
            .map(|part| {
                let shared = Arc::clone(&shared);
                std::thread::spawn(move || work(&shared, part))
            })
            .collect();
        Self { shared, workers, one_run: Mutex::new(()) }
    }

    /// A dispatcher of every thread the machine has.
    pub fn of_the_machine() -> Self {
        Self::new(std::thread::available_parallelism().map_or(1, usize::from))
    }

    /// How many threads there are: the most parts a job run is split
    /// into.
    pub fn threads(&self) -> usize {
        self.workers.len() + 1
    }

    /// Runs `job` once a part, part 0 on this thread and each other on
    /// a worker not busy with a queued job, and returns once every part
    /// started has finished: a part's panic is raised here, after. One
    /// job is run at a time: a second caller waits for the first. A
    /// job run must not itself call this -- it would wait on itself.
    pub fn run(&self, job: &(dyn Fn(usize) + Sync)) {
        if self.workers.is_empty() {
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
        let erased: *const Job = unsafe { std::mem::transmute::<*const (dyn Fn(usize) + Sync + '_), *const Job>(job) };
        {
            let mut state = self.shared.state.lock().expect("the dispatcher's lock");
            state.job = Some(JobPointer(erased));
            state.generation += 1;
            state.running = self.workers.len() - state.busy;
            state.panicked = false;
        }
        self.shared.work_given.notify_all();
        let own = catch_unwind(AssertUnwindSafe(|| job(0)));
        let mut state = self.shared.state.lock().expect("the dispatcher's lock");
        while state.running > 0 {
            state = self.shared.job_done.wait(state).expect("the dispatcher's lock");
        }
        state.job = None;
        let panicked = state.panicked;
        drop(state);
        if let Err(panic) = own {
            resume_unwind(panic);
        }
        assert!(!panicked, "a part of the job panicked on a worker");
    }

    /// Queues `job`, done once by a worker when one is free -- or here
    /// and now, there being no worker. What it makes it hands over
    /// itself; its panic is its own to catch, and is dropped here.
    pub fn queue(&self, job: impl FnOnce() + Send + 'static) {
        if self.workers.is_empty() {
            let _ = catch_unwind(AssertUnwindSafe(job));
            return;
        }
        self.shared.state.lock().expect("the dispatcher's lock").queued.push_back(Box::new(job));
        self.shared.work_given.notify_one();
    }
}

/// What a worker found to do.
enum Work {
    /// Its part of a job run.
    Part(JobPointer),
    /// A job queued.
    Queued(Queued),
}

/// A worker's life: waits for work, a job run before one queued, does
/// it, says so.
fn work(shared: &Shared, part: usize) {
    let mut seen = 0;
    loop {
        let work = {
            let mut state = shared.state.lock().expect("the dispatcher's lock");
            loop {
                if state.quit {
                    return;
                }
                if state.generation != seen {
                    seen = state.generation;
                    break Work::Part(state.job.expect("a job handed out"));
                }
                if let Some(queued) = state.queued.pop_front() {
                    state.busy += 1;
                    break Work::Queued(queued);
                }
                state = shared.work_given.wait(state).expect("the dispatcher's lock");
            }
        };
        match work {
            Work::Part(job) => {
                // Safety: `Dispatcher::run` holds the job until this part is done.
                let result = catch_unwind(AssertUnwindSafe(|| unsafe { (*job.0)(part) }));
                let mut state = shared.state.lock().expect("the dispatcher's lock");
                state.panicked |= result.is_err();
                state.running -= 1;
                if state.running == 0 {
                    shared.job_done.notify_one();
                }
            }
            Work::Queued(queued) => {
                let _ = catch_unwind(AssertUnwindSafe(queued));
                let mut state = shared.state.lock().expect("the dispatcher's lock");
                state.busy -= 1;
                // A job run meanwhile did not count on this worker: its part is not run.
                seen = state.generation;
            }
        }
    }
}

impl Drop for Dispatcher {
    /// Stops the workers, each once its job is done, and waits for
    /// them; jobs queued and not begun are dropped.
    fn drop(&mut self) {
        self.shared.state.lock().expect("the dispatcher's lock").quit = true;
        self.shared.work_given.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}
