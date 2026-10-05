//! A world saved and loaded: it goes on exactly as it would have -- to
//! the cell, the entity and the random number, however often it is
//! stopped, hot superchunks and cold -- its files are where and what
//! they are said to be, and files that are not a save are refused.
//!
//! `cargo test`

use chunk_storage::disk::{self, DiskError};
use chunk_storage::SuperchunkImage;
use coordinates::{CellCartesian, SuperchunkIndex};
use simulation::entity_store::{Attribute, Header};
use std::path::PathBuf;
use world::{self, transient_data, World};

/// A folder of its own for the test `name`, emptied.
fn folder(name: &str) -> PathBuf {
    let folder = transient_data::saves().join("tests").join(name);
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// Every hot cell of grass and dirt, every entity with its attributes,
/// the tick, every random stream, every cold superchunk's image and
/// kept state, and the superchunks warming and cooling with when each is due: what
/// two worlds the same hold the same.
type Everything = (Vec<u64>, Vec<(Header, Vec<Attribute>)>, u64, Vec<(SuperchunkIndex, u64)>, Vec<(SuperchunkIndex, SuperchunkImage, Vec<u64>)>, Vec<(SuperchunkIndex, u64)>, Vec<(SuperchunkIndex, u64)>);

/// [`Everything`] `world` holds.
fn everything(world: &World) -> Everything {
    let cells = world.info.layers.clone().into_iter().flat_map(|layer| world.arena.run(layer)).flat_map(|(_, bucket)| bucket.words().to_vec()).collect();
    let all = world.entities.iter().map(|entity| (entity.header, entity.attributes.to_vec())).collect();
    let cold = world.cold.iter().map(|(&superchunk, words)| (superchunk, world.storage.image(superchunk).expect("a cold superchunk's image").clone(), words.clone())).collect();
    (cells, all, world.entities.now(), world.simulation.random_states().collect(), cold, world.warming().collect(), world.cooling().collect())
}

/// Grass and sheep ticked, saved, and ticked on; the save loaded and
/// ticked as far: the two are the same world.
#[test]
fn a_world_loaded_goes_on_as_the_one_saved() {
    let folder = folder("goes_on");
    let mut first = world::generate(crate::land_seed(0), 3_000);
    first.info.name = "Pasture".to_string();
    for _ in 0..1_500 {
        first.tick();
    }
    let saved = world::save(&folder, &mut first).expect("saved");
    assert_eq!((saved.superchunks, saved.entities), (first.storage.superchunks().count(), first.entities.len()));

    let mut second = world::load(&folder).expect("loaded");
    assert_eq!((second.info.name.as_str(), second.info.seed, second.info.tick), ("Pasture", crate::land_seed(0), 1_500));
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
/// superchunk is warming -- is, at a tick agreed, the world that ran
/// straight to it: every cell, every entity, every random number.
#[test]
fn a_world_saved_and_loaded_mid_run_comes_to_the_same() {
    const UNTIL: u64 = 4_000;
    let mut straight = world::generate(crate::land_seed(0), 4_000);
    let mut warming = None;
    while straight.entities.now() < UNTIL {
        straight.tick();
        if warming.is_none() && straight.warming().next().is_some() {
            warming = Some(straight.entities.now());
        }
    }
    let warming = warming.expect("a superchunk warming on the way");

    let folder = folder("mid_run");
    let mut stopped = world::generate(crate::land_seed(0), 4_000);
    let mut stops = vec![1, 700, 701, 1_900, 3_333, warming, UNTIL];
    stops.sort_unstable();
    stops.dedup();
    for stop in stops {
        while stopped.entities.now() < stop {
            stopped.tick();
        }
        world::save(&folder, &mut stopped).expect("saved");
        // What ran is dropped whole: the next stretch runs on what the files hold alone.
        stopped = world::load(&folder).expect("loaded");
        assert_eq!(stopped.info.tick, stop);
    }
    assert!(straight.entities.len() > 4_000, "{} sheep: a flock that bred", straight.entities.len());
    assert!(everything(&straight) == everything(&stopped), "the same at tick {UNTIL}");
}

/// A save is a folder: a world file and a hot file in text, and two
/// files a superchunk named by its superchunk index in hexadecimal.
#[test]
fn a_save_is_a_directory_of_files_named_by_superchunk_index() {
    let folder = folder("files");
    let mut first = world::generate(99, 10);
    first.info.name = "Nine fields".to_string();
    world::save(&folder, &mut first).expect("saved");
    let text = std::fs::read_to_string(folder.join("world")).expect("the world's file");
    assert_eq!(text, "tilesim world 1\nname = Nine fields\nseed = 99\ntick = 0\nlayers = 2 3 4 5 6 7 24 25 26 27 28 29 30 31 8 9\n");
    let hot: String = first.arena.superchunk_indices().iter().map(|superchunk| format!("{:011x}\n", superchunk.0)).collect();
    assert_eq!(std::fs::read_to_string(folder.join("hot")).expect("the hot file"), format!("tilesim hot 2\n{hot}"), "the nine hot, none cooling or warming");
    let mut names: Vec<String> = std::fs::read_dir(folder.join("superchunks")).expect("the superchunks").map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    let expected: Vec<String> = disk::saved_superchunks(&folder).expect("listed").iter().flat_map(|superchunk| ["image", "state"].map(|kind| format!("{:011x}.{kind}", superchunk.0))).collect();
    assert_eq!(names, expected);
    assert_eq!(names.len(), 2 * 9, "the origin and its halo");
    assert_eq!(disk::saved_superchunks(&folder).unwrap(), first.arena.superchunk_indices());
}

/// What is not a save is refused, saying which file and why.
#[test]
fn files_that_are_not_a_save_are_refused() {
    let folder = folder("refused");
    assert!(matches!(world::load(&folder), Err(DiskError::Io(..))), "no such folder");
    world::save(&folder, &mut world::generate(1, 10)).expect("saved");
    let state = std::fs::read_dir(folder.join("superchunks")).unwrap().map(|entry| entry.unwrap().path()).find(|path| path.extension().unwrap() == "state").unwrap();
    let whole = std::fs::read(&state).unwrap();
    std::fs::write(&state, &whole[..whole.len() - 8]).unwrap();
    assert!(matches!(world::load(&folder), Err(DiskError::Invalid(path, what)) if path == state && what == "cut short"));
    std::fs::write(&state, &whole).unwrap();
    assert!(world::load(&folder).is_ok());
    std::fs::write(folder.join("world"), "something else\n").unwrap();
    assert!(matches!(world::load(&folder), Err(DiskError::Invalid(..))));
}

/// In a world generated, the ground has walls, and no sheep ever steps
/// through one: across or down only between cells at most a step apart
/// in height, diagonally only where both ways round are such steps.
#[test]
fn sheep_never_step_through_a_wall() {
    use std::collections::HashMap;
    // Small polygons joined by cliffs, and a seed whose origin superchunk has walls enough.
    let shape = terrain::Shape { span: 8, highest: 552, narrow: 2, wide: 2, sea: 0, finer_depth: 3, ..terrain::Shape::DEFAULT };
    let seed = (utilities::seed::counted()..).find(|&seed| terrain::Terrain::generate_shaped(&shape, seed, coordinates::WORLD_MIDDLE).wall_counts().iter().sum::<u64>() > 5_000).expect("a walled origin");
    let mut made = world::generate_flocks_with(world::Generation { shape, ..world::Generation::DEFAULT }, seed, &[coordinates::WORLD_MIDDLE], 4_000);
    // The superchunk's heights and a cell more all round, worked out once: asked for at every sheep, every tick.
    let (corner, side) = (coordinates::WORLD_MIDDLE.top_left().cartesian(), coordinates::SUPERCHUNK_SIDE_CELLS as usize + 2);
    let mut lands = terrain::mesh::Lands::new(&shape, seed);
    let heights: Vec<_> = (0..side * side).map(|index| lands.height(corner.x.wrapping_add((index % side) as u32).wrapping_sub(1), corner.y.wrapping_add((index / side) as u32).wrapping_sub(1))).collect();
    let high = |at: coordinates::CellIndex| {
        let cell = at.cartesian();
        let (across, down) = (cell.x.wrapping_sub(corner.x).wrapping_add(1) as usize, cell.y.wrapping_sub(corner.y).wrapping_add(1) as usize);
        // A sheep strayed past the superchunk: the generator asked.
        if across < side && down < side { heights[down * side + across] } else { terrain::height_shaped(&shape, seed, cell.x, cell.y) }
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
                let walled = corners.iter().any(|corner| terrain::wall(high(was), high((*corner).into())) || terrain::wall(high((*corner).into()), high(at)));
                assert!(!walled, "from height {} to {}: {from:?} to {to:?}", high(was), high(at));
                moved += 1;
            }
            beside_walls += (0..9).any(|way| at.offset(way % 3 - 1, way / 3 - 1).is_some_and(|beside| terrain::wall(high(at), high(beside)))) as usize;
        }
    }
    // Few steps: on ground nearly all grass a sheep seldom has to walk.
    assert!(moved > 50 && beside_walls > 10_000, "{moved} steps, {beside_walls} sheep-ticks beside a wall");
}
