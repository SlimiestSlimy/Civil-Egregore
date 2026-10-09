//! The dispatcher: every part of a job run once, on threads kept between
//! jobs, borrowing what the caller holds; a part's panic raised to the
//! caller, after every part is done; jobs queued done once each, by
//! the workers or, there being none, at once; and a worker busy with
//! one sitting a job run out.
//!
//! `cargo test`

use utilities::dispatcher::Dispatcher;
use std::sync::mpsc::channel;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Every part runs once a job, job after job, and a job borrowing the
/// caller's data sees it whole.
#[test]
fn every_part_runs_once_a_job() {
    let dispatcher = Dispatcher::new(4);
    assert_eq!(dispatcher.threads(), 4);
    let runs: Vec<AtomicUsize> = (0..4).map(|_| AtomicUsize::new(0)).collect();
    let numbers: Vec<usize> = (0..1000).collect();
    let sums: Vec<AtomicUsize> = (0..4).map(|_| AtomicUsize::new(0)).collect();
    for _ in 0..100 {
        dispatcher.run(&|part| {
            runs[part].fetch_add(1, Ordering::Relaxed);
            let sum: usize = numbers.iter().skip(part).step_by(4).sum();
            sums[part].store(sum, Ordering::Relaxed);
        });
    }
    assert!(runs.iter().all(|runs| runs.load(Ordering::Relaxed) == 100));
    assert_eq!(sums.iter().map(|sum| sum.load(Ordering::Relaxed)).sum::<usize>(), numbers.iter().sum());
}

/// One thread is the caller's alone.
#[test]
fn one_thread_runs_on_the_caller() {
    let dispatcher = Dispatcher::new(1);
    let caller = std::thread::current().id();
    dispatcher.run(&|part| assert_eq!((part, std::thread::current().id()), (0, caller)));
}

/// A part's panic on a worker reaches the caller, and the dispatcher
/// still runs jobs after.
#[test]
fn a_workers_panic_reaches_the_caller() {
    let dispatcher = Dispatcher::new(3);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        dispatcher.run(&|part| assert_ne!(part, 2, "part 2 fails"));
    }));
    assert!(result.is_err());
    let ran = AtomicUsize::new(0);
    dispatcher.run(&|_| {
        ran.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(ran.load(Ordering::Relaxed), 3);
}

/// Jobs queued are each done once, off the caller's thread while there
/// are workers and on it when there are none; one that panics takes
/// nothing with it.
#[test]
fn jobs_queued_are_done_once_each() {
    for threads in [1, 3] {
        let dispatcher = Dispatcher::new(threads);
        let (made, done) = channel();
        dispatcher.queue(|| panic!("a job of its own failing"));
        for job in 0..50u32 {
            let made = made.clone();
            dispatcher.queue(move || made.send((job, std::thread::current().id())).expect("the test waiting"));
        }
        let mut jobs: Vec<(u32, std::thread::ThreadId)> = (0..50).map(|_| done.recv().expect("a job done")).collect();
        jobs.sort_unstable_by_key(|job| job.0);
        assert!(jobs.iter().map(|job| job.0).eq(0..50), "{threads} threads");
        assert!(jobs.iter().all(|job| (job.1 == std::thread::current().id()) == (threads == 1)), "{threads} threads");
    }
}

/// A job run while workers are busy with queued jobs is run by the
/// rest, and returns without them; free again, they take their parts.
#[test]
fn a_worker_busy_with_a_queued_job_sits_a_run_out() {
    let dispatcher = Dispatcher::new(3);
    let (begun_by, begun) = channel();
    let (release, released) = channel::<()>();
    let released = std::sync::Mutex::new(released);
    let released = std::sync::Arc::new(released);
    for _ in 0..2 {
        let (begun_by, released) = (begun_by.clone(), std::sync::Arc::clone(&released));
        dispatcher.queue(move || {
            begun_by.send(()).expect("the test waiting");
            let _ = released.lock().expect("the release").recv();
        });
    }
    // One job holds the release, the other waits on its lock: both workers busy.
    begun.recv().expect("one begun");
    begun.recv().expect("the other begun");
    let ran = AtomicUsize::new(0);
    dispatcher.run(&|part| {
        assert_eq!(part, 0, "only the caller is free");
        ran.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(ran.load(Ordering::Relaxed), 1);
    drop(release);
    // Free again, every part is run: a worker still finishing may sit one more out.
    let ran = AtomicUsize::new(0);
    while ran.swap(0, Ordering::Relaxed) != 3 {
        dispatcher.run(&|_| {
            ran.fetch_add(1, Ordering::Relaxed);
        });
    }
}
