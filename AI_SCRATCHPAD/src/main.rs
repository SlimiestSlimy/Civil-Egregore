//! The AI's own scratchpad: what it tests Civil Egregore with as it
//! works on it, free for it to use. No part of the program: nothing
//! depends on this crate, and nothing may.
//!
//! `cargo run --release -p AI_SCRATCHPAD -- <package>...`: the mods read,
//! registered and loaded as the server loads them, every type listed,
//! and each sca proc with what it is sampled on.
//! `cargo run --release -p AI_SCRATCHPAD -- ticks <basegame package>`:
//! ticks a second of one chunk, grass dying and spreading, at a few
//! coverages.

use protocol::type_registry::TypeRegistry;
use server::hot_chunk::HotChunk;
use server::sca_proc::ScaProc;
use server::server_handle::ServerHandle;
use server::server_manager::{LoadedScaProc, ServerManager};
use std::path::PathBuf;
use utilities::coordinates::{CellIndex, ChunkIndex, CELLS_IN_CHUNK};
use utilities::rng::RandomSource;

/// The fine tier's tick test's proc.
struct GrassDiesOrSpreads {
    grass: u64,
}

impl ScaProc for GrassDiesOrSpreads {
    fn call(&self, server: &mut ServerHandle<'_>, _sampled_plane_type_id: u64, tile_corner: CellIndex) {
        let roll = server.random_stream().draw_below(40);
        if roll < 4 {
            server.set().compare(self.grass, tile_corner, 1).write(self.grass, tile_corner, 0).end();
        } else if roll < 9 {
            let (dx, dy) = [(1, 0), (-1, 0), (0, 1), (0, -1)][server.random_stream().draw_below(4) as usize];
            let Some(spread_to) = tile_corner.at_cartesian_offset(dx, dy).filter(|cell| cell.chunk() == tile_corner.chunk()) else { return };
            server.set().compare(self.grass, tile_corner, 1).compare(self.grass, spread_to, 0).write(self.grass, spread_to, 1).end();
        }
    }
}

fn main() -> Result<(), String> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if let ["ticks", basegame] = arguments.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        let manifests = protocol::manifests::read_mods(&[PathBuf::from(basegame)]).map_err(|refusal| refusal.to_string())?;
        let type_registry = TypeRegistry::of_manifests(&manifests).of_side(protocol::type_kinds::Side::Server);
        let grass = type_registry.type_id_of("basegame_common>grass").expect("grass");
        let grass_tick = type_registry.type_id_of("basegame_server>grass_tick").expect("grass tick");
        for (coverage_percent, seconds_per_sample) in [(0, 1), (1, 1), (50, 1), (100, 1)] {
            let chance_per_tick = protocol::simulation_time::SimulationSeconds::read(&seconds_per_sample.to_string()).expect("seconds").chance_per_tick();
            let sca_proc = LoadedScaProc { type_id: grass_tick, sampled_plane_type_ids: vec![grass], chance_per_tick, code: Box::new(GrassDiesOrSpreads { grass }) };
            let mut chunk = HotChunk::new(ChunkIndex(0), 1);
            let mut random_numbers = RandomSource::new(7);
            for place in 0..CELLS_IN_CHUNK {
                if random_numbers.draw_below(100) < coverage_percent {
                    chunk.write_tile(grass, CellIndex::from_chunk_place(ChunkIndex(0), place), 1, &type_registry);
                }
            }
            let mut server_manager = ServerManager::new(type_registry.clone(), vec![sca_proc]);
            server_manager.insert_hot_chunk(chunk);
            let ticks = 256;
            let started = std::time::Instant::now();
            for _ in 0..ticks {
                server_manager.tick();
            }
            let seconds = started.elapsed().as_secs_f64();
            println!("{coverage_percent:>3}% grass: {:>8.0} ticks/s, {:.3} ms a tick", f64::from(ticks) / seconds, seconds * 1000.0 / f64::from(ticks));
        }
        return Ok(());
    }
    let packages: Vec<PathBuf> = arguments.iter().map(PathBuf::from).collect();
    let manifests = protocol::manifests::read_mods(&packages).map_err(|refusal| refusal.to_string())?;
    let server_manager = server::mod_loading::load_server_mods(&TypeRegistry::of_manifests(&manifests));
    for registered in server_manager.type_registry().types() {
        println!("{:#018x} {}", registered.type_id(), registered.qualified_name());
    }
    for sca_proc in server_manager.sca_procs() {
        println!("sca proc {:#018x}: sampled on {:x?}, {:?} a tick", sca_proc.type_id, sca_proc.sampled_plane_type_ids, sca_proc.chance_per_tick);
    }
    Ok(())
}
