//! A world saved and loaded: it goes on exactly as it would have -- to
//! the cell, the entity and the random number, however often it is
//! stopped, hot superchunks and cold -- its files are where and what
//! they are said to be, and files that are not a save, or a save of
//! other types, are refused.
//!
//! `cargo test`

use crate::worlds::{world_contents, folder};
use server::chunk_storage::disk::{self, DiskError};
use utilities::coordinates::CellCartesian;
use basegame::entity_rules::sheep::EATEN_COUNT;

/// Grass and sheep ticked, saved, and ticked on; the save loaded and
/// ticked as far: the two are the same world.
#[test]
fn a_world_loaded_goes_on_as_the_one_saved() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    let folder = folder("goes_on");
    let mut first_world = crate::worlds::one_flock_world(server::WorldStart { seed, ..server::WorldStart::default() }, 3_000);
    for _ in 0..1_500 {
        first_world.tick();
    }
    let save_written = server::save(&folder, &mut first_world).expect("saved");
    assert_eq!((save_written.superchunks, save_written.entities_written), (first_world.chunk_storage().superchunks().count(), first_world.entity_store().len()));

    let mut second_world = server::load_world(&folder, basegame::default_types::register_types).expect("loaded");
    assert_eq!((second_world.world_info().seed, second_world.world_info().tick, second_world.generation_numbers()), (seed, 1_500, first_world.generation_numbers()));
    assert_eq!(second_world.world_info().layers, first_world.world_info().layers);
    assert!(world_contents(&first_world) == world_contents(&second_world), "loaded as saved");

    let (mut grass_eaten, sheep_rule) = (0, first_world.rule_place(crate::worlds::type_id_named(basegame::base_types::SHEEP_RULE)));
    for _ in 0..3_000 {
        let report = first_world.tick().rules;
        second_world.tick();
        grass_eaten += report.rules.counts_of(sheep_rule)[EATEN_COUNT];
    }
    assert!(grass_eaten > 300, "{grass_eaten} eaten: a world doing something");
    assert!(world_contents(&first_world) == world_contents(&second_world), "the same 3,000 ticks on");
}

/// A world saved and loaded again and again mid run -- once while a
/// superchunk is warming, always -- is, at a tick agreed, the world that ran
/// straight to it: every cell, every entity, every random number. And
/// at every tick and every load, what a world holds together holds
/// (`docs/server.md`, "What a world holds together").
#[test]
fn a_world_saved_and_loaded_mid_run_comes_to_the_same() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    // At least 4,000 ticks, and on until a superchunk has been warming: on some seeds the flock is long in nearing an edge.
    let mut straight_world = crate::worlds::one_flock_world(server::WorldStart { seed, ..server::WorldStart::default() }, 4_000);
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
    let mut stopped_world = crate::worlds::one_flock_world(server::WorldStart { seed, ..server::WorldStart::default() }, 4_000);
    let mut stop_ticks = vec![1, 700, 701, 1_900, 3_333, warming, last_tick];
    stop_ticks.sort_unstable();
    stop_ticks.dedup();
    for stop_tick in stop_ticks {
        while stopped_world.entity_store().current_tick() < stop_tick {
            stopped_world.tick();
        }
        server::save(&folder, &mut stopped_world).expect("saved");
        // What ran is dropped whole: the next stretch runs on what the files hold alone.
        stopped_world = server::load_world(&folder, basegame::default_types::register_types).expect("loaded");
        assert_eq!((stopped_world.world_info().tick, stopped_world.broken_invariant()), (stop_tick, None));
    }
    assert_ne!(straight_world.entity_store().len(), 4_000, "a flock that was born into or died of, over the run");
    // The one that ran straight saved too: a superchunk gone cold on the way has its last cells in the writeback ring until a save, or the ring's need of room, puts them in its image.
    server::save(&folder.join("straight"), &mut straight_world).expect("saved");
    assert!(world_contents(&straight_world) == world_contents(&stopped_world), "the same at tick {last_tick}");
}

/// A save is a folder: a world file -- what it is made from, the kind
/// of entity it is hot about, and how it is generated -- and a hot file, both CSV, and two
/// files a superchunk named by its superchunk index in hexadecimal.
#[test]
fn a_save_is_a_directory_of_files_named_by_superchunk_index() {
    let folder = folder("files");
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    let mut saved_world = crate::worlds::one_flock_world(server::WorldStart { seed, ..server::WorldStart::default() }, 10);
    server::save(&folder, &mut saved_world).expect("saved");
    let world_file_text = std::fs::read_to_string(folder.join("world.csv")).expect("the world's file");
    let generation: String = saved_world.generation_numbers().iter().map(|(name, value)| format!("generation {name},{value}\n")).collect();
    // Every layer the world's types have, a wide plane as the layers it is kept cold as.
    let layers: Vec<String> = saved_world.frozen_types().layer_types().into_iter().flat_map(|layer_type| layer_type.each_bitplane()).map(|plane_type| plane_type.0.to_string()).collect();
    assert_eq!(world_file_text, format!("world,is\nformat,4\nseed,{}\ntick,0\nlayers,{}\nunpopulated,{}\n{generation}", utilities::seed::seed_as_hex(seed), layers.join(" "), utilities::coordinates::WORLD_MIDDLE.0), "no name: the folder's; the origin unpopulated until the first tick");
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
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    let mut world = crate::worlds::one_flock_world(server::WorldStart { seed, camera_loads: true, ..server::WorldStart::default() }, 10);
    server::save(&folder, &mut world).expect("saved");
    let saved_info = disk::read_world(&folder).expect("read");
    let path = folder.join("world.csv");
    let world_file_text = std::fs::read_to_string(&path).expect("the world's file");
    let mut file_rows: Vec<&str> = world_file_text.lines().collect();
    file_rows[1..].reverse();
    std::fs::write(&path, file_rows.join("\n")).expect("written");
    assert_eq!(disk::read_world(&folder).expect("read turned round"), saved_info);
    std::fs::write(&path, format!("world,is\nseed,{}\n", utilities::seed::seed_as_hex(seed))).expect("written");
    let bare_info = disk::read_world(&folder).expect("read with the seed alone");
    assert_eq!(bare_info, disk::WorldInfo { seed, tick: 0, layers: Vec::new(), side_in_superchunks: None, forced_hot: false, camera_loads: false, unpopulated: Vec::new(), generation_numbers: Vec::new() });
    assert_eq!(basegame::worldgen::WorldGeneration::of_numbers(&bare_info.generation_numbers).numbers(), basegame::worldgen::WorldGeneration::DEFAULT.numbers(), "generated as by default");
    for refused_text in ["world,is\ntick,3\n", "world,is\nseed,0x7\nseed,0x8\n", "world,is\nformat,1\nseed,0x7\n", "seed,0x7\nworld,is\n"] {
        std::fs::write(&path, refused_text).expect("written");
        assert!(matches!(disk::read_world(&folder), Err(DiskError::InvalidFile(..))), "{refused_text}");
    }
}

/// What is not a save is refused, saying which file and why.
#[test]
fn files_that_are_not_a_save_are_refused() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    let folder = folder("refused");
    assert!(matches!(server::load_world(&folder, basegame::default_types::register_types), Err(DiskError::IoFailed(..))), "no such folder");
    server::save(&folder, &mut crate::worlds::one_flock_world(server::WorldStart { seed, ..server::WorldStart::default() }, 10)).expect("saved");
    let state_path = std::fs::read_dir(folder.join("superchunks")).unwrap().map(|dir_entry| dir_entry.unwrap().path()).find(|path| path.extension().unwrap() == "state").unwrap();
    let whole_state = std::fs::read(&state_path).unwrap();
    std::fs::write(&state_path, &whole_state[..whole_state.len() - 8]).unwrap();
    assert!(matches!(server::load_world(&folder, basegame::default_types::register_types), Err(DiskError::InvalidFile(path, invalid_reason)) if path == state_path && invalid_reason == "cut short"));
    std::fs::write(&state_path, &whole_state).unwrap();
    assert!(server::load_world(&folder, basegame::default_types::register_types).is_ok());
    std::fs::write(folder.join("world.csv"), "something else\n").unwrap();
    assert!(matches!(server::load_world(&folder, basegame::default_types::register_types), Err(DiskError::InvalidFile(..))));
}

/// A save holds the world's types by name and ID, in the order
/// registered, and a world loaded has those: each the type the code
/// registers under its name, of the ID saved for it; one the save does
/// not name, an ID past them all. A name none registers, an ID sharing
/// a number with another's, or a name said twice, is refused, naming
/// the type.
#[test]
fn a_world_is_loaded_with_the_types_it_was_made_with() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    let folder = folder("types");
    let mut world = crate::worlds::one_flock_world(server::WorldStart { seed, ..server::WorldStart::default() }, 10);
    server::save(&folder, &mut world).expect("saved");
    let types_path = folder.join(protocol::world_types::TYPES_FILE);
    let types_text = std::fs::read_to_string(&types_path).expect("the types file");
    let lines: Vec<String> = types_text.lines().map(str::to_string).collect();
    let world_lines = world.frozen_types().types().iter().map(|registered| format!("{},{:016x}", registered.name(), registered.type_id()));
    assert_eq!(lines, std::iter::once("type,id".to_string()).chain(world_lines).collect::<Vec<_>>());
    let line_of = |type_name: &str| lines.iter().position(|line| line.split(',').next() == Some(type_name)).expect("saved");
    let with_lines = |change: &dyn Fn(&mut Vec<String>)| {
        let mut changed_lines = lines.clone();
        change(&mut changed_lines);
        changed_lines.join("\n") + "\n"
    };
    // A type the save does not name -- as in a world saved before the game added it: registered all the same, given an ID past every saved one.
    std::fs::write(&types_path, with_lines(&|changed_lines| _ = changed_lines.remove(line_of("water depth")))).expect("written");
    let loaded_world = server::load_world(&folder, basegame::default_types::register_types).expect("loaded without it");
    let water_depth = loaded_world.frozen_types().types().iter().find(|registered| registered.name() == "water depth").expect("registered");
    let saved_numbers_end = world.frozen_types().types().iter().filter(|registered| registered.name() != "water depth").map(|registered| registered.numbers().end).max().expect("types");
    assert!(water_depth.numbers().start >= saved_numbers_end && loaded_world.frozen_types().types().len() == lines.len() - 1);
    // A type drawn, renamed, of an ID sharing a number with another, or said twice. The order types are saved in is nothing to them.
    let mut seeded_rng = utilities::rng::RandomSource::new(seed);
    let drawn = 1 + seeded_rng.draw_below(lines.len() as u64 - 1) as usize;
    let (drawn_name, drawn_id) = lines[drawn].split_once(',').expect("a name and an ID");
    // Another type's ID: the one saved beside it, which shares its numbers.
    let other_id = lines[if drawn > 1 { drawn - 1 } else { drawn + 1 }].split_once(',').expect("a name and an ID").1.to_string();
    for (changed_text, named) in [
        (with_lines(&|changed_lines| changed_lines[drawn] = format!("unheard of,{drawn_id}")), "unheard of"),
        (with_lines(&|changed_lines| changed_lines[drawn] = format!("{drawn_name},{other_id}")), drawn_name),
        (with_lines(&|changed_lines| changed_lines.insert(drawn, lines[drawn].clone())), drawn_name),
    ] {
        std::fs::write(&types_path, &changed_text).expect("written");
        let Err(DiskError::InvalidFile(path, what_is_wrong)) = server::load_world(&folder, basegame::default_types::register_types) else { panic!("loaded with `{named}` amiss:\n{changed_text}") };
        assert!(path == types_path && what_is_wrong.contains(named), "{what_is_wrong}");
    }
    std::fs::write(&types_path, &types_text).expect("written");
    assert!(server::load_world(&folder, basegame::default_types::register_types).is_ok());
}

/// In a world generated, the ground has walls, and no sheep ever steps
/// through one: across or down only between cells at most a step apart
/// in height, diagonally only where both ways round are such steps.
#[test]
fn sheep_never_step_through_a_wall() {
    use std::collections::HashMap;
    // Small polygons joined by cliffs, and a seed whose origin superchunk has walls enough.
    let land_shape = basegame::worldgen::LandShape { vertex_spacing_power: 8, highest_land: 552, narrowest_blend: 2, widest_blend: 2, ocean_share: 0, finer_depth: 3, ..basegame::worldgen::LandShape::DEFAULT };
    let seed = (crate::test_seed::TestSeedHandle::open().seed()..).find(|&seed| basegame::worldgen::Terrain::generate_shaped(&land_shape, seed, utilities::coordinates::CellArea { top_left: utilities::coordinates::WORLD_MIDDLE_CELL, side_cells: utilities::coordinates::SUPERCHUNK_SIDE_CELLS }).wall_counts().iter().sum::<u64>() > 5_000).expect("a walled origin");
    let mut made_world = crate::worlds::start_world(server::WorldStart { seed, world_size: server::WorldSize::SquareOfSide { side_superchunks: 1, forced_hot: false }, generation_numbers: basegame::worldgen::WorldGeneration { land_shape, sheep_per_superchunk: 4_000, ..basegame::worldgen::WorldGeneration::DEFAULT }.numbers(), ..server::WorldStart::default() });
    // The superchunk's heights and a cell more all round, worked out once: asked for at every sheep, every tick.
    let (corner_cartesian, side_cells) = (utilities::coordinates::WORLD_MIDDLE.top_left().cartesian(), utilities::coordinates::SUPERCHUNK_SIDE_CELLS as usize + 2);
    let mut mesh_lands = basegame::worldgen::mesh::WorldLand::new(&land_shape, seed);
    let heights: Vec<_> = (0..side_cells * side_cells).map(|cell_number| mesh_lands.height(corner_cartesian.x.wrapping_add((cell_number % side_cells) as u32).wrapping_sub(1), corner_cartesian.y.wrapping_add((cell_number / side_cells) as u32).wrapping_sub(1))).collect();
    let height_at = |asked_cell: utilities::coordinates::CellIndex| {
        let cell = asked_cell.cartesian();
        let (cells_across, cells_down) = (cell.x.wrapping_sub(corner_cartesian.x).wrapping_add(1) as usize, cell.y.wrapping_sub(corner_cartesian.y).wrapping_add(1) as usize);
        // A sheep strayed past the superchunk: the generator asked.
        if cells_across < side_cells && cells_down < side_cells { heights[cells_down * side_cells + cells_across] } else { basegame::worldgen::height_shaped(&land_shape, seed, cell.x, cell.y) }
    };
    // Each sheep told from the others by a tag; a lamb, born after, has none, and is not followed.
    // The flock made the first tick, by the world's generation.
    made_world.tick();
    crate::worlds::tag_sheep(&mut made_world);
    let mut stood_cell_by_tag: HashMap<u64, utilities::coordinates::CellIndex> = made_world.entity_store().iter().map(|tagged_sheep| (crate::worlds::tag_of(&tagged_sheep).expect("tagged"), tagged_sheep.header.stood_cell)).collect();
    let (mut steps_moved, mut beside_walls) = (0, 0);
    for _ in 0..1_500 {
        made_world.tick();
        for ticked_sheep in made_world.entity_store().iter() {
            let stood_cell = ticked_sheep.header.stood_cell;
            if let Some(was_cell) = crate::worlds::tag_of(&ticked_sheep).and_then(|sheep_tag| stood_cell_by_tag.insert(sheep_tag, stood_cell)).filter(|&previous_cell| previous_cell != stood_cell) {
                let (step_from_cartesian, step_to_cartesian) = (was_cell.cartesian(), stood_cell.cartesian());
                // The cells of each way round: the straight step's alone, or the diagonal's two corners.
                let corner_cells = [CellCartesian { x: step_to_cartesian.x, y: step_from_cartesian.y }, CellCartesian { x: step_from_cartesian.x, y: step_to_cartesian.y }];
                let step_walled = corner_cells.iter().any(|corner_cell| basegame::worldgen::wall(height_at(was_cell), height_at((*corner_cell).into())) || basegame::worldgen::wall(height_at((*corner_cell).into()), height_at(stood_cell)));
                assert!(!step_walled, "from height {} to {}: {step_from_cartesian:?} to {step_to_cartesian:?}", height_at(was_cell), height_at(stood_cell));
                steps_moved += 1;
            }
            beside_walls += (0..9).any(|way| stood_cell.at_cartesian_offset(way % 3 - 1, way / 3 - 1).is_some_and(|beside_cell| basegame::worldgen::wall(height_at(stood_cell), height_at(beside_cell)))) as usize;
        }
    }
    // Few steps: on ground nearly all grass a sheep seldom has to walk.
    assert!(steps_moved > 50 && beside_walls > 10_000, "{steps_moved} steps, {beside_walls} sheep-ticks beside a wall");
}

/// A plane of tiles -- a number a tile 8 by 8 cells -- holds what is
/// written to it, read at any cell of the tile, through ticks, a save
/// and a load; a tile nothing was written to holds 0. Tiles, numbers
/// and the cells they are read at drawn from the seed.
#[test]
fn a_plane_of_tiles_holds_a_number_a_tile_through_ticks_and_a_save() {
    let seed = crate::worlds::seed_with_land(crate::test_seed::TestSeedHandle::open().seed(), &basegame::worldgen::WorldGeneration::DEFAULT);
    use server::bitplane_manager::Write;
    let folder = folder("tiles");
    let mut test_random = utilities::rng::RandomSource::new(crate::test_seed::TestSeedHandle::open().seed());
    let (frozen_types, test_tiles) = crate::worlds::with_test_tiles();
    let mut world = server::start_world(server::WorldStart { seed, world_size: server::WorldSize::SquareOfSide { side_superchunks: 2, forced_hot: true }, generation_numbers: crate::worlds::flocked(50), ..server::WorldStart::default() }, &frozen_types);
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
    let check_held = |world: &server::World, when_text: &str, held_random: &mut utilities::rng::RandomSource| {
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
    server::save(&folder, &mut world).expect("saved");
    let loaded_world = server::load_world(&folder, crate::worlds::types_with_test_tiles).expect("loaded");
    check_held(&loaded_world, "loaded from a save", &mut test_random);
    assert!(world_contents(&world) == world_contents(&loaded_world), "loaded as saved");
}
