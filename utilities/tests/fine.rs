//! The fine tier: one case a test, made by hand, each pinning one behaviour -- instant.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

mod fixed_list {
    //! The fixed-capacity list.
    //!
    //! `cargo test`

    use utilities::fixed_list::FixedList;

    /// Items come back in order, clearing keeps the room, and pushing
    /// past the capacity panics.
    #[test]
    fn holds_up_to_its_capacity() {
        let mut list: FixedList<u8, 3> = FixedList::new();
        for item in [1, 2, 3] {
            list.push(item);
        }
        assert_eq!(&*list, &[1, 2, 3]);
        assert_eq!(list.pop(), Some(3));
        list.clear();
        assert!(list.is_empty());
        for item in [4, 5, 6] {
            list.push(item);
        }
        assert!(std::panic::catch_unwind(move || list.push(7)).is_err());
    }
}

mod hash {
    //! Hashing: keys to slots, and words mixed.
    //!
    //! `cargo test`

    use utilities::hash::{mix, slot, GOLDEN_RATIO};
    use utilities::rng::Rng;

    /// Consecutive keys land in every slot of a table, each slot in range,
    /// and a word mixed is what the random source draws from its state.
    #[test]
    fn keys_spread_over_every_slot_and_words_mix_as_draws_do() {
        for slots in [2, 16, 1 << 10] {
            let mut hit = vec![false; slots];
            for key in 0..slots as u64 * 4 {
                hit[slot(key, slots)] = true;
            }
            assert!(hit.iter().all(|&hit| hit), "{slots} slots");
        }
        let mut random = Rng::new(99);
        assert_eq!(random.draw(), mix(99u64.wrapping_add(GOLDEN_RATIO)));
    }
}

mod process_memory {
    //! The process's memory: read, and tracked over a run.
    //!
    //! `cargo test`

    use utilities::diagnostics::process_memory::{process_memory, MemoryTrack};

    /// On Linux the process holds some memory, never more than its peak, and
    /// a track of it averages between nothing and the peak.
    #[test]
    fn memory_is_read_and_tracked() {
        let Some(memory) = process_memory() else {
            return;
        };
        assert!(memory.resident > 0 && memory.resident <= memory.peak);
        let mut track = MemoryTrack::default();
        // Kept from being optimized away, so the memory is really held.
        let held = std::hint::black_box(vec![1u8; 32 << 20]);
        track.read();
        drop(held);
        track.read();
        let (average, peak) = (track.average().expect("read"), track.peak().expect("read"));
        assert!(average > 0 && average <= peak && peak >= 32 << 20);
    }
}

mod rng {

    /// Streams of one seed are apart: no draw of one turns up early in
    /// another, as it would were a stream the seed moved along.
    #[test]
    fn streams_of_a_seed_share_no_draws() {
        let mut seen = std::collections::HashSet::new();
        for stream in 0..64u64 {
            let mut random = utilities::rng::Rng::for_stream(42, stream);
            for _ in 0..1_000 {
                assert!(seen.insert(random.draw()), "a draw of stream {stream} seen before");
            }
        }
    }
}

mod table {
    //! The table printer's text forms: a table and a report written out and
    //! read back.
    //!
    //! `cargo test`

    use utilities::csv::{lines, Line};
    use utilities::diagnostics::table::report::Report;
    use utilities::transient_data::TRANSIENT_DATA;
    use utilities::diagnostics::table::Table;

    /// A table with every awkward field -- a comma, a quote, a newline, an
    /// empty one, one reading as a comment or a divider -- and dividers comes back
    /// from CSV field for field, alone and inside a report.
    #[test]
    fn a_table_round_trips_through_csv() {
        let awkward = ["a, b", "say \"so\"", "two\nlines", "", "# not a note", "---"];
        let mut table = Table::new(&["name", "stacked\nheading"]);
        for field in awkward {
            table.row(&[field, "1"]);
            table.divider();
        }
        let csv = table.to_csv();
        assert_eq!(Table::from_csv(&csv).to_csv(), csv);
        let read: Vec<String> = lines(&csv)
            .into_iter()
            .filter_map(|line| match line {
                Line::Row(fields) => Some(fields[0].clone()),
                _ => None,
            })
            .collect();
        assert_eq!(read, ["name"].into_iter().chain(awkward).collect::<Vec<_>>());

        let mut report = Report::new("round trip", "a test");
        report.note("a note");
        report.add("first", table);
        report.add("second", Table::from_csv(&csv));
        let text = report.to_text();
        assert_eq!(Report::from_text("round trip", &text).to_text(), text);

        // Kept in the transient data's measurements and read back from there:
        // the same, with the commit it was measured on noted.
        let folder = TRANSIENT_DATA.measurements();
        let kept = Report::from_text("round trip", &text).keep(&folder);
        assert!(kept.starts_with(TRANSIENT_DATA.under("")) && kept.exists());
        let read_back = Report::read(&folder, "round trip").expect("kept").to_text();
        let without_commit: String = read_back.lines().filter(|line| !line.starts_with("# commit ")).map(|line| format!("{line}\n")).collect();
        assert_eq!(without_commit, text);
    }
}

mod commands {
    //! Commands: the first word finds one, which is given the rest; a
    //! parameter not given is its default.
    //!
    //! `cargo test`

    use utilities::commands::{dispatch, Command, Parameter};

    /// A command that says, as why it failed, the line it ran on and the
    /// sum of its two numbers: what it was given, seen from outside.
    const SUM: Command = Command {
        name: "sum",
        does: "adds two numbers",
        parameters: &[Parameter::new("first", "2"), Parameter::new("second", "3"), Parameter::new("note", "")],
        run: |given| Err(format!("{} = {}{}", given.resolved(), given.number::<u32>("first")? + given.number::<u32>("second")?, given.given("note").unwrap_or(""))),
    };

    /// The command named is run on the words after its name, a parameter
    /// not given being its default, and its line says so; a word that names
    /// none, or no word, gives the usage; a number that is none is refused.
    #[test]
    fn the_first_word_names_the_command_and_the_rest_are_its_parameters() {
        let run = |arguments: &[&str]| dispatch("program crate", &[SUM], arguments).expect_err("the command says what it was given");
        assert_eq!(run(&["sum"]), "program crate sum 2 3 = 5");
        assert_eq!(run(&["sum", "10"]), "program crate sum 10 3 = 13");
        assert_eq!(run(&["sum", "10", "20", "!"]), "program crate sum 10 20 ! = 30!");
        assert_eq!(run(&["sum", "ten"]), "first: `ten` is not a number");
        for unnamed in [&[][..], &["product", "1"]] {
            let usage = run(unnamed);
            assert!(usage.contains("program crate sum") && usage.contains("[first = 2] [second = 3] [note]") && usage.contains("adds two numbers"), "{usage}");
        }
    }
}

mod dispatcher {
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
}

mod settings {
    //! Settings: a file written, in CSV, is read back the same, another's rows
    //! left as they are; a machine with no file is given the default
    //! settings, and one with a file keeps what it has.
    //!
    //! `cargo test`

    use utilities::settings::{file, folder, world_in, world_name, Settings, FILE, FOLDER, WORLDS};

    /// The file is the one file in Civil Egregore's one folder.
    #[test]
    fn the_file_is_in_civil_egregores_folder() {
        assert!(folder().ends_with(FOLDER) && file() == folder().join(FILE));
    }

    /// Settings set, changed and unset are written and read back as they
    /// stand, a row each under the one naming the columns; a setting never set has none.
    #[test]
    fn settings_are_kept_a_line_each() {
        let path = utilities::transient_data::TransientData::of(env!("CARGO_MANIFEST_DIR")).under("tests/settings").join(FILE);
        let mut settings = Settings::default();
        settings.set("wall length", Some("0.75".to_string()));
        settings.set("another's", Some("kept as it is".to_string()));
        settings.set("ocean level", Some("300".to_string()));
        settings.set("ocean level", Some("412".to_string()));
        settings.set("wall length", None);
        settings.set("never set", None);
        settings.write_to(&path).expect("written");
        assert_eq!(std::fs::read_to_string(&path).expect("the file"), "setting,value\nanother's,kept as it is\nocean level,412\n");
        let read = Settings::read_from(&path);
        assert_eq!(read, settings);
        assert_eq!((read.number::<u32>("ocean level"), read.number::<f32>("another's"), read.get("wall length")), (Some(412), None, None));
        assert_eq!(Settings::read_from(&path.with_file_name("none")), Settings::default());
    }

    /// With no file, the default settings are copied to where it belongs;
    /// with one, it is left as it is, and what it has stands before the
    /// default settings, which give the rest.
    #[test]
    fn the_default_settings_never_replace_a_machines() {
        let folder = utilities::transient_data::TransientData::of(env!("CARGO_MANIFEST_DIR")).under("tests/settings");
        let (none, some) = (folder.join("started"), folder.join("kept"));
        _ = std::fs::remove_file(&none);
        let defaults = Settings::defaults();
        assert!(defaults.get(WORLDS).is_some(), "the default settings name the worlds' folder");
        assert_eq!(Settings::read_or_start(&none), defaults);
        assert_eq!(Settings::read_from(&none), defaults, "the default settings, copied");

        let own = "setting,value\nworlds,/somewhere/else\nanother's,7\n";
        std::fs::write(&some, own).expect("written");
        let read = Settings::read_or_start(&some);
        assert_eq!(std::fs::read_to_string(&some).expect("the file"), own, "the machine's file as it was");
        assert_eq!((read.get(WORLDS), read.get("another's")), (Some("/somewhere/else"), Some("7")));
        let (name, value) = ("ocean share", defaults.get("ocean share").expect("a default"));
        assert_eq!(read.get(name), Some(value), "what the file lacks, from the default settings");
    }

    /// A world named plainly is in the worlds' folder; a path is itself.
    #[test]
    fn a_world_named_plainly_is_in_the_worlds_folder() {
        let worlds = folder().join(WORLDS);
        assert_eq!(world_in(&worlds, "Meadow"), worlds.join("Meadow"));
        assert_eq!(world_in(&worlds, " Up: the <hills>? "), worlds.join("Up the hills"), "only what a folder may be named");
        assert_eq!((world_name("con.txt"), world_name("Lpt7"), world_name("Common"), world_name(" ./\\. ")), (Some("_con.txt".to_string()), Some("_Lpt7".to_string()), Some("Common".to_string()), None));
        let path = std::path::Path::new("some").join("where");
        assert_eq!(world_in(&worlds, path.to_str().expect("text")), path);
    }
}
