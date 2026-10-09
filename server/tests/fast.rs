//! The fast tier: small worlds grown from a seed and run for a few thousand ticks, judged -- seconds.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fast`

mod tests;

mod commands {
    //! The commands: a world made, run and looked at, as the command line
    //! would.

    use std::fs;
    use server::commands::{info, new, run};
    use server::transient_data::saves;

    /// For worlds of ten sheep and of a thousand: made -- the origin and its
    /// halo, nine superchunks -- run twice, the second run going on from
    /// where the first saved, and looked at, each command saying what it
    /// did, the world named as its folder is; a second world in the same
    /// folder refused.
    #[test]
    fn a_world_is_made_run_and_looked_at() {
        for sheep in [10, 1_000] {
            let name = format!("commands_{sheep}");
            let folder = saves().join("tests").join(&name);
            let _ = fs::remove_dir_all(&folder);
            let folder = folder.as_path();
            let made = new(folder, &["5", &sheep.to_string()]).expect("made");
            assert!(made.starts_with(&format!("{name}, seed 0x0000000000000005: 9 superchunks, {sheep} entities, ")), "{made}");
            let first = run(folder, &["40"]).expect("run");
            assert!(first.starts_with(&format!("{name}: tick 0 -> 40, ")), "{first}");
            let second = run(folder, &["60"]).expect("run on");
            assert!(second.starts_with(&format!("{name}: tick 40 -> 100, ")), "{second}");
            assert!(info(folder).expect("looked at").starts_with(&format!("{name}: seed 0x0000000000000005, at tick 100, ")));
            assert!(new(folder, &[]).is_err(), "a world there already");
            fs::remove_dir_all(folder).expect("removed");
        }
    }
}

mod halos {
    //! Halos: between ticks, the superchunks hot and not cooling, or
    //! warming, are exactly the 3x3 about every superchunk a sheep stands
    //! in; a superchunk gone cold
    //! comes back as it was, to the cell, the entity and the random number.
    //!
    //! `cargo test`

    use chunk_storage::mock::GRASS;
    use coordinates::{SuperchunkIndex, WORLD_MIDDLE};
    use bitplane_manager::{Write, WriteOp};
    use entity_manager::{Attribute, EntityId, EntityType, Header, NEVER};
    use server::host::frame::Viewport;
    use server::{about, HaloChange, World, CAMERA_SIDE, COOL_TICKS, HOT_ENTITY, WARM_TICKS};

    /// The superchunks holding a hot entity.
    fn hot_entities(world: &World) -> Vec<SuperchunkIndex> {
        world.entities.superchunks().iter().filter(|superchunk| superchunk.iter().any(|entity| entity.header.kind == HOT_ENTITY)).map(|superchunk| superchunk.index()).collect()
    }

    /// A flock wandering off its origin for 6,000 ticks, or as many as it takes a halo to move: after every tick
    /// the superchunks hot and not cooling, or warming, are the halos about
    /// the sheep, never both; every one warming turns hot, wanted still or not,
    /// and every one cooling cold, when due; every superchunk ever made is hot or cold and
    /// never both; and the world has grown.
    #[test]
    fn the_hot_superchunks_are_the_halos() {
        let mut world = server::start(server::Start { seed: crate::tests::land_seed(1), sheep: 4_000, ..server::Start::default() });
        assert_eq!(world.arena.superchunk_indices(), about([WORLD_MIDDLE].into_iter()), "the origin's halo");
        let mut moved = HaloChange::default();
        // 6,000 ticks at least, and on until a superchunk has been generated: on some seeds the flock is long in nearing an edge.
        while world.entities.now() < 6_000 || moved.generated == 0 {
            assert!(world.entities.now() < 60_000, "the halos moved");
            moved += world.tick().halos;
            let hot = world.arena.superchunk_indices();
            let warming: Vec<(SuperchunkIndex, u64)> = world.warming().collect();
            let cooling: Vec<(SuperchunkIndex, u64)> = world.cooling().collect();
            assert!(cooling.iter().all(|(superchunk, _)| hot.binary_search(superchunk).is_ok()), "cooling, hot still");
            let staying = hot.iter().filter(|superchunk| cooling.binary_search_by_key(superchunk, |(cooling, _)| cooling).is_err());
            // A warming is never given up: one its halo has left is warming still, and no halo's.
            let wanted = about(hot_entities(&world).into_iter());
            let mut halos: Vec<SuperchunkIndex> = staying.copied().chain(warming.iter().map(|&(superchunk, _)| superchunk).filter(|superchunk| wanted.binary_search(superchunk).is_ok())).collect();
            halos.sort_unstable();
            assert!(warming.iter().all(|(superchunk, _)| hot.binary_search(superchunk).is_err()), "warming, not hot yet");
            assert_eq!(halos, wanted, "hot and not cooling, or warming, never both");
            let now = world.entities.now();
            let within = |due: &[(SuperchunkIndex, u64)], ticks: u64| due.iter().all(|&(_, due)| due > now && due <= now + ticks);
            assert!(within(&warming, WARM_TICKS) && within(&cooling, COOL_TICKS), "each due within its time");
            assert!(hot.iter().all(|superchunk| !world.cold.contains_key(superchunk)), "hot or cold, never both");
            assert_eq!(world.storage.superchunks().count(), hot.len() + world.cold.len(), "every superchunk made, hot or cold");
        }
        // Every one reached turned hot, or is warming still: none is given up.
        assert!(moved.generated > 0 && moved.reached == moved.generated + moved.restored + world.warming().count(), "the halos moved, every one reached made hot: {moved:?}");
    }

    /// A superchunk a halo reaches is warming for its time: a write to it is
    /// missed and an entity put there lost, until it turns hot at the tick
    /// it is due -- still wanted or not -- and then both hold.
    #[test]
    fn a_superchunk_warming_takes_nothing_until_it_turns_hot() {
        let mut world = server::start(server::Start { seed: crate::tests::land_seed(1), sheep: 4_000, ..server::Start::default() });
        let try_both = |world: &mut World, cell| {
            world.arena.queue(GRASS, Write::cell(cell, WriteOp::Flip));
            let missed = world.arena.apply().missed;
            world.entities.queue_put(Header { id: EntityId(u64::MAX), kind: EntityType(99), at: cell, wake: NEVER }, &[]);
            (missed, world.entities.apply().lost)
        };
        while world.warming().next().is_none() {
            assert!(world.entities.now() < 60_000, "the halos moved");
            world.tick();
        }
        let (superchunk, due) = world.warming().next().expect("one warming");
        let cell = superchunk.chunks().next().expect("a chunk").top_left();
        assert_eq!(try_both(&mut world, cell), (1, 1), "warming: the write missed, the entity lost");
        while world.entities.now() < due {
            assert!(world.arena.superchunk_indices().binary_search(&superchunk).is_err(), "not hot before it is due");
            world.tick();
        }
        assert!(world.arena.superchunk_indices().binary_search(&superchunk).is_ok(), "hot when due, its halo there still or gone");
        assert_eq!(try_both(&mut world, cell), (0, 0), "hot: both hold");
    }

    /// A lone sheep taken away: its halo is cooling, hot still, for its
    /// time; the sheep back before it is due, the halo stays hot; taken away
    /// again, cooling across a save and a load, it goes cold when due.
    #[test]
    fn a_superchunk_cooling_stays_hot_until_due() {
        let mut world = server::start(server::Start { seed: crate::tests::land_seed(1), sheep: 1, ..server::Start::default() });
        let halo = world.arena.superchunk_indices();
        let sheep = world.entities.iter().map(|entity| (entity.header, entity.attributes.to_vec())).next().expect("the sheep");
        let take_away = |world: &mut World| {
            world.entities.queue_remove(&sheep.0);
            world.entities.apply();
            world.tick().halos
        };
        let cooling = |world: &World| world.cooling().map(|(superchunk, _)| superchunk).collect::<Vec<_>>();

        assert_eq!(take_away(&mut world).cooled, 0, "none cold at once");
        let due = world.entities.now() + COOL_TICKS;
        assert!(cooling(&world) == halo && world.cooling().all(|(_, at)| at == due), "the halo cooling, due {COOL_TICKS} on");
        while world.entities.now() < due - COOL_TICKS / 2 {
            world.tick();
        }
        // Back asleep over the tick that follows: awake, it might step over an edge and move its halo.
        world.entities.queue_put(Header { wake: sheep.0.wake.max(world.entities.now() + 2), ..sheep.0 }, &sheep.1);
        world.entities.apply();
        let back = world.tick().halos;
        assert!(back == HaloChange::default() && world.cooling().next().is_none() && world.arena.superchunk_indices() == halo, "the sheep back: the halo hot as it was, nothing made");

        take_away(&mut world);
        let due = world.entities.now() + COOL_TICKS;
        let folder = server::transient_data::saves().join("tests").join("cooling");
        server::save(&folder, &mut world).expect("saved");
        world = server::load(&folder).expect("loaded");
        assert!(cooling(&world) == halo && world.cooling().all(|(_, at)| at == due), "cooling as it was");
        let mut cooled = 0;
        while world.entities.now() < due {
            assert_eq!(world.arena.superchunk_indices(), halo, "hot until due");
            cooled += world.tick().halos.cooled;
        }
        assert!(cooled == halo.len() && world.arena.superchunk_indices().is_empty() && world.cooling().next().is_none(), "all cold when due");
    }

    /// Every superchunk made cold, then the origin's halo hot again: its
    /// cells, its sheep and their attributes, and its random numbers are as
    /// they were -- and it ticks on as it would have. Once its bitmaps
    /// lingering are kept; once flushed and let go, so decoded from its images.
    #[test]
    fn a_superchunk_gone_cold_comes_back_as_it_was() {
        let mut world = server::start(server::Start { seed: crate::tests::land_seed(2), sheep: 2_000, ..server::Start::default() });
        // 500 ticks, and on to one with nothing warming or cooling: made cold and hot again by hand, one warming would not turn hot when it was due.
        while world.entities.now() < 500 || world.warming().next().is_some() || world.cooling().next().is_some() {
            assert!(world.entities.now() < 60_000, "the halos at rest");
            world.tick();
        }
        let ticked = world.entities.now();
        let halo = world.arena.superchunk_indices();
        type Held = (Vec<u64>, Vec<(Header, Vec<Attribute>)>, Vec<(SuperchunkIndex, u64)>);
        let held = |world: &World| -> Held {
            let cells = world.info.layers.clone().into_iter().flat_map(|layer| world.arena.run(layer)).flat_map(|(_, bucket)| bucket.words().to_vec()).collect();
            (cells, world.entities.iter().map(|entity| (entity.header, entity.attributes.to_vec())).collect(), world.simulation.random_states().collect())
        };
        let before = held(&world);
        let mut twin = server::start(server::Start { seed: crate::tests::land_seed(2), sheep: 2_000, ..server::Start::default() });
        for _ in 0..ticked {
            twin.tick();
        }

        let cooled = world.keep_hot(&[]);
        assert_eq!((cooled.cooled, world.arena.superchunk_indices().len(), world.entities.len()), (halo.len(), 0, 0), "all cold");
        assert_eq!(world.cold.len(), halo.len());
        let restored = world.keep_hot(&halo);
        assert_eq!((restored.restored, restored.generated), (halo.len(), 0), "restored as they were, none generated");
        assert!(held(&world) == before, "as it was");

        for _ in 0..500 {
            world.tick();
            twin.tick();
        }
        assert!(held(&world) == held(&twin), "ticks on as it would have");

        // On to a tick with nothing warming or cooling: on some seeds a sheep is by an edge just now.
        let unsettled = |world: &World| world.warming().next().is_some() || world.cooling().next().is_some();
        for _ in 0..20_000 {
            if !unsettled(&world) {
                break;
            }
            world.tick();
            twin.tick();
        }
        // Cold again, and saved: every change flushed into the images, the bitmaps lingering let go; made hot, decoded from those images.
        let halos = world.arena.superchunk_indices();
        assert!(world.warming().next().is_none() && world.cooling().next().is_none() && halos == twin.arena.superchunk_indices(), "the two alike, none warming or cooling");
        world.keep_hot(&[]);
        server::save(&server::transient_data::saves().join("tests").join("gone_cold"), &mut world).expect("saved");
        assert_eq!(world.arena.lingering(), 0, "flushed, so let go");
        world.keep_hot(&halos);
        for _ in 0..500 {
            world.tick();
            twin.tick();
        }
        assert!(held(&world) == held(&twin), "decoded as it was, and ticks on as it would have");
    }

    /// A world of a size has sheep on every superchunk of it, and is
    /// never hot outside it, whoever wants it and whatever the flock
    /// does; a save keeps its size. Forced hot, all of it is hot with no
    /// sheep at all, and stays so, saved and loaded.
    #[test]
    fn a_world_of_a_size_is_hot_within_it_only() {
        let side = 4;
        let size = server::Size::Limited { side, forced: false };
        let mut world = server::start(server::Start { seed: crate::tests::land_seed(1), size, sheep: 500, ..server::Start::default() });
        let hot = world.arena.superchunk_indices();
        assert!(hot.len() == (side * side) as usize && hot.iter().all(|&superchunk| world.halos.hot.within(superchunk)), "sheep everywhere in it: all of it hot, nothing outside it");
        assert!(hot.iter().all(|&superchunk| world.entities.superchunk(superchunk).is_some_and(|kept| !kept.is_empty())), "sheep on every superchunk");
        world.keep_hot(&about(hot.into_iter()));
        assert_eq!(world.arena.superchunk_indices().len(), (side * side) as usize, "nothing made hot outside it, whoever wants it");
        for _ in 0..2 * WARM_TICKS {
            world.tick();
            assert!(world.arena.superchunk_indices().into_iter().chain(world.warming().map(|(superchunk, _)| superchunk)).all(|superchunk| world.halos.hot.within(superchunk)));
        }
        let folder = server::transient_data::saves().join("tests").join("sized");
        _ = std::fs::remove_dir_all(&folder);
        server::save(&folder, &mut world).expect("saved");
        assert_eq!(server::load(&folder).expect("loaded").halos.hot, world.halos.hot, "its size, kept");

        let forced = server::Size::Limited { side, forced: true };
        let mut world = server::start(server::Start { seed: crate::tests::land_seed(1), size: forced, sheep: 0, ..server::Start::default() });
        assert_eq!(world.arena.superchunk_indices().len(), (side * side) as usize, "forced hot: all of it, no sheep needed");
        _ = std::fs::remove_dir_all(&folder);
        server::save(&folder, &mut world).expect("saved");
        let mut loaded = server::load(&folder).expect("loaded");
        assert_eq!(loaded.halos.hot, world.halos.hot, "forced, kept");
        for _ in 0..=WARM_TICKS {
            loaded.tick();
        }
        assert_eq!(loaded.arena.superchunk_indices().len(), (side * side) as usize, "forced hot: all of it, all the while");
        assert_eq!((loaded.warming().count(), loaded.cooling().count()), (0, 0));
    }

    /// A world whose camera loads superchunks: a viewport far wider than
    /// [`CAMERA_SIDE`] keeps only so many a side about its middle, and
    /// no viewport none; two superchunks of the viewport, far from the flock,
    /// turn hot as a halo's would, each generated in the viewport given
    /// a flock of its own, and those generated about them for those
    /// flocks' halos none. A save keeps its camera flock; a world whose
    /// camera loads nothing keeps no viewport.
    #[test]
    fn the_camera_loads_the_superchunks_of_the_viewport() {
        let sheep = 20;
        let mut world = server::start(server::Start { seed: crate::tests::land_seed(2), sheep, camera_loads: true, ..server::Start::default() });
        assert_eq!(world.info.camera_flock, Some(sheep as u64));
        let (x, y) = WORLD_MIDDLE.cartesian();
        world.keep_viewport(Some(Viewport { first: (x - 50, y - 50), last: (x + 50, y + 50) }));
        let wide = world.halos.viewport();
        assert!(wide.len() == (CAMERA_SIDE * CAMERA_SIDE) as usize && wide.contains(&WORLD_MIDDLE), "{} in the viewport", wide.len());
        world.keep_viewport(None);
        assert!(world.halos.viewport().is_empty(), "no viewport, nothing kept for it");
        world.keep_viewport(Some(Viewport { first: (x + 10, y), last: (x + 11, y) }));
        let viewport = [SuperchunkIndex::from_cartesian(x + 10, y), SuperchunkIndex::from_cartesian(x + 11, y)];
        let sheep_on = |world: &World, superchunk: SuperchunkIndex| world.entities.superchunk(superchunk).map_or(0, |kept| kept.iter().filter(|entity| entity.header.kind == HOT_ENTITY).count());
        let about_view = about(viewport.into_iter());
        let (mut seen, mut beside) = (0, 0);
        while seen < 2 || beside == 0 {
            assert!(world.entities.now() < 4 * WARM_TICKS, "the halos about the flocks of the viewport generated");
            world.tick();
            for &superchunk in world.halos.generated() {
                if viewport.contains(&superchunk) {
                    assert_eq!(sheep_on(&world, superchunk), sheep, "generated in the viewport: a flock of its own");
                    seen += 1;
                } else {
                    assert_eq!(sheep_on(&world, superchunk), 0, "generated for a halo alone: nothing on it");
                    beside += usize::from(about_view.contains(&superchunk));
                }
            }
        }
        assert!(viewport.iter().all(|superchunk| world.arena.superchunk_indices().contains(superchunk)));
        let folder = crate::tests::folder("camera");
        server::save(&folder, &mut world).expect("saved");
        assert_eq!(server::load(&folder).expect("loaded").info.camera_flock, Some(sheep as u64), "its camera flock, kept");

        let mut unseen = server::start(server::Start { seed: crate::tests::land_seed(2), sheep, ..server::Start::default() });
        unseen.keep_viewport(Some(Viewport { first: (x + 10, y), last: (x + 11, y) }));
        assert!(unseen.halos.viewport().is_empty() && unseen.info.camera_flock.is_none(), "its camera loads nothing");
    }
}

mod world {
    //! A world saved and loaded: it goes on exactly as it would have -- to
    //! the cell, the entity and the random number, however often it is
    //! stopped, hot superchunks and cold -- its files are where and what
    //! they are said to be, and files that are not a save are refused.
    //!
    //! `cargo test`

    use crate::tests::{everything, folder};
    use chunk_storage::disk::{self, DiskError};
    use coordinates::CellCartesian;

    /// Grass and sheep ticked, saved, and ticked on; the save loaded and
    /// ticked as far: the two are the same world.
    #[test]
    fn a_world_loaded_goes_on_as_the_one_saved() {
        let folder = folder("goes_on");
        let mut first = server::start(server::Start { seed: crate::tests::land_seed(0), sheep: 3_000, ..server::Start::default() });
        for _ in 0..1_500 {
            first.tick();
        }
        let saved = server::save(&folder, &mut first).expect("saved");
        assert_eq!((saved.superchunks, saved.entities), (first.storage.superchunks().count(), first.entities.len()));

        let mut second = server::load(&folder).expect("loaded");
        assert_eq!((second.info.seed, second.info.tick, second.generation), (crate::tests::land_seed(0), 1_500, first.generation));
        assert_eq!(second.info.layers, first.info.layers);
        assert!(everything(&first) == everything(&second), "loaded as saved");

        let (mut eaten, mut born) = (0, 0);
        for _ in 0..3_000 {
            let report = first.tick().rules;
            second.tick();
            (eaten, born) = (eaten + report.rules.sheep.eaten, born + report.rules.sheep.births);
        }
        assert!(eaten > 300 && born > 10, "{eaten} eaten, {born} born: a world doing something");
        assert!(everything(&first) == everything(&second), "the same 3,000 ticks on");
    }

    /// A world saved and loaded again and again mid run -- once while a
    /// superchunk is warming, always -- is, at a tick agreed, the world that ran
    /// straight to it: every cell, every entity, every random number.
    #[test]
    fn a_world_saved_and_loaded_mid_run_comes_to_the_same() {
        // At least 4,000 ticks, and on until a superchunk has been warming: on some seeds the flock is long in nearing an edge.
        let mut straight = server::start(server::Start { seed: crate::tests::land_seed(0), sheep: 4_000, ..server::Start::default() });
        let mut warming = None;
        while straight.entities.now() < 4_000 || warming.is_none() {
            assert!(straight.entities.now() < 60_000, "a superchunk warming on the way");
            straight.tick();
            if warming.is_none() && straight.warming().next().is_some() {
                warming = Some(straight.entities.now());
            }
        }
        let (warming, until) = (warming.expect("seen above"), straight.entities.now());

        let folder = folder("mid_run");
        let mut stopped = server::start(server::Start { seed: crate::tests::land_seed(0), sheep: 4_000, ..server::Start::default() });
        let mut stops = vec![1, 700, 701, 1_900, 3_333, warming, until];
        stops.sort_unstable();
        stops.dedup();
        for stop in stops {
            while stopped.entities.now() < stop {
                stopped.tick();
            }
            server::save(&folder, &mut stopped).expect("saved");
            // What ran is dropped whole: the next stretch runs on what the files hold alone.
            stopped = server::load(&folder).expect("loaded");
            assert_eq!(stopped.info.tick, stop);
        }
        assert!(straight.entities.len() > 4_000, "{} sheep: a flock that bred", straight.entities.len());
        // The one that ran straight saved too: a superchunk gone cold on the way has its last cells in the writeback ring until a save, or the ring's need of room, puts them in its image.
        server::save(&folder.join("straight"), &mut straight).expect("saved");
        assert!(everything(&straight) == everything(&stopped), "the same at tick {until}");
    }

    /// A save is a folder: a world file -- what it is made from, the kind
    /// of entity it is hot about, and how it is generated -- and a hot file, both CSV, and two
    /// files a superchunk named by its superchunk index in hexadecimal.
    #[test]
    fn a_save_is_a_directory_of_files_named_by_superchunk_index() {
        let folder = folder("files");
        let mut first = server::start(server::Start { seed: 99, sheep: 10, ..server::Start::default() });
        server::save(&folder, &mut first).expect("saved");
        let text = std::fs::read_to_string(folder.join("world.csv")).expect("the world's file");
        let generation: String = first.generation.numbers().iter().map(|(name, value)| format!("generation {name},{value}\n")).collect();
        assert_eq!(text, format!("world,is\nformat,2\nseed,0x0000000000000063\ntick,0\nlayers,2 3 4 5 6 7 24 8 9\nhot entity,{}\n{generation}", server::HOT_ENTITY.0), "no name: the folder's");
        let hot: String = first.arena.superchunk_indices().iter().map(|superchunk| format!("{:011x},hot,\n", superchunk.0)).collect();
        assert_eq!(std::fs::read_to_string(folder.join("hot.csv")).expect("the hot file"), format!("superchunk,is,until\n{hot}"), "the nine hot, none cooling or warming");
        let mut names: Vec<String> = std::fs::read_dir(folder.join("superchunks")).expect("the superchunks").map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        let expected: Vec<String> = disk::saved_superchunks(&folder).expect("listed").iter().flat_map(|superchunk| ["image", "state"].map(|kind| format!("{:011x}.{kind}", superchunk.0))).collect();
        assert_eq!(names, expected);
        assert_eq!(names.len(), 2 * 9, "the origin and its halo");
        assert_eq!(disk::saved_superchunks(&folder).unwrap(), first.arena.superchunk_indices());
    }

    /// A world's file is read whatever the order of its rows under the
    /// one naming its columns, what it lacks as by default -- but its
    /// seed, which it must have; a name said twice, or a format not the
    /// one written, is refused.
    #[test]
    fn a_world_file_is_read_in_any_order() {
        let folder = folder("any_order");
        let mut world = server::start(server::Start { seed: 7, sheep: 10, camera_loads: true, ..server::Start::default() });
        server::save(&folder, &mut world).expect("saved");
        let saved = disk::read_world(&folder).expect("read");
        let path = folder.join("world.csv");
        let text = std::fs::read_to_string(&path).expect("the world's file");
        let mut rows: Vec<&str> = text.lines().collect();
        rows[1..].reverse();
        std::fs::write(&path, rows.join("\n")).expect("written");
        assert_eq!(disk::read_world(&folder).expect("read turned round"), saved);
        std::fs::write(&path, "world,is\nseed,0x7\n").expect("written");
        let bare = disk::read_world(&folder).expect("read with the seed alone");
        assert_eq!(bare, disk::WorldInfo { seed: 7, tick: 0, layers: Vec::new(), side: None, forced: false, hot_entity: None, camera_flock: None, generation: Vec::new() });
        assert_eq!(worldgen::Generation::of_numbers(&bare.generation).numbers(), worldgen::Generation::DEFAULT.numbers(), "generated as by default");
        for refused in ["world,is\ntick,3\n", "world,is\nseed,0x7\nseed,0x8\n", "world,is\nformat,1\nseed,0x7\n", "seed,0x7\nworld,is\n"] {
            std::fs::write(&path, refused).expect("written");
            assert!(matches!(disk::read_world(&folder), Err(DiskError::Invalid(..))), "{refused}");
        }
    }

    /// What is not a save is refused, saying which file and why.
    #[test]
    fn files_that_are_not_a_save_are_refused() {
        let folder = folder("refused");
        assert!(matches!(server::load(&folder), Err(DiskError::Io(..))), "no such folder");
        server::save(&folder, &mut server::start(server::Start { seed: 1, sheep: 10, ..server::Start::default() })).expect("saved");
        let state = std::fs::read_dir(folder.join("superchunks")).unwrap().map(|entry| entry.unwrap().path()).find(|path| path.extension().unwrap() == "state").unwrap();
        let whole = std::fs::read(&state).unwrap();
        std::fs::write(&state, &whole[..whole.len() - 8]).unwrap();
        assert!(matches!(server::load(&folder), Err(DiskError::Invalid(path, what)) if path == state && what == "cut short"));
        std::fs::write(&state, &whole).unwrap();
        assert!(server::load(&folder).is_ok());
        std::fs::write(folder.join("world.csv"), "something else\n").unwrap();
        assert!(matches!(server::load(&folder), Err(DiskError::Invalid(..))));
    }

    /// In a world generated, the ground has walls, and no sheep ever steps
    /// through one: across or down only between cells at most a step apart
    /// in height, diagonally only where both ways round are such steps.
    #[test]
    fn sheep_never_step_through_a_wall() {
        use std::collections::HashMap;
        // Small polygons joined by cliffs, and a seed whose origin superchunk has walls enough.
        let shape = worldgen::Shape { span: 8, highest: 552, narrow: 2, wide: 2, sea: 0, finer_depth: 3, ..worldgen::Shape::DEFAULT };
        let seed = (utilities::seed::counted()..).find(|&seed| worldgen::Terrain::generate_shaped(&shape, seed, coordinates::WORLD_MIDDLE).wall_counts().iter().sum::<u64>() > 5_000).expect("a walled origin");
        let mut made = server::start(server::Start { seed, generation: worldgen::Generation { shape, ..worldgen::Generation::DEFAULT }, sheep: 4_000, ..server::Start::default() });
        // The superchunk's heights and a cell more all round, worked out once: asked for at every sheep, every tick.
        let (corner, side) = (coordinates::WORLD_MIDDLE.top_left().cartesian(), coordinates::SUPERCHUNK_SIDE_CELLS as usize + 2);
        let mut lands = worldgen::mesh::Lands::new(&shape, seed);
        let heights: Vec<_> = (0..side * side).map(|index| lands.height(corner.x.wrapping_add((index % side) as u32).wrapping_sub(1), corner.y.wrapping_add((index / side) as u32).wrapping_sub(1))).collect();
        let high = |at: coordinates::CellIndex| {
            let cell = at.cartesian();
            let (across, down) = (cell.x.wrapping_sub(corner.x).wrapping_add(1) as usize, cell.y.wrapping_sub(corner.y).wrapping_add(1) as usize);
            // A sheep strayed past the superchunk: the generator asked.
            if across < side && down < side { heights[down * side + across] } else { worldgen::height_shaped(&shape, seed, cell.x, cell.y) }
        };
        let mut stood: HashMap<u64, coordinates::CellIndex> = made.entities.iter().map(|sheep| (sheep.header.id.0, sheep.header.at)).collect();
        let (mut moved, mut beside_walls) = (0, 0);
        for _ in 0..1_500 {
            made.tick();
            for sheep in made.entities.iter() {
                let at = sheep.header.at;
                if let Some(was) = stood.insert(sheep.header.id.0, at).filter(|&was| was != at) {
                    let (from, to) = (was.cartesian(), at.cartesian());
                    // The cells of each way round: the straight step's alone, or the diagonal's two corners.
                    let corners = [CellCartesian { x: to.x, y: from.y }, CellCartesian { x: from.x, y: to.y }];
                    let walled = corners.iter().any(|corner| worldgen::wall(high(was), high((*corner).into())) || worldgen::wall(high((*corner).into()), high(at)));
                    assert!(!walled, "from height {} to {}: {from:?} to {to:?}", high(was), high(at));
                    moved += 1;
                }
                beside_walls += (0..9).any(|way| at.offset(way % 3 - 1, way / 3 - 1).is_some_and(|beside| worldgen::wall(high(at), high(beside)))) as usize;
            }
        }
        // Few steps: on ground nearly all grass a sheep seldom has to walk.
        assert!(moved > 50 && beside_walls > 10_000, "{moved} steps, {beside_walls} sheep-ticks beside a wall");
    }
}
