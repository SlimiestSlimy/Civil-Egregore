//! The commands ([`COMMANDS`]): the program's own, each given the rest
//! of the command line after its folder and giving the line to print --
//! or why it could not -- and each crate's diagnostics tools, reached by
//! the crate's name ([`dispatch`]).

use chunk_storage::disk;
use chunk_storage::mock::GRASS;
use std::path::Path;
use std::time::Instant;
use utilities::commands::{Command, Given, Parameter};
use world::HaloChange;

/// The program's name: what its commands are reached by.
const CALLED: &str = "tilesim";

/// A world's folder.
const FOLDER: &str = "folder";
/// A crate's tool, and what the tool takes: handed on as given.
const TOOL: &str = "tool, and what it takes";

/// The program's commands: its own, and each crate's tools, reached by
/// the crate's name and handed the rest of the line as it is -- which
/// tools a crate has, and what they take, is the crate's to know.
pub const COMMANDS: [Command; 5] = [
    Command {
        name: "new",
        does: "makes a world from a seed, a flock on its origin, and saves it in the folder",
        parameters: &[Parameter::new(FOLDER, ""), Parameter::new("name", "World"), Parameter::new("seed", "1"), Parameter::new("sheep", "")],
        run: |given| printed(given, new),
    },
    Command { name: "run", does: "loads the world in the folder, ticks it, and saves it", parameters: &[Parameter::new(FOLDER, ""), Parameter::new("ticks", "10000")], run: |given| printed(given, run) },
    Command { name: "info", does: "says what the world in the folder is", parameters: &[Parameter::new(FOLDER, "")], run: |given| printed(given, |folder, _| info(folder)) },
    Command { name: "world", does: "the world's diagnostics tools; none named, they are listed", parameters: &[Parameter::new(TOOL, "")], run: |given| world::diagnostics::tool::dispatch(given.arguments()) },
    Command { name: "tessera", does: "Tessera's diagnostics tools; none named, they are listed", parameters: &[Parameter::new(TOOL, "")], run: |given| tessera::diagnostics::tool::dispatch(given.arguments()) },
];

/// Does what `arguments`, the command line after the program's name,
/// asks: the command its first word names, given the rest.
pub fn dispatch(arguments: &[&str]) -> Result<(), String> {
    utilities::commands::dispatch(CALLED, &COMMANDS, arguments)
}

/// Runs `command` on the folder given and what follows it, and prints
/// the line it gives.
fn printed(given: &Given, command: impl FnOnce(&Path, &[&str]) -> Result<String, String>) -> Result<(), String> {
    let folder = given.text(FOLDER)?;
    println!("{}", command(Path::new(folder), &given.arguments()[1..])?);
    Ok(())
}

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
    let (warming, cooling) = (loaded.warming().count(), loaded.cooling().count());
    let HaloChange { reached, generated, restored, cooled } = halos;
    Ok(format!(
        "{}: tick {} -> {}, {:.0} ticks a second; {} entities; {hot} of {} superchunks hot, {cooling} of them cooling, {warming} warming, {grass} cells of grass on them; superchunks {reached} reached, {generated} generated, {restored} restored, {cooled} cooled; {} bytes saved",
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
