//! The commands, each given the rest of the command line after its
//! folder, and giving the line to print -- or why it could not.

use chunk_storage::disk;
use chunk_storage::mock::GRASS;
use std::path::Path;
use std::time::Instant;

/// How to call the program.
pub const USAGE: &str = "tilesim new <folder> [name] [seed] [superchunks]\ntilesim run <folder> [ticks]\ntilesim info <folder>";

/// `argument` as a number, or `default` if not given.
fn number(argument: Option<&&str>, default: u64) -> Result<u64, String> {
    argument.map_or(Ok(default), |argument| argument.parse().map_err(|_| format!("`{argument}` is not a number")))
}

/// Makes a world from a seed and saves it in `folder`.
pub fn new(folder: &Path, rest: &[&str]) -> Result<String, String> {
    if folder.join("world").exists() {
        return Err(format!("{} is a world already", folder.display()));
    }
    let name = rest.first().copied().unwrap_or("World");
    let (seed, superchunks) = (number(rest.get(1), 1)?, number(rest.get(2), 16)? as u32);
    let mut made = world::generate(seed, superchunks);
    let saved = world::save(folder, name, seed, &mut made.arena, &mut made.storage, &made.entities, &made.simulation).map_err(|error| error.to_string())?;
    Ok(format!("{name}, seed {seed}: {} superchunks, {} entities, {} bytes in {}", saved.superchunks, saved.entities, saved.bytes, folder.display()))
}

/// Loads the world in `folder`, ticks it, and saves it.
pub fn run(folder: &Path, rest: &[&str]) -> Result<String, String> {
    let ticks = number(rest.first(), 10_000)?;
    let mut loaded = world::load(folder).map_err(|error| error.to_string())?;
    let (name, seed) = (loaded.info.name.clone(), loaded.info.seed);
    let start = Instant::now();
    for _ in 0..ticks {
        world::tick(&mut loaded.simulation, &mut loaded.arena, &mut loaded.entities, seed);
    }
    let seconds = start.elapsed().as_secs_f64();
    let saved = world::save(folder, &name, seed, &mut loaded.arena, &mut loaded.storage, &loaded.entities, &loaded.simulation).map_err(|error| error.to_string())?;
    let grass: u64 = loaded.arena.superchunks().iter().map(|superchunk| loaded.arena.superchunk_count(GRASS, superchunk.index()) as u64).sum();
    Ok(format!("{name}: tick {} -> {}, {:.0} ticks a second; {} entities, {grass} cells of grass; {} bytes saved", loaded.info.tick, loaded.entities.now(), ticks as f64 / seconds, saved.entities, saved.bytes))
}

/// Says what the world in `folder` is.
pub fn info(folder: &Path) -> Result<String, String> {
    let info = disk::read_world(folder).map_err(|error| error.to_string())?;
    let superchunks = disk::saved_superchunks(folder).map_err(|error| error.to_string())?;
    Ok(format!("{}: seed {}, at tick {}, {} superchunks, layer types {:?}", info.name, info.seed, info.tick, superchunks.len(), info.layers.iter().map(|layer| layer.0).collect::<Vec<_>>()))
}
