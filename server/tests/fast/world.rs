//! A world saved and loaded: it goes on exactly as it would have -- to
//! the cell, the entity and the random number, however often it is
//! stopped, hot superchunks and cold -- its files are where and what
//! they are said to be, and files that are not a save are refused.
//!
//! `cargo test`

use crate::tests::{everything, folder};
use chunk_storage::disk::{self, DiskError};
use coordinates::CellCartesian;
use entity_rules::sheep::{BIRTHS, EATEN};

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
    assert_eq!((saved.superchunks, saved.entities), (first.storage().superchunks().count(), first.entities().len()));

    let mut second = server::load(&folder).expect("loaded");
    assert_eq!((second.info().seed, second.info().tick, second.generation()), (crate::tests::land_seed(0), 1_500, first.generation()));
    assert_eq!(second.info().layers, first.info().layers);
    assert!(everything(&first) == everything(&second), "loaded as saved");

    let (mut eaten, mut born) = (0, 0);
    for _ in 0..3_000 {
        let report = first.tick().rules;
        second.tick();
        (eaten, born) = (eaten + report.rules.of(server::SHEEP_RULE)[EATEN], born + report.rules.of(server::SHEEP_RULE)[BIRTHS]);
    }
    assert!(eaten > 300 && born > 10, "{eaten} eaten, {born} born: a world doing something");
    assert!(everything(&first) == everything(&second), "the same 3,000 ticks on");
}

/// A world saved and loaded again and again mid run -- once while a
/// superchunk is warming, always -- is, at a tick agreed, the world that ran
/// straight to it: every cell, every entity, every random number. And
/// at every tick and every load, what a world holds together holds
/// (`docs/server.md`, "What a world holds together").
#[test]
fn a_world_saved_and_loaded_mid_run_comes_to_the_same() {
    // At least 4,000 ticks, and on until a superchunk has been warming: on some seeds the flock is long in nearing an edge.
    let mut straight = server::start(server::Start { seed: crate::tests::land_seed(0), sheep: 4_000, ..server::Start::default() });
    let mut warming = None;
    while straight.entities().now() < 4_000 || warming.is_none() {
        assert!(straight.entities().now() < 60_000, "a superchunk warming on the way");
        straight.tick();
        assert_eq!(straight.broken_invariant(), None, "at tick {}", straight.entities().now());
        if warming.is_none() && straight.warming().next().is_some() {
            warming = Some(straight.entities().now());
        }
    }
    let (warming, until) = (warming.expect("seen above"), straight.entities().now());

    let folder = folder("mid_run");
    let mut stopped = server::start(server::Start { seed: crate::tests::land_seed(0), sheep: 4_000, ..server::Start::default() });
    let mut stops = vec![1, 700, 701, 1_900, 3_333, warming, until];
    stops.sort_unstable();
    stops.dedup();
    for stop in stops {
        while stopped.entities().now() < stop {
            stopped.tick();
        }
        server::save(&folder, &mut stopped).expect("saved");
        // What ran is dropped whole: the next stretch runs on what the files hold alone.
        stopped = server::load(&folder).expect("loaded");
        assert_eq!((stopped.info().tick, stopped.broken_invariant()), (stop, None));
    }
    assert!(straight.entities().len() > 4_000, "{} sheep: a flock that bred", straight.entities().len());
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
    let seed = crate::tests::land_seed(1);
    let mut first = server::start(server::Start { seed, sheep: 10, ..server::Start::default() });
    server::save(&folder, &mut first).expect("saved");
    let text = std::fs::read_to_string(folder.join("world.csv")).expect("the world's file");
    let generation: String = first.generation().numbers().iter().map(|(name, value)| format!("generation {name},{value}\n")).collect();
    // Every layer the registry has, a wide plane as the layers it is kept cold as.
    let layers: Vec<String> = type_registry::layer_types().into_iter().flat_map(|layer_type| layer_type.planes()).map(|plane| plane.0.to_string()).collect();
    assert_eq!(text, format!("world,is\nformat,2\nseed,{}\ntick,0\nlayers,{}\nhot entity,{}\n{generation}", utilities::seed::hex(seed), layers.join(" "), server::HOT_ENTITY.0), "no name: the folder's");
    let hot: String = first.arena().superchunk_indices().iter().map(|superchunk| format!("{:011x},hot,\n", superchunk.0)).collect();
    assert_eq!(std::fs::read_to_string(folder.join("hot.csv")).expect("the hot file"), format!("superchunk,is,until\n{hot}"), "the nine hot, none cooling or warming");
    let mut names: Vec<String> = std::fs::read_dir(folder.join("superchunks")).expect("the superchunks").map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    let expected: Vec<String> = disk::saved_superchunks(&folder).expect("listed").iter().flat_map(|superchunk| ["image", "state"].map(|kind| format!("{:011x}.{kind}", superchunk.0))).collect();
    assert_eq!(names, expected);
    assert_eq!(names.len(), 2 * 9, "the origin and its halo");
    assert_eq!(disk::saved_superchunks(&folder).unwrap(), first.arena().superchunk_indices());
}

/// A world's file is read whatever the order of its rows under the
/// one naming its columns, what it lacks as by default -- but its
/// seed, which it must have; a name said twice, or a format not the
/// one written, is refused.
#[test]
fn a_world_file_is_read_in_any_order() {
    let folder = folder("any_order");
    let seed = crate::tests::land_seed(2);
    let mut world = server::start(server::Start { seed, sheep: 10, camera_loads: true, ..server::Start::default() });
    server::save(&folder, &mut world).expect("saved");
    let saved = disk::read_world(&folder).expect("read");
    let path = folder.join("world.csv");
    let text = std::fs::read_to_string(&path).expect("the world's file");
    let mut rows: Vec<&str> = text.lines().collect();
    rows[1..].reverse();
    std::fs::write(&path, rows.join("\n")).expect("written");
    assert_eq!(disk::read_world(&folder).expect("read turned round"), saved);
    std::fs::write(&path, format!("world,is\nseed,{}\n", utilities::seed::hex(seed))).expect("written");
    let bare = disk::read_world(&folder).expect("read with the seed alone");
    assert_eq!(bare, disk::WorldInfo { seed, tick: 0, layers: Vec::new(), side: None, forced: false, hot_entity: None, camera_flock: None, without_camera_flock: Vec::new(), generation: Vec::new() });
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
    server::save(&folder, &mut server::start(server::Start { seed: crate::tests::land_seed(4), sheep: 10, ..server::Start::default() })).expect("saved");
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
    // Each sheep told from the others by a tag; a lamb, born after, has none, and is not followed.
    crate::tests::tag_sheep(&mut made);
    let mut stood: HashMap<u64, coordinates::CellIndex> = made.entities().iter().map(|sheep| (crate::tests::tag(&sheep).expect("tagged"), sheep.header.at)).collect();
    let (mut moved, mut beside_walls) = (0, 0);
    for _ in 0..1_500 {
        made.tick();
        for sheep in made.entities().iter() {
            let at = sheep.header.at;
            if let Some(was) = crate::tests::tag(&sheep).and_then(|tag| stood.insert(tag, at)).filter(|&was| was != at) {
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

/// The weather's planes -- the clouds and the climate, a number a tile
/// 8 by 8 cells -- hold what is written to them: read at any cell of
/// the tile, through ticks, a save and a load. Tiles, numbers and the
/// cells they are read at drawn from the seed.
#[test]
fn the_weathers_planes_hold_a_number_a_tile_through_ticks_and_a_save() {
    use bitplane_manager::Write;
    use type_registry::{CLIMATE, CLOUD, WEATHER_TILE_SCALE};
    let folder = folder("weather");
    let mut random = utilities::rng::Rng::new(utilities::seed::counted());
    let mut world = server::start(server::Start { seed: crate::tests::land_seed(0), size: server::Size::Limited { side: 2, forced: true }, sheep: 50, ..server::Start::default() });
    let (corner, side) = (world.arena().superchunk_indices()[0].top_left().cartesian(), 2 * coordinates::SUPERCHUNK_SIDE_CELLS);
    // A cell of the tile `(x, y)` -- in tiles from the world's corner -- drawn anew each time.
    let tile_side = 1 << WEATHER_TILE_SCALE;
    let cell_of = |(x, y): (u32, u32), random: &mut utilities::rng::Rng| -> coordinates::CellIndex {
        CellCartesian { x: corner.x + x * tile_side + random.below(u64::from(tile_side)) as u32, y: corner.y + y * tile_side + random.below(u64::from(tile_side)) as u32 }.into()
    };
    let mut tiles = std::collections::BTreeMap::new();
    for _ in 0..random.between(200, 400) {
        let tile = (random.below(u64::from(side / tile_side)) as u32, random.below(u64::from(side / tile_side)) as u32);
        tiles.insert(tile, (1 + random.below(u64::from(CLOUD.most())) as u32, 1 + random.below(u64::from(CLIMATE.most())) as u32));
    }
    let clouds: Vec<Write> = tiles.iter().map(|(&tile, &(cloud, _))| Write::value(CLOUD, cell_of(tile, &mut random), cloud)).collect();
    let climates: Vec<Write> = tiles.iter().map(|(&tile, &(_, climate))| Write::value(CLIMATE, cell_of(tile, &mut random), climate)).collect();
    assert_eq!(world.write_cells(CLOUD.layer_type(), clouds).changed, tiles.len() as u64, "a cloud written a tile");
    assert_eq!(world.write_cells(CLIMATE.layer_type(), climates).changed, tiles.len() as u64, "a climate written a tile");
    let held = |world: &server::World, when: &str, random: &mut utilities::rng::Rng| {
        for (&tile, &(cloud, climate)) in &tiles {
            let read = (world.arena().value(CLOUD, cell_of(tile, random)), world.arena().value(CLIMATE, cell_of(tile, random)));
            assert_eq!(read, (Ok(cloud), Ok(climate)), "the tile {tile:?}, {when}");
        }
        // A tile nothing was written to holds none.
        let bare = (0..).map(|_| (random.below(u64::from(side / tile_side)) as u32, random.below(u64::from(side / tile_side)) as u32)).find(|tile| !tiles.contains_key(tile)).expect("a tile not written");
        assert_eq!((world.arena().value(CLOUD, cell_of(bare, random)), world.arena().value(CLIMATE, cell_of(bare, random))), (Ok(0), Ok(0)), "a bare tile, {when}");
    };
    held(&world, "as written", &mut random);
    for _ in 0..random.between(20, 60) {
        world.tick();
    }
    held(&world, "after ticks", &mut random);
    server::save(&folder, &mut world).expect("saved");
    let loaded = server::load(&folder).expect("loaded");
    held(&loaded, "loaded from a save", &mut random);
    assert!(everything(&world) == everything(&loaded), "loaded as saved");
}
