//! Where a world saved and loaded parts from the same world run
//! straight: the first tick after a stop at which the two differ, and
//! in what -- entities, random streams, the hot, warming and cooling
//! superchunks, the cold ones, the cells -- and at each stop the cold
//! superchunks' images that differ, before the world run straight is
//! flushed and after: one not flushed keeps a cooled superchunk's last
//! changes in the writeback ring, its image behind them.

use server::{Start, World};
use utilities::commands::Given;

/// Ticks between two stops.
pub const TICKS_BETWEEN_STOPS: &str = "ticks between stops";

/// Ticks the worlds are run.
pub const TICKS: &str = "ticks";

/// Sheep the world starts with.
pub const SHEEP: &str = "sheep";

/// What of `straight` is not as in `stopped`, by name: none if they are
/// the same. The cells are looked at only if `with_cells`: they are
/// the slow part.
fn differences(straight: &World, stopped: &World, with_cells: bool) -> Vec<&'static str> {
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
    note("cells", !with_cells || cells(straight) == cells(stopped));
    differing
}

/// Prints how the images of the cold superchunks of `straight` are not
/// as those of `stopped`: a line a layer whose words differ, with
/// whether the cells they decode to do.
fn print_cold_images_differing(straight: &World, stopped: &World) {
    let mut codec = chunk_storage::LayerCodec::new();
    for &superchunk in straight.cold.keys() {
        let (Some(one), Some(other)) = (straight.storage.image(superchunk), stopped.storage.image(superchunk)) else {
            println!("image,{superchunk:?},one has none");
            continue;
        };
        if one == other {
            continue;
        }
        println!("image,{superchunk:?},{} words against {}; heights the same: {}; water the same: {}", one.words().len(), other.words().len(), one.height_words() == other.height_words(), one.water_words() == other.water_words());
        for place in 0..coordinates::CHUNKS_IN_SUPERCHUNK {
            let (types, other_types): (Vec<_>, Vec<_>) = (one.layer_types(place).collect(), other.layer_types(place).collect());
            if types != other_types {
                println!("layers,chunk {place},{types:?} against {other_types:?}");
            }
            for layer_type in types.into_iter().filter(|layer_type| other_types.contains(layer_type)) {
                let (words, other_words) = (one.layer(place, layer_type).expect("listed"), other.layer(place, layer_type).expect("listed"));
                if words != other_words {
                    let (mut cells, mut other_cells) = ([0u64; bitmap::WORDS], [0u64; bitmap::WORDS]);
                    codec.decode(words, &mut cells);
                    codec.decode(other_words, &mut other_cells);
                    println!("layer,chunk {place} {layer_type:?},{} words against {}; cells the same: {}; set {} against {}", words.len(), other_words.len(), cells == other_cells, cells.iter().map(|word| word.count_ones()).sum::<u32>(), other_cells.iter().map(|word| word.count_ones()).sum::<u32>());
                }
            }
        }
    }
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
        let differing = differences(&straight, &stopped, now.is_multiple_of(between));
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
            let differing = differences(&straight, &stopped, true);
            println!("{now},loaded,{}", differing.join(" + "));
            print_cold_images_differing(&straight, &stopped);
            // Whether what differs is only what the world run straight has yet to flush.
            straight.write_back_and_flush_all();
            println!("{now},flushed,{}", straight.cold.keys().filter(|&&superchunk| straight.storage.image(superchunk) != stopped.storage.image(superchunk)).count());
            if !differing.is_empty() {
                break;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&folder);
    Ok(())
}
