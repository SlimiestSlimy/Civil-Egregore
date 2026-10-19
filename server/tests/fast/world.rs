//! A world saved and loaded: it goes on exactly as it would have -- to
//! the cell, the entity and the random number, however often it is
//! stopped, hot superchunks and cold -- its files are where and what
//! they are said to be, and files that are not a save, or a save of
//! other types, are refused.
//!
//! `cargo test`

use protocol::server_parameters::{CleanServerParameters, ParameterValue, CAMERA_LOADS, FORCED_HOT, SEED, SIDE, WORLDGEN};
use crate::whole_worlds::{world_contents, folder};
use crate::chunk_storage::disk;
use crate::DiskError;
use utilities::coordinates::CellCartesian;
use crate::test_game::STEPS_COUNT;

/// Grass and sheep ticked, saved, and ticked on; the save loaded and
/// ticked as far: the two are the same world.
#[test]
fn a_world_loaded_goes_on_as_the_one_saved() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let folder = folder("goes_on");
    let (mut first_world, _) = crate::test_game::one_flock_world(CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)), 3_000);
    for _ in 0..1_500 {
        first_world.tick();
    }
    let save_written = crate::save(&folder, &mut first_world).expect("saved");
    assert_eq!((save_written.superchunks, save_written.entities_written), (first_world.chunk_storage().superchunks().count(), first_world.entity_store().len()));

    let mut second_world = crate::whole_worlds::opened(&folder).expect("loaded");
    assert_eq!((second_world.world_info().seed, second_world.world_info().tick, second_world.worldgen_parameters()), (seed, 1_500, first_world.worldgen_parameters()));
    assert_eq!(second_world.world_info().layers, first_world.world_info().layers);
    assert!(world_contents(&first_world) == world_contents(&second_world), "loaded as saved");

    let (mut steps_taken, walkers_rule) = (0, crate::test_game::walkers_rule(&first_world));
    for _ in 0..3_000 {
        let report = first_world.tick().rules;
        second_world.tick();
        steps_taken += report.rules.counts_of(walkers_rule)[STEPS_COUNT];
    }
    assert!(steps_taken > 300, "{steps_taken} steps: a world doing something");
    assert!(world_contents(&first_world) == world_contents(&second_world), "the same 3,000 ticks on");
}

/// A world saved and loaded again and again mid run -- once while a
/// superchunk is warming, always -- is, at a tick agreed, the world that ran
/// straight to it: every cell, every entity, every random number. And
/// at every tick and every load, what a world holds together holds
/// (`docs/server.md`, "What a world holds together").
#[test]
fn a_world_saved_and_loaded_mid_run_comes_to_the_same() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    // At least 4,000 ticks, and on until a superchunk has been warming: on some seeds the flock is long in nearing an edge.
    let (mut straight_world, _) = crate::test_game::one_flock_world(CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)), 4_000);
    let mut warming = None;
    while straight_world.entity_store().current_tick() < 4_000 || warming.is_none() {
        assert!(straight_world.entity_store().current_tick() < 60_000, "a superchunk warming on the way");
        straight_world.tick();
        assert_eq!(straight_world.broken_invariant(), None, "at tick {}", straight_world.entity_store().current_tick());
        if warming.is_none() && straight_world.warming().next().is_some() {
            warming = Some(straight_world.entity_store().current_tick());
        }
    }
    let (warming, last_tick) = (warming.expect("seen above"), straight_world.entity_store().current_tick());

    let folder = folder("mid_run");
    let (mut stopped_world, _) = crate::test_game::one_flock_world(CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)), 4_000);
    let mut stop_ticks = vec![1, 700, 701, 1_900, 3_333, warming, last_tick];
    stop_ticks.sort_unstable();
    stop_ticks.dedup();
    for stop_tick in stop_ticks {
        while stopped_world.entity_store().current_tick() < stop_tick {
            stopped_world.tick();
        }
        crate::save(&folder, &mut stopped_world).expect("saved");
        // What ran is dropped whole: the next stretch runs on what the files hold alone.
        stopped_world = crate::whole_worlds::opened(&folder).expect("loaded");
        assert_eq!((stopped_world.world_info().tick, stopped_world.broken_invariant()), (stop_tick, None));
    }
    assert_ne!(straight_world.entity_store().len(), 4_000, "a flock that was born into or died of, over the run");
    // The one that ran straight saved too: a superchunk gone cold on the way has its last cells in the writeback ring until a save, or the ring's need of room, puts them in its image.
    crate::save(&folder.join("straight"), &mut straight_world).expect("saved");
    assert!(world_contents(&straight_world) == world_contents(&stopped_world), "the same at tick {last_tick}");
}

/// A save is a folder: a world file -- what it is made from, the kind
/// of entity it is hot about, and how it is generated -- and a hot file, both CSV, and two
/// files a superchunk named by its superchunk index in hexadecimal.
#[test]
fn a_save_is_a_directory_of_files_named_by_superchunk_index() {
    let folder = folder("files");
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut saved_world, _) = crate::test_game::one_flock_world(CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)), 10);
    crate::save(&folder, &mut saved_world).expect("saved");
    let world_file_text = std::fs::read_to_string(folder.join("world.csv")).expect("the world's file");
    let worldgen: String = saved_world.worldgen_parameters().iter().map(|(name, value)| format!("worldgen {name},{value}\n")).collect();
    // No layers: they are the registry's.
    assert_eq!(world_file_text, format!("world,is\nformat,6\nseed,{}\ntick,0\nunpopulated,{}\n{worldgen}", utilities::seed::seed_as_hex(seed), utilities::coordinates::WORLD_MIDDLE.0), "no name: the folder's; the origin unpopulated until the first tick");
    let hot: String = saved_world.bitmap_arena().superchunk_indices().iter().map(|superchunk| format!("{:011x},hot,\n", superchunk.0)).collect();
    assert_eq!(std::fs::read_to_string(folder.join("hot.csv")).expect("the hot file"), format!("superchunk,is,until\n{hot}"), "the nine hot, none cooling or warming");
    let mut file_names: Vec<String> = std::fs::read_dir(folder.join("superchunks")).expect("the superchunks").map(|dir_entry| dir_entry.unwrap().file_name().into_string().unwrap()).collect();
    file_names.sort();
    let expected_names: Vec<String> = disk::saved_superchunks(&folder).expect("listed").iter().flat_map(|superchunk| ["image", "state"].map(|kind| format!("{:011x}.{kind}", superchunk.0))).collect();
    assert_eq!(file_names, expected_names);
    assert_eq!(file_names.len(), 2 * 9, "the origin and its halo");
    assert_eq!(disk::saved_superchunks(&folder).unwrap(), saved_world.bitmap_arena().superchunk_indices());
}

/// A world's file is read whatever the order of its rows under the
/// one naming its columns, what it lacks as by default -- but its
/// seed, which it must have; a name said twice, or a format not the
/// one written, is refused.
#[test]
fn a_world_file_is_read_in_any_order() {
    let folder = folder("any_order");
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let (mut world, _) = crate::test_game::one_flock_world(CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)).with(CAMERA_LOADS, crate::whole_worlds::given()), 10);
    crate::save(&folder, &mut world).expect("saved");
    let saved_info = disk::read_world(&folder).expect("read");
    let path = folder.join("world.csv");
    let world_file_text = std::fs::read_to_string(&path).expect("the world's file");
    let mut file_rows: Vec<&str> = world_file_text.lines().collect();
    file_rows[1..].reverse();
    std::fs::write(&path, file_rows.join("\n")).expect("written");
    assert_eq!(disk::read_world(&folder).expect("read turned round"), saved_info);
    std::fs::write(&path, format!("world,is\nseed,{}\n", utilities::seed::seed_as_hex(seed))).expect("written");
    let bare_info = disk::read_world(&folder).expect("read with the seed alone");
    assert_eq!(bare_info, disk::WorldInfo { seed, tick: 0, layers: Vec::new(), side_in_superchunks: None, forced_hot: false, camera_loads: false, unpopulated: Vec::new(), worldgen_parameters: Vec::new() });
    let generation = crate::simulation::rules::worldgen_of(world.frozen_type_registry()).expect("the test game's generation");
    assert_eq!((generation.parameters_read)(&bare_info.worldgen_parameters), vec![(crate::test_game::WALKERS_PER_SUPERCHUNK.to_string(), 0)], "generated as by default");
    for refused_text in ["world,is\ntick,3\n", "world,is\nseed,0x7\nseed,0x8\n", "world,is\nformat,1\nseed,0x7\n", "seed,0x7\nworld,is\n"] {
        std::fs::write(&path, refused_text).expect("written");
        assert!(matches!(disk::read_world(&folder), Err(DiskError::InvalidFile(..))), "{refused_text}");
    }
}

/// What is not a save is refused, saying which file and why.
#[test]
fn files_that_are_not_a_save_are_refused() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let folder = folder("refused");
    assert!(matches!(crate::whole_worlds::opened(&folder), Err(DiskError::IoFailed(..))), "no such folder");
    crate::save(&folder, &mut crate::test_game::one_flock_world(CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)), 10).0).expect("saved");
    let state_path = std::fs::read_dir(folder.join("superchunks")).unwrap().map(|dir_entry| dir_entry.unwrap().path()).find(|path| path.extension().unwrap() == "state").unwrap();
    let whole_state = std::fs::read(&state_path).unwrap();
    std::fs::write(&state_path, &whole_state[..whole_state.len() - 8]).unwrap();
    assert!(matches!(crate::whole_worlds::opened(&folder), Err(DiskError::InvalidFile(path, invalid_reason)) if path == state_path && invalid_reason == "cut short"));
    std::fs::write(&state_path, &whole_state).unwrap();
    assert!(crate::whole_worlds::opened(&folder).is_ok());
    std::fs::write(folder.join("world.csv"), "something else\n").unwrap();
    assert!(matches!(crate::whole_worlds::opened(&folder), Err(DiskError::InvalidFile(..))));
}

/// A save holds the world's types by name and ID, in the order
/// registered, and a world loaded has those: each the type the code
/// registers under its name, of the ID saved for it; one the save does
/// not name, an ID past them all. A name none registers, an ID sharing
/// a number with another's, or a name said twice, is refused, naming
/// the type.
#[test]
fn a_world_is_loaded_with_the_types_it_was_made_with() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let folder = folder("types");
    let (mut world, _) = crate::test_game::one_flock_world(CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)), 10);
    crate::save(&folder, &mut world).expect("saved");
    let types_path = folder.join(protocol::world_types::TYPES_FILE);
    let types_text = std::fs::read_to_string(&types_path).expect("the types file");
    let lines: Vec<String> = types_text.lines().map(str::to_string).collect();
    let world_lines = world.frozen_type_registry().types().iter().map(|registered| format!("{},{:016x}", registered.name(), registered.type_id()));
    assert_eq!(lines, std::iter::once("type,id".to_string()).chain(world_lines).collect::<Vec<_>>());
    let line_of = |type_name: &str| lines.iter().position(|line| line.split(',').next() == Some(type_name)).expect("saved");
    let with_lines = |change: &dyn Fn(&mut Vec<String>)| {
        let mut changed_lines = lines.clone();
        change(&mut changed_lines);
        changed_lines.join("\n") + "\n"
    };
    // A type the save does not name -- as in a world saved before the game added it: registered all the same, given an ID past every saved one.
    let named = |frozen_type_registry: &protocol::type_registry::FrozenTypeRegistry, type_name: &str| frozen_type_registry.types().iter().find(|registered| registered.name() == type_name).cloned();
    std::fs::write(&types_path, with_lines(&|changed_lines| _ = changed_lines.remove(line_of(&protocol::type_registry::in_bundle(crate::test_game::BUNDLE, "tiles"))))).expect("written");
    let loaded_world = crate::whole_worlds::opened(&folder).expect("loaded without it");
    let tiles = named(loaded_world.frozen_type_registry(), &protocol::type_registry::in_bundle(crate::test_game::BUNDLE, "tiles")).expect("registered");
    let last_saved_number = world.frozen_type_registry().types().iter().filter(|registered| registered.name() != protocol::type_registry::in_bundle(crate::test_game::BUNDLE, "tiles")).map(|registered| registered.type_number()).max().expect("types");
    assert!(tiles.type_number() > last_saved_number && loaded_world.frozen_type_registry().types().len() == lines.len() - 1);
    // A type drawn, renamed, of an ID sharing a number with another, or said twice. The order types are saved in is nothing to them.
    let mut seeded_rng = utilities::rng::RandomSource::new(seed);
    let drawn = 1 + seeded_rng.draw_below(lines.len() as u64 - 1) as usize;
    let (drawn_name, drawn_id) = lines[drawn].split_once(',').expect("a name and an ID");
    // Another type's ID: the one saved beside it.
    let other_id = lines[if drawn > 1 { drawn - 1 } else { drawn + 1 }].split_once(',').expect("a name and an ID").1.to_string();
    for (changed_text, named) in [
        (with_lines(&|changed_lines| changed_lines[drawn] = format!("unheard of,{drawn_id}")), "unheard of"),
        (with_lines(&|changed_lines| changed_lines[drawn] = format!("{drawn_name},{other_id}")), drawn_name),
        (with_lines(&|changed_lines| changed_lines.insert(drawn, lines[drawn].clone())), drawn_name),
    ] {
        std::fs::write(&types_path, &changed_text).expect("written");
        let Err(DiskError::InvalidFile(path, what_is_wrong)) = crate::whole_worlds::opened(&folder) else { panic!("loaded with `{named}` amiss:\n{changed_text}") };
        assert!(path == types_path && what_is_wrong.contains(named), "{what_is_wrong}");
    }
    std::fs::write(&types_path, &types_text).expect("written");
    assert!(crate::whole_worlds::opened(&folder).is_ok());
}

/// A plane of tiles -- a number a tile 8 by 8 cells -- holds what is
/// written to it, read at any cell of the tile, through ticks, a save
/// and a load; a tile nothing was written to holds 0. Tiles, numbers
/// and the cells they are read at drawn from the seed.
#[test]
fn a_plane_of_tiles_holds_a_number_a_tile_through_ticks_and_a_save() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    use crate::bitplane_manager::Write;
    let folder = folder("tiles");
    let mut test_random = utilities::rng::RandomSource::new(crate::test_seed::TestSeedHandle::open().seed());
    let (frozen_type_registry, test_game) = crate::test_game::TestGame::new_world_types();
    let test_tiles = test_game.tiles;
    let mut world = crate::start_world(&CleanServerParameters::default().with(SEED, ParameterValue::Hex(seed)).with(SIDE, ParameterValue::Integer(2)).with(FORCED_HOT, crate::whole_worlds::given()).with(WORLDGEN, crate::test_game::walkers(50)), &frozen_type_registry).expect("the world started");
    let (corner_cartesian, side_cells) = (world.bitmap_arena().superchunk_indices()[0].top_left().cartesian(), 2 * utilities::coordinates::SUPERCHUNK_SIDE_CELLS);
    // A cell of the tile `(x, y)` -- in tiles from the world's corner -- drawn anew each time.
    let tile_side = test_tiles.layer_type().tile_side();
    let cell_of = |(x, y): (u32, u32), cell_random: &mut utilities::rng::RandomSource| -> utilities::coordinates::CellIndex {
        CellCartesian { x: corner_cartesian.x + x * tile_side + cell_random.draw_below(u64::from(tile_side)) as u32, y: corner_cartesian.y + y * tile_side + cell_random.draw_below(u64::from(tile_side)) as u32 }.into()
    };
    let mut tiles = std::collections::BTreeMap::new();
    for _ in 0..test_random.draw_between(200, 400) {
        let tile = (test_random.draw_below(u64::from(side_cells / tile_side)) as u32, test_random.draw_below(u64::from(side_cells / tile_side)) as u32);
        tiles.insert(tile, 1 + test_random.draw_below(u64::from(test_tiles.most_value())) as u32);
    }
    let writes: Vec<Write> = tiles.iter().map(|(&tile, &number)| Write::value(test_tiles, cell_of(tile, &mut test_random), number)).collect();
    assert_eq!(world.write_cells(test_tiles.layer_type(), writes).cells_changed, tiles.len() as u64, "a number written a tile");
    let check_held = |world: &crate::World, when_text: &str, held_random: &mut utilities::rng::RandomSource| {
        for (&tile, &number) in &tiles {
            assert_eq!(world.bitmap_arena().value(test_tiles, cell_of(tile, held_random)), Ok(number), "the tile {tile:?}, {when_text}");
        }
        let bare_tile = (0..).map(|_| (held_random.draw_below(u64::from(side_cells / tile_side)) as u32, held_random.draw_below(u64::from(side_cells / tile_side)) as u32)).find(|tile| !tiles.contains_key(tile)).expect("a tile not written");
        assert_eq!(world.bitmap_arena().value(test_tiles, cell_of(bare_tile, held_random)), Ok(0), "a bare tile, {when_text}");
    };
    check_held(&world, "as written", &mut test_random);
    for _ in 0..test_random.draw_between(20, 60) {
        world.tick();
    }
    check_held(&world, "after ticks", &mut test_random);
    crate::save(&folder, &mut world).expect("saved");
    let loaded_world = crate::whole_worlds::opened(&folder).expect("loaded");
    check_held(&loaded_world, "loaded from a save", &mut test_random);
    assert!(world_contents(&world) == world_contents(&loaded_world), "loaded as saved");
}

/// A world started of a folder no world is saved in is made new, as the
/// rest of its parameters say, and saved there as it starts: started of
/// the folder again, it is opened -- the same seed, at the same tick --
/// and its parameters of a new world are nothing to it.
#[test]
fn a_world_folder_holding_no_world_is_given_the_new_one() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let folder = folder("made_in_its_folder");
    assert!(!crate::holds_a_world(&folder));
    let made_world = crate::start_world(&crate::whole_worlds::world_at(&folder).with(SEED, ParameterValue::Hex(seed)), &crate::test_game::TestGame::new_world_types().0).expect("made and saved");
    assert!(crate::holds_a_world(&folder), "saved as it started");
    let opened_world = crate::start_world(&crate::whole_worlds::world_at(&folder).with(SEED, ParameterValue::Hex(seed ^ 1)), &std::sync::Arc::new(crate::read_types(&folder, crate::test_game::TestGame::register_types).expect("its types"))).expect("opened");
    assert_eq!((opened_world.world_info().seed, opened_world.world_info().tick), (made_world.world_info().seed, made_world.world_info().tick), "the world saved, not another seed's");
    assert_eq!(made_world.world_info().seed, seed);
    _ = std::fs::remove_dir_all(&folder);
}
