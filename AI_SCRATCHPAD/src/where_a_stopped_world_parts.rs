//! Where a world saved and loaded parts from the same world run
//! straight: the first tick after a stop at which the two differ, and
//! in what -- entities, random streams, the hot, warming and cooling
//! superchunks, the cold ones, the cells.

use server::{Start, World};
use utilities::commands::Given;

/// Ticks between two stops.
pub const TICKS_BETWEEN_STOPS: &str = "ticks between stops";

/// Ticks the worlds are run.
pub const TICKS: &str = "ticks";

/// Sheep the world starts with.
pub const SHEEP: &str = "sheep";

/// What of `straight` is not as in `stopped`, by name: none if they are
/// the same.
fn differences(straight: &World, stopped: &World) -> Vec<&'static str> {
    let mut differing = Vec::new();
    let entities = |world: &World| world.entities.iter().map(|entity| (entity.header, entity.attributes.to_vec())).collect::<Vec<_>>();
    let cells = |world: &World| world.info.layers.clone().into_iter().flat_map(|layer| world.arena.run(layer)).flat_map(|(_, bucket)| bucket.words().to_vec()).collect::<Vec<u64>>();
    let mut note = |name: &'static str, same: bool| if !same { differing.push(name) };
    note("tick", straight.entities.now() == stopped.entities.now());
    note("hot superchunks", straight.arena.superchunk_indices() == stopped.arena.superchunk_indices());
    note("warming", straight.warming().collect::<Vec<_>>() == stopped.warming().collect::<Vec<_>>());
    note("cooling", straight.cooling().collect::<Vec<_>>() == stopped.cooling().collect::<Vec<_>>());
    note("random streams", straight.simulation.random_states().collect::<Vec<_>>() == stopped.simulation.random_states().collect::<Vec<_>>());
    note("cold superchunks", straight.cold.keys().collect::<Vec<_>>() == stopped.cold.keys().collect::<Vec<_>>());
    note("cold states", straight.cold == stopped.cold);
    note("entities", entities(straight) == entities(stopped));
    note("cells", cells(straight) == cells(stopped));
    differing
}

/// Runs the probe: a line a stop, and the first tick the worlds differ.
pub fn run(given: &Given) -> Result<(), String> {
    let (between, ticks, sheep): (u64, u64, usize) = (given.number(TICKS_BETWEEN_STOPS)?, given.number(TICKS)?, given.number(SHEEP)?);
    let seed = crate::asking_the_host::seed(given)?.ok_or("a seed other than 0")?;
    let folder = std::env::temp_dir().join(format!("AI_SCRATCHPAD_where_a_stopped_world_parts_{}", std::process::id()));
    let mut straight = server::start(Start { seed, sheep, ..Start::default() });
    let mut stopped = server::start(Start { seed, sheep, ..Start::default() });
    println!("tick,what,differing");
    while straight.entities.now() < ticks {
        straight.tick();
        stopped.tick();
        let now = straight.entities.now();
        let differing = differences(&straight, &stopped);
        if !differing.is_empty() {
            println!("{now},ticked,{}", differing.join(" + "));
            println!("hot,{:?},{:?}", straight.arena.superchunk_indices(), stopped.arena.superchunk_indices());
            println!("warming,{:?},{:?}", straight.warming().collect::<Vec<_>>(), stopped.warming().collect::<Vec<_>>());
            println!("cooling,{:?},{:?}", straight.cooling().collect::<Vec<_>>(), stopped.cooling().collect::<Vec<_>>());
            println!("cold,{:?},{:?}", straight.cold.keys().collect::<Vec<_>>(), stopped.cold.keys().collect::<Vec<_>>());
            println!("entities,{},{}", straight.entities.len(), stopped.entities.len());
            break;
        }
        if now.is_multiple_of(between) {
            server::save(&folder, &mut stopped).map_err(|error| format!("{error:?}"))?;
            stopped = server::load(&folder).map_err(|error| format!("{error:?}"))?;
            let differing = differences(&straight, &stopped);
            println!("{now},loaded,{}", differing.join(" + "));
            if !differing.is_empty() {
                break;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&folder);
    Ok(())
}
