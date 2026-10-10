//! The server's commands ([`COMMANDS`]), run by `Civil_Egregore server
//! <command>`: a world made in a folder, run and looked at, and the
//! diagnostics tools (`docs/reference.md`, "commands.rs").

use crate::diagnostics::tool::{check, pasture, throughput, EVERY, FLOCK, GRASS as GRASS_SHARE, SEED, SHEEP, SUPERCHUNKS, THREADS, TICKS};
use crate::HaloChange;
use chunk_storage::disk;
use type_registry::GRASS;
use std::path::Path;
use std::time::Instant;
use utilities::commands::{Command, Given, Parameter};

/// A world's folder: a name in the worlds' folder, or a path.
const FOLDER: &str = "folder";

/// Whether a world is made forced hot throughout, not about its
/// flock's halo: 0, halo; anything else, forced.
const FORCED: &str = "forced";

/// The server's commands: a world made, run and looked at, and its
/// diagnostics tools.
pub const COMMANDS: [Command; 6] = [
    Command {
        name: "new",
        does: "makes a world from a seed and saves it in the folder: so many superchunks along a side, or 0 for as far as it goes; so many sheep on every superchunk of a world with a side, on its origin of one without; forced hot throughout if told -- only with a side -- else about the sheep's halos; on so many threads, or every one the machine has if 0",
        parameters: &[Parameter::new(FOLDER, ""), Parameter::new("seed", "1"), Parameter::new("sheep", ""), Parameter::new("side", "0"), Parameter::new(FORCED, "0"), Parameter::new(THREADS, "0")],
        run: |given| printed(given, new),
    },
    Command { name: "run", does: "loads the world in the folder, ticks it, and saves it", parameters: &[Parameter::new(FOLDER, ""), Parameter::new("ticks", "10000")], run: |given| printed(given, run) },
    Command { name: "info", does: "says what the world in the folder is", parameters: &[Parameter::new(FOLDER, "")], run: |given| printed(given, |folder, _| info(folder)) },
    Command {
        name: "throughput",
        does: "ticks grass flat out: each phase's time, the writes a second, the memory held",
        parameters: &[Parameter::new(TICKS, "500"), Parameter::new(GRASS_SHARE, "333"), Parameter::new(SUPERCHUNKS, "16"), Parameter::new(THREADS, "0")],
        run: throughput,
    },
    Command {
        name: "pasture",
        does: "ticks grass and sheep flat out: the flock, what the sheep did, each rule's time, the memory held",
        parameters: &[Parameter::new(TICKS, "2000"), Parameter::new(GRASS_SHARE, "333"), Parameter::new(FLOCK, "4000"), Parameter::new(SUPERCHUNKS, "16"), Parameter::new(THREADS, "0")],
        run: pasture,
    },
    Command {
        name: "check",
        does: "ticks a world from a seed and prints its hash every so many ticks, part by part: the same rows on every machine and any number of threads, if the simulation is deterministic",
        parameters: &[Parameter::new(SEED, "0"), Parameter::new(TICKS, "2000"), Parameter::new(EVERY, "100"), Parameter::new(SHEEP, "4000"), Parameter::new(THREADS, "0")],
        run: check,
    },
];

/// Runs `command` on the folder given and what follows it, and prints
/// the line it gives.
fn printed(given: &Given, command: impl FnOnce(&Path, &[&str]) -> Result<String, String>) -> Result<(), String> {
    let folder = given.text(FOLDER)?;
    println!("{}", command(&utilities::settings::world(folder), &given.arguments()[1..])?);
    Ok(())
}

/// The name of the world in `folder`: the folder's.
fn name(folder: &Path) -> String {
    folder.file_name().map_or_else(|| folder.display().to_string(), |name| name.to_string_lossy().into_owned())
}

/// `argument` as a number, or `default` if not given.
fn number(argument: Option<&&str>, default: u64) -> Result<u64, String> {
    argument.map_or(Ok(default), |argument| argument.parse().map_err(|_| format!("`{argument}` is not a number")))
}

/// Makes a world from a seed and saves it in `folder`, as the rest of
/// the line says: its sheep, side, whether forced hot, and threads
/// (`docs/reference.md`, "commands.rs").
pub fn new(folder: &Path, rest: &[&str]) -> Result<String, String> {
    if folder.join(disk::WORLD_FILE).exists() {
        return Err(format!("{} is a world already", folder.display()));
    }
    let seed = rest.first().map_or(Ok(1), |seed| utilities::seed::of_hex(seed).ok_or_else(|| format!("`{seed}` is not a seed: 64 bits, in hexadecimal")))?;
    let sheep = number(rest.get(1), crate::FLOCK as u64)? as usize;
    let forced = number(rest.get(3), 0)? != 0;
    let size = crate::Size::of_side(number(rest.get(2), 0)? as u32, forced)?;
    let threads = Some(number(rest.get(4), 0)? as usize).filter(|&threads| threads > 0);
    let mut made = crate::start(crate::Start { seed, size, threads, sheep, ..crate::Start::default() });
    let saved = crate::save(folder, &mut made).map_err(|error| error.to_string())?;
    Ok(format!("{}, seed {}: {} superchunks, {} entities, {} bytes in {}", name(folder), utilities::seed::hex(seed), saved.superchunks, saved.entities, saved.bytes, folder.display()))
}

/// Loads the world in `folder`, ticks it, and saves it.
pub fn run(folder: &Path, rest: &[&str]) -> Result<String, String> {
    let ticks = number(rest.first(), 10_000)?;
    let mut loaded = crate::load(folder).map_err(|error| error.to_string())?;
    let start = Instant::now();
    let mut halos = HaloChange::default();
    for _ in 0..ticks {
        halos += loaded.tick().halos;
    }
    let seconds = start.elapsed().as_secs_f64();
    let hot = loaded.arena.superchunks().len();
    let grass: u64 = loaded.arena.superchunks().iter().map(|superchunk| loaded.arena.superchunk_count(GRASS, superchunk.index()) as u64).sum();
    let saved = crate::save(folder, &mut loaded).map_err(|error| error.to_string())?;
    let (warming, cooling) = (loaded.warming().count(), loaded.cooling().count());
    let HaloChange { reached, generated, restored, cooled } = halos;
    Ok(format!(
        "{}: tick {} -> {}, {:.0} ticks a second; {} entities; {hot} of {} superchunks hot, {cooling} of them cooling, {warming} warming, {grass} cells of grass on them; superchunks {reached} reached, {generated} generated, {restored} restored, {cooled} cooled; {} bytes saved",
        name(folder),
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
    let size = info.side.map_or("of no size".to_string(), |side| format!("{side} superchunks a side"));
    Ok(format!("{}: seed {}, at tick {}, {size}, {} superchunks, layer types {:?}", name(folder), utilities::seed::hex(info.seed), info.tick, superchunks.len(), info.layers.iter().map(|layer| layer.0).collect::<Vec<_>>()))
}
