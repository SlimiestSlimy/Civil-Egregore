//! The dispatcher: every part of a job run once, on threads kept between
//! jobs, borrowing what the caller holds; a part's panic raised to the
//! caller, after every part is done; jobs queued done once each, by
//! the workers or, there being none, at once; and a worker busy with
//! one sitting a job run out; jobs run from several threads at once
//! each whole, one after another; and a queued job running one.
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
    assert_eq!(dispatcher.thread_count(), 4);
    let runs_per_part: Vec<AtomicUsize> = (0..4).map(|_| AtomicUsize::new(0)).collect();
    let summed_numbers: Vec<usize> = (0..1000).collect();
    let part_sums: Vec<AtomicUsize> = (0..4).map(|_| AtomicUsize::new(0)).collect();
    for _ in 0..100 {
        dispatcher.run_in_parts(&|part| {
            runs_per_part[part].fetch_add(1, Ordering::Relaxed);
            let part_sum: usize = summed_numbers.iter().skip(part).step_by(4).sum();
            part_sums[part].store(part_sum, Ordering::Relaxed);
        });
    }
    assert!(runs_per_part.iter().all(|part_runs| part_runs.load(Ordering::Relaxed) == 100));
    assert_eq!(part_sums.iter().map(|stored_sum| stored_sum.load(Ordering::Relaxed)).sum::<usize>(), summed_numbers.iter().sum());
}

/// One thread is the caller's alone.
#[test]
fn one_thread_runs_on_the_caller() {
    let dispatcher = Dispatcher::new(1);
    let caller_thread = std::thread::current().id();
    dispatcher.run_in_parts(&|part| assert_eq!((part, std::thread::current().id()), (0, caller_thread)));
}

/// A part's panic on a worker reaches the caller, and the dispatcher
/// still runs jobs after.
#[test]
fn a_workers_panic_reaches_the_caller() {
    let dispatcher = Dispatcher::new(3);
    let run_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        dispatcher.run_in_parts(&|part| assert_ne!(part, 2, "part 2 fails"));
    }));
    assert!(run_result.is_err());
    let parts_ran = AtomicUsize::new(0);
    dispatcher.run_in_parts(&|_| {
        parts_ran.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(parts_ran.load(Ordering::Relaxed), 3);
}

/// Jobs queued are each done once, off the caller's thread while there
/// are workers and on it when there are none; one that panics takes
/// nothing with it.
#[test]
fn jobs_queued_are_done_once_each() {
    for thread_count in [1, 3] {
        let dispatcher = Dispatcher::new(thread_count);
        let (job_sender, job_receiver) = channel();
        dispatcher.queue_job(|| panic!("a job of its own failing"));
        for job in 0..50u32 {
            let cloned_sender = job_sender.clone();
            dispatcher.queue_job(move || cloned_sender.send((job, std::thread::current().id())).expect("the test waiting"));
        }
        let mut jobs: Vec<(u32, std::thread::ThreadId)> = (0..50).map(|_| job_receiver.recv().expect("a job done")).collect();
        jobs.sort_unstable_by_key(|job| job.0);
        assert!(jobs.iter().map(|job| job.0).eq(0..50), "{thread_count} threads");
        assert!(jobs.iter().all(|job| (job.1 == std::thread::current().id()) == (thread_count == 1)), "{thread_count} threads");
    }
}

/// A job run while workers are busy with queued jobs is run by the
/// rest, and returns without them; free again, they take their parts.
#[test]
fn a_worker_busy_with_a_queued_job_sits_a_run_out() {
    let dispatcher = Dispatcher::new(3);
    let (begun_by, begun_receiver) = channel();
    let (release_sender, release_receiver) = channel::<()>();
    let locked_release = std::sync::Mutex::new(release_receiver);
    let shared_release = std::sync::Arc::new(locked_release);
    for _ in 0..2 {
        let (begun_by, job_release) = (begun_by.clone(), std::sync::Arc::clone(&shared_release));
        dispatcher.queue_job(move || {
            begun_by.send(()).expect("the test waiting");
            let _ = job_release.lock().expect("the release").recv();
        });
    }
    // One job holds the release, the other waits on its lock: both workers busy.
    begun_receiver.recv().expect("one begun");
    begun_receiver.recv().expect("the other begun");
    let busy_parts_ran = AtomicUsize::new(0);
    dispatcher.run_in_parts(&|part| {
        assert_eq!(part, 0, "only the caller is free");
        busy_parts_ran.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(busy_parts_ran.load(Ordering::Relaxed), 1);
    drop(release_sender);
    // Free again, every part is run: a worker still finishing may sit one more out.
    let free_parts_ran = AtomicUsize::new(0);
    while free_parts_ran.swap(0, Ordering::Relaxed) != 3 {
        dispatcher.run_in_parts(&|_| {
            free_parts_ran.fetch_add(1, Ordering::Relaxed);
        });
    }
}

/// Jobs run from several threads at once are each run whole, as if
/// one after another: every part of each at most once, part 0 always,
/// and no part of another's job.
#[test]
fn jobs_run_from_several_threads_are_each_whole() {
    let dispatcher = Dispatcher::new(4);
    std::thread::scope(|thread_scope| {
        for caller_number in 0..4usize {
            let dispatcher = &dispatcher;
            thread_scope.spawn(move || {
                for run_number in 0..500 {
                    // What this run alone holds: a part that ran after it returned, or another run's, would be seen.
                    let parts: Vec<AtomicUsize> = (0..4).map(|_| AtomicUsize::new(0)).collect();
                    dispatcher.run_in_parts(&|part| {
                        parts[part].fetch_add(1, Ordering::Relaxed);
                    });
                    let part_runs: Vec<usize> = parts.iter().map(|part| part.load(Ordering::Relaxed)).collect();
                    assert!(part_runs[0] == 1 && part_runs.iter().all(|&part_ran| part_ran <= 1), "caller {caller_number}, run {run_number}: {part_runs:?}");
                }
            });
        }
    });
}

/// A queued job may run a job itself: it is done, and does not wait
/// for ever.
#[test]
fn a_queued_job_runs_a_job() {
    let dispatcher = std::sync::Arc::new(Dispatcher::new(4));
    let (result_sender, result_receiver) = channel();
    let inner_dispatcher = std::sync::Arc::clone(&dispatcher);
    dispatcher.queue_job(move || {
        let parts = AtomicUsize::new(0);
        inner_dispatcher.run_in_parts(&|_| {
            parts.fetch_add(1, Ordering::Relaxed);
        });
        // Leaves the worker without the dispatcher: the test's is the last let go of.
        result_sender.send((parts.load(Ordering::Relaxed), inner_dispatcher)).expect("the test waits");
    });
    let (parts, _within) = result_receiver.recv_timeout(std::time::Duration::from_secs(30)).expect("the queued job's run to end");
    assert!((1..=4).contains(&parts), "{parts} parts");
}
