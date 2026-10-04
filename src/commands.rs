//! The commands, each given the rest of the command line after its
//! folder, and giving the line to print -- or why it could not.

use chunk_storage::disk;
use chunk_storage::mock::GRASS;
use std::path::Path;
use std::time::Instant;
use world::HaloChange;

/// How to call the program.
pub const USAGE: &str = "tilesim new <folder> [name] [seed] [sheep]\ntilesim run <folder> [ticks]\ntilesim info <folder>";

/// `argument` as a number, or `default` if not given.
fn number(argument: Option<&&str>, default: u64) -> Result<u64, String> {
    argument.map_or(Ok(default), |argument| argument.parse().map_err(|_| format!("`{argument}` is not a number")))
}

/// Makes a world from a seed, a flock on its origin, and saves it in
/// `folder`.
pub fn new(folder: &Path, rest: &[&str]) -> Result<String, String> {
    if folder.join("world").exists() {
        return Err(format!("{} is a world already", folder.display()));
    }
    let name = rest.first().copied().unwrap_or("World");
    let (seed, sheep) = (number(rest.get(1), 1)?, number(rest.get(2), world::FLOCK as u64)? as usize);
    let mut made = world::generate(seed, sheep);
    made.info.name = name.to_string();
    let saved = world::save(folder, &mut made).map_err(|error| error.to_string())?;
    Ok(format!("{name}, seed {seed}: {} superchunks, {} entities, {} bytes in {}", saved.superchunks, saved.entities, saved.bytes, folder.display()))
}

/// Loads the world in `folder`, ticks it, and saves it.
pub fn run(folder: &Path, rest: &[&str]) -> Result<String, String> {
    let ticks = number(rest.first(), 10_000)?;
    let mut loaded = world::load(folder).map_err(|error| error.to_string())?;
    let start = Instant::now();
    let mut halos = HaloChange::default();
    for _ in 0..ticks {
        halos += loaded.tick().halos;
    }
    let seconds = start.elapsed().as_secs_f64();
    let hot = loaded.arena.superchunks().len();
    let grass: u64 = loaded.arena.superchunks().iter().map(|superchunk| loaded.arena.superchunk_count(GRASS, superchunk.index()) as u64).sum();
    let saved = world::save(folder, &mut loaded).map_err(|error| error.to_string())?;
    let HaloChange { generated, warmed, cooled } = halos;
    Ok(format!(
        "{}: tick {} -> {}, {:.0} ticks a second; {} entities; {hot} of {} superchunks hot, {grass} cells of grass on them; superchunks {generated} generated, {warmed} warmed, {cooled} cooled; {} bytes saved",
        loaded.info.name,
        loaded.info.tick,
        loaded.entities.now(),
        ticks as f64 / seconds,
        saved.entities,
        saved.superchunks,
        saved.bytes
    ))
}

/// Says what the world in `folder` is.
pub fn info(folder: &Path) -> Result<String, String> {
    let info = disk::read_world(folder).map_err(|error| error.to_string())?;
    let superchunks = disk::saved_superchunks(folder).map_err(|error| error.to_string())?;
    Ok(format!("{}: seed {}, at tick {}, {} superchunks, layer types {:?}", info.name, info.seed, info.tick, superchunks.len(), info.layers.iter().map(|layer| layer.0).collect::<Vec<_>>()))
}
